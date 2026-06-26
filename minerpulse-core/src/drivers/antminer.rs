use super::parse::{get_parameter, parse_f64, parse_u64};
use super::MinerDriver;
use crate::error::MinerPulseError;
use crate::model::{
    FanStats, HashrateStats, MinerIdentity, MinerSnapshot, MinerVendor, PoolInfo, ThermalStats,
};
use crate::tcp::TcpCgminerClient;
use serde_json::Value;

pub struct AntminerDriver;

impl MinerDriver for AntminerDriver {
    fn id(&self) -> &'static str {
        "antminer"
    }

    fn detect(stats_response: &str) -> bool
    where
        Self: Sized,
    {
        if detect_antminer_json(stats_response) {
            return true;
        }
        get_parameter(stats_response, "Type")
            .map(|kind| kind.contains("Antminer"))
            .unwrap_or(false)
    }

    fn fetch_snapshot(
        &self,
        client: &TcpCgminerClient,
        host: &str,
        port: u16,
    ) -> Result<MinerSnapshot, MinerPulseError> {
        let stats = client
            .send_receive(host, port, "stats", "", true)
            .or_else(|_| client.send_receive(host, port, "stats", "", false))?;

        let pools_raw = client
            .send_receive(host, port, "pools", "", true)
            .unwrap_or_default();

        Ok(parse_antminer_snapshot(&stats, &pools_raw))
    }
}

fn detect_antminer_json(raw: &str) -> bool {
    let trimmed = raw.trim();
    if !trimmed.starts_with('{') {
        return false;
    }

    let value: Value = match serde_json::from_str(trimmed) {
        Ok(v) => v,
        Err(_) => return false,
    };

    value
        .get("STATS")
        .and_then(|s| s.as_array())
        .and_then(|items| items.first())
        .and_then(|item| item.get("Type"))
        .and_then(|t| t.as_str())
        .map(|t| t.contains("Antminer"))
        .unwrap_or(false)
}

fn json_f64(obj: &Value, key: &str) -> Option<f64> {
    obj.get(key).and_then(|v| {
        v.as_f64()
            .or_else(|| v.as_str().and_then(parse_f64))
            .or_else(|| v.as_i64().map(|n| n as f64))
    })
}

fn json_u64(obj: &Value, key: &str) -> Option<u64> {
    obj.get(key).and_then(|v| {
        v.as_u64()
            .or_else(|| v.as_i64().map(|n| n.max(0) as u64))
            .or_else(|| v.as_str().and_then(parse_u64))
    })
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
                            .or_else(|| pool.get("Stratum Active"))
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

pub fn parse_antminer_snapshot(stats_raw: &str, pools_raw: &str) -> MinerSnapshot {
    if stats_raw.trim().starts_with('{') {
        if let Some(snapshot) = parse_antminer_json(stats_raw, pools_raw) {
            return snapshot;
        }
    }

    parse_antminer_pipe(stats_raw, pools_raw)
}

fn parse_antminer_json(stats_raw: &str, pools_raw: &str) -> Option<MinerSnapshot> {
    let value: Value = serde_json::from_str(stats_raw.trim()).ok()?;
    let stats = value.get("STATS")?.as_array()?.first()?;

    let model = stats
        .get("Type")
        .and_then(|v| v.as_str())
        .unwrap_or("Antminer")
        .to_string();

    let firmware = stats
        .get("Miner")
        .or_else(|| stats.get("BMMiner"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let mut hashrate = HashrateStats::default();
    hashrate.avg5s_ghs = json_f64(stats, "GHS 5s").unwrap_or(0.0);
    hashrate.avg_ghs = json_f64(stats, "GHS av").unwrap_or(0.0);
    hashrate.current_ghs = hashrate.avg5s_ghs;

    let mut thermal = ThermalStats::default();
    thermal.inlet_c = json_f64(stats, "Temperature");

    let mut fans = FanStats::default();
    if let Some(fan_in) = json_u64(stats, "Fan Speed In") {
        fans.rpm.push(fan_in as u32);
    }
    if let Some(fan_out) = json_u64(stats, "Fan Speed Out") {
        fans.rpm.push(fan_out as u32);
    }

    let status = stats
        .get("Status")
        .and_then(|v| v.as_str())
        .unwrap_or("Unknown")
        .to_string();

    let uptime_sec = json_u64(stats, "Elapsed");

    let mut raw_log = stats_raw.to_string();
    if !pools_raw.is_empty() {
        raw_log.push_str("\n---\n");
        raw_log.push_str(pools_raw);
    }

    Some(MinerSnapshot {
        identity: MinerIdentity {
            vendor: MinerVendor::Antminer,
            model,
            firmware,
            driver_id: "antminer".to_string(),
        },
        hashrate,
        thermal,
        fans,
        power: Default::default(),
        pools: parse_pools_json(pools_raw),
        raw_log,
        status,
        uptime_sec,
    })
}

fn parse_antminer_pipe(stats_raw: &str, pools_raw: &str) -> MinerSnapshot {
    let model = get_parameter(stats_raw, "Type").unwrap_or_else(|| "Antminer".to_string());

    let mut hashrate = HashrateStats::default();
    hashrate.avg5s_ghs = get_parameter(stats_raw, "GHS 5s")
        .and_then(|s| parse_f64(&s))
        .unwrap_or(0.0);
    hashrate.avg_ghs = get_parameter(stats_raw, "GHS av")
        .and_then(|s| parse_f64(&s))
        .unwrap_or(0.0);
    hashrate.current_ghs = hashrate.avg5s_ghs;

    let mut thermal = ThermalStats::default();
    thermal.inlet_c = get_parameter(stats_raw, "Temperature").and_then(|s| parse_f64(&s));

    let mut fans = FanStats::default();
    if let Some(fan_in) = get_parameter(stats_raw, "Fan Speed In").and_then(|s| parse_u64(&s)) {
        fans.rpm.push(fan_in as u32);
    }
    if let Some(fan_out) = get_parameter(stats_raw, "Fan Speed Out").and_then(|s| parse_u64(&s)) {
        fans.rpm.push(fan_out as u32);
    }

    let mut raw_log = stats_raw.to_string();
    if !pools_raw.is_empty() {
        raw_log.push_str("\n---\n");
        raw_log.push_str(pools_raw);
    }

    MinerSnapshot {
        identity: MinerIdentity {
            vendor: MinerVendor::Antminer,
            model,
            firmware: String::new(),
            driver_id: "antminer".to_string(),
        },
        hashrate,
        thermal,
        fans,
        power: Default::default(),
        pools: parse_pools_json(pools_raw),
        raw_log,
        status: get_parameter(stats_raw, "Status").unwrap_or_else(|| "Unknown".to_string()),
        uptime_sec: get_parameter(stats_raw, "Elapsed").and_then(|s| parse_u64(&s)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_antminer_json_stats() {
        let sample = r#"{"STATUS":[{"STATUS":"S"}],"STATS":[{"Type":"Antminer L7","GHS 5s":"9500.00"}]}"#;
        assert!(AntminerDriver::detect(sample));
    }

    #[test]
    fn parses_antminer_json_snapshot() {
        let stats = r#"{"STATS":[{"Type":"Antminer L7","GHS 5s":"9500.00","GHS av":"9400.00","Temperature":65,"Elapsed":3600}]}"#;
        let snap = parse_antminer_snapshot(stats, "");
        assert_eq!(snap.identity.model, "Antminer L7");
        assert!((snap.hashrate.avg5s_ghs - 9500.0).abs() < 0.01);
        assert_eq!(snap.uptime_sec, Some(3600));
    }
}
