use minerpulse_core::drivers::registry::{fetch_whatsminer, fetch_with_detect};
use minerpulse_core::{
    enable_api_switch, import_file_content, list_scan_subnets as discover_subnets, load_mpulse,
    probe_whatsminer_access, save_session, save_snapshot, scan_network, scan_network_streaming,
    test_luci_credentials, compute_needs_setup, EntitlementGate, ErrorResponse, FetchOptions, MinerPulseError,
    MinerSnapshot, MpulseFile, RateLimiter, ScanRequest, ScanResult, ScanSubnet, SubscriptionTier,
    TcpCgminerClient, WhatsminerAccessInfo, WhatsminerLuciAuth,
    MAX_SESSION_DURATION_SEC, normalize_poll_rate_hz, poll_interval_ms, poll_wait_after_tick,
};
use minerpulse_core::model::BoardChipMap;

mod license;
mod miner_credentials;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindowBuilder, WindowEvent};
use tokio::time::timeout;

const READ_MINER_TIMEOUT: Duration = Duration::from_secs(15);

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

/// One active manual read; starting a new read cancels the previous job cooperatively.
struct ReadSession {
    active_cancel: Mutex<Option<Arc<AtomicBool>>>,
}

impl ReadSession {
    fn new() -> Self {
        Self {
            active_cancel: Mutex::new(None),
        }
    }

    fn begin(&self) -> Arc<AtomicBool> {
        let mut guard = self.active_cancel.lock().unwrap();
        if let Some(previous) = guard.take() {
            previous.store(true, Ordering::SeqCst);
        }
        let cancel = Arc::new(AtomicBool::new(false));
        *guard = Some(cancel.clone());
        cancel
    }

    fn cancel_active(&self) {
        let mut guard = self.active_cancel.lock().unwrap();
        if let Some(active) = guard.take() {
            active.store(true, Ordering::SeqCst);
        }
    }

    fn finish(&self, cancel: &Arc<AtomicBool>) {
        let mut guard = self.active_cancel.lock().unwrap();
        if guard
            .as_ref()
            .is_some_and(|active| Arc::ptr_eq(active, cancel))
        {
            *guard = None;
        }
    }
}

struct AppState {
    rate_limiter: Mutex<RateLimiter>,
    tier: Mutex<SubscriptionTier>,
    last_snapshot: Mutex<Option<minerpulse_core::MinerSnapshot>>,
    scan: ScanSession,
    poll: PollSession,
}

/// One miner network operation at a time (read, probe, auth test).
struct MinerIoGate(std::sync::Arc<std::sync::Mutex<()>>);

impl MinerIoGate {
    fn new() -> Self {
        Self(std::sync::Arc::new(std::sync::Mutex::new(())))
    }
}

fn read_fetch_options(
    auth: Option<WhatsminerAuthRequest>,
    cloud_auth: Option<WhatsminerLuciAuth>,
    cancel: Option<Arc<AtomicBool>>,
) -> FetchOptions {
    let luci_auth = auth
        .map(|value| WhatsminerLuciAuth {
            username: value.username,
            password: value.password,
        })
        .or(cloud_auth);
    FetchOptions {
        luci_auth,
        fast_poll: true,
        fetch_chips: true,
        cancel,
    }
}

#[tauri::command]
fn cancel_read_miner(read_session: State<ReadSession>) {
    read_session.cancel_active();
}

#[tauri::command]
async fn read_miner(
    app: AppHandle,
    request: ReadMinerRequest,
) -> Result<ReadMinerResponse, ErrorResponse> {
    let tier = *app.state::<AppState>().tier.lock().unwrap();
    let g = gate(tier);

    if !g.can_poll() {
        let state = app.state::<AppState>();
        let mut limiter = state.rate_limiter.lock().unwrap();
        limiter
            .try_acquire()
            .map_err(|e| ErrorResponse::from(&e))?;
    }

    let port = request.port.unwrap_or(4028);
    let cloud_auth = if request.whatsminer_auth.is_none() {
        app.state::<miner_credentials::MinerCredentialsState>()
            .try_resolve_auth_for_ip(&request.ip)
    } else {
        None
    };
    let read_session = app.state::<ReadSession>();
    let cancel = read_session.begin();
    let options = read_fetch_options(request.whatsminer_auth, cloud_auth, Some(cancel.clone()));
    let ip = request.ip.clone();
    let gate = app.state::<MinerIoGate>().0.clone();

    let join = tauri::async_runtime::spawn_blocking(move || {
        let _io = gate.lock().unwrap_or_else(|e| e.into_inner());
        let client = TcpCgminerClient::for_read();
        fetch_with_detect(&client, &ip, port, &options)
    });
    let fetch_result = timeout(READ_MINER_TIMEOUT, join).await;
    if fetch_result.is_err() {
        cancel.store(true, Ordering::SeqCst);
    }
    read_session.finish(&cancel);

    let snapshot = fetch_result
        .map_err(|_| ErrorResponse {
            code: minerpulse_core::ErrorCode::ConnTimeout,
            args: None,
        })?
        .map_err(|_| ErrorResponse {
            code: minerpulse_core::ErrorCode::InvalidInput,
            args: None,
        })?
        .map_err(|e| ErrorResponse::from(&e))?;

    if let Some(access) = &snapshot.whatsminer_access {
        if let Some(mac) = &access.mac {
            app.state::<miner_credentials::MinerCredentialsState>()
                .remember_ip_mac(&request.ip, mac);
        }
    }

    *app.state::<AppState>()
        .last_snapshot
        .lock()
        .unwrap() = Some(snapshot.clone());

    Ok(ReadMinerResponse { snapshot })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

fn fetch_options_from_request(
    auth: Option<WhatsminerAuthRequest>,
    cloud_auth: Option<WhatsminerLuciAuth>,
) -> FetchOptions {
    let luci_auth = auth
        .map(|value| WhatsminerLuciAuth {
            username: value.username,
            password: value.password,
        })
        .or(cloud_auth);
    FetchOptions {
        luci_auth,
        fast_poll: false,
        fetch_chips: false,
        cancel: None,
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

#[derive(Debug, Serialize, Deserialize)]
struct WhatsminerHostRequest {
    ip: String,
    #[serde(default)]
    whatsminer_auth: Option<WhatsminerAuthRequest>,
}

#[derive(Debug, Serialize)]
struct ProbeWhatsminerResponse {
    access: WhatsminerAccessInfo,
}

#[tauri::command(rename = "probe_whatsminer_access")]
async fn probe_whatsminer_access_command(
    app: AppHandle,
    creds: State<'_, miner_credentials::MinerCredentialsState>,
    request: WhatsminerHostRequest,
) -> Result<ProbeWhatsminerResponse, ErrorResponse> {
    let cloud_auth = if request.whatsminer_auth.is_none() {
        creds.try_resolve_auth_for_ip(&request.ip)
    } else {
        None
    };
    let options = fetch_options_from_request(request.whatsminer_auth, cloud_auth);
    let ip = request.ip.clone();
    let gate = app.state::<MinerIoGate>().0.clone();
    let status = tauri::async_runtime::spawn_blocking(move || {
        let _io = gate.lock().unwrap_or_else(|e| e.into_inner());
        probe_whatsminer_access(&ip, &options, false)
    })
        .await
        .map_err(|_| ErrorResponse {
            code: minerpulse_core::ErrorCode::InvalidInput,
            args: None,
        })?;
    let needs_setup = compute_needs_setup(&status, true, true);
    if let Some(mac) = &status.mac {
        creds.remember_ip_mac(&request.ip, mac);
    }
    Ok(ProbeWhatsminerResponse {
        access: status.to_info(needs_setup),
    })
}

#[derive(Debug, Serialize, Deserialize)]
struct EnableWhatsminerApiRequest {
    ip: String,
    username: String,
    password: String,
}

#[derive(Debug, Serialize)]
struct EnableWhatsminerApiResponse {
    enabled: bool,
    access: WhatsminerAccessInfo,
}

#[tauri::command]
async fn enable_whatsminer_api(
    app: AppHandle,
    creds: State<'_, miner_credentials::MinerCredentialsState>,
    request: EnableWhatsminerApiRequest,
) -> Result<EnableWhatsminerApiResponse, ErrorResponse> {
    let ip = request.ip.clone();
    let username = request.username.clone();
    let password = request.password.clone();
    let gate = app.state::<MinerIoGate>().0.clone();
    let enabled = tauri::async_runtime::spawn_blocking({
        let gate = gate.clone();
        let ip = ip.clone();
        let username = username.clone();
        let password = password.clone();
        move || {
            let _io = gate.lock().unwrap_or_else(|e| e.into_inner());
            enable_api_switch(&ip, &username, &password)
        }
    })
    .await
    .map_err(|_| ErrorResponse {
        code: minerpulse_core::ErrorCode::InvalidInput,
        args: None,
    })?;
    let options = read_fetch_options(
        Some(WhatsminerAuthRequest {
            username: request.username.clone(),
            password: request.password.clone(),
        }),
        None,
        None,
    );
    let ip = request.ip.clone();
    let status = tauri::async_runtime::spawn_blocking(move || {
        let _io = gate.lock().unwrap_or_else(|e| e.into_inner());
        probe_whatsminer_access(&ip, &options, false)
    })
        .await
        .map_err(|_| ErrorResponse {
            code: minerpulse_core::ErrorCode::InvalidInput,
            args: None,
        })?;
    if let Some(mac) = &status.mac {
        creds.remember_ip_mac(&request.ip, mac);
    }
    Ok(EnableWhatsminerApiResponse {
        enabled,
        access: status.to_info(!enabled || !status.luci_auth_ok),
    })
}

#[derive(Debug, Serialize, Deserialize)]
struct TestWhatsminerCredentialsRequest {
    ip: String,
    username: String,
    password: String,
}

#[derive(Debug, Serialize)]
struct TestWhatsminerCredentialsResponse {
    ok: bool,
    mac: Option<String>,
}

#[tauri::command(rename = "test_whatsminer_credentials")]
async fn test_whatsminer_credentials_command(
    app: AppHandle,
    creds: State<'_, miner_credentials::MinerCredentialsState>,
    request: TestWhatsminerCredentialsRequest,
) -> Result<TestWhatsminerCredentialsResponse, ErrorResponse> {
    let ip = request.ip.clone();
    let username = request.username.clone();
    let password = request.password.clone();
    let gate = app.state::<MinerIoGate>().0.clone();
    let (ok, mac) = tauri::async_runtime::spawn_blocking(move || {
        let _io = gate.lock().unwrap_or_else(|e| e.into_inner());
        let ok = test_luci_credentials(&ip, &username, &password);
        let mac = minerpulse_core::drivers::whatsminer::access::fetch_device_info(&ip)
            .and_then(|info| info.mac);
        (ok, mac)
    })
    .await
    .map_err(|_| ErrorResponse {
        code: minerpulse_core::ErrorCode::InvalidInput,
        args: None,
    })?;
    if ok {
        if let Some(ref mac) = mac {
            creds.remember_ip_mac(&request.ip, mac);
        }
    }
    Ok(TestWhatsminerCredentialsResponse { ok, mac })
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
    creds: State<'_, miner_credentials::MinerCredentialsState>,
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
    let cloud_auth = if request.whatsminer_auth.is_none() {
        creds.resolve_auth_for_ip(&request.ip)
    } else {
        None
    };
    let fetch_options = fetch_options_from_request(request.whatsminer_auth, cloud_auth);
    let cancel = Arc::clone(&state.poll.cancel);
    let app_for_poll = app.clone();
    let poll_ip = request.ip.clone();

    tauri::async_runtime::spawn(async move {
        let poll_result = tauri::async_runtime::spawn_blocking(move || {
            run_poll_loop(
                &app_for_poll,
                &poll_ip,
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

    let version_meta = serde_json::from_str::<Value>(env!("MINERPULSE_VERSION_JSON")).unwrap_or_else(|_| {
        serde_json::json!({
            "version": "0.0.0",
            "build": 0,
            "product": "Miner Pulse"
        })
    });
    let scan_title = format_app_display(
        version_meta["product"].as_str().unwrap_or("Miner Pulse"),
        version_meta["version"].as_str().unwrap_or("0.0.0"),
        version_meta["build"].as_u64().unwrap_or(0) as u32,
    );

    let scan_window = WebviewWindowBuilder::new(&app, SCAN_WINDOW_LABEL, WebviewUrl::App("/scan".into()))
        .title(&scan_title)
        .inner_size(540.0, 680.0)
        .min_inner_size(420.0, 480.0)
        .center()
        .decorations(false)
        .parent(&main)
        .map_err(|_| io_error())?
        .always_on_top(true)
        .shadow(true)
        .build()
        .map_err(|_| io_error())?;

    configure_window_chrome(&scan_window, true);

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
fn is_windows_10() -> bool {
    let info = os_info::get();
    if info.os_type() != os_info::Type::Windows {
        return false;
    }
    info.version()
        .to_string()
        .rsplit('.')
        .next()
        .and_then(|s| s.parse::<u32>().ok())
        .map_or(false, |build| build < 22000)
}

/// Win10: OS shadow off (colored side stripes). Win11: OS shadow on + DWM border = app bg.
#[cfg(windows)]
fn configure_window_chrome(window: &tauri::WebviewWindow, dark: bool) {
    let win10 = is_windows_10();
    let _ = window.set_shadow(!win10);
    if !win10 {
        apply_windows_frame(window, dark);
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
    // BGR: match --bg-base so Win11 does not paint the default accent border.
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
fn configure_window_chrome(window: &tauri::WebviewWindow, dark: bool) {
    let _ = dark;
    let _ = window.set_shadow(true);
}

#[cfg(not(windows))]
fn apply_windows_frame(_window: &tauri::WebviewWindow, _dark: bool) {}

#[tauri::command]
fn sync_window_frame(app: tauri::AppHandle, theme: String) {
    let dark = theme == "dark";
    for label in [MAIN_WINDOW_LABEL, SCAN_WINDOW_LABEL] {
        if let Some(window) = app.get_webview_window(label) {
            configure_window_chrome(&window, dark);
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .manage(MinerIoGate::new())
        .manage(ReadSession::new())
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
            license::get_license_info,
            license::activate_license,
            license::login_license,
            license::logout_license,
            license::refresh_license,
            miner_credentials::sync_miner_credentials,
            miner_credentials::save_miner_credential,
            miner_credentials::list_miner_credentials,
            probe_whatsminer_access_command,
            enable_whatsminer_api,
            test_whatsminer_credentials_command,
            cancel_read_miner,
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
            let license = license::LicenseState::new(app.handle())
                .map_err(|e| format!("license init failed: {:?}", e.code))?;
            app.manage(license);
            let miner_creds = miner_credentials::MinerCredentialsState::new(app.handle())
                .map_err(|e| format!("miner credentials init failed: {:?}", e.code))?;
            app.manage(miner_creds);
            miner_credentials::MinerCredentialsState::spawn_periodic_sync(app.handle().clone());
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let sync_app = app_handle.clone();
                if let Some(state) = app_handle.try_state::<license::LicenseState>() {
                    state.sync_on_startup(sync_app).await;
                }
            });
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
                configure_window_chrome(&window, true);
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
