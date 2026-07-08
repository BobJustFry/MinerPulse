use minerpulse_core::{
    drivers::whatsminer::access::normalize_mac,
    ErrorCode, ErrorResponse, WhatsminerLuciAuth,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Manager};

use crate::license::LicenseState;

const CREDENTIALS_FILE: &str = "miner-credentials.json";
pub const SYNC_INTERVAL_MS: u64 = 900_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MinerCredentialEntry {
    mac: String,
    username: String,
    password: String,
    #[serde(default)]
    updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct MinerCredentialsStore {
    #[serde(default)]
    credentials: Vec<MinerCredentialEntry>,
    #[serde(default)]
    ip_mac: HashMap<String, String>,
    #[serde(default)]
    last_sync_unix: i64,
}

pub struct MinerCredentialsState {
    store: Mutex<MinerCredentialsStore>,
    client: reqwest::Client,
    path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinerCredentialMeta {
    pub mac: String,
    pub username: String,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinerCredentialSyncItem {
    pub mac: String,
    pub username: String,
    pub password: String,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SyncResponse {
    credentials: Vec<MinerCredentialSyncItem>,
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

fn cred_err(message: &str) -> ErrorResponse {
    ErrorResponse {
        code: ErrorCode::InvalidInput,
        args: Some(serde_json::json!({ "message": message })),
    }
}

impl MinerCredentialsState {
    pub fn new(app: &AppHandle) -> Result<Self, ErrorResponse> {
        let dir = app
            .path()
            .app_data_dir()
            .map_err(|_| cred_err("app_data_dir"))?;
        fs::create_dir_all(&dir).map_err(|_| cred_err("io_error"))?;
        let path = dir.join(CREDENTIALS_FILE);
        let store = if path.exists() {
            fs::read_to_string(&path)
                .ok()
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_default()
        } else {
            MinerCredentialsStore::default()
        };
        Ok(Self {
            store: Mutex::new(store),
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(20))
                .build()
                .map_err(|_| cred_err("http_client"))?,
            path,
        })
    }

    fn save(&self, store: &MinerCredentialsStore) -> Result<(), ErrorResponse> {
        let json = serde_json::to_string_pretty(store).map_err(|_| cred_err("serialize"))?;
        fs::write(&self.path, json).map_err(|_| cred_err("io_error"))?;
        Ok(())
    }

    fn access_token(app: &AppHandle) -> Option<String> {
        app.try_state::<LicenseState>()?
            .access_token()
    }

    pub fn remember_ip_mac(&self, ip: &str, mac: &str) {
        let mac = normalize_mac(mac);
        let mut store = self.store.lock().unwrap();
        store.ip_mac.insert(ip.to_string(), mac);
        let _ = self.save(&store);
    }

    /// Best-effort IP↔MAC cache update that never blocks the caller (read path).
    /// Returns `false` if the store was busy and the mapping was skipped.
    pub fn try_remember_ip_mac(&self, ip: &str, mac: &str) -> bool {
        let mac = normalize_mac(mac);
        let Ok(mut store) = self.store.try_lock() else {
            return false;
        };
        if store.ip_mac.get(ip).map(String::as_str) == Some(mac.as_str()) {
            return true;
        }
        store.ip_mac.insert(ip.to_string(), mac);
        let _ = self.save(&store);
        true
    }

    pub fn try_resolve_auth_for_ip(&self, ip: &str) -> Option<WhatsminerLuciAuth> {
        let mac = self.store.try_lock().ok()?.ip_mac.get(ip).cloned()?;
        self.try_resolve_auth_for_mac(&mac)
    }

    pub fn try_resolve_auth_for_mac(&self, mac: &str) -> Option<WhatsminerLuciAuth> {
        let mac = normalize_mac(mac);
        let store = self.store.try_lock().ok()?;
        store
            .credentials
            .iter()
            .find(|entry| entry.mac == mac)
            .map(|entry| WhatsminerLuciAuth {
                username: entry.username.clone(),
                password: entry.password.clone(),
            })
    }

    pub fn resolve_auth_for_ip(&self, ip: &str) -> Option<WhatsminerLuciAuth> {
        let mac = self.store.lock().unwrap().ip_mac.get(ip).cloned()?;
        self.resolve_auth_for_mac(&mac)
    }

    pub fn resolve_auth_for_mac(&self, mac: &str) -> Option<WhatsminerLuciAuth> {
        let mac = normalize_mac(mac);
        let store = self.store.lock().unwrap();
        store
            .credentials
            .iter()
            .find(|entry| entry.mac == mac)
            .map(|entry| WhatsminerLuciAuth {
                username: entry.username.clone(),
                password: entry.password.clone(),
            })
    }

    pub fn upsert_local(&self, mac: &str, username: &str, password: &str) -> Result<(), ErrorResponse> {
        let mac = normalize_mac(mac);
        let mut store = self.store.lock().unwrap();
        if let Some(entry) = store.credentials.iter_mut().find(|e| e.mac == mac) {
            entry.username = username.trim().to_string();
            entry.password = password.to_string();
            entry.updated_at = None;
        } else {
            store.credentials.push(MinerCredentialEntry {
                mac,
                username: username.trim().to_string(),
                password: password.to_string(),
                updated_at: None,
            });
        }
        self.save(&store)
    }

    pub fn list_local(&self) -> Vec<MinerCredentialMeta> {
        let store = self.store.lock().unwrap();
        store
            .credentials
            .iter()
            .map(|entry| MinerCredentialMeta {
                mac: entry.mac.clone(),
                username: entry.username.clone(),
                updated_at: entry.updated_at.clone(),
            })
            .collect()
    }

    pub async fn push_remote(
        &self,
        app: &AppHandle,
        mac: &str,
        username: &str,
        password: &str,
    ) -> Result<(), ErrorResponse> {
        let token = Self::access_token(app).ok_or_else(|| cred_err("not_logged_in"))?;
        let url = format!("{}/v1/account/miner-credentials", api_base());
        let body = serde_json::json!({
            "mac": normalize_mac(mac),
            "username": username.trim(),
            "password": password,
        });
        let res = self
            .client
            .put(url)
            .bearer_auth(token)
            .json(&body)
            .send()
            .await
            .map_err(|_| cred_err("network_error"))?;
        if !res.status().is_success() {
            return Err(cred_err("save_failed"));
        }
        self.upsert_local(mac, username, password)?;
        Ok(())
    }

    pub async fn pull_remote(&self, app: &AppHandle) -> Result<Vec<MinerCredentialMeta>, ErrorResponse> {
        let token = Self::access_token(app).ok_or_else(|| cred_err("not_logged_in"))?;
        let url = format!("{}/v1/account/miner-credentials/sync", api_base());
        let res = self
            .client
            .post(url)
            .bearer_auth(token)
            .send()
            .await
            .map_err(|_| cred_err("network_error"))?;
        if !res.status().is_success() {
            return Err(cred_err("sync_failed"));
        }
        let body: SyncResponse = res.json().await.map_err(|_| cred_err("parse_error"))?;
        let mut store = self.store.lock().unwrap();
        for item in body.credentials {
            let mac = normalize_mac(&item.mac);
            let updated_at = item.updated_at.clone();
            if let Some(entry) = store.credentials.iter_mut().find(|e| e.mac == mac) {
                entry.username = item.username;
                entry.password = item.password;
                entry.updated_at = updated_at;
            } else {
                store.credentials.push(MinerCredentialEntry {
                    mac,
                    username: item.username,
                    password: item.password,
                    updated_at,
                });
            }
        }
        store.last_sync_unix = now_unix();
        self.save(&store)?;
        Ok(self.list_local())
    }

    pub async fn sync_if_logged_in(&self, app: &AppHandle) {
        if Self::access_token(app).is_none() {
            return;
        }
        let _ = self.pull_remote(app).await;
    }

    pub fn spawn_periodic_sync(app: AppHandle) {
        tauri::async_runtime::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_millis(SYNC_INTERVAL_MS)).await;
                if let Some(state) = app.try_state::<MinerCredentialsState>() {
                    state.sync_if_logged_in(&app).await;
                }
            }
        });
    }

    pub fn schedule_sync(app: &AppHandle) {
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            if let Some(state) = app.try_state::<MinerCredentialsState>() {
                state.sync_if_logged_in(&app).await;
            }
        });
    }
}

#[tauri::command]
pub async fn sync_miner_credentials(
    app: AppHandle,
    state: tauri::State<'_, MinerCredentialsState>,
) -> Result<Vec<MinerCredentialMeta>, ErrorResponse> {
    state.pull_remote(&app).await
}

#[tauri::command]
pub async fn save_miner_credential(
    app: AppHandle,
    state: tauri::State<'_, MinerCredentialsState>,
    mac: String,
    username: String,
    password: String,
    ip: Option<String>,
) -> Result<(), ErrorResponse> {
    state.upsert_local(&mac, &username, &password)?;
    if let Some(ip) = ip {
        state.remember_ip_mac(&ip, &mac);
    }
    if MinerCredentialsState::access_token(&app).is_some() {
        let _ = state.push_remote(&app, &mac, &username, &password).await;
    }
    Ok(())
}

#[tauri::command]
pub fn list_miner_credentials(
    state: tauri::State<'_, MinerCredentialsState>,
) -> Vec<MinerCredentialMeta> {
    state.list_local()
}
