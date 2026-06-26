use super::parse::{get_parameter, get_parameter_bracket, parse_f64, parse_i32, parse_u64};
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
        if is_avalon_estats_log(stats_response) {
            return true;
        }
        if let Some(id) = get_parameter(stats_response, "ID") {
            if id.contains("AV") {
                return true;
            }
        }
        get_parameter(stats_response, "Ver").is_some()
            || get_parameter_bracket(stats_response, "Ver[").is_some()
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

fn is_avalon_estats_log(raw: &str) -> bool {
    raw.contains("CMD=estats") || raw.contains("MM ID0=") || raw.contains("ID=AVA")
}

fn flatten_log(raw: &str) -> String {
    raw.replace('\r', "")
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn parse_bracket_floats(text: &str, key: &str) -> Vec<f64> {
    get_parameter_bracket(text, key)
        .map(|value| {
            value
                .split_whitespace()
                .filter_map(|part| parse_f64(part))
                .collect()
        })
        .unwrap_or_default()
}

fn avalon_model_from_firmware(firmware: &str) -> String {
    if firmware.is_empty() {
        return "Avalon".to_string();
    }
    if firmware.contains("Pro") || firmware.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        return format!("Avalon {firmware}");
    }
    format!("Avalon-{firmware}")
}

fn parse_avalon_status(text: &str) -> String {
    if let Some(raw) = get_parameter_bracket(text, "SYSTEMSTATU[") {
        let first_line = raw.lines().next().unwrap_or(&raw).trim();
        if let Some(work) = first_line.strip_prefix("Work:") {
            return work.trim().to_string();
        }
        if !first_line.is_empty() {
            return first_line.to_string();
        }
    }
    get_parameter(text, "Work").unwrap_or_else(|| "Unknown".to_string())
}

fn parse_avalon_pools_from_lcd(raw: &str) -> Vec<PoolInfo> {
    let url = get_parameter(raw, "Current Pool");
    let worker = get_parameter(raw, "User");
    url.map(|pool_url| {
        vec![PoolInfo {
            url: pool_url,
            worker: worker.unwrap_or_default(),
            status: "Active".to_string(),
            accepted: 0,
            rejected: 0,
        }]
    })
    .unwrap_or_default()
}

pub fn parse_avalon_estats_log(raw: &str) -> MinerSnapshot {
    let flattened = flatten_log(raw);
    let firmware = get_parameter_bracket(&flattened, "Ver[")
        .or_else(|| get_parameter(&flattened, "Ver"))
        .unwrap_or_default();
    let model = avalon_model_from_firmware(&firmware);

    let mut hashrate = HashrateStats::default();
    hashrate.avg5s_ghs = get_parameter(raw, "GHS 5s")
        .and_then(|s| parse_f64(&s))
        .or_else(|| parse_f64(&get_parameter_bracket(&flattened, "GHSspd[").unwrap_or_default()))
        .unwrap_or(0.0);
    hashrate.avg_ghs = get_parameter(raw, "GHS av")
        .and_then(|s| parse_f64(&s))
        .or_else(|| {
            get_parameter_bracket(&flattened, "GHSavg[")
                .and_then(|s| parse_f64(&s))
        })
        .unwrap_or(0.0);
    hashrate.current_ghs = hashrate.avg5s_ghs;
    hashrate.per_board_ghs = parse_bracket_floats(&flattened, "MGHS[");

    let mut thermal = ThermalStats::default();
    thermal.inlet_c = get_parameter_bracket(&flattened, "Temp[")
        .and_then(|s| parse_f64(&s))
        .or_else(|| get_parameter(raw, "Temperature").and_then(|s| parse_f64(&s)));
    thermal.per_board_max_c = parse_bracket_floats(&flattened, "MTmax[");
    if thermal.per_board_max_c.is_empty() {
        if let Some(tmax) = get_parameter_bracket(&flattened, "TMax[")
            .or_else(|| get_parameter(&flattened, "TMax"))
            .and_then(|s| parse_f64(&s))
        {
            thermal.per_board_max_c.push(tmax);
        }
    }
    let board_avg = parse_bracket_floats(&flattened, "MTavg[");
    thermal.per_chip_c = board_avg
        .into_iter()
        .map(|value| value.round() as i32)
        .collect();

    let mut fans = FanStats::default();
    for index in 1..=8 {
        let key = format!("Fan{index}[");
        if let Some(raw_rpm) = get_parameter_bracket(&flattened, &key) {
            if let Some(rpm) = parse_u64(&raw_rpm) {
                fans.rpm.push(rpm as u32);
            }
        }
    }

    let mut power = PowerStats::default();
    power.voltage = get_parameter_bracket(&flattened, "Vo[")
        .and_then(|s| parse_f64(&s))
        .or_else(|| get_parameter(&flattened, "Voltage").and_then(|s| parse_f64(&s)));
    power.watts = get_parameter_bracket(&flattened, "MPO[")
        .and_then(|s| parse_f64(&s))
        .or_else(|| get_parameter(&flattened, "Power").and_then(|s| parse_f64(&s)));

    let status = parse_avalon_status(&flattened);
    let uptime_sec = get_parameter(&flattened, "Elapsed")
        .and_then(|s| parse_u64(&s))
        .or_else(|| get_parameter(raw, "Elapsed").and_then(|s| parse_u64(&s)));

    let hw_errors = get_parameter_bracket(&flattened, "HW[")
        .and_then(|s| parse_u64(&s))
        .or_else(|| get_parameter(&flattened, "Hardware Errors").and_then(|s| parse_u64(&s)));

    let board_count = hashrate
        .per_board_ghs
        .len()
        .max(thermal.per_board_max_c.len())
        .max(thermal.per_chip_c.len());
    let fan_count_matches_boards = !fans.rpm.is_empty() && fans.rpm.len() == board_count;

    let boards = if board_count == 0 {
        Vec::new()
    } else {
        (0..board_count)
            .map(|index| BoardStats {
                label: format!("Board {}", index + 1),
                hashrate_ghs: hashrate.per_board_ghs.get(index).copied(),
                temp_c: thermal
                    .per_board_max_c
                    .get(index)
                    .copied()
                    .or_else(|| {
                        thermal
                            .per_chip_c
                            .get(index)
                            .map(|value| *value as f64)
                    }),
                fan_rpm: if fan_count_matches_boards {
                    fans.rpm.get(index).copied()
                } else {
                    None
                },
                status: String::new(),
            })
            .collect()
    };

    let pools = parse_avalon_pools_from_lcd(raw);

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
        pools,
        shares_accepted: None,
        shares_rejected: None,
        hw_errors,
        raw_log: raw.to_string(),
        status,
        uptime_sec,
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
    if is_avalon_estats_log(raw) {
        let mut snapshot = parse_avalon_estats_log(raw);
        if snapshot.pools.is_empty() {
            snapshot.pools = parse_avalon_pools(pools_raw);
        }
        if !pools_raw.is_empty() && !snapshot.raw_log.contains("--- pools ---") {
            snapshot.raw_log.push_str("\n--- pools ---\n");
            snapshot.raw_log.push_str(pools_raw);
        }
        return snapshot;
    }

    let cleaned = raw.replace('\'', "").replace("  ", " ");

    let firmware = get_parameter(&cleaned, "Ver").unwrap_or_default();
    let model = avalon_model_from_firmware(&firmware);

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

    #[test]
    fn parses_avalon_estats_log_sample() {
        let sample = r#"CMD=estats
ID=AVA100
Elapsed=1797
MM ID0=Ver[1126Pro-S-64-21071301_test] Temp[27] TMax[135] Fan1[1000] Fan2[1100] MGHS[26208.88 27221.62] MTmax[87 135] MTavg[69 68] Vo[358] HW[14] SYSTEMSTATU[Work: In Work]
CMD=lcd
Elapsed=1797
GHS av=53466.99
GHS 5s=57320.03
Current Pool=stratum+tcp://pool.example:443
User=worker1"#;
        let snap = parse_avalon_estats_log(sample);
        assert!((snap.hashrate.avg5s_ghs - 57320.03).abs() < 0.01);
        assert_eq!(snap.boards.len(), 2);
        assert_eq!(snap.fans.rpm.len(), 2);
        assert_eq!(snap.hw_errors, Some(14));
        assert_eq!(snap.pools.len(), 1);
        assert_eq!(snap.status, "In Work");
    }
}
