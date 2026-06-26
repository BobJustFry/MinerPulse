use super::json_util::{array_items, first_in_array, json_f64, json_str, json_u64};
use super::parse::{get_parameter, parse_f64, parse_u64};
use super::MinerDriver;
use crate::error::MinerPulseError;
use crate::model::{
    BoardStats, FanStats, HashrateStats, MinerIdentity, MinerSnapshot, MinerVendor, PoolInfo,
    PowerStats, ThermalStats,
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

        let devs_raw = client
            .send_receive(host, port, "devs", "", true)
            .unwrap_or_default();

        Ok(parse_antminer_snapshot(&stats, &pools_raw, &devs_raw))
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

    first_in_array(&value, "STATS")
        .and_then(|stats| json_str(stats, "Type"))
        .map(|kind| kind.contains("Antminer"))
        .unwrap_or(false)
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

    array_items(&value, "POOLS")
        .map(|pools| {
            pools
                .iter()
                .filter_map(|pool| {
                    Some(PoolInfo {
                        url: json_str(pool, "URL")?.to_string(),
                        worker: json_str(pool, "User")
                            .or_else(|| json_str(pool, "Stratum Active"))
                            .unwrap_or("")
                            .to_string(),
                        status: json_str(pool, "Status").unwrap_or("Unknown").to_string(),
                        accepted: json_u64(pool, "Accepted").unwrap_or(0),
                        rejected: json_u64(pool, "Rejected").unwrap_or(0),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn parse_antminer_devs(raw: &str) -> (Vec<BoardStats>, Option<u64>) {
    let trimmed = raw.trim();
    if !trimmed.starts_with('{') {
        return (Vec::new(), None);
    }

    let value: Value = match serde_json::from_str(trimmed) {
        Ok(v) => v,
        Err(_) => return (Vec::new(), None),
    };

    let Some(devs) = array_items(&value, "DEVS") else {
        return (Vec::new(), None);
    };

    let mut boards = Vec::new();
    let mut hw_errors = None;

    for (index, dev) in devs.iter().enumerate() {
        let label = json_str(dev, "Name")
            .or_else(|| json_str(dev, "ID"))
            .unwrap_or("Board")
            .to_string();

        let hashrate_ghs = json_f64(dev, "GHS 5s")
            .or_else(|| json_f64(dev, "GHS av"))
            .or_else(|| json_f64(dev, "MHS 5s").map(|mhs| mhs / 1000.0));

        boards.push(BoardStats {
            label: if label == "Board" {
                format!("Board {}", index + 1)
            } else {
                label
            },
            hashrate_ghs,
            temp_c: json_f64(dev, "Temperature"),
            fan_rpm: json_u64(dev, "Fan Speed").map(|rpm| rpm as u32),
            status: json_str(dev, "Status").unwrap_or("").to_string(),
        });

        if hw_errors.is_none() {
            hw_errors = json_u64(dev, "Hardware Errors");
        }
    }

    (boards, hw_errors)
}

pub fn parse_antminer_snapshot(stats_raw: &str, pools_raw: &str, devs_raw: &str) -> MinerSnapshot {
    let mut snapshot = if stats_raw.trim().starts_with('{') {
        parse_antminer_json(stats_raw, pools_raw).unwrap_or_else(|| {
            parse_antminer_pipe(stats_raw, pools_raw)
        })
    } else {
        parse_antminer_pipe(stats_raw, pools_raw)
    };

    let (dev_boards, dev_hw_errors) = parse_antminer_devs(devs_raw);
    if !dev_boards.is_empty() {
        snapshot.boards = dev_boards;
    } else if snapshot.boards.is_empty() {
        snapshot.boards = boards_from_vectors(
            &snapshot.hashrate.per_board_ghs,
            &snapshot.thermal.per_board_max_c,
            &snapshot.fans.rpm,
        );
    }

    if snapshot.hw_errors.is_none() {
        snapshot.hw_errors = dev_hw_errors;
    }

    if !devs_raw.is_empty() {
        snapshot.raw_log.push_str("\n--- devs ---\n");
        snapshot.raw_log.push_str(devs_raw);
    }

    snapshot
}

fn boards_from_vectors(
    hashrates: &[f64],
    temps: &[f64],
    fans: &[u32],
) -> Vec<BoardStats> {
    let count = hashrates.len().max(temps.len()).max(fans.len());
    if count == 0 {
        return Vec::new();
    }

    (0..count)
        .map(|index| BoardStats {
            label: format!("Board {}", index + 1),
            hashrate_ghs: hashrates.get(index).copied(),
            temp_c: temps.get(index).copied(),
            fan_rpm: fans.get(index).copied(),
            status: String::new(),
        })
        .collect()
}

fn parse_antminer_json(stats_raw: &str, pools_raw: &str) -> Option<MinerSnapshot> {
    let value: Value = serde_json::from_str(stats_raw.trim()).ok()?;
    let stats = first_in_array(&value, "STATS")?;

    let model = json_str(stats, "Type").unwrap_or("Antminer").to_string();
    let firmware = json_str(stats, "Miner")
        .or_else(|| json_str(stats, "BMMiner"))
        .or_else(|| json_str(stats, "CompileTime"))
        .unwrap_or("")
        .to_string();

    let mut hashrate = HashrateStats::default();
    hashrate.avg5s_ghs = json_f64(stats, "GHS 5s").unwrap_or(0.0);
    hashrate.avg_ghs = json_f64(stats, "GHS av")
        .or_else(|| json_f64(stats, "GHS 30m"))
        .unwrap_or(0.0);
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
    for index in 1..=4 {
        let key = format!("Fan{index}");
        if let Some(rpm) = json_u64(stats, &key) {
            fans.rpm.push(rpm as u32);
        }
    }

    let mut power = PowerStats::default();
    power.watts = json_f64(stats, "Power")
        .or_else(|| json_f64(stats, "Power Consumption"))
        .or_else(|| json_f64(stats, "Power Rate"));
    power.voltage = json_f64(stats, "Voltage");

    let status = json_str(stats, "Status").unwrap_or("Unknown").to_string();
    let uptime_sec = json_u64(stats, "Elapsed");

    let mut raw_log = stats_raw.to_string();
    if !pools_raw.is_empty() {
        raw_log.push_str("\n--- pools ---\n");
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
        power,
        boards: Vec::new(),
        pools: parse_pools_json(pools_raw),
        shares_accepted: json_u64(stats, "Accepted"),
        shares_rejected: json_u64(stats, "Rejected"),
        hw_errors: json_u64(stats, "Hardware Errors"),
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

    let mut power = PowerStats::default();
    power.watts = get_parameter(stats_raw, "Power").and_then(|s| parse_f64(&s));

    let mut raw_log = stats_raw.to_string();
    if !pools_raw.is_empty() {
        raw_log.push_str("\n--- pools ---\n");
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
        power,
        boards: Vec::new(),
        pools: parse_pools_json(pools_raw),
        shares_accepted: get_parameter(stats_raw, "Accepted").and_then(|s| parse_u64(&s)),
        shares_rejected: get_parameter(stats_raw, "Rejected").and_then(|s| parse_u64(&s)),
        hw_errors: get_parameter(stats_raw, "Hardware Errors").and_then(|s| parse_u64(&s)),
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
        let stats = r#"{"STATS":[{"Type":"Antminer L7","GHS 5s":"9500.00","GHS av":"9400.00","Temperature":65,"Accepted":100,"Rejected":1,"Elapsed":3600}]}"#;
        let snap = parse_antminer_snapshot(stats, "", "");
        assert_eq!(snap.identity.model, "Antminer L7");
        assert!((snap.hashrate.avg5s_ghs - 9500.0).abs() < 0.01);
        assert_eq!(snap.shares_accepted, Some(100));
        assert_eq!(snap.uptime_sec, Some(3600));
    }

    #[test]
    fn parses_antminer_devs_boards() {
        let devs = r#"{"DEVS":[{"Name":"chain0","GHS 5s":3100.0,"Temperature":68,"Status":"Alive"}]}"#;
        let (boards, _) = parse_antminer_devs(devs);
        assert_eq!(boards.len(), 1);
        assert_eq!(boards[0].label, "chain0");
    }
}
