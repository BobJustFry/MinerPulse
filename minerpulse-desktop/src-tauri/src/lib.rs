use minerpulse_core::drivers::registry::{fetch_whatsminer, fetch_with_detect};
use minerpulse_core::{
    import_file_content, list_scan_subnets as discover_subnets, load_mpulse, save_session,
    save_snapshot, scan_network, scan_network_streaming, EntitlementGate, ErrorResponse,
    FetchOptions, MinerPulseError, MinerSnapshot, MpulseFile, RateLimiter, ScanRequest,
    ScanResult, ScanSubnet, SubscriptionTier, TcpCgminerClient, WhatsminerLuciAuth,
    MAX_SESSION_DURATION_SEC, normalize_poll_rate_hz, poll_interval_ms, poll_wait_after_tick,
};
use minerpulse_core::model::BoardChipMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
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

struct PollSession {
    cancel: Arc<AtomicBool>,
    running: Arc<AtomicBool>,
    recording: Arc<AtomicBool>,
}

impl PollSession {
    fn new() -> Self {
        Self {
            cancel: Arc::new(AtomicBool::new(false)),
            running: Arc::new(AtomicBool::new(false)),
            recording: Arc::new(AtomicBool::new(false)),
        }
    }

    fn try_begin(&self, recording: bool) -> Result<(), ErrorResponse> {
        if self.running.swap(true, Ordering::SeqCst) {
            return Err(ErrorResponse {
                code: minerpulse_core::ErrorCode::InvalidInput,
                args: None,
            });
        }
        self.cancel.store(false, Ordering::SeqCst);
        self.recording.store(recording, Ordering::SeqCst);
        Ok(())
    }

    fn finish(&self) {
        self.running.store(false, Ordering::SeqCst);
        self.recording.store(false, Ordering::SeqCst);
    }

    fn stop(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

struct AppState {
    rate_limiter: Mutex<RateLimiter>,
    tier: Mutex<SubscriptionTier>,
    last_snapshot: Mutex<Option<minerpulse_core::MinerSnapshot>>,
    scan: ScanSession,
    poll: PollSession,
}

#[derive(Debug, Serialize, Deserialize)]
struct WhatsminerAuthRequest {
    #[serde(default)]
    username: String,
    #[serde(default)]
    password: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct ReadMinerRequest {
    ip: String,
    port: Option<u16>,
    #[serde(default)]
    whatsminer_auth: Option<WhatsminerAuthRequest>,
}

fn fetch_options_from_request(auth: Option<WhatsminerAuthRequest>) -> FetchOptions {
    FetchOptions {
        whatsminer_luci: auth.map(|value| WhatsminerLuciAuth {
            username: value.username,
            password: value.password,
        }),
        fast_poll: false,
    }
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

fn default_tier() -> SubscriptionTier {
    #[cfg(debug_assertions)]
    {
        SubscriptionTier::Service
    }
    #[cfg(not(debug_assertions))]
    {
        SubscriptionTier::Free
    }
}

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
fn set_tier(state: State<'_, AppState>, tier: SubscriptionTier) -> Result<(), ErrorResponse> {
    #[cfg(not(debug_assertions))]
    {
        let _ = (state, tier);
        return Err(ErrorResponse {
            code: minerpulse_core::ErrorCode::NotSupported,
            args: None,
        });
    }
    #[cfg(debug_assertions)]
    {
        *state.tier.lock().unwrap() = tier;
        Ok(())
    }
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
    let options = fetch_options_from_request(request.whatsminer_auth);
    let snapshot =
        fetch_with_detect(&client, &request.ip, port, &options).map_err(|e| ErrorResponse::from(&e))?;

    *state.last_snapshot.lock().unwrap() = Some(snapshot.clone());

    Ok(ReadMinerResponse { snapshot })
}

#[derive(Debug, Serialize, Deserialize)]
struct StartPollRequest {
    ip: String,
    port: Option<u16>,
    poll_rate_hz: Option<u32>,
    record_path: Option<String>,
    #[serde(default)]
    whatsminer_auth: Option<WhatsminerAuthRequest>,
}

#[derive(Debug, Clone, Serialize)]
struct PollSnapshotPayload {
    snapshot: MinerSnapshot,
    t_ms: u64,
    frame_index: u32,
    recording: bool,
}

#[derive(Debug, Clone, Serialize)]
struct PollFinishedPayload {
    reason: String,
    saved_path: Option<String>,
    frame_count: u32,
    error: Option<String>,
}

#[derive(Debug, Serialize)]
struct PollStatusResponse {
    running: bool,
    recording: bool,
}

#[tauri::command]
fn get_poll_status(state: State<'_, AppState>) -> PollStatusResponse {
    PollStatusResponse {
        running: state.poll.running.load(Ordering::Relaxed),
        recording: state.poll.recording.load(Ordering::Relaxed),
    }
}

#[tauri::command]
async fn start_poll(
    app: AppHandle,
    state: State<'_, AppState>,
    request: StartPollRequest,
) -> Result<(), ErrorResponse> {
    let tier = *state.tier.lock().unwrap();
    if !gate(tier).can_poll() {
        return Err(ErrorResponse {
            code: minerpulse_core::ErrorCode::NotSupported,
            args: None,
        });
    }

    let recording = request.record_path.is_some();
    if recording && !gate(tier).can_record_session() {
        return Err(ErrorResponse {
            code: minerpulse_core::ErrorCode::NotSupported,
            args: None,
        });
    }

    state.poll.try_begin(recording)?;

    let port = request.port.unwrap_or(4028);
    let poll_rate_hz = normalize_poll_rate_hz(request.poll_rate_hz);
    let record_path = request.record_path.map(PathBuf::from);
    let fetch_options = fetch_options_from_request(request.whatsminer_auth);
    let cancel = Arc::clone(&state.poll.cancel);
    let app_for_poll = app.clone();

    tauri::async_runtime::spawn(async move {
        let poll_result = tauri::async_runtime::spawn_blocking(move || {
            run_poll_loop(
                &app_for_poll,
                &request.ip,
                port,
                poll_rate_hz,
                tier,
                recording,
                record_path,
                cancel,
                fetch_options,
            )
        })
        .await;

        if let Some(state) = app.try_state::<AppState>() {
            state.poll.finish();
        }

        if poll_result.is_err() {
            let _ = app.emit(
                "poll://finished",
                PollFinishedPayload {
                    reason: "error".into(),
                    saved_path: None,
                    frame_count: 0,
                    error: Some("poll_failed".into()),
                },
            );
        }
    });

    Ok(())
}

const WHATSMINER_FULL_REFRESH_FRAMES: u32 = 15;

fn run_poll_loop(
    app: &AppHandle,
    ip: &str,
    port: u16,
    poll_rate_hz: u32,
    tier: SubscriptionTier,
    recording: bool,
    record_path: Option<PathBuf>,
    cancel: Arc<AtomicBool>,
    fetch_options: FetchOptions,
) {
    let client = TcpCgminerClient::for_polling();
    let started = Instant::now();
    let max_duration = Duration::from_secs(MAX_SESSION_DURATION_SEC);
    let interval = Duration::from_millis(poll_interval_ms(poll_rate_hz));
    let mut frame_index = 0u32;
    let mut cached_driver = String::new();
    let mut cached_board_chips = Vec::<BoardChipMap>::new();
    let mut session = if recording {
        Some(MpulseFile::new_session(
            ip,
            "unknown",
            tier,
            poll_rate_hz,
        ))
    } else {
        None
    };
    let mut finish_reason = "stopped".to_string();
    let mut finish_error: Option<String> = None;
    let mut saved_path: Option<String> = None;

    while !cancel.load(Ordering::Relaxed) {
        if recording && started.elapsed() >= max_duration {
            finish_reason = "max_duration".into();
            break;
        }

        let tick_start = Instant::now();

        let full_refresh =
            frame_index == 0 || frame_index % WHATSMINER_FULL_REFRESH_FRAMES == 0;
        let mut options = fetch_options.clone();
        options.fast_poll = cached_driver == "whatsminer" && !full_refresh;

        let fetch_result = if cached_driver == "whatsminer" {
            fetch_whatsminer(&client, ip, port, &options)
        } else {
            fetch_with_detect(&client, ip, port, &options)
        };

        match fetch_result {
            Ok(mut snapshot) => {
                if cached_driver.is_empty() {
                    cached_driver = snapshot.identity.driver_id.clone();
                }
                if options.fast_poll {
                    if snapshot.board_chips.is_empty() && !cached_board_chips.is_empty() {
                        snapshot.board_chips = cached_board_chips.clone();
                    }
                } else if !snapshot.board_chips.is_empty() {
                    cached_board_chips = snapshot.board_chips.clone();
                }

                let t_ms = started.elapsed().as_millis() as u64;
                if let Some(session) = session.as_mut() {
                    if session.driver_id == "unknown" {
                        session.driver_id = snapshot.identity.driver_id.clone();
                    }
                    session.push_recorded_frame(t_ms, snapshot.clone());
                }

                if let Some(state) = app.try_state::<AppState>() {
                    *state.last_snapshot.lock().unwrap() = Some(snapshot.clone());
                }

                let _ = app.emit(
                    "poll://snapshot",
                    PollSnapshotPayload {
                        snapshot,
                        t_ms,
                        frame_index,
                        recording,
                    },
                );
                frame_index += 1;
            }
            Err(err) => {
                finish_error = Some(format!("{:?}", err.code()));
                finish_reason = "error".into();
                break;
            }
        }

        if cancel.load(Ordering::Relaxed) {
            break;
        }
        if recording && started.elapsed() >= max_duration {
            finish_reason = "max_duration".into();
            break;
        }

        if recording && started.elapsed() >= max_duration {
            finish_reason = "max_duration".into();
            break;
        }

        let wait = poll_wait_after_tick(tick_start, interval, Instant::now());
        let mut slept = Duration::ZERO;
        while slept < wait {
            if cancel.load(Ordering::Relaxed) {
                break;
            }
            let step = Duration::from_millis(200).min(wait - slept);
            std::thread::sleep(step);
            slept += step;
        }
    }

    let frame_count = frame_index;
    if let (Some(mut session), Some(path)) = (session, record_path) {
        session.trim_to_max_duration(MAX_SESSION_DURATION_SEC * 1000);
        if session.frames.is_empty() {
            finish_error = Some("no_frames".into());
        } else if let Err(err) = save_session(&path, &session) {
            finish_error = Some(format!("{:?}", err.code()));
        } else {
            saved_path = Some(path.to_string_lossy().to_string());
        }
    }

    let _ = app.emit(
        "poll://finished",
        PollFinishedPayload {
            reason: finish_reason,
            saved_path,
            frame_count,
            error: finish_error,
        },
    );
}

#[tauri::command]
fn stop_poll(state: State<'_, AppState>) {
    state.poll.stop();
}

#[tauri::command]
fn load_session_file(path: String) -> Result<MpulseFile, ErrorResponse> {
    load_mpulse(PathBuf::from(path).as_path()).map_err(|e| ErrorResponse::from(&e))
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
    let path = PathBuf::from(&path);
    let meta = std::fs::metadata(&path).map_err(|_| io_error())?;

    if path
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("mpulse"))
    {
        let file = load_mpulse(&path).map_err(|e| ErrorResponse::from(&e))?;
        let frame = file
            .frames
            .last()
            .or_else(|| file.frames.first())
            .ok_or(MinerPulseError::with_code(
                minerpulse_core::ErrorCode::ParseFailed,
            ))
            .map_err(|e| ErrorResponse::from(&e))?;

        return Ok(ParseImportResponse {
            snapshot: frame.snapshot.clone(),
            source_label: path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("import.mpulse")
                .to_string(),
            miner_ip: Some(file.miner_ip),
        });
    }

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

#[derive(Debug, Serialize, Deserialize)]
struct SendMinerCommandRequest {
    ip: String,
    port: Option<u16>,
    driver_id: String,
    parameter: String,
}

#[derive(Debug, Serialize)]
struct SendMinerCommandResponse {
    response: String,
}

#[tauri::command]
fn send_miner_command(
    state: State<'_, AppState>,
    request: SendMinerCommandRequest,
) -> Result<SendMinerCommandResponse, ErrorResponse> {
    let tier = *state.tier.lock().unwrap();
    if !gate(tier).can_poll() {
        return Err(ErrorResponse {
            code: minerpulse_core::ErrorCode::NotSupported,
            args: None,
        });
    }

    if request.driver_id != "avalon" {
        return Err(ErrorResponse {
            code: minerpulse_core::ErrorCode::NotSupported,
            args: None,
        });
    }

    let port = request.port.unwrap_or(4028);
    let client = TcpCgminerClient::default();
    let response = minerpulse_core::send_ascset(&client, &request.ip, port, &request.parameter)
        .map_err(|e| ErrorResponse::from(&e))?;

    Ok(SendMinerCommandResponse { response })
}

fn format_app_display(product: &str, version: &str, build: u32) -> String {
    format!("{product} {version} ({build})")
}

#[tauri::command]
fn get_app_version() -> AppVersionInfo {
    let meta = serde_json::from_str::<Value>(env!("MINERPULSE_VERSION_JSON")).unwrap_or_else(|_| {
        serde_json::json!({
            "version": "0.0.0",
            "build": 0,
            "product": "Miner Pulse"
        })
    });
    let version = meta["version"].as_str().unwrap_or("0.0.0").to_string();
    let build = meta["build"].as_u64().unwrap_or(0) as u32;
    let product = meta["product"].as_str().unwrap_or("Miner Pulse").to_string();
    AppVersionInfo {
        display: format_app_display(&product, &version, build),
        version,
        build,
        product,
    }
}

#[cfg(windows)]
fn apply_windows_frame(window: &tauri::WebviewWindow, dark: bool) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_BORDER_COLOR};

    let Ok(raw) = window.hwnd() else {
        return;
    };
    let hwnd = HWND(raw.0 as _);
    // Match --bg-base in app.css (dark #0f1117, light #f5f6f8), BGR for DWM.
    let border_color: u32 = if dark { 0x0017_110f } else { 0x00f8_f6f5 };
    unsafe {
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_BORDER_COLOR,
            &border_color as *const _ as _,
            std::mem::size_of::<u32>() as u32,
        );
    }
}

#[cfg(not(windows))]
fn apply_windows_frame(_window: &tauri::WebviewWindow, _dark: bool) {}

#[tauri::command]
fn sync_window_frame(app: tauri::AppHandle, theme: String) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    apply_windows_frame(&window, theme == "dark");
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
            tier: Mutex::new(default_tier()),
            last_snapshot: Mutex::new(None),
            scan: ScanSession::new(),
            poll: PollSession::new(),
        })
        .invoke_handler(tauri::generate_handler![
            get_entitlements,
            set_tier,
            read_miner,
            start_poll,
            stop_poll,
            get_poll_status,
            load_session_file,
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
            send_miner_command,
            sync_window_frame,
        ])
        .setup(|app| {
            if let Some(window) = app.get_webview_window("main") {
                let meta = serde_json::from_str::<serde_json::Value>(env!("MINERPULSE_VERSION_JSON"))
                    .unwrap_or_else(|_| {
                        serde_json::json!({
                            "version": "0.0.0",
                            "build": 0,
                            "product": "Miner Pulse"
                        })
                    });
                let version = meta["version"].as_str().unwrap_or("0.0.0");
                let build = meta["build"].as_u64().unwrap_or(0) as u32;
                let product = meta["product"].as_str().unwrap_or("Miner Pulse");
                let title = format_app_display(product, version, build);
                let _ = window.set_title(&title);
                let _ = window.set_shadow(false);
                apply_windows_frame(&window, true);
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
