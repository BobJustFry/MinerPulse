use super::access::{api_request, enable_api_switch, fetch_device_info, generate_api_token};
use super::luci::test_luci_credentials;
#[cfg(test)]
use super::access::parse_device_info;
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
    pub target_freq_pct: Option<i32>,
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
    SetTargetFreq { percent: i32 },
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ApiSwitchEnableResult {
    pub ok: bool,
    pub message: Option<&'static str>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum V3ApiSwitchAttempt {
    Enabled,
    Unsupported,
    Failed,
}

pub fn action_requires_v3_write(action: &WhatsminerControlAction) -> bool {
    !matches!(
        action,
        WhatsminerControlAction::SetMining { .. }
            | WhatsminerControlAction::SetPowerMode { .. }
            | WhatsminerControlAction::Reboot
    )
}

pub fn read_control_state(host: &str) -> Result<WhatsminerControlState, MinerPulseError> {
    read_control_state_with_auth(host, None)
}

pub fn read_control_state_with_auth(
    host: &str,
    _password: Option<&str>,
) -> Result<WhatsminerControlState, MinerPulseError> {
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

pub fn change_super_password(
    host: &str,
    username: &str,
    old_password: &str,
    new_password: &str,
) -> Result<(), MinerPulseError> {
    match change_super_password_api(host, old_password, new_password) {
        Ok(()) => Ok(()),
        Err(err) if api_password_change_blocked(&err) => {
            if super::luci::change_super_password_luci(host, username, old_password, new_password) {
                Ok(())
            } else {
                Err(MinerPulseError::Coded {
                    code: ErrorCode::InvalidInput,
                    message: Some("default_password_change_blocked".into()),
                })
            }
        }
        Err(err) => Err(err),
    }
}

fn api_password_change_blocked(err: &MinerPulseError) -> bool {
    match err {
        MinerPulseError::Coded { message, .. } => message
            .as_deref()
            .map(|m| {
                let lower = m.to_ascii_lowercase();
                lower.contains("password") || lower.contains("default_password")
            })
            .unwrap_or(false),
    }
}

fn change_super_password_api(
    host: &str,
    old_password: &str,
    new_password: &str,
) -> Result<(), MinerPulseError> {
    const CMD: &str = "set.user.change_passwd";
    let device = fetch_device_info(host).ok_or_else(MinerPulseError::conn_failed)?;
    let salt = device
        .salt
        .as_deref()
        .ok_or_else(MinerPulseError::conn_failed)?;
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let token = generate_api_token(CMD, old_password, salt, ts);
    let plain = json!({
        "account": "super",
        "old": old_password,
        "new": new_password,
    })
    .to_string();
    let param = super::crypto::encrypt_v3_param(CMD, old_password, salt, ts, &plain)?;
    let payload = json!({
        "cmd": CMD,
        "param": param,
        "ts": ts,
        "token": token,
        "account": "super",
    });
    let raw = api_request(host, &payload.to_string()).ok_or_else(MinerPulseError::conn_failed)?;
    let value: Value = serde_json::from_str(&raw).map_err(|_| MinerPulseError::with_code(ErrorCode::ParseFailed))?;
    if value.get("code").and_then(|c| c.as_i64()) != Some(0) {
        let message = value
            .get("msg")
            .map(value_to_string)
            .unwrap_or_else(|| "change_password_failed".into());
        return Err(MinerPulseError::Coded {
            code: ErrorCode::InvalidInput,
            message: Some(message),
        });
    }
    Ok(())
}

/// Harmless write probe: re-apply current LED mode via API v3.
pub fn probe_v3_write_access(host: &str, password: &str) -> bool {
    let led_mode = fetch_device_led_mode(host).unwrap_or_else(|| "auto".into());
    match signed_api_request_with_account(host, password, "super", "set.system.led", json!(led_mode))
    {
        Ok(response) => response.get("code").and_then(|c| c.as_i64()) == Some(0),
        Err(_) => false,
    }
}

/// Harmless write probe via legacy 4028 (same path as WhatsMinerTool Remote Ctrl).
pub fn probe_legacy_write_access(host: &str, port: u16, password: &str) -> bool {
    let enabled = read_control_state(host)
        .ok()
        .and_then(|s| s.fast_boot)
        .unwrap_or(false);
    let cmd = if enabled {
        r#"{"token":"{sign}","cmd":"enable_btminer_fast_boot"}"#
    } else {
        r#"{"token":"{sign}","cmd":"disable_btminer_fast_boot"}"#
    };
    send_legacy_command(host, port, password, cmd)
        .map(|response| response.get("STATUS").and_then(|v| v.as_str()) == Some("S"))
        .unwrap_or(false)
}

fn action_has_legacy_path(action: &WhatsminerControlAction) -> bool {
    legacy_command_template(action).is_some()
}

pub fn apply_control_action(
    host: &str,
    port: u16,
    username: &str,
    password: &str,
    action: &WhatsminerControlAction,
) -> Result<WhatsminerControlApplyResult, MinerPulseError> {
    if matches!(action, WhatsminerControlAction::SetApiSwitch { enabled: true }) {
        return apply_api_switch_enable(host, username, password, action);
    }

    let pre_state = read_control_state(host).ok();
    let enabling_api_switch = matches!(
        action,
        WhatsminerControlAction::SetApiSwitch { enabled: true }
    ) && pre_state
        .as_ref()
        .is_some_and(|s| s.api_switch == Some(false));

    // WhatsMinerTool path: TCP 4028 + get_token (API 2.x manual).
    if action_has_legacy_path(action) {
        return apply_control_action_legacy(host, port, password, action);
    }

    // API 3.0 path: TCP 4433 set.* (whatsminer-api-3.0.0 doc).
    let v3_result = apply_control_action_v3(host, password, action);
    match v3_result {
        Ok(result) if result.ok => Ok(result),
        Ok(result) => {
            if let Some(fallback) =
                try_enable_api_switch_luci(host, username, password, action, enabling_api_switch)
            {
                return Ok(fallback);
            }
            Ok(result)
        }
        Err(err) => {
            if let Some(fallback) =
                try_enable_api_switch_luci(host, username, password, action, enabling_api_switch)
            {
                return Ok(fallback);
            }
            Err(err)
        }
    }
}

fn classify_v3_api_switch_response(value: &Value) -> V3ApiSwitchAttempt {
    let code = value.get("code").and_then(|c| c.as_i64()).unwrap_or(-1);
    if code == 0 {
        return V3ApiSwitchAttempt::Enabled;
    }
    let msg = value
        .get("msg")
        .and_then(|m| m.as_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if code == -2 && msg.contains("invalid command") {
        return V3ApiSwitchAttempt::Unsupported;
    }
    V3ApiSwitchAttempt::Failed
}

fn try_enable_api_switch_v3(host: &str, password: &str) -> V3ApiSwitchAttempt {
    let response = match signed_api_request(host, password, "set.system.apiswitch", json!("enable")) {
        Ok(value) => value,
        Err(_) => return V3ApiSwitchAttempt::Failed,
    };
    classify_v3_api_switch_response(&response)
}

fn api_switch_is_on(host: &str) -> bool {
    fetch_device_info(host).is_some_and(|info| info.api_switch == Some(true))
}

/// Enable API Switch via LuCI and, when supported, API 3.0. Returns a stable message code on failure.
pub fn enable_api_switch_detailed(
    host: &str,
    username: &str,
    password: &str,
) -> ApiSwitchEnableResult {
    if enable_api_switch(host, username, password) || api_switch_is_on(host) {
        return ApiSwitchEnableResult {
            ok: true,
            message: None,
        };
    }

    let v3 = try_enable_api_switch_v3(host, password);
    if v3 == V3ApiSwitchAttempt::Enabled && api_switch_is_on(host) {
        return ApiSwitchEnableResult {
            ok: true,
            message: None,
        };
    }

    let message = if !test_luci_credentials(host, username, password) {
        Some("api_switch_web_auth_failed")
    } else if v3 == V3ApiSwitchAttempt::Unsupported {
        Some("api_switch_manual_required")
    } else {
        Some("api_switch_enable_failed")
    };
    ApiSwitchEnableResult {
        ok: false,
        message,
    }
}

fn apply_api_switch_enable(
    host: &str,
    username: &str,
    password: &str,
    action: &WhatsminerControlAction,
) -> Result<WhatsminerControlApplyResult, MinerPulseError> {
    let result = enable_api_switch_detailed(host, username, password);
    if result.ok {
        let verified = verify_action_with_auth(host, Some(password), action).unwrap_or(false);
        return Ok(WhatsminerControlApplyResult {
            ok: verified,
            message: if verified {
                None
            } else {
                Some("verify_failed".into())
            },
            state: read_control_state_with_auth(host, Some(password)).ok(),
        });
    }
    Ok(WhatsminerControlApplyResult {
        ok: false,
        message: result.message.map(str::to_string),
        state: read_control_state_with_auth(host, Some(password)).ok(),
    })
}

fn try_enable_api_switch_luci(
    host: &str,
    username: &str,
    password: &str,
    action: &WhatsminerControlAction,
    enabling_api_switch: bool,
) -> Option<WhatsminerControlApplyResult> {
    if !enabling_api_switch {
        return None;
    }
    if !enable_api_switch(host, username, password) {
        return None;
    }
    let verified = verify_action_with_auth(host, Some(password), action).unwrap_or(false);
    Some(WhatsminerControlApplyResult {
        ok: verified,
        message: if verified {
            None
        } else if action_may_need_reboot(action) {
            Some("reboot_required".into())
        } else {
            Some("verify_failed".into())
        },
        state: read_control_state_with_auth(host, Some(password)).ok(),
    })
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
            state: read_control_state_with_auth(host, Some(password)).ok(),
        });
    }

    if matches!(action, WhatsminerControlAction::Reboot) {
        return Ok(WhatsminerControlApplyResult {
            ok: true,
            message: Some("reboot".into()),
            state: None,
        });
    }

    let verified = verify_action_with_auth(host, Some(password), action)?;
    Ok(WhatsminerControlApplyResult {
        ok: verified,
        message: if verified {
            None
        } else if action_may_need_reboot(action) {
            Some("reboot_required".into())
        } else {
            Some("verify_failed".into())
        },
        state: read_control_state_with_auth(host, Some(password)).ok(),
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
    let response = send_legacy_command(host, port, password, &cmd)?;
    let ok = response.get("STATUS").and_then(|v| v.as_str()) == Some("S");
    thread::sleep(VERIFY_DELAY);
    if !ok {
        return Ok(WhatsminerControlApplyResult {
            ok: false,
            message: response
                .get("Msg")
                .and_then(|v| v.as_str())
                .map(str::to_string),
            state: read_control_state_with_auth(host, Some(password)).ok(),
        });
    }
    let verified = verify_action_with_auth(host, Some(password), action).unwrap_or(false);
    Ok(WhatsminerControlApplyResult {
        ok: verified,
        message: if verified {
            None
        } else if action_may_need_reboot(action) {
            Some("reboot_required".into())
        } else {
            Some("verify_failed".into())
        },
        state: read_control_state_with_auth(host, Some(password)).ok(),
    })
}

fn legacy_command_template(action: &WhatsminerControlAction) -> Option<String> {
    Some(match action {
        WhatsminerControlAction::SetMining { enabled: true } => {
            r#"{"token":"{sign}","cmd":"power_on"}"#.into()
        }
        WhatsminerControlAction::SetMining { enabled: false } => {
            r#"{"token":"{sign}","cmd":"power_off"}"#.into()
        }
        WhatsminerControlAction::SetFastBoot { enabled: true } => {
            r#"{"token":"{sign}","cmd":"enable_btminer_fast_boot"}"#.into()
        }
        WhatsminerControlAction::SetFastBoot { enabled: false } => {
            r#"{"token":"{sign}","cmd":"disable_btminer_fast_boot"}"#.into()
        }
        WhatsminerControlAction::SetWebPools { enabled: true } => {
            r#"{"token":"{sign}","cmd":"enable_web_pools"}"#.into()
        }
        WhatsminerControlAction::SetWebPools { enabled: false } => {
            r#"{"token":"{sign}","cmd":"disable_web_pools"}"#.into()
        }
        WhatsminerControlAction::SetLed { mode } if mode == "auto" => {
            r#"{"token":"{sign}","cmd":"set_led","param":"auto"}"#.into()
        }
        WhatsminerControlAction::SetPowerMode { mode } => match mode.as_str() {
            "low" => r#"{"token":"{sign}","cmd":"set_low_power"}"#.into(),
            "high" => r#"{"token":"{sign}","cmd":"set_high_power"}"#.into(),
            _ => r#"{"token":"{sign}","cmd":"set_normal_power"}"#.into(),
        },
        WhatsminerControlAction::SetPowerLimit { watts } => {
            format!(r#"{{"token":"{{sign}}","cmd":"adjust_power_limit","power_limit":"{watts}"}}"#)
        }
        WhatsminerControlAction::SetTargetFreq { percent } => {
            format!(r#"{{"token":"{{sign}}","cmd":"set_target_freq","percent":"{percent}"}}"#)
        }
        WhatsminerControlAction::SetUpfreqSpeed { speed } => {
            format!(r#"{{"token":"{{sign}}","cmd":"adjust_upfreq_speed","upfreq_speed":"{speed}"}}"#)
        }
        WhatsminerControlAction::SetPowerPercent { percent } => {
            format!(r#"{{"token":"{{sign}}","cmd":"set_power_pct","percent":"{percent}"}}"#)
        }
        WhatsminerControlAction::Reboot => r#"{"token":"{sign}","cmd":"reboot"}"#.into(),
        WhatsminerControlAction::RestoreSettings => {
            r#"{"token":"{sign}","cmd":"factory_reset"}"#.into()
        }
        WhatsminerControlAction::SetLed { .. } | WhatsminerControlAction::SetApiSwitch { .. } => {
            return None;
        }
    })
}

pub fn export_miner_log(
    host: &str,
    port: u16,
    password: &str,
    cancel: Option<&std::sync::atomic::AtomicBool>,
) -> Result<Vec<u8>, MinerPulseError> {
    match super::legacy4028::download_logs(host, port, password, cancel) {
        Ok(bytes) => Ok(bytes),
        Err(e) if e.code() == ErrorCode::OperationCancelled => Err(e),
        Err(_) => super::access::download_log_v3(host, cancel),
    }
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
        target_freq_pct: json_i64(miner_setting, "target-freq").and_then(|v| i32::try_from(v).ok()),
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

pub fn action_may_need_reboot(action: &WhatsminerControlAction) -> bool {
    matches!(
        action,
        WhatsminerControlAction::SetApiSwitch { .. }
            | WhatsminerControlAction::SetFastBoot { .. }
            | WhatsminerControlAction::SetWebPools { .. }
            | WhatsminerControlAction::SetPowerLimit { .. }
            | WhatsminerControlAction::SetTargetFreq { .. }
            | WhatsminerControlAction::SetUpfreqSpeed { .. }
            | WhatsminerControlAction::SetPowerPercent { .. }
            | WhatsminerControlAction::RestoreSettings
    )
}

fn verify_action_with_auth(
    host: &str,
    password: Option<&str>,
    action: &WhatsminerControlAction,
) -> Result<bool, MinerPulseError> {
    thread::sleep(VERIFY_DELAY);
    for _ in 0..VERIFY_ATTEMPTS {
        if let Ok(state) = read_control_state_with_auth(host, password) {
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
    fn classifies_v3_api_switch_unsupported() {
        let value = serde_json::json!({
            "code": -2,
            "msg": "invalid command",
            "desc": "set.system.apiswitch"
        });
        assert_eq!(
            classify_v3_api_switch_response(&value),
            V3ApiSwitchAttempt::Unsupported
        );
    }

    #[test]
    fn classifies_v3_api_switch_enabled() {
        let value = serde_json::json!({
            "code": 0,
            "msg": "ok",
            "desc": "set.system.apiswitch"
        });
        assert_eq!(
            classify_v3_api_switch_response(&value),
            V3ApiSwitchAttempt::Enabled
        );
    }

    #[test]
    fn writes_blocked_false_when_api_switch_off() {
        let device = parse_device_info(
            r#"{"code":0,"msg":{"system":{"apiswitch":"0"},"miner":{"working":"true","type":"M50"},"salt":"abc"}}"#,
        )
        .unwrap();
        let miner = serde_json::json!({ "power-mode": "normal" });
        let state = build_control_state(&device, &miner, None);
        assert_eq!(state.api_switch, Some(false));
        assert!(!state.writes_blocked);
    }

    #[test]
    #[ignore = "live miner on LAN"]
    fn live_export_miner_log() {
        let bytes = export_miner_log("192.168.35.31", 4028, "admin", None).expect("export");
        assert!(!bytes.is_empty());
        eprintln!("export_miner_log {} bytes", bytes.len());
    }

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
    fn parses_negative_target_freq() {
        let device = parse_device_info(
            r#"{"code":0,"msg":{"system":{"apiswitch":"1"},"miner":{"working":"true","type":"M50"},"salt":"abc"}}"#,
        )
        .unwrap();
        let miner = serde_json::json!({ "target-freq": -25 });
        let state = build_control_state(&device, &miner, None);
        assert_eq!(state.target_freq_pct, Some(-25));
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
    fn live_enable_api_switch_35() {
        let host = "192.168.35.35";
        let before = read_control_state(host).expect("read");
        eprintln!("before api_switch={:?}", before.api_switch);
        let result = apply_control_action(
            host,
            4028,
            "admin",
            "admin",
            &WhatsminerControlAction::SetApiSwitch { enabled: true },
        )
        .expect("apply");
        eprintln!("result ok={} msg={:?}", result.ok, result.message);
    }

    #[test]
    #[ignore = "live miner on LAN"]
    fn live_toggle_mining_roundtrip() {
        let off = apply_control_action(
            LIVE_HOST,
            4028,
            "admin",
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
            "admin",
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
            "admin",
            LIVE_PASS,
            &WhatsminerControlAction::SetFastBoot { enabled: target },
        )
        .expect("apply");
        eprintln!("toggle fast_boot -> {target}: {:?}", result);
        assert!(result.ok, "{:?}", result.message);
        let restore = apply_control_action(
            LIVE_HOST,
            4028,
            "admin",
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
            "admin",
            LIVE_PASS,
            &WhatsminerControlAction::SetWebPools { enabled: target },
        )
        .expect("apply");
        eprintln!("toggle web_pools -> {target}: {:?}", result);
        assert!(result.ok, "{:?}", result.message);
        let restore = apply_control_action(
            LIVE_HOST,
            4028,
            "admin",
            LIVE_PASS,
            &WhatsminerControlAction::SetWebPools {
                enabled: !target,
            },
        )
        .expect("restore");
        assert!(restore.ok, "{:?}", restore.message);
    }
}
