use minerpulse_core::{
    save_snapshot, EntitlementGate, ErrorResponse, MinerPulseError, MpulseFile, RateLimiter,
    SubscriptionTier, TcpCgminerClient,
};
use minerpulse_core::drivers::registry::fetch_with_detect;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::State;
struct AppState {
    rate_limiter: Mutex<RateLimiter>,
    tier: Mutex<SubscriptionTier>,
    last_snapshot: Mutex<Option<minerpulse_core::MinerSnapshot>>,
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

fn gate(tier: SubscriptionTier) -> EntitlementGate {
    EntitlementGate::new(tier)
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
    let snapshot = fetch_with_detect(&client, &request.ip, port).map_err(|e| ErrorResponse::from(&e))?;

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
        })
        .invoke_handler(tauri::generate_handler![
            get_entitlements,
            set_tier,
            read_miner,
            save_snapshot_file,
            get_app_version,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
