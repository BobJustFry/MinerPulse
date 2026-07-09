use super::access::{api_request, fetch_device_info, generate_api_token, parse_device_info};
use super::legacy4028::send_legacy_command;
use crate::error::{ErrorCode, MinerPulseError};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::thread;
use std::time::Duration;

const VERIFY_DELAY: Duration = Duration::from_millis(1500);
const VERIFY_ATTEMPTS: usize = 3;
const VERIFY_INTERVAL: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WhatsminerControlState {
    pub api_switch: Option<bool>,
    pub api_reachable: bool,
    pub mining: Option<bool>,
    pub fast_boot: Option<bool>,
    pub web_pools: Option<bool>,
    pub led_mode: Option<String>,
    pub power_mode: Option<String>,
    pub power_limit_w: Option<u32>,
    pub target_freq_pct: Option<u32>,
    pub upfreq_speed: Option<u32>,
    pub power_percent: Option<u32>,
    pub heat_mode: Option<String>,
    pub protection_mode: Option<bool>,
    pub timezone: Option<String>,
    pub ntp_servers: Vec<String>,
    pub model: Option<String>,
    pub writes_blocked: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WhatsminerControlAction {
    SetMining { enabled: bool },
    SetApiSwitch { enabled: bool },
    SetFastBoot { enabled: bool },
    SetWebPools { enabled: bool },
    SetLed { mode: String },
    SetPowerMode { mode: String },
    SetPowerLimit { watts: u32 },
    SetTargetFreq { percent: u32 },
    SetUpfreqSpeed { speed: u32 },
    SetPowerPercent { percent: u32 },
    Reboot,
    RestoreSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WhatsminerControlApplyResult {
    pub ok: bool,
    pub message: Option<String>,
    pub state: Option<WhatsminerControlState>,
}

pub fn read_control_state(host: &str) -> Result<WhatsminerControlState, MinerPulseError> {
    let device = fetch_device_info(host).ok_or_else(MinerPulseError::conn_failed)?;
    if !device.code_ok {
        return Err(MinerPulseError::Coded {
            code: ErrorCode::ConnFailed,
            message: Some("device_info".into()),
        });
    }

    let miner_setting = fetch_json_cmd(host, "get.miner.setting")?;
    let system_setting = fetch_json_cmd(host, "get.system.setting").ok();
    let led_mode = fetch_device_led_mode(host);

    let mut state = build_control_state(&device, &miner_setting, system_setting.as_ref());
    state.led_mode = led_mode;
    state.writes_blocked = false;
    Ok(state)
}

pub fn apply_control_action(
    host: &str,
    port: u16,
    password: &str,
    action: &WhatsminerControlAction,
) -> Result<WhatsminerControlApplyResult, MinerPulseError> {
    let v3_result = apply_control_action_v3(host, password, action);
    match v3_result {
        Ok(result) if result.ok || !is_password_blocked(result.message.as_deref()) => return Ok(result),
        Ok(result) => {
            if let Ok(legacy) = apply_control_action_legacy(host, port, password, action) {
                return Ok(legacy);
            }
            Ok(WhatsminerControlApplyResult {
                ok: false,
                message: result.message.or(Some("default_password_blocked".into())),
                state: read_control_state(host).ok().map(|mut s| {
                    s.writes_blocked = true;
                    s
                }),
            })
        }
        Err(err) => {
            if let Ok(legacy) = apply_control_action_legacy(host, port, password, action) {
                return Ok(legacy);
            }
            Err(err)
        }
    }
}

fn is_password_blocked(message: Option<&str>) -> bool {
    message
        .map(|m| {
            let lower = m.to_ascii_lowercase();
            lower.contains("password") || lower.contains("default_password_blocked")
        })
        .unwrap_or(false)
}

fn apply_control_action_v3(
    host: &str,
    password: &str,
    action: &WhatsminerControlAction,
) -> Result<WhatsminerControlApplyResult, MinerPulseError> {
    let (cmd, param) = action_to_cmd(action)?;
    let response = signed_api_request(host, password, cmd, param)?;
    let code = response.get("code").and_then(|c| c.as_i64()).unwrap_or(-1);
    if code != 0 {
        let desc = response
            .get("msg")
            .map(value_to_string)
            .filter(|s| !s.is_empty() && s != "null")
            .or_else(|| response.get("desc").map(value_to_string))
            .unwrap_or_else(|| format!("code {code}"));
        return Ok(WhatsminerControlApplyResult {
            ok: false,
            message: Some(desc),
            state: read_control_state(host).ok(),
        });
    }

    if matches!(action, WhatsminerControlAction::Reboot) {
        return Ok(WhatsminerControlApplyResult {
            ok: true,
            message: Some("reboot".into()),
            state: None,
        });
    }

    let verified = verify_action(host, action)?;
    Ok(WhatsminerControlApplyResult {
        ok: verified,
        message: if verified {
            None
        } else {
            Some("verify_failed".into())
        },
        state: read_control_state(host).ok(),
    })
}

fn apply_control_action_legacy(
    host: &str,
    port: u16,
    password: &str,
    action: &WhatsminerControlAction,
) -> Result<WhatsminerControlApplyResult, MinerPulseError> {
    let cmd = legacy_command_template(action).ok_or_else(|| {
        MinerPulseError::Coded {
            code: ErrorCode::NotSupported,
            message: None,
        }
    })?;
    let response = send_legacy_command(host, port, password, cmd)?;
    let ok = response.get("STATUS").and_then(|v| v.as_str()) == Some("S");
    thread::sleep(VERIFY_DELAY);
    Ok(WhatsminerControlApplyResult {
        ok,
        message: if ok {
            None
        } else {
            response
                .get("Msg")
                .and_then(|v| v.as_str())
                .map(str::to_string)
        },
        state: read_control_state(host).ok(),
    })
}

fn legacy_command_template(action: &WhatsminerControlAction) -> Option<&'static str> {
    Some(match action {
        WhatsminerControlAction::SetMining { enabled: true } => {
            r#"{"token":"{sign}","cmd":"power_on"}"#
        }
        WhatsminerControlAction::SetMining { enabled: false } => {
            r#"{"token":"{sign}","cmd":"power_off"}"#
        }
        WhatsminerControlAction::SetPowerMode { mode } => match mode.as_str() {
            "low" => r#"{"token":"{sign}","cmd":"set_low_power"}"#,
            "high" => r#"{"token":"{sign}","cmd":"set_high_power"}"#,
            _ => r#"{"token":"{sign}","cmd":"set_normal_power"}"#,
        },
        WhatsminerControlAction::Reboot => r#"{"token":"{sign}","cmd":"reboot"}"#,
        _ => return None,
    })
}

pub fn export_miner_log(host: &str, password: &str) -> Result<Vec<u8>, MinerPulseError> {
    let response = signed_api_request(host, password, "get.log.download", json!(""))?;
    let code = response.get("code").and_then(|c| c.as_i64()).unwrap_or(-1);
    if code != 0 {
        return Err(MinerPulseError::Coded {
            code: ErrorCode::ParseFailed,
            message: response
                .get("desc")
                .and_then(|v| v.as_str())
                .map(str::to_string),
        });
    }
    let msg = response.get("msg").ok_or_else(|| MinerPulseError::with_code(ErrorCode::ParseFailed))?;
    if let Some(text) = msg.as_str() {
        return Ok(text.as_bytes().to_vec());
    }
    if let Some(data) = msg.get("data").and_then(|v| v.as_str()) {
        use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
        return BASE64
            .decode(data)
            .map_err(|_| MinerPulseError::with_code(ErrorCode::ParseFailed));
    }
    serde_json::to_vec(msg).map_err(|_| MinerPulseError::with_code(ErrorCode::ParseFailed))
}

fn action_to_cmd(action: &WhatsminerControlAction) -> Result<(&'static str, Value), MinerPulseError> {
    Ok(match action {
        WhatsminerControlAction::SetMining { enabled } => (
            "set.miner.service",
            json!(if *enabled { "start" } else { "stop" }),
        ),
        WhatsminerControlAction::SetApiSwitch { enabled } => (
            "set.system.apiswitch",
            json!(if *enabled { "enable" } else { "disable" }),
        ),
        WhatsminerControlAction::SetFastBoot { enabled } => (
            "set.miner.fastboot",
            json!(if *enabled { "enable" } else { "disable" }),
        ),
        WhatsminerControlAction::SetWebPools { enabled } => (
            "set.system.webpools",
            json!(if *enabled { "enable" } else { "disable" }),
        ),
        WhatsminerControlAction::SetLed { mode } => ("set.system.led", json!(mode)),
        WhatsminerControlAction::SetPowerMode { mode } => ("set.miner.power_mode", json!(mode)),
        WhatsminerControlAction::SetPowerLimit { watts } => ("set.miner.power_limit", json!(watts)),
        WhatsminerControlAction::SetTargetFreq { percent } => {
            ("set.miner.target_freq", json!(percent))
        }
        WhatsminerControlAction::SetUpfreqSpeed { speed } => ("set.miner.upfreq_speed", json!(speed)),
        WhatsminerControlAction::SetPowerPercent { percent } => {
            ("set.miner.power_percent", json!(percent))
        }
        WhatsminerControlAction::Reboot => ("set.system.reboot", json!("")),
        WhatsminerControlAction::RestoreSettings => ("set.miner.restore_setting", json!("")),
    })
}

fn signed_api_request(
    host: &str,
    password: &str,
    cmd: &str,
    param: Value,
) -> Result<Value, MinerPulseError> {
    signed_api_request_with_account(host, password, "super", cmd, param)
}

pub fn signed_api_request_with_account(
    host: &str,
    password: &str,
    account: &str,
    cmd: &str,
    param: Value,
) -> Result<Value, MinerPulseError> {
    let device = fetch_device_info(host).ok_or_else(MinerPulseError::conn_failed)?;
    let salt = device
        .salt
        .as_deref()
        .ok_or_else(MinerPulseError::conn_failed)?;
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let token = generate_api_token(cmd, password, salt, ts);
    let payload = json!({
        "cmd": cmd,
        "param": param,
        "ts": ts,
        "token": token,
        "account": account,
    });
    let raw = api_request(host, &payload.to_string()).ok_or_else(MinerPulseError::conn_failed)?;
    serde_json::from_str(&raw).map_err(|_| MinerPulseError::with_code(ErrorCode::ParseFailed))
}

fn fetch_json_cmd(host: &str, cmd: &str) -> Result<Value, MinerPulseError> {
    let payload = json!({ "cmd": cmd });
    let raw = api_request(host, &payload.to_string()).ok_or_else(MinerPulseError::conn_failed)?;
    let value: Value = serde_json::from_str(&raw).map_err(|_| MinerPulseError::with_code(ErrorCode::ParseFailed))?;
    if value.get("code").and_then(|c| c.as_i64()) != Some(0) {
        return Err(MinerPulseError::with_code(ErrorCode::ParseFailed));
    }
    value
        .get("msg")
        .cloned()
        .ok_or_else(|| MinerPulseError::with_code(ErrorCode::ParseFailed))
}

fn fetch_device_led_mode(host: &str) -> Option<String> {
    let raw = api_request(host, r#"{"cmd":"get.device.info"}"#)?;
    let value: Value = serde_json::from_str(&raw).ok()?;
    value
        .get("msg")
        .and_then(|m| m.get("system"))
        .and_then(|s| s.get("ledstatus"))
        .and_then(|v| v.as_str())
        .map(str::to_string)
}

fn build_control_state(
    device: &super::access::DeviceInfoProbe,
    miner_setting: &Value,
    system_setting: Option<&Value>,
) -> WhatsminerControlState {
    let heat_mode = json_str(miner_setting, "heat-mode");
    WhatsminerControlState {
        api_switch: device.api_switch,
        api_reachable: device.code_ok,
        mining: device.working,
        fast_boot: parse_enable_flag(json_str(miner_setting, "fast-boot").as_deref()),
        web_pools: json_i64(miner_setting, "web-pool")
            .or_else(|| system_setting.and_then(|s| json_i64(s, "web-pool")))
            .map(|v| v != 0),
        led_mode: None,
        power_mode: json_str(miner_setting, "power-mode"),
        power_limit_w: json_u32(miner_setting, "power-limit"),
        target_freq_pct: json_u32(miner_setting, "target-freq"),
        upfreq_speed: json_u32(miner_setting, "upfreq-speed"),
        power_percent: json_u32(miner_setting, "power-percent").or_else(|| json_u32(miner_setting, "power")),
        heat_mode: heat_mode.clone(),
        protection_mode: heat_mode.as_deref().map(|m| m != "normal"),
        timezone: system_setting.and_then(|s| json_str(s, "zonename")),
        ntp_servers: system_setting
            .and_then(|s| s.get("ntp-server"))
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default(),
        model: device.model.clone(),
        writes_blocked: false,
    }
}

fn verify_action(host: &str, action: &WhatsminerControlAction) -> Result<bool, MinerPulseError> {
    thread::sleep(VERIFY_DELAY);
    for _ in 0..VERIFY_ATTEMPTS {
        if let Ok(state) = read_control_state(host) {
            if action_matches_state(action, &state) {
                return Ok(true);
            }
        }
        thread::sleep(VERIFY_INTERVAL);
    }
    Ok(false)
}

fn action_matches_state(action: &WhatsminerControlAction, state: &WhatsminerControlState) -> bool {
    match action {
        WhatsminerControlAction::SetMining { enabled } => state.mining == Some(*enabled),
        WhatsminerControlAction::SetApiSwitch { enabled } => state.api_switch == Some(*enabled),
        WhatsminerControlAction::SetFastBoot { enabled } => {
            state.fast_boot == Some(*enabled)
        }
        WhatsminerControlAction::SetWebPools { enabled } => state.web_pools == Some(*enabled),
        WhatsminerControlAction::SetLed { mode } => {
            state.led_mode.as_deref() == Some(mode.as_str())
        }
        WhatsminerControlAction::SetPowerMode { mode } => {
            state.power_mode.as_deref() == Some(mode.as_str())
        }
        WhatsminerControlAction::SetPowerLimit { watts } => state.power_limit_w == Some(*watts),
        WhatsminerControlAction::SetTargetFreq { percent } => {
            state.target_freq_pct == Some(*percent)
        }
        WhatsminerControlAction::SetUpfreqSpeed { speed } => state.upfreq_speed == Some(*speed),
        WhatsminerControlAction::SetPowerPercent { percent } => {
            state.power_percent == Some(*percent)
        }
        WhatsminerControlAction::Reboot | WhatsminerControlAction::RestoreSettings => true,
    }
}

fn parse_enable_flag(raw: Option<&str>) -> Option<bool> {
    match raw?.trim().to_ascii_lowercase().as_str() {
        "enable" | "enabled" | "1" | "true" | "on" => Some(true),
        "disable" | "disabled" | "0" | "false" | "off" => Some(false),
        _ => None,
    }
}

fn json_str(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(|v| {
        v.as_str()
            .map(str::to_string)
            .or_else(|| v.as_i64().map(|n| n.to_string()))
    })
}

fn json_i64(value: &Value, key: &str) -> Option<i64> {
    value.get(key).and_then(|v| {
        v.as_i64()
            .or_else(|| v.as_str().and_then(|s| s.trim().parse().ok()))
    })
}

fn json_u32(value: &Value, key: &str) -> Option<u32> {
    json_i64(value, key).and_then(|v| u32::try_from(v).ok())
}

fn value_to_string(value: &Value) -> String {
    value
        .as_str()
        .map(str::to_string)
        .unwrap_or_else(|| value.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const LIVE_HOST: &str = "192.168.35.31";
    const LIVE_PASS: &str = "admin";

    #[test]
    fn parses_control_state_fields() {
        let device = parse_device_info(
            r#"{"code":0,"msg":{"system":{"apiswitch":"1"},"miner":{"working":"true","type":"M50"},"salt":"abc"}}"#,
        )
        .unwrap();
        let miner = serde_json::json!({
            "power-limit": 3200,
            "upfreq-speed": 5,
            "power-mode": "normal",
            "fast-boot": "disable",
            "heat-mode": "normal",
            "target-freq": 0,
            "power": 0
        });
        let state = build_control_state(&device, &miner, None);
        assert_eq!(state.mining, Some(true));
        assert_eq!(state.fast_boot, Some(false));
        assert_eq!(state.power_mode.as_deref(), Some("normal"));
    }

    #[test]
    #[ignore = "live miner on LAN"]
    fn live_read_control_state() {
        let state = read_control_state(LIVE_HOST).expect("read");
        eprintln!("{state:#?}");
        assert!(state.api_reachable);
    }

    #[test]
    #[ignore = "live miner on LAN"]
    fn live_toggle_mining_roundtrip() {
        let off = apply_control_action(
            LIVE_HOST,
            4028,
            LIVE_PASS,
            &WhatsminerControlAction::SetMining { enabled: false },
        )
        .expect("stop");
        eprintln!("mining off: {:?}", off);
        assert!(off.ok, "{:?}", off.message);
        std::thread::sleep(std::time::Duration::from_secs(5));
        let on = apply_control_action(
            LIVE_HOST,
            4028,
            LIVE_PASS,
            &WhatsminerControlAction::SetMining { enabled: true },
        )
        .expect("start");
        eprintln!("mining on: {:?}", on);
        assert!(on.ok, "{:?}", on.message);
    }

    #[test]
    #[ignore = "live miner on LAN"]
    fn live_probe_signed_fastboot() {
        let response = signed_api_request_with_account(
            LIVE_HOST,
            LIVE_PASS,
            "super",
            "set.miner.fastboot",
            json!("enable"),
        )
        .expect("request");
        eprintln!("{}", serde_json::to_string_pretty(&response).unwrap());
    }

    #[test]
    #[ignore = "live miner on LAN"]
    fn live_toggle_fast_boot_roundtrip() {
        let state = read_control_state(LIVE_HOST).expect("read");
        let target = !state.fast_boot.unwrap_or(false);
        let result = apply_control_action(
            LIVE_HOST,
            4028,
            LIVE_PASS,
            &WhatsminerControlAction::SetFastBoot { enabled: target },
        )
        .expect("apply");
        eprintln!("toggle fast_boot -> {target}: {:?}", result);
        assert!(result.ok, "{:?}", result.message);
        let restore = apply_control_action(
            LIVE_HOST,
            4028,
            LIVE_PASS,
            &WhatsminerControlAction::SetFastBoot {
                enabled: !target,
            },
        )
        .expect("restore");
        assert!(restore.ok, "{:?}", restore.message);
    }

    #[test]
    #[ignore = "live miner on LAN"]
    fn live_toggle_web_pools_roundtrip() {
        let state = read_control_state(LIVE_HOST).expect("read");
        let target = !state.web_pools.unwrap_or(true);
        let result = apply_control_action(
            LIVE_HOST,
            4028,
            LIVE_PASS,
            &WhatsminerControlAction::SetWebPools { enabled: target },
        )
        .expect("apply");
        eprintln!("toggle web_pools -> {target}: {:?}", result);
        assert!(result.ok, "{:?}", result.message);
        let restore = apply_control_action(
            LIVE_HOST,
            4028,
            LIVE_PASS,
            &WhatsminerControlAction::SetWebPools {
                enabled: !target,
            },
        )
        .expect("restore");
        assert!(restore.ok, "{:?}", restore.message);
    }
}
