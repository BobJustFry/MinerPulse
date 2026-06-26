use minerpulse_core::drivers::registry::fetch_with_detect;
use minerpulse_core::{
    import_file_content, list_scan_subnets as discover_subnets, save_snapshot, scan_network,
    scan_network_streaming, EntitlementGate, ErrorResponse, MinerPulseError, MinerSnapshot,
    MpulseFile, RateLimiter, ScanRequest, ScanResult, ScanSubnet, SubscriptionTier,
    TcpCgminerClient,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindowBuilder, WindowEvent};

struct ScanSession {
    cancel: Arc<AtomicBool>,
    running: Arc<AtomicBool>,
}

impl ScanSession {
    fn new() -> Self {
        Self {
            cancel: Arc::new(AtomicBool::new(false)),
            running: Arc::new(AtomicBool::new(false)),
        }
    }

    fn try_begin(&self) -> Result<(), ErrorResponse> {
        if self.running.swap(true, Ordering::SeqCst) {
            return Err(ErrorResponse {
                code: minerpulse_core::ErrorCode::InvalidInput,
                args: None,
            });
        }
        self.cancel.store(false, Ordering::SeqCst);
        Ok(())
    }

    fn finish(&self) {
        self.running.store(false, Ordering::SeqCst);
    }
}

struct AppState {
    rate_limiter: Mutex<RateLimiter>,
    tier: Mutex<SubscriptionTier>,
    last_snapshot: Mutex<Option<minerpulse_core::MinerSnapshot>>,
    scan: ScanSession,
}

#[derive(Debug, Serialize, Deserialize)]
struct ReadMinerRequest {
    ip: String,
    port: Option<u16>,
}

#[derive(Debug, Serialize)]
struct ReadMinerResponse {
    snapshot: minerpulse_core::MinerSnapshot,
}

#[derive(Debug, Serialize, Deserialize)]
struct SaveSnapshotRequest {
    ip: String,
    path: String,
}

#[derive(Debug, Serialize)]
struct EntitlementsResponse {
    tier: SubscriptionTier,
    can_poll: bool,
    can_record_session: bool,
    can_play: bool,
    can_show_charts: bool,
    can_save_snapshot: bool,
    min_read_interval_sec: u64,
}

#[derive(Debug, Clone, Serialize)]
struct ScanProgressPayload {
    scanned: u32,
    total: u32,
    found_count: u32,
    range_label: String,
}

#[derive(Debug, Clone, Serialize)]
struct ScanFinishedPayload {
    cancelled: bool,
    error: Option<String>,
    scanned: u32,
    total: u32,
    found_count: u32,
    range_label: String,
}

const SCAN_WINDOW_LABEL: &str = "scan";
const MAIN_WINDOW_LABEL: &str = "main";

fn gate(tier: SubscriptionTier) -> EntitlementGate {
    EntitlementGate::new(tier)
}

fn io_error() -> ErrorResponse {
    ErrorResponse {
        code: minerpulse_core::ErrorCode::IoError,
        args: None,
    }
}

fn block_main_window(app: &AppHandle) -> Result<(), ErrorResponse> {
    let main = app
        .get_webview_window(MAIN_WINDOW_LABEL)
        .ok_or_else(io_error)?;
    main.set_enabled(false).map_err(|_| io_error())
}

fn unblock_main_window(app: &AppHandle) {
    if let Some(main) = app.get_webview_window(MAIN_WINDOW_LABEL) {
        let _ = main.set_enabled(true);
    }
}

fn cancel_active_scan(app: &AppHandle) {
    if let Some(state) = app.try_state::<AppState>() {
        state.scan.cancel.store(true, Ordering::Relaxed);
    }
}

fn attach_scan_window_cleanup(app: AppHandle, scan_window: tauri::WebviewWindow) {
    let app_for_event = app.clone();
    scan_window.on_window_event(move |event| {
        if matches!(
            event,
            WindowEvent::CloseRequested { .. } | WindowEvent::Destroyed
        ) {
            cancel_active_scan(&app_for_event);
            unblock_main_window(&app_for_event);
        }
    });
}

#[tauri::command]
fn get_entitlements(state: State<'_, AppState>) -> EntitlementsResponse {
    let tier = *state.tier.lock().unwrap();
    let g = gate(tier);
    EntitlementsResponse {
        tier,
        can_poll: g.can_poll(),
        can_record_session: g.can_record_session(),
        can_play: g.can_play(),
        can_show_charts: g.can_show_charts(),
        can_save_snapshot: g.can_save_snapshot(),
        min_read_interval_sec: g.min_read_interval_sec(),
    }
}

#[tauri::command]
fn set_tier(state: State<'_, AppState>, tier: SubscriptionTier) {
    *state.tier.lock().unwrap() = tier;
}

#[tauri::command]
fn read_miner(
    state: State<'_, AppState>,
    request: ReadMinerRequest,
) -> Result<ReadMinerResponse, ErrorResponse> {
    let tier = *state.tier.lock().unwrap();
    let g = gate(tier);

    if !g.can_poll() {
        let mut limiter = state.rate_limiter.lock().unwrap();
        limiter
            .try_acquire()
            .map_err(|e| ErrorResponse::from(&e))?;
    }

    let port = request.port.unwrap_or(4028);
    let client = TcpCgminerClient::default();
    let snapshot =
        fetch_with_detect(&client, &request.ip, port).map_err(|e| ErrorResponse::from(&e))?;

    *state.last_snapshot.lock().unwrap() = Some(snapshot.clone());

    Ok(ReadMinerResponse { snapshot })
}

#[tauri::command]
fn save_snapshot_file(
    state: State<'_, AppState>,
    request: SaveSnapshotRequest,
) -> Result<String, ErrorResponse> {
    let tier = *state.tier.lock().unwrap();
    let snapshot = state
        .last_snapshot
        .lock()
        .unwrap()
        .clone()
        .ok_or(MinerPulseError::with_code(
            minerpulse_core::ErrorCode::NoSnapshot,
        ))
        .map_err(|e| ErrorResponse::from(&e))?;

    let file = MpulseFile::snapshot(snapshot, &request.ip, tier);
    let path = PathBuf::from(&request.path);
    save_snapshot(&path, &file).map_err(|e| ErrorResponse::from(&e))?;

    Ok(path.to_string_lossy().to_string())
}

#[tauri::command]
fn list_scan_subnets() -> Vec<ScanSubnet> {
    discover_subnets()
}

#[tauri::command]
async fn open_scan_window(app: AppHandle) -> Result<(), ErrorResponse> {
    if let Some(window) = app.get_webview_window(SCAN_WINDOW_LABEL) {
        block_main_window(&app)?;
        window.show().map_err(|_| io_error())?;
        window.set_focus().map_err(|_| io_error())?;
        return Ok(());
    }

    let main = app
        .get_webview_window(MAIN_WINDOW_LABEL)
        .ok_or_else(io_error)?;

    let scan_window = WebviewWindowBuilder::new(&app, SCAN_WINDOW_LABEL, WebviewUrl::App("/scan".into()))
        .title("MinerPulse — Scan")
        .inner_size(540.0, 680.0)
        .min_inner_size(420.0, 480.0)
        .center()
        .parent(&main)
        .map_err(|_| io_error())?
        .always_on_top(true)
        .build()
        .map_err(|_| io_error())?;

    block_main_window(&app)?;
    attach_scan_window_cleanup(app.clone(), scan_window);

    Ok(())
}

#[tauri::command]
async fn start_scan(
    app: AppHandle,
    state: State<'_, AppState>,
    request: ScanRequest,
) -> Result<(), ErrorResponse> {
    state.scan.try_begin()?;

    let cancel = Arc::clone(&state.scan.cancel);
    let app_for_scan = app.clone();

    tauri::async_runtime::spawn(async move {
        let scan_result = tauri::async_runtime::spawn_blocking(move || {
            let mut last_total = 0u32;

            let result = scan_network_streaming(
                request,
                cancel.clone(),
                |scanned, total, found_count, range_label| {
                    last_total = total;
                    let payload = ScanProgressPayload {
                        scanned,
                        total,
                        found_count,
                        range_label: range_label.to_string(),
                    };
                    let _ = app_for_scan.emit("scan://progress", payload);
                },
                |miner| {
                    let _ = app_for_scan.emit("scan://found", miner);
                },
            );

            (result, last_total, cancel.load(Ordering::Relaxed))
        })
        .await;

        let (scan_outcome, last_total, cancelled) = match scan_result {
            Ok(value) => value,
            Err(_) => {
                if let Some(state) = app.try_state::<AppState>() {
                    state.scan.finish();
                }
                let _ = app.emit(
                    "scan://finished",
                    ScanFinishedPayload {
                        cancelled: false,
                        error: Some("scan_failed".into()),
                        scanned: 0,
                        total: 0,
                        found_count: 0,
                        range_label: String::new(),
                    },
                );
                return;
            }
        };

        if let Some(state) = app.try_state::<AppState>() {
            state.scan.finish();
        }

        match scan_outcome {
            Ok(result) => {
                let _ = app.emit(
                    "scan://finished",
                    ScanFinishedPayload {
                        cancelled,
                        error: None,
                        scanned: result.scanned,
                        total: last_total,
                        found_count: result.miners.len() as u32,
                        range_label: result.range_label,
                    },
                );
            }
            Err(err) => {
                let _ = app.emit(
                    "scan://finished",
                    ScanFinishedPayload {
                        cancelled,
                        error: Some(format!("{:?}", err.code())),
                        scanned: 0,
                        total: last_total,
                        found_count: 0,
                        range_label: String::new(),
                    },
                );
            }
        }
    });

    Ok(())
}

#[tauri::command]
fn cancel_scan(state: State<'_, AppState>) {
    state.scan.cancel.store(true, Ordering::Relaxed);
}

#[tauri::command]
async fn scan_miners(request: ScanRequest) -> Result<ScanResult, ErrorResponse> {
    tauri::async_runtime::spawn_blocking(move || scan_network(request))
        .await
        .map_err(|_| io_error())?
        .map_err(|e| ErrorResponse::from(&e))
}

#[derive(Debug, Serialize)]
struct ParseImportResponse {
    snapshot: MinerSnapshot,
    source_label: String,
    miner_ip: Option<String>,
}

#[tauri::command]
fn parse_import_file(content: String, filename: Option<String>) -> Result<ParseImportResponse, ErrorResponse> {
    let result = import_file_content(&content, filename.as_deref()).map_err(|e| ErrorResponse::from(&e))?;
    Ok(ParseImportResponse {
        snapshot: result.snapshot,
        source_label: result.source_label,
        miner_ip: result.miner_ip,
    })
}

#[tauri::command]
fn import_file_path(path: String) -> Result<ParseImportResponse, ErrorResponse> {
    let path = PathBuf::from(path);
    let meta = std::fs::metadata(&path).map_err(|_| io_error())?;
    if meta.len() > minerpulse_core::MAX_IMPORT_BYTES as u64 {
        return Err(ErrorResponse {
            code: minerpulse_core::ErrorCode::InvalidInput,
            args: None,
        });
    }

    let content = std::fs::read_to_string(&path).map_err(|_| io_error())?;
    let filename = path
        .file_name()
        .and_then(|name| name.to_str())
        .map(str::to_string);

    let result = import_file_content(&content, filename.as_deref()).map_err(|e| ErrorResponse::from(&e))?;
    Ok(ParseImportResponse {
        snapshot: result.snapshot,
        source_label: result.source_label,
        miner_ip: result.miner_ip,
    })
}

#[tauri::command]
fn remember_snapshot(state: State<'_, AppState>, snapshot: MinerSnapshot) {
    *state.last_snapshot.lock().unwrap() = Some(snapshot);
}

#[derive(Debug, Serialize)]
struct AppVersionInfo {
    version: String,
    build: u32,
    product: String,
    display: String,
}

#[tauri::command]
fn get_app_version() -> AppVersionInfo {
    let meta = serde_json::from_str::<Value>(env!("MINERPULSE_VERSION_JSON")).unwrap_or_else(|_| {
        serde_json::json!({
            "version": "0.0.0",
            "build": 0,
            "product": "MinerPulse"
        })
    });
    let version = meta["version"].as_str().unwrap_or("0.0.0").to_string();
    let build = meta["build"].as_u64().unwrap_or(0) as u32;
    let product = meta["product"].as_str().unwrap_or("MinerPulse").to_string();
    AppVersionInfo {
        display: format!("{version} (build {build})"),
        version,
        build,
        product,
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .manage(AppState {
            rate_limiter: Mutex::new(RateLimiter::new(10)),
            tier: Mutex::new(SubscriptionTier::Free),
            last_snapshot: Mutex::new(None),
            scan: ScanSession::new(),
        })
        .invoke_handler(tauri::generate_handler![
            get_entitlements,
            set_tier,
            read_miner,
            save_snapshot_file,
            get_app_version,
            list_scan_subnets,
            open_scan_window,
            start_scan,
            cancel_scan,
            scan_miners,
            parse_import_file,
            import_file_path,
            remember_snapshot,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
