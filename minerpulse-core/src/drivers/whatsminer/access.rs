use super::options::WhatsminerFetchOptions;
use crate::model::WhatsminerAccessInfo;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use super::luci::enable_api_switch_luci;

pub const WHATSMINER_API_PORT: u16 = 4433;

#[derive(Debug, Clone, Default)]
pub struct WhatsminerAccessStatus {
    pub mac: Option<String>,
    pub api_switch: Option<bool>,
    pub luci_reachable: bool,
    pub luci_auth_ok: bool,
    pub api_reachable: bool,
    pub api_auth_ok: bool,
    /// Miner model from `get.device.info` → `msg.miner.type` (e.g. "M30S++").
    pub model: Option<String>,
    /// Mining state from `get.device.info` → `msg.miner.working`.
    pub working: Option<bool>,
}

impl WhatsminerAccessStatus {
    pub fn to_info(&self, needs_setup: bool) -> WhatsminerAccessInfo {
        WhatsminerAccessInfo {
            mac: self.mac.clone(),
            api_switch: self.api_switch,
            luci_reachable: self.luci_reachable,
            luci_auth_ok: self.luci_auth_ok,
            api_reachable: self.api_reachable,
            api_auth_ok: self.api_auth_ok,
            needs_setup,
        }
    }
}

pub fn probe_whatsminer_access(
    host: &str,
    options: &WhatsminerFetchOptions,
    skip_luci_probe: bool,
) -> WhatsminerAccessStatus {
    let mut status = WhatsminerAccessStatus::default();

    if let Some(info) = fetch_device_info(host) {
        status.api_reachable = true;
        status.api_auth_ok = info.code_ok;
        status.mac = info.mac;
        status.api_switch = info.api_switch;
        status.model = info.model;
        status.working = info.working;
    }

    if skip_luci_probe {
        status.luci_reachable = true;
        status.luci_auth_ok = true;
    } else {
        for (username, password) in options.luci_credential_pairs() {
            if super::luci::verify_luci_login(host, &username, &password) {
                status.luci_reachable = true;
                status.luci_auth_ok = true;
                break;
            }
        }

        if !status.luci_auth_ok {
            status.luci_reachable = super::luci::luci_reachable(host);
        }
    }

    if status.mac.is_none() {
        for (username, password) in options.luci_credential_pairs() {
            if let Some(mac) = super::luci::fetch_lan_macaddr(host, &username, &password) {
                status.mac = Some(mac);
                break;
            }
        }
    }

    status
}

pub fn enable_api_switch(host: &str, username: &str, password: &str) -> bool {
    if enable_api_switch_luci(host, username, password) {
        if let Some(info) = fetch_device_info(host) {
            return info.api_switch == Some(true);
        }
    }
    false
}

pub fn compute_needs_setup(
    status: &WhatsminerAccessStatus,
    snapshot_empty: bool,
    board_chips_empty: bool,
) -> bool {
    if !snapshot_empty && !board_chips_empty {
        return false;
    }
    if !snapshot_empty && board_chips_empty {
        return true;
    }
    let api_off = status.api_switch == Some(false);
    !status.luci_auth_ok || api_off || snapshot_empty
}

/// Minimal access probe after a fast read with empty chips (avoids slow LuCI login loops).
pub fn probe_whatsminer_access_fast(host: &str) -> WhatsminerAccessStatus {
    let mut status = WhatsminerAccessStatus::default();
    if let Some(info) = fetch_device_info_fast(host) {
        status.api_reachable = true;
        status.api_auth_ok = info.code_ok;
        status.mac = info.mac;
        status.api_switch = info.api_switch;
        status.model = info.model;
        status.working = info.working;
    }
    status
}

#[derive(Debug, Clone)]
pub struct DeviceInfoProbe {
    pub code_ok: bool,
    pub mac: Option<String>,
    pub api_switch: Option<bool>,
    pub salt: Option<String>,
    pub model: Option<String>,
    pub working: Option<bool>,
}

pub fn fetch_device_info(host: &str) -> Option<DeviceInfoProbe> {
    fetch_device_info_timed(host, Duration::from_secs(2), Duration::from_secs(3))
}

pub fn fetch_device_info_fast(host: &str) -> Option<DeviceInfoProbe> {
    fetch_device_info_timed(host, Duration::from_millis(1200), Duration::from_millis(1200))
}

fn fetch_device_info_timed(
    host: &str,
    connect_timeout: Duration,
    io_timeout: Duration,
) -> Option<DeviceInfoProbe> {
    let response = api_transact_timed(host, r#"{"cmd":"get.device.info"}"#, connect_timeout, io_timeout).ok()?;
    parse_device_info(&response)
}

pub fn fetch_device_info_param(host: &str, param: &str) -> Option<Value> {
    let payload = format!(r#"{{"cmd":"get.device.info","param":"{param}"}}"#);
    let response = api_request(host, &payload)?;
    let value: Value = serde_json::from_str(&response).ok()?;
    if value.get("code").and_then(|c| c.as_i64()) != Some(0) {
        return None;
    }
    value.get("msg").cloned()
}

pub fn api_request(host: &str, json_payload: &str) -> Option<String> {
    api_transact(host, json_payload).ok()
}

fn api_transact(host: &str, json_payload: &str) -> Result<String, ()> {
    api_transact_timed(
        host,
        json_payload,
        Duration::from_secs(2),
        Duration::from_secs(3),
    )
}

fn api_transact_timed(
    host: &str,
    json_payload: &str,
    connect_timeout: Duration,
    io_timeout: Duration,
) -> Result<String, ()> {
    let addr = format!("{host}:{WHATSMINER_API_PORT}");
    let mut stream = TcpStream::connect_timeout(
        &addr.parse().map_err(|_| ())?,
        connect_timeout,
    )
    .map_err(|_| ())?;
    stream.set_read_timeout(Some(io_timeout)).ok();
    stream.set_write_timeout(Some(io_timeout)).ok();

    let bytes = json_payload.as_bytes();
    let len = bytes.len() as u32;
    stream.write_all(&len.to_le_bytes()).map_err(|_| ())?;
    stream.write_all(bytes).map_err(|_| ())?;

    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf).map_err(|_| ())?;
    let resp_len = u32::from_le_bytes(len_buf) as usize;
    if resp_len == 0 || resp_len > 256 * 1024 {
        return Err(());
    }

    let mut buf = vec![0u8; resp_len];
    stream.read_exact(&mut buf).map_err(|_| ())?;
    String::from_utf8(buf).map_err(|_| ())
}

pub fn parse_device_info(json: &str) -> Option<DeviceInfoProbe> {
    let value: Value = serde_json::from_str(json).ok()?;
    let code_ok = value.get("code").and_then(|c| c.as_i64()) == Some(0);
    let msg = value.get("msg")?;

    let mac = msg
        .get("network")
        .and_then(|n| n.get("mac"))
        .and_then(|v| v.as_str())
        .map(normalize_mac);

    let api_switch = msg
        .get("system")
        .and_then(|s| s.get("apiswitch"))
        .and_then(parse_switch_flag);

    let salt = msg
        .get("salt")
        .and_then(|v| v.as_str())
        .map(str::to_string);

    let miner = msg.get("miner");
    let model = miner
        .and_then(|m| m.get("type"))
        .and_then(|v| v.as_str())
        .map(clean_model);
    let working = miner
        .and_then(|m| m.get("working"))
        .and_then(parse_switch_flag);

    Some(DeviceInfoProbe {
        code_ok,
        mac,
        api_switch,
        salt,
        model,
        working,
    })
}

/// `M30S++_VH70` → `M30S++` (drop the hash-board suffix after `_`).
fn clean_model(raw: &str) -> String {
    let trimmed = raw.trim();
    trimmed
        .split('_')
        .next()
        .filter(|s| !s.is_empty())
        .unwrap_or(trimmed)
        .to_string()
}

pub fn generate_api_token(cmd: &str, password: &str, salt: &str, ts: i64) -> String {
    let input = format!("{cmd}{password}{salt}{ts}");
    let hash = Sha256::digest(input.as_bytes());
    let encoded = BASE64.encode(hash);
    encoded.chars().take(8).collect()
}

pub fn normalize_mac(raw: &str) -> String {
    crate::drivers::parse::normalize_mac_address(raw)
}

fn parse_switch_flag(value: &Value) -> Option<bool> {
    match value {
        Value::String(s) => match s.trim() {
            "1" | "true" | "on" | "enable" | "enabled" => Some(true),
            "0" | "false" | "off" | "disable" | "disabled" => Some(false),
            _ => None,
        },
        Value::Number(n) => n.as_i64().map(|v| v != 0),
        Value::Bool(b) => Some(*b),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_device_info_sample() {
        let sample = r#"{
            "code": 0,
            "msg": {
                "network": { "mac": "ca:01:14:00:04:eb" },
                "system": { "apiswitch": "1" },
                "salt": "px5hoXa9"
            }
        }"#;
        let info = parse_device_info(sample).unwrap();
        assert!(info.code_ok);
        assert_eq!(info.mac.as_deref(), Some("CA:01:14:00:04:EB"));
        assert_eq!(info.api_switch, Some(true));
        assert_eq!(info.salt.as_deref(), Some("px5hoXa9"));
    }

    #[test]
    fn needs_setup_true_when_telemetry_but_chips_missing() {
        let status = WhatsminerAccessStatus {
            api_switch: Some(true),
            luci_auth_ok: true,
            luci_reachable: true,
            api_reachable: true,
            api_auth_ok: true,
            ..Default::default()
        };
        assert!(compute_needs_setup(&status, false, true));
    }

    #[test]
    fn token_is_eight_chars() {
        let token = generate_api_token("set.miner.pools", "abcdefg", "QbVy1Ou3", 2_111_801_5);
        assert_eq!(token.len(), 8);
    }

    #[test]
    #[ignore = "requires miners on local network"]
    fn bench_live_miner_reads() {
        use crate::drivers::whatsminer::options::WhatsminerFetchOptions;
        use crate::tcp::TcpCgminerClient;
        use crate::fetch_with_detect;
        use std::time::Instant;

        let client = TcpCgminerClient::for_read();
        let options = WhatsminerFetchOptions::fast_read();
        for ip in ["192.168.35.42", "192.168.35.35", "192.168.35.33"] {
            let t = Instant::now();
            match fetch_with_detect(&client, ip, 4028, &options) {
                Ok(s) => eprintln!(
                    "fetch_with_detect {ip} OK model={} chips={} needs_setup={:?} {:?}",
                    s.identity.model,
                    s.board_chips.len(),
                    s.whatsminer_access.as_ref().map(|a| a.needs_setup),
                    t.elapsed()
                ),
                Err(e) => eprintln!("fetch_with_detect {ip} ERR {e:?} {:?}", t.elapsed()),
            }
        }
    }
}
