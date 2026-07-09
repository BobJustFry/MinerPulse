use super::access::api_request;
use crate::error::{ErrorCode, MinerPulseError};
use serde_json::{json, Value};

#[derive(Debug, Clone, Default)]
pub struct FanSettings {
    pub poweroff_cool: Option<bool>,
    pub zero_speed: Option<bool>,
    pub temp_offset: Option<i32>,
}

pub fn read_fan_settings(host: &str) -> Result<FanSettings, MinerPulseError> {
    let payload = json!({ "cmd": "get.fan.setting" });
    let raw = api_request(host, &payload.to_string()).ok_or_else(MinerPulseError::conn_failed)?;
    let value: Value = serde_json::from_str(&raw).map_err(|_| MinerPulseError::with_code(ErrorCode::ParseFailed))?;
    if value.get("code").and_then(|c| c.as_i64()) != Some(0) {
        return Err(MinerPulseError::with_code(ErrorCode::ParseFailed));
    }
    let msg = value.get("msg").ok_or_else(|| MinerPulseError::with_code(ErrorCode::ParseFailed))?;
    Ok(FanSettings {
        poweroff_cool: parse_switch(msg.get("fan-poweroff-cool")),
        zero_speed: parse_switch(msg.get("fan-zero-speed")),
        temp_offset: msg
            .get("fan-temp-offset")
            .and_then(|v| v.as_i64())
            .and_then(|v| i32::try_from(v).ok()),
    })
}

fn parse_switch(value: Option<&Value>) -> Option<bool> {
    match value {
        Some(Value::Number(n)) => n.as_i64().map(|v| v != 0),
        Some(Value::String(s)) => match s.trim() {
            "1" | "true" | "enable" | "enabled" => Some(true),
            "0" | "false" | "disable" | "disabled" => Some(false),
            _ => None,
        },
        Some(Value::Bool(b)) => Some(*b),
        _ => None,
    }
}

pub fn is_liquid_cooling(miner_setting: &Value) -> bool {
    miner_setting
        .get("eeprom-liquid-cooling")
        .and_then(|v| v.as_str())
        .map(|s| s.chars().any(|c| c != '0' && c != '-'))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_liquid_from_eeprom() {
        let miner = serde_json::json!({ "eeprom-liquid-cooling": "1-0-0" });
        assert!(is_liquid_cooling(&miner));
        let air = serde_json::json!({ "eeprom-liquid-cooling": "0-0-0" });
        assert!(!is_liquid_cooling(&air));
    }
}
