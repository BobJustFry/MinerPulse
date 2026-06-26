use super::parse::{get_parameter, parse_f64, parse_i32, parse_u64};
use super::MinerDriver;
use crate::error::MinerPulseError;
use crate::model::{
    BoardStats, FanStats, HashrateStats, MinerIdentity, MinerSnapshot, MinerVendor, PoolInfo,
    PowerStats, ThermalStats,
};
use crate::tcp::TcpCgminerClient;

pub struct AvalonDriver;

impl MinerDriver for AvalonDriver {
    fn id(&self) -> &'static str {
        "avalon"
    }

    fn detect(stats_response: &str) -> bool
    where
        Self: Sized,
    {
        if let Some(id) = get_parameter(stats_response, "ID") {
            if id.contains("AV") {
                return true;
            }
        }
        get_parameter(stats_response, "Ver").is_some()
    }

    fn fetch_snapshot(
        &self,
        client: &TcpCgminerClient,
        host: &str,
        port: u16,
    ) -> Result<MinerSnapshot, MinerPulseError> {
        let raw = client.send_receive(host, port, "estats+lcd", "", false)?;
        let pools_raw = client
            .send_receive(host, port, "pools", "", true)
            .or_else(|_| client.send_receive(host, port, "pools", "", false))
            .unwrap_or_default();

        Ok(parse_estats(&raw, &pools_raw))
    }
}

fn parse_avalon_pools(raw: &str) -> Vec<PoolInfo> {
    let trimmed = raw.trim();
    if trimmed.starts_with('{') {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) {
            if let Some(pools) = value.get("POOLS").and_then(|p| p.as_array()) {
                return pools
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
                            accepted: pool
                                .get("Accepted")
                                .and_then(|v| v.as_u64())
                                .unwrap_or(0),
                            rejected: pool
                                .get("Rejected")
                                .and_then(|v| v.as_u64())
                                .unwrap_or(0),
                        })
                    })
                    .collect();
            }
        }
    }

    Vec::new()
}

pub fn parse_estats(raw: &str, pools_raw: &str) -> MinerSnapshot {
    let cleaned = raw.replace('\'', "").replace("  ", " ");

    let firmware = get_parameter(&cleaned, "Ver").unwrap_or_default();
    let model = if firmware.is_empty() {
        "Unknown".to_string()
    } else {
        format!("Avalon-{firmware}")
    };

    let mut hashrate = HashrateStats::default();
    if let Some(v) = get_parameter(&cleaned, "GHS 5s").and_then(|s| parse_f64(&s)) {
        hashrate.avg5s_ghs = v;
        hashrate.current_ghs = v;
    }
    if let Some(v) = get_parameter(&cleaned, "GHS av").and_then(|s| parse_f64(&s)) {
        hashrate.avg_ghs = v;
    }

    for i in 0..8 {
        let key = format!("GHS {i}");
        if let Some(v) = get_parameter(&cleaned, &key).and_then(|s| parse_f64(&s)) {
            hashrate.per_board_ghs.push(v);
        }
    }

    let mut thermal = ThermalStats::default();
    thermal.inlet_c = get_parameter(&cleaned, "Temp").and_then(|s| parse_f64(&s));
    if let Some(tmax) = get_parameter(&cleaned, "TMax").and_then(|s| parse_f64(&s)) {
        thermal.per_board_max_c.push(tmax);
    }
    for i in 0..8 {
        let key = format!("TMax[{i}]");
        if let Some(v) = get_parameter(&cleaned, &key).and_then(|s| parse_f64(&s)) {
            if i >= thermal.per_board_max_c.len() {
                thermal.per_board_max_c.push(v);
            }
        }
        let chip_key = format!("TAvg[{i}]");
        if let Some(v) = get_parameter(&cleaned, &chip_key).and_then(|s| parse_i32(&s)) {
            thermal.per_chip_c.push(v);
        }
    }

    let mut fans = FanStats::default();
    for i in 1..=8 {
        let key = format!("Fan{i}");
        if let Some(rpm) = get_parameter(&cleaned, &key).and_then(|s| parse_u64(&s)) {
            fans.rpm.push(rpm as u32);
        }
    }

    let mut power = PowerStats::default();
    power.watts = get_parameter(&cleaned, "Power").and_then(|s| parse_f64(&s));
    power.voltage = get_parameter(&cleaned, "Voltage").and_then(|s| parse_f64(&s));

    let status = get_parameter(&cleaned, "Work").unwrap_or_else(|| "Unknown".to_string());

    let board_count = hashrate
        .per_board_ghs
        .len()
        .max(thermal.per_board_max_c.len())
        .max(fans.rpm.len());

    let boards = (0..board_count)
        .map(|index| BoardStats {
            label: format!("Board {}", index + 1),
            hashrate_ghs: hashrate.per_board_ghs.get(index).copied(),
            temp_c: thermal.per_board_max_c.get(index).copied(),
            fan_rpm: fans.rpm.get(index).copied(),
            status: String::new(),
        })
        .collect();

    let mut raw_log = raw.to_string();
    if !pools_raw.is_empty() {
        raw_log.push_str("\n--- pools ---\n");
        raw_log.push_str(pools_raw);
    }

    MinerSnapshot {
        identity: MinerIdentity {
            vendor: MinerVendor::Avalon,
            model,
            firmware: firmware.clone(),
            driver_id: "avalon".to_string(),
        },
        hashrate,
        thermal,
        fans,
        power,
        boards,
        pools: parse_avalon_pools(pools_raw),
        shares_accepted: get_parameter(&cleaned, "Accepted").and_then(|s| parse_u64(&s)),
        shares_rejected: get_parameter(&cleaned, "Rejected").and_then(|s| parse_u64(&s)),
        hw_errors: get_parameter(&cleaned, "Hardware Errors").and_then(|s| parse_u64(&s)),
        raw_log,
        status,
        uptime_sec: get_parameter(&cleaned, "Elapsed").and_then(|s| parse_u64(&s)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_avalon_by_ver() {
        assert!(AvalonDriver::detect("Ver=1346|Temp=30"));
    }

    #[test]
    fn builds_avalon_board_rows() {
        let sample = "Ver=1346|GHS 0=1200|GHS 1=1180|TMax=72|Fan1=4200|Fan2=4300|Temp=28";
        let snap = parse_estats(sample, "");
        assert_eq!(snap.boards.len(), 2);
        assert_eq!(snap.fans.rpm.len(), 2);
    }
}
