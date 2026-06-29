use minerpulse_core::{ErrorCode, ErrorResponse, SubscriptionTier};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Manager, State};

use crate::AppState;

const LICENSE_FILE: &str = "license.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DeviceIdentity {
    hwid: String,
    os: String,
    os_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LicenseStore {
    access_token: Option<String>,
    refresh_token: Option<String>,
    tier: SubscriptionTier,
    plan_name: Option<String>,
    expires_at: Option<String>,
    user_email: Option<String>,
    user_nickname: Option<String>,
    last_sync_unix: i64,
    offline_grace_days: u32,
    #[serde(alias = "device_fingerprint")]
    hwid: String,
    os: String,
    os_version: String,
}

impl Default for LicenseStore {
    fn default() -> Self {
        let device = device_identity();
        Self {
            access_token: None,
            refresh_token: None,
            tier: SubscriptionTier::Free,
            plan_name: None,
            expires_at: None,
            user_email: None,
            user_nickname: None,
            last_sync_unix: 0,
            offline_grace_days: 14,
            hwid: device.hwid,
            os: device.os,
            os_version: device.os_version,
        }
    }
}

pub struct LicenseState {
    store: Mutex<LicenseStore>,
    client: reqwest::Client,
    path: PathBuf,
}

#[derive(Debug, Serialize, Deserialize)]
struct ActivateResponse {
    access_token: String,
    refresh_token: String,
    tier: String,
    plan_name: String,
    expires_at: Option<String>,
    offline_grace_days: u32,
}

#[derive(Debug, Serialize, Deserialize)]
struct RefreshResponse {
    access_token: String,
    tier: String,
    plan_name: String,
    expires_at: Option<String>,
    offline_grace_days: u32,
}

#[derive(Debug, Serialize, Deserialize)]
struct LoginResponse {
    access_token: String,
    refresh_token: String,
    tier: String,
    plan_name: Option<String>,
    expires_at: Option<String>,
    subscription_active: bool,
    user: LoginUser,
}

#[derive(Debug, Serialize, Deserialize)]
struct LoginUser {
    email: String,
    nickname: String,
}

#[derive(Debug, Serialize)]
pub struct LicenseInfo {
    pub tier: SubscriptionTier,
    pub plan_name: Option<String>,
    pub expires_at: Option<String>,
    pub user_email: Option<String>,
    pub user_nickname: Option<String>,
    pub licensed: bool,
}

fn api_base() -> &'static str {
    option_env!("MINERPULSE_LICENSE_API_URL").unwrap_or("https://api.mpulse.bob4.fun")
}

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn device_identity() -> DeviceIdentity {
    let hwid = machine_uid::get().unwrap_or_else(|_| fallback_hwid());
    let info = os_info::get();
    DeviceIdentity {
        hwid,
        os: info.os_type().to_string(),
        os_version: info.version().to_string(),
    }
}

fn fallback_hwid() -> String {
    let digest = Sha256::digest(b"minerpulse-device-fallback");
    hex::encode(digest)
}

fn ensure_device_identity(store: &mut LicenseStore) {
    let current = device_identity();
    if store.hwid.is_empty() {
        store.hwid = current.hwid;
    }
    store.os = current.os;
    store.os_version = current.os_version;
}

fn device_payload(store: &LicenseStore, app_version: Option<&str>) -> serde_json::Value {
    let mut payload = serde_json::json!({
        "hwid": store.hwid,
        "os": store.os,
        "os_version": store.os_version,
    });
    if let Some(version) = app_version {
        payload["app_version"] = serde_json::Value::String(version.to_string());
    }
    payload
}

fn tier_from_api(raw: &str) -> SubscriptionTier {
    match raw.to_uppercase().as_str() {
        "CLIENT" => SubscriptionTier::Client,
        "SERVICE" => SubscriptionTier::Service,
        _ => SubscriptionTier::Free,
    }
}

fn license_err(message: &str) -> ErrorResponse {
    ErrorResponse {
        code: ErrorCode::InvalidInput,
        args: Some(serde_json::json!({ "message": message })),
    }
}

impl LicenseState {
    pub fn new(app: &AppHandle) -> Result<Self, ErrorResponse> {
        let dir = app
            .path()
            .app_data_dir()
            .map_err(|_| license_err("app_data_dir"))?;
        fs::create_dir_all(&dir).map_err(|_| license_err("io_error"))?;
        let path = dir.join(LICENSE_FILE);
        let mut store = if path.exists() {
            fs::read_to_string(&path)
                .ok()
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_default()
        } else {
            LicenseStore::default()
        };
        ensure_device_identity(&mut store);
        Ok(Self {
            store: Mutex::new(store),
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(20))
                .build()
                .map_err(|_| license_err("http_client"))?,
            path,
        })
    }

    fn save(&self, store: &LicenseStore) -> Result<(), ErrorResponse> {
        let json = serde_json::to_string_pretty(store).map_err(|_| license_err("serialize"))?;
        fs::write(&self.path, json).map_err(|_| license_err("io_error"))?;
        Ok(())
    }

    fn apply_tier(&self, app: &AppHandle, tier: SubscriptionTier) {
        if let Some(state) = app.try_state::<AppState>() {
            *state.tier.lock().unwrap() = tier;
        }
    }

    fn effective_tier(store: &LicenseStore) -> SubscriptionTier {
        if store.last_sync_unix == 0 {
            return SubscriptionTier::Free;
        }
        let elapsed_days = (now_unix() - store.last_sync_unix).max(0) / 86400;
        if elapsed_days as u32 > store.offline_grace_days {
            return SubscriptionTier::Free;
        }
        store.tier
    }

    pub fn info(&self) -> LicenseInfo {
        let store = self.store.lock().unwrap();
        let tier = Self::effective_tier(&store);
        LicenseInfo {
            tier,
            plan_name: store.plan_name.clone(),
            expires_at: store.expires_at.clone(),
            user_email: store.user_email.clone(),
            user_nickname: store.user_nickname.clone(),
            licensed: tier != SubscriptionTier::Free,
        }
    }

    pub async fn sync_on_startup(&self, app: AppHandle) {
        let refresh = {
            let store = self.store.lock().unwrap();
            store.refresh_token.clone()
        };
        if refresh.is_some() {
            let _ = self.refresh_remote(&app).await;
        }
        let tier = {
            let store = self.store.lock().unwrap();
            Self::effective_tier(&store)
        };
        self.apply_tier(&app, tier);
    }

    async fn refresh_remote(&self, app: &AppHandle) -> Result<(), ErrorResponse> {
        let (refresh, device) = {
            let mut store = self.store.lock().unwrap();
            ensure_device_identity(&mut store);
            (
                store
                    .refresh_token
                    .clone()
                    .ok_or_else(|| license_err("no_refresh_token"))?,
                device_payload(&store, Some(env!("CARGO_PKG_VERSION"))),
            )
        };

        let url = format!("{}/v1/license/refresh", api_base());
        let mut body = serde_json::Map::new();
        body.insert("refresh_token".to_string(), serde_json::Value::String(refresh));
        if let serde_json::Value::Object(device_obj) = device {
            for (key, value) in device_obj {
                body.insert(key, value);
            }
        }
        let res = self
            .client
            .post(&url)
            .json(&serde_json::Value::Object(body))
            .send()
            .await
            .map_err(|_| license_err("network_error"))?;

        if !res.status().is_success() {
            return Err(license_err("refresh_failed"));
        }

        let body: RefreshResponse = res.json().await.map_err(|_| license_err("parse_error"))?;
        let mut store = self.store.lock().unwrap();
        store.access_token = Some(body.access_token);
        store.tier = tier_from_api(&body.tier);
        store.plan_name = Some(body.plan_name);
        store.expires_at = body.expires_at;
        store.offline_grace_days = body.offline_grace_days;
        store.last_sync_unix = now_unix();
        let tier = Self::effective_tier(&store);
        self.save(&store)?;
        self.apply_tier(app, tier);
        Ok(())
    }

    pub async fn activate(&self, app: AppHandle, code: String) -> Result<LicenseInfo, ErrorResponse> {
        let device = {
            let mut store = self.store.lock().unwrap();
            ensure_device_identity(&mut store);
            device_payload(&store, Some(env!("CARGO_PKG_VERSION")))
        };
        let url = format!("{}/v1/license/activate", api_base());
        let mut body = serde_json::Map::new();
        body.insert("code".to_string(), serde_json::Value::String(code.trim().to_string()));
        if let serde_json::Value::Object(device_obj) = device {
            for (key, value) in device_obj {
                body.insert(key, value);
            }
        }
        let res = self
            .client
            .post(&url)
            .json(&serde_json::Value::Object(body))
            .send()
            .await
            .map_err(|_| license_err("network_error"))?;

        let status = res.status();
        let data: serde_json::Value = res.json().await.unwrap_or_default();
        if !status.is_success() {
            let msg = data
                .get("error")
                .and_then(|v| v.as_str())
                .unwrap_or("activate_failed");
            return Err(license_err(msg));
        }

        let body: ActivateResponse =
            serde_json::from_value(data).map_err(|_| license_err("parse_error"))?;
        let mut store = self.store.lock().unwrap();
        store.access_token = Some(body.access_token);
        store.refresh_token = Some(body.refresh_token);
        store.tier = tier_from_api(&body.tier);
        store.plan_name = Some(body.plan_name);
        store.expires_at = body.expires_at;
        store.offline_grace_days = body.offline_grace_days;
        store.last_sync_unix = now_unix();
        let info = LicenseInfo {
            tier: store.tier,
            plan_name: store.plan_name.clone(),
            expires_at: store.expires_at.clone(),
            user_email: store.user_email.clone(),
            user_nickname: store.user_nickname.clone(),
            licensed: store.tier != SubscriptionTier::Free,
        };
        self.save(&store)?;
        self.apply_tier(&app, store.tier);
        Ok(info)
    }

    pub async fn login(
        &self,
        app: AppHandle,
        email: String,
        password: String,
    ) -> Result<LicenseInfo, ErrorResponse> {
        let device = {
            let mut store = self.store.lock().unwrap();
            ensure_device_identity(&mut store);
            device_payload(&store, Some(env!("CARGO_PKG_VERSION")))
        };
        let url = format!("{}/v1/auth/login", api_base());
        let mut body = serde_json::Map::new();
        body.insert("email".to_string(), serde_json::Value::String(email.trim().to_string()));
        body.insert("password".to_string(), serde_json::Value::String(password));
        if let serde_json::Value::Object(device_obj) = device {
            for (key, value) in device_obj {
                body.insert(key, value);
            }
        }
        let res = self
            .client
            .post(&url)
            .json(&serde_json::Value::Object(body))
            .send()
            .await
            .map_err(|_| license_err("network_error"))?;

        let status = res.status();
        let data: serde_json::Value = res.json().await.unwrap_or_default();
        if !status.is_success() {
            let msg = data
                .get("error")
                .and_then(|v| v.as_str())
                .unwrap_or("login_failed");
            return Err(license_err(msg));
        }

        let body: LoginResponse =
            serde_json::from_value(data).map_err(|_| license_err("parse_error"))?;
        let mut store = self.store.lock().unwrap();
        store.access_token = Some(body.access_token);
        store.refresh_token = Some(body.refresh_token);
        store.tier = tier_from_api(&body.tier);
        store.plan_name = body.plan_name;
        store.expires_at = body.expires_at;
        store.user_email = Some(body.user.email);
        store.user_nickname = Some(body.user.nickname);
        store.offline_grace_days = 14;
        store.last_sync_unix = now_unix();
        if !body.subscription_active {
            store.tier = SubscriptionTier::Free;
        }
        let info = LicenseInfo {
            tier: store.tier,
            plan_name: store.plan_name.clone(),
            expires_at: store.expires_at.clone(),
            user_email: store.user_email.clone(),
            user_nickname: store.user_nickname.clone(),
            licensed: store.tier != SubscriptionTier::Free,
        };
        self.save(&store)?;
        self.apply_tier(&app, store.tier);
        Ok(info)
    }

    pub fn logout(&self, app: &AppHandle) -> Result<LicenseInfo, ErrorResponse> {
        let mut store = self.store.lock().unwrap();
        *store = LicenseStore {
            hwid: store.hwid.clone(),
            os: store.os.clone(),
            os_version: store.os_version.clone(),
            ..LicenseStore::default()
        };
        self.save(&store)?;
        self.apply_tier(app, SubscriptionTier::Free);
        Ok(self.info())
    }
}

#[tauri::command]
pub async fn get_license_info(state: State<'_, LicenseState>) -> Result<LicenseInfo, ErrorResponse> {
    Ok(state.info())
}

#[tauri::command]
pub async fn activate_license(
    app: AppHandle,
    state: State<'_, LicenseState>,
    code: String,
) -> Result<LicenseInfo, ErrorResponse> {
    state.activate(app, code).await
}

#[tauri::command]
pub async fn login_license(
    app: AppHandle,
    state: State<'_, LicenseState>,
    email: String,
    password: String,
) -> Result<LicenseInfo, ErrorResponse> {
    state.login(app, email, password).await
}

#[tauri::command]
pub async fn logout_license(
    app: AppHandle,
    state: State<'_, LicenseState>,
) -> Result<LicenseInfo, ErrorResponse> {
    state.logout(&app)
}

#[tauri::command]
pub async fn refresh_license(
    app: AppHandle,
    state: State<'_, LicenseState>,
) -> Result<LicenseInfo, ErrorResponse> {
    state.refresh_remote(&app).await?;
    Ok(state.info())
}
