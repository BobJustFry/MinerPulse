use super::access::{api_request, fetch_device_info, generate_api_token};
use super::crypto::encrypt_v3_param;
use crate::error::{ErrorCode, MinerPulseError};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub const POOL_SLOT_COUNT: usize = 3;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WhatsminerPoolConfig {
    pub url: String,
    pub worker: String,
    pub password: String,
}

impl WhatsminerPoolConfig {
    pub fn empty() -> Self {
        Self {
            url: String::new(),
            worker: String::new(),
            password: String::new(),
        }
    }
}

pub fn normalize_pool_slots(mut pools: Vec<WhatsminerPoolConfig>) -> Vec<WhatsminerPoolConfig> {
    pools.truncate(POOL_SLOT_COUNT);
    while pools.len() < POOL_SLOT_COUNT {
        pools.push(WhatsminerPoolConfig::empty());
    }
    pools
}

pub fn read_pool_configs(host: &str) -> Result<Vec<WhatsminerPoolConfig>, MinerPulseError> {
    let payload = json!({ "cmd": "get.miner.status", "param": "pools" });
    let raw = api_request(host, &payload.to_string()).ok_or_else(MinerPulseError::conn_failed)?;
    let value: Value = serde_json::from_str(&raw).map_err(|_| MinerPulseError::with_code(ErrorCode::ParseFailed))?;
    if value.get("code").and_then(|c| c.as_i64()) != Some(0) {
        return Err(MinerPulseError::with_code(ErrorCode::ParseFailed));
    }
    let pools = value
        .get("msg")
        .and_then(|m| m.get("pools"))
        .and_then(|p| p.as_array())
        .map(|arr| {
            arr.iter()
                .map(|pool| WhatsminerPoolConfig {
                    url: pool
                        .get("url")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    worker: pool
                        .get("account")
                        .or_else(|| pool.get("worker"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    password: String::new(),
                })
                .collect()
        })
        .unwrap_or_default();
    Ok(normalize_pool_slots(pools))
}

pub fn set_pool_configs(
    host: &str,
    password: &str,
    pools: &[WhatsminerPoolConfig],
) -> Result<(), MinerPulseError> {
    const CMD: &str = "set.miner.pools";
    let device = fetch_device_info(host).ok_or_else(MinerPulseError::conn_failed)?;
    let salt = device
        .salt
        .as_deref()
        .ok_or_else(MinerPulseError::conn_failed)?;
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let token = generate_api_token(CMD, password, salt, ts);
    let param_body: Vec<Value> = pools
        .iter()
        .map(|p| {
            json!({
                "pool": p.url.trim(),
                "worker": p.worker.trim(),
                "passwd": p.password,
            })
        })
        .collect();
    let plain = serde_json::to_string(&param_body)
        .map_err(|_| MinerPulseError::with_code(ErrorCode::ParseFailed))?;
    let param = encrypt_v3_param(CMD, password, salt, ts, &plain)?;
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
            .and_then(|m| m.as_str())
            .map(str::to_string)
            .unwrap_or_else(|| "set_pools_failed".into());
        return Err(MinerPulseError::Coded {
            code: ErrorCode::InvalidInput,
            message: Some(message),
        });
    }
    Ok(())
}

pub fn pools_match_expected(host: &str, expected: &[WhatsminerPoolConfig]) -> bool {
    let Ok(actual) = read_pool_configs(host) else {
        return false;
    };
    let expected = normalize_pool_slots(expected.to_vec());
    let actual = normalize_pool_slots(actual);
    expected.iter().zip(actual.iter()).all(|(a, b)| {
        a.url.trim() == b.url.trim() && a.worker.trim() == b.worker.trim()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_to_three_slots() {
        let slots = normalize_pool_slots(vec![WhatsminerPoolConfig {
            url: "stratum+tcp://x:3333".into(),
            worker: "w".into(),
            password: String::new(),
        }]);
        assert_eq!(slots.len(), POOL_SLOT_COUNT);
        assert_eq!(slots[0].url, "stratum+tcp://x:3333");
        assert!(slots[1].url.is_empty());
    }
}
