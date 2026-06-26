use super::MinerDriver;
use crate::error::MinerPulseError;
use crate::model::{
    FanStats, HashrateStats, MinerIdentity, MinerSnapshot, MinerVendor, PoolInfo, PowerStats,
    ThermalStats,
};
use crate::tcp::TcpCgminerClient;
use serde_json::Value;

pub struct WhatsminerDriver;

impl MinerDriver for WhatsminerDriver {
    fn id(&self) -> &'static str {
        "whatsminer"
    }

    fn detect(response: &str) -> bool
    where
        Self: Sized,
    {
        classify_whatsminer(response).is_some()
    }

    fn fetch_snapshot(
        &self,
        client: &TcpCgminerClient,
        host: &str,
        port: u16,
    ) -> Result<MinerSnapshot, MinerPulseError> {
        let summary = client.send_payload(host, port, r#"{"cmd":"summary"}"#)?;
        let pools = client
            .send_payload(host, port, r#"{"cmd":"pools"}"#)
            .unwrap_or_default();

        Ok(parse_whatsminer_snapshot(&summary, &pools))
    }
}

pub fn classify_whatsminer(json: &str) -> Option<(MinerVendor, String)> {
    let trimmed = json.trim();
    if !trimmed.starts_with('{') {
        return None;
    }

    let value: Value = serde_json::from_str(trimmed).ok()?;
    let status_msg = value
        .get("STATUS")
        .and_then(|s| s.as_array())
        .and_then(|items| items.first())
        .and_then(|item| item.get("Msg"))
        .and_then(|msg| msg.as_str());

    let has_summary = value.get("SUMMARY").is_some();
    if !has_summary && status_msg != Some("Summary") {
        return None;
    }

    let model = value
        .get("SUMMARY")
        .and_then(|s| s.as_array())
        .and_then(|items| items.first())
        .and_then(|item| {
            item.get("Miner Type")
                .or_else(|| item.get("Type"))
                .and_then(|v| v.as_str())
                .map(str::to_string)
        })
        .unwrap_or_else(|| "WhatsMiner".to_string());

    Some((MinerVendor::Whatsminer, model))
}

fn json_f64(obj: &Value, key: &str) -> Option<f64> {
    obj.get(key).and_then(|v| {
        v.as_f64()
            .or_else(|| v.as_str().and_then(|s| s.trim().parse().ok()))
            .or_else(|| v.as_i64().map(|n| n as f64))
    })
}

fn json_u64(obj: &Value, key: &str) -> Option<u64> {
    obj.get(key).and_then(|v| {
        v.as_u64()
            .or_else(|| v.as_i64().map(|n| n.max(0) as u64))
            .or_else(|| v.as_str().and_then(|s| s.trim().parse().ok()))
    })
}

fn mhs_to_ghs(mhs: f64) -> f64 {
    mhs / 1000.0
}

fn parse_pools_json(raw: &str) -> Vec<PoolInfo> {
    let trimmed = raw.trim();
    if !trimmed.starts_with('{') {
        return Vec::new();
    }

    let value: Value = match serde_json::from_str(trimmed) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };

    value
        .get("POOLS")
        .and_then(|p| p.as_array())
        .map(|pools| {
            pools
                .iter()
                .filter_map(|pool| {
                    Some(PoolInfo {
                        url: pool.get("URL")?.as_str()?.to_string(),
                        worker: pool
                            .get("User")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                        status: pool
                            .get("Status")
                            .and_then(|v| v.as_str())
                            .unwrap_or("Unknown")
                            .to_string(),
                        accepted: json_u64(pool, "Accepted").unwrap_or(0),
                        rejected: json_u64(pool, "Rejected").unwrap_or(0),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

pub fn parse_whatsminer_snapshot(summary_raw: &str, pools_raw: &str) -> MinerSnapshot {
    let value: Value = serde_json::from_str(summary_raw.trim()).unwrap_or(Value::Null);
    let summary = value
        .get("SUMMARY")
        .and_then(|s| s.as_array())
        .and_then(|items| items.first());

    let model = summary
        .and_then(|item| {
            item.get("Miner Type")
                .or_else(|| item.get("Type"))
                .and_then(|v| v.as_str())
        })
        .unwrap_or("WhatsMiner")
        .to_string();

    let firmware = summary
        .and_then(|item| item.get("Firmware Version").and_then(|v| v.as_str()))
        .unwrap_or("")
        .to_string();

    let mut hashrate = HashrateStats::default();
    if let Some(item) = summary {
        hashrate.avg5s_ghs = json_f64(item, "MHS 5s").map(mhs_to_ghs).unwrap_or(0.0);
        hashrate.avg_ghs = json_f64(item, "MHS av")
            .or_else(|| json_f64(item, "MHS 1m"))
            .map(mhs_to_ghs)
            .unwrap_or(0.0);
        hashrate.current_ghs = hashrate.avg5s_ghs;
    }

    let mut thermal = ThermalStats::default();
    if let Some(item) = summary {
        thermal.inlet_c = json_f64(item, "Temperature")
            .or_else(|| json_f64(item, "Chip Temp Avg"));
    }

    let mut fans = FanStats::default();
    if let Some(item) = summary {
        if let Some(fan_in) = json_u64(item, "Fan Speed In") {
            fans.rpm.push(fan_in as u32);
        }
        if let Some(fan_out) = json_u64(item, "Fan Speed Out") {
            fans.rpm.push(fan_out as u32);
        }
        if fans.rpm.is_empty() {
            if let Some(fan) = json_u64(item, "Fan Speed") {
                fans.rpm.push(fan as u32);
            }
        }
    }

    let mut power = PowerStats::default();
    if let Some(item) = summary {
        power.watts = json_f64(item, "Power");
    }

    let status = summary
        .and_then(|item| item.get("Status").and_then(|v| v.as_str()))
        .unwrap_or("Unknown")
        .to_string();

    let uptime_sec = summary.and_then(|item| {
        json_u64(item, "Uptime").or_else(|| json_u64(item, "Elapsed"))
    });

    let mut raw_log = summary_raw.to_string();
    if !pools_raw.is_empty() {
        raw_log.push_str("\n---\n");
        raw_log.push_str(pools_raw);
    }

    MinerSnapshot {
        identity: MinerIdentity {
            vendor: MinerVendor::Whatsminer,
            model,
            firmware,
            driver_id: "whatsminer".to_string(),
        },
        hashrate,
        thermal,
        fans,
        power,
        pools: parse_pools_json(pools_raw),
        raw_log,
        status,
        uptime_sec,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_whatsminer_summary_json() {
        let sample = r#"{"STATUS":[{"Msg":"Summary","STATUS":"S"}],"SUMMARY":[{"Miner Type":"M50","MHS 5s":70668114.52}]}"#;
        let (vendor, model) = classify_whatsminer(sample).unwrap();
        assert_eq!(vendor, MinerVendor::Whatsminer);
        assert_eq!(model, "M50");
    }

    #[test]
    fn converts_mhs_to_ghs() {
        let sample = r#"{"SUMMARY":[{"Miner Type":"M50","MHS 5s":70668114.52,"MHS av":70000000.0}]}"#;
        let snap = parse_whatsminer_snapshot(sample, "");
        assert!((snap.hashrate.avg5s_ghs - 70668.11452).abs() < 0.01);
    }
}
