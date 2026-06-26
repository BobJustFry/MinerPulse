use super::json_util::{
    array_items, collect_fan_rpms, collect_temp_boards, json_f64, json_hashrate_ghs, json_str,
    json_u64, merge_stats_objects,
};
use super::parse::{get_parameter, parse_f64, parse_u64};
use super::MinerDriver;
use crate::error::MinerPulseError;
use crate::model::{
    BoardStats, FanStats, HashrateStats, MinerIdentity, MinerSnapshot, MinerVendor, PoolInfo,
    PowerStats, ThermalStats,
};
use crate::tcp::TcpCgminerClient;
use serde_json::Value;

const HASHRATE_KEYS: [&str; 8] = [
    "GHS 5s",
    "GHS av",
    "GHS 30m",
    "GHS 5m",
    "MHS 5s",
    "MHS av",
    "rate_5s",
    "rate_avg",
];

const AVG_HASHRATE_KEYS: [&str; 6] = [
    "GHS av",
    "GHS 30m",
    "GHS 5m",
    "MHS av",
    "MHS 1m",
    "rate_avg",
];

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

        let summary = client
            .send_receive(host, port, "summary", "", true)
            .unwrap_or_default();

        let pools_raw = client
            .send_receive(host, port, "pools", "", true)
            .unwrap_or_default();

        let devs_raw = client
            .send_receive(host, port, "devs", "", true)
            .unwrap_or_default();

        Ok(parse_antminer_snapshot(
            &stats,
            &summary,
            &pools_raw,
            &devs_raw,
        ))
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

    if let Some(merged) = merge_stats_objects(&value) {
        if json_str(&merged, "Type")
            .map(|kind| kind.contains("Antminer"))
            .unwrap_or(false)
        {
            return true;
        }
        if json_hashrate_ghs(&merged, &HASHRATE_KEYS).is_some() {
            return true;
        }
    }

    if let Some(summary) = merge_summary_objects(&value) {
        return json_str(&summary, "Type")
            .map(|kind| kind.contains("Antminer"))
            .unwrap_or(false)
            || json_hashrate_ghs(&summary, &HASHRATE_KEYS).is_some();
    }

    false
}

fn merge_summary_objects(value: &Value) -> Option<Value> {
    let items = array_items(value, "SUMMARY")?;
    let mut merged = serde_json::Map::new();
    for item in items {
        let Some(obj) = item.as_object() else {
            continue;
        };
        for (key, val) in obj {
            merged.insert(key.clone(), val.clone());
        }
    }
    if merged.is_empty() {
        None
    } else {
        Some(Value::Object(merged))
    }
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
                        worker: json_str(pool, "User").unwrap_or("").to_string(),
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

        let hashrate_ghs = json_hashrate_ghs(dev, &HASHRATE_KEYS);

        boards.push(BoardStats {
            label: if label == "Board" {
                format!("Board {}", index + 1)
            } else {
                label
            },
            hashrate_ghs,
            temp_c: json_f64(dev, "Temperature").or_else(|| json_temp_from_dev(dev)),
            fan_rpm: json_u64(dev, "Fan Speed").map(|rpm| rpm as u32),
            status: json_str(dev, "Status").unwrap_or("").to_string(),
            ..Default::default()
        });

        if hw_errors.is_none() {
            hw_errors = json_u64(dev, "Hardware Errors");
        }
    }

    (boards, hw_errors)
}

fn json_temp_from_dev(dev: &Value) -> Option<f64> {
    for index in 1..=4 {
        let key = format!("temp{index}");
        if let Some(value) = json_f64(dev, &key) {
            return Some(value);
        }
    }
    None
}

pub fn detect_antminer_summary(raw: &str) -> bool {
    let trimmed = raw.trim();
    if !trimmed.starts_with('{') {
        return false;
    }

    let value: Value = match serde_json::from_str(trimmed) {
        Ok(v) => v,
        Err(_) => return false,
    };

    let Some(summary) = merge_summary_objects(&value) else {
        return false;
    };

    if json_str(&summary, "Type")
        .map(|kind| kind.contains("Antminer"))
        .unwrap_or(false)
    {
        return true;
    }

    json_f64(&summary, "GHS 5s").is_some() && json_str(&summary, "Miner Type").is_none()
}

pub fn split_antminer_log(raw: &str) -> (String, String, String, String) {
    let mut stats = String::new();
    let mut summary = String::new();
    let mut pools = String::new();
    let mut devs = String::new();
    let mut section = "stats";

    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed == "--- pools ---" {
            section = "pools";
            continue;
        }
        if trimmed == "--- summary ---" {
            section = "summary";
            continue;
        }
        if trimmed == "--- devs ---" {
            section = "devs";
            continue;
        }

        let target = match section {
            "pools" => &mut pools,
            "summary" => &mut summary,
            "devs" => &mut devs,
            _ => &mut stats,
        };
        target.push_str(line);
        target.push('\n');
    }

    (
        stats.trim().to_string(),
        summary.trim().to_string(),
        pools.trim().to_string(),
        devs.trim().to_string(),
    )
}

fn hashrate_scale(stats: &Value, model: &str) -> f64 {
    if model.contains("E9") || model.contains("D7") {
        return 1.0 / 1000.0;
    }
    if let Some(theory) = json_f64(stats, "total_rateideal") {
        if theory > 0.0 && theory < 50_000.0 {
            return 1.0 / 1000.0;
        }
    }
    if let (Some(ghs5), Some(theory)) = (
        json_f64(stats, "GHS 5s"),
        json_f64(stats, "total_rateideal"),
    ) {
        if ghs5 > 0.0 && ghs5 < 50_000.0 && theory > 0.0 && theory < 50_000.0 {
            return 1.0 / 1000.0;
        }
    }
    1.0
}

fn scaled_ghs(stats: &Value, key: &str, scale: f64) -> Option<f64> {
    json_f64(stats, key).map(|value| {
        if key.contains("MHS") || key.contains("mhs") {
            value / 1000.0
        } else {
            value * scale
        }
    })
}

fn parse_dash_temps(raw: &str) -> Vec<i32> {
    raw.split('-')
        .filter_map(|part| part.trim().parse::<i32>().ok())
        .collect()
}

fn chain_count(stats: &Value) -> usize {
    if let Some(count) = json_u64(stats, "miner_count") {
        if count > 0 {
            return count as usize;
        }
    }

    (1..=16)
        .rev()
        .find(|index| {
            json_f64(stats, &format!("chain_rate{index}")).is_some()
                || json_str(stats, &format!("chain_acs{index}")).is_some()
        })
        .unwrap_or(0)
}

fn chain_status(stats: &Value, index: usize) -> String {
    let key = format!("chain_acs{index}");
    json_str(stats, &key)
        .map(|acs| {
            let normalized = acs.to_ascii_lowercase();
            if !normalized.is_empty() && normalized.chars().all(|c| c == 'o') {
                "Alive".to_string()
            } else if normalized.contains('x') {
                "Error".to_string()
            } else if normalized.is_empty() {
                String::new()
            } else {
                "Mixed".to_string()
            }
        })
        .unwrap_or_default()
}

fn inlet_temp_from_stats(stats: &Value) -> Option<f64> {
    let mut values = Vec::new();
    for index in 1..=16 {
        for key in [format!("temp_in_pcb_{index}"), format!("temp_in_chip_{index}")] {
            if let Some(raw) = json_str(stats, &key) {
                for value in parse_dash_temps(raw) {
                    values.push(value as f64);
                }
            }
        }
    }
    if values.is_empty() {
        return json_temp_c(stats);
    }
    values.into_iter().reduce(f64::min)
}

fn boards_from_stats_chains(
    stats: &Value,
    model: &str,
) -> (Vec<f64>, Vec<BoardStats>, Vec<i32>) {
    let count = chain_count(stats);
    if count == 0 {
        return (Vec::new(), Vec::new(), Vec::new());
    }

    let scale = hashrate_scale(stats, model);
    let mut per_board_ghs = Vec::new();
    let mut per_chip_c = Vec::new();
    let mut boards = Vec::new();

    for index in 1..=count {
        let hashrate = scaled_ghs(stats, &format!("chain_rate{index}"), scale).or_else(|| {
            json_str(stats, &format!("CHAIN AVG HASHRATE{index}"))
                .and_then(|raw| raw.split_whitespace().next())
                .and_then(|num| num.parse::<f64>().ok())
                .map(|mh| mh / 1000.0)
        });
        per_board_ghs.push(hashrate.unwrap_or(0.0));

        let board_temp = json_f64(stats, &format!("temp{index}"));
        let chip_max = json_f64(stats, &format!("temp2_{index}"));
        let mut chip_temps = Vec::new();
        for key in [format!("temp_in_chip_{index}"), format!("temp_out_chip_{index}")] {
            if let Some(raw) = json_str(stats, &key) {
                chip_temps.extend(parse_dash_temps(raw));
            }
        }

        let chip_min = chip_temps.iter().copied().min().map(|v| v as f64);
        let chip_avg = if chip_temps.is_empty() {
            None
        } else {
            Some(chip_temps.iter().sum::<i32>() as f64 / chip_temps.len() as f64)
        };
        let chip_max_from_list = chip_temps.iter().copied().max().map(|v| v as f64);

        per_chip_c.extend(chip_temps);

        boards.push(BoardStats {
            label: format!("Chain {index}"),
            hashrate_ghs: hashrate,
            temp_c: board_temp.or(chip_max),
            fan_rpm: None,
            status: chain_status(stats, index),
            chip_temp_min_c: chip_min,
            chip_temp_avg_c: chip_avg,
            chip_temp_max_c: chip_max.or(chip_max_from_list),
            effective_chips: json_u64(stats, &format!("chain_acn{index}")).map(|v| v as u32),
        });
    }

    (per_board_ghs, boards, per_chip_c)
}

fn apply_chain_stats(snapshot: &mut MinerSnapshot, stats: &Value) {
    let (per_board_ghs, boards, per_chip_c) =
        boards_from_stats_chains(stats, &snapshot.identity.model);
    if boards.is_empty() {
        return;
    }

    snapshot.boards = boards;
    if !per_board_ghs.is_empty() {
        snapshot.hashrate.per_board_ghs = per_board_ghs;
    }
    if !per_chip_c.is_empty() {
        snapshot.thermal.per_chip_c = per_chip_c;
    }

    let board_max: Vec<f64> = snapshot
        .boards
        .iter()
        .filter_map(|board| board.chip_temp_max_c.or(board.temp_c))
        .collect();
    if !board_max.is_empty() {
        snapshot.thermal.per_board_max_c = board_max;
    }
}

pub fn parse_antminer_snapshot(
    stats_raw: &str,
    summary_raw: &str,
    pools_raw: &str,
    devs_raw: &str,
) -> MinerSnapshot {
    let mut snapshot = if stats_raw.trim().starts_with('{') {
        parse_antminer_json(stats_raw, pools_raw).unwrap_or_else(|| {
            parse_antminer_pipe(stats_raw, pools_raw)
        })
    } else {
        parse_antminer_pipe(stats_raw, pools_raw)
    };

    if summary_raw.trim().starts_with('{') {
        enrich_from_summary(&mut snapshot, summary_raw);
    }

    let (dev_boards, dev_hw_errors) = parse_antminer_devs(devs_raw);
    if !dev_boards.is_empty() {
        snapshot.boards = dev_boards;
        if snapshot.hashrate.avg5s_ghs <= 0.0 {
            let total: f64 = snapshot
                .boards
                .iter()
                .filter_map(|board| board.hashrate_ghs)
                .sum();
            if total > 0.0 {
                snapshot.hashrate.avg5s_ghs = total;
                snapshot.hashrate.current_ghs = total;
            }
        }
    } else if snapshot.boards.is_empty() {
        snapshot.boards = boards_from_vectors(
            &snapshot.hashrate.per_board_ghs,
            &snapshot.thermal.per_board_max_c,
            &snapshot.fans.rpm,
        );
    }

    if snapshot.boards.is_empty() && stats_raw.trim().starts_with('{') {
        if let Ok(value) = serde_json::from_str::<Value>(stats_raw.trim()) {
            if let Some(stats) = merge_stats_objects(&value) {
                apply_chain_stats(&mut snapshot, &stats);
            }
        }
    }

    if snapshot.pools.is_empty() && !pools_raw.trim().is_empty() {
        snapshot.pools = parse_pools_json(pools_raw);
    }

    if snapshot.hw_errors.is_none() {
        snapshot.hw_errors = dev_hw_errors;
    }

    if !summary_raw.is_empty() && !snapshot.raw_log.contains("--- summary ---") {
        snapshot.raw_log.push_str("\n--- summary ---\n");
        snapshot.raw_log.push_str(summary_raw);
    }
    if !devs_raw.is_empty() && !snapshot.raw_log.contains("--- devs ---") {
        snapshot.raw_log.push_str("\n--- devs ---\n");
        snapshot.raw_log.push_str(devs_raw);
    }

    snapshot
}

fn enrich_from_summary(snapshot: &mut MinerSnapshot, summary_raw: &str) {
    let Ok(value) = serde_json::from_str::<Value>(summary_raw.trim()) else {
        return;
    };
    let Some(summary) = merge_summary_objects(&value) else {
        return;
    };

    if snapshot.identity.model.is_empty() || snapshot.identity.model == "Antminer" {
        if let Some(model) = json_str(&summary, "Type") {
            snapshot.identity.model = model.to_string();
        }
    }

    if snapshot.hashrate.avg5s_ghs <= 0.0 {
        let scale = hashrate_scale(&summary, &snapshot.identity.model);
        snapshot.hashrate.avg5s_ghs = scaled_ghs(&summary, "GHS 5s", scale)
            .or_else(|| json_hashrate_ghs(&summary, &HASHRATE_KEYS))
            .unwrap_or(0.0);
        snapshot.hashrate.current_ghs = snapshot.hashrate.avg5s_ghs;
    }
    if snapshot.hashrate.avg_ghs <= 0.0 {
        let scale = hashrate_scale(&summary, &snapshot.identity.model);
        snapshot.hashrate.avg_ghs = scaled_ghs(&summary, "GHS av", scale)
            .or_else(|| scaled_ghs(&summary, "GHS 30m", scale))
            .or_else(|| json_hashrate_ghs(&summary, &AVG_HASHRATE_KEYS))
            .unwrap_or(0.0);
    }

    if snapshot.thermal.inlet_c.is_none() {
        snapshot.thermal.inlet_c = json_temp_c(&summary);
    }
    if snapshot.fans.rpm.is_empty() {
        snapshot.fans.rpm = collect_fan_rpms(&summary);
    }
    if snapshot.power.watts.is_none() {
        snapshot.power.watts = json_f64(&summary, "Power")
            .or_else(|| json_f64(&summary, "Power Consumption"));
    }
    if snapshot.uptime_sec.is_none() {
        snapshot.uptime_sec = json_u64(&summary, "Elapsed");
    }
    if snapshot.shares_accepted.is_none() {
        snapshot.shares_accepted = json_u64(&summary, "Accepted");
    }
    if snapshot.shares_rejected.is_none() {
        snapshot.shares_rejected = json_u64(&summary, "Rejected");
    }
    if snapshot.hw_errors.is_none() {
        snapshot.hw_errors = json_u64(&summary, "Hardware Errors");
    }
}

fn json_temp_c(obj: &Value) -> Option<f64> {
    super::json_util::json_temp_c(obj)
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
            ..Default::default()
        })
        .collect()
}

fn parse_antminer_json(stats_raw: &str, pools_raw: &str) -> Option<MinerSnapshot> {
    let value: Value = serde_json::from_str(stats_raw.trim()).ok()?;
    let stats = merge_stats_objects(&value)?;

    let model = json_str(&stats, "Type").unwrap_or("Antminer").to_string();
    let firmware = json_str(&stats, "Miner")
        .or_else(|| json_str(&stats, "BMMiner"))
        .or_else(|| json_str(&stats, "CompileTime"))
        .unwrap_or("")
        .to_string();

    let scale = hashrate_scale(&stats, &model);
    let mut hashrate = HashrateStats::default();
    hashrate.avg5s_ghs = scaled_ghs(&stats, "GHS 5s", scale)
        .or_else(|| json_hashrate_ghs(&stats, &HASHRATE_KEYS))
        .unwrap_or(0.0);
    hashrate.avg_ghs = scaled_ghs(&stats, "GHS av", scale)
        .or_else(|| scaled_ghs(&stats, "rate_30m", scale))
        .or_else(|| json_hashrate_ghs(&stats, &AVG_HASHRATE_KEYS))
        .unwrap_or(0.0);
    hashrate.current_ghs = hashrate.avg5s_ghs;

    let mut thermal = ThermalStats::default();
    thermal.inlet_c = inlet_temp_from_stats(&stats);
    thermal.per_board_max_c = collect_temp_boards(&stats);
    if let Some(max) = json_f64(&stats, "temp_max") {
        if thermal.per_board_max_c.is_empty() {
            thermal.per_board_max_c.push(max);
        }
    }

    let mut fans = FanStats::default();
    fans.rpm = collect_fan_rpms(&stats);

    let mut power = PowerStats::default();
    power.watts = json_f64(&stats, "Power")
        .or_else(|| json_f64(&stats, "Power Consumption"))
        .or_else(|| json_f64(&stats, "Power Rate"));
    power.voltage = json_f64(&stats, "Voltage");

    let status = json_str(&stats, "Status")
        .or_else(|| json_str(&stats, "Work"))
        .unwrap_or("Unknown")
        .to_string();
    let uptime_sec = json_u64(&stats, "Elapsed");

    let mut raw_log = stats_raw.to_string();
    if !pools_raw.is_empty() {
        raw_log.push_str("\n--- pools ---\n");
        raw_log.push_str(pools_raw);
    }

    let mut snapshot = MinerSnapshot {
        identity: MinerIdentity {
            vendor: MinerVendor::Antminer,
            model: model.clone(),
            firmware,
            driver_id: "antminer".to_string(),
        },
        hashrate,
        thermal,
        fans,
        power,
        boards: Vec::new(),
        pools: parse_pools_json(pools_raw),
        shares_accepted: json_u64(&stats, "Accepted"),
        shares_rejected: json_u64(&stats, "Rejected"),
        hw_errors: json_u64(&stats, "Hardware Errors")
            .or_else(|| json_u64(&stats, "no_matching_work")),
        raw_log,
        status,
        uptime_sec,
        ..Default::default()
    };

    apply_chain_stats(&mut snapshot, &stats);
    Some(snapshot)
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
        ..Default::default()
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
    fn parses_split_stats_entries() {
        let stats = r#"{"STATS":[{"BMMiner":"1.0.0","Type":"Antminer L7"},{"GHS 5s":9500.0,"GHS av":9400.0,"Temperature":65.0,"Fan Speed In":3200,"Elapsed":3600}]}"#;
        let snap = parse_antminer_snapshot(stats, "", "", "");
        assert_eq!(snap.identity.model, "Antminer L7");
        assert!((snap.hashrate.avg5s_ghs - 9500.0).abs() < 0.01);
        assert_eq!(snap.fans.rpm.len(), 1);
        assert_eq!(snap.uptime_sec, Some(3600));
    }

    #[test]
    fn parses_antminer_mhs_fields() {
        let stats = r#"{"STATS":[{"Type":"Antminer E9","MHS 5s":2400000.0,"MHS av":2380000.0,"Temperature":58.0}]}"#;
        let snap = parse_antminer_snapshot(stats, "", "", "");
        assert!((snap.hashrate.avg5s_ghs - 2400.0).abs() < 0.01);
    }

    #[test]
    fn parses_antminer_devs_boards() {
        let devs = r#"{"DEVS":[{"Name":"chain0","GHS 5s":3100.0,"Temperature":68,"Status":"Alive"}]}"#;
        let (boards, _) = parse_antminer_devs(devs);
        assert_eq!(boards.len(), 1);
        assert_eq!(boards[0].label, "chain0");
    }

    #[test]
    fn parses_e9_pro_stats_chains_and_scales_mhs() {
        let stats = r#"{"STATUS":[{"STATUS":"S"}],"STATS":[{"BMMiner":"2.12","Miner":"14.152-2.0.0","Type":"Antminer E9 Pro"},{"STATS":2,"GHS 5s":4008.63,"GHS av":3972.28,"total_rateideal":3800.0,"miner_count":2,"fan1":5760,"fan2":5760,"fan3":5760,"fan4":5760,"temp1":56,"temp2":57,"temp2_1":71,"temp2_2":72,"temp_max":72,"temp_in_chip_1":"64-63-71-71","temp_in_chip_2":"64-62-70-70","chain_rate1":2.003863936,"chain_rate2":1.986573568,"chain_acs1":"oooooooo","chain_acs2":"oooooooo","chain_acn1":8,"chain_acn2":8}]}"#;
        let summary = r#"{"SUMMARY":[{"Elapsed":11912,"GHS 5s":4008.63,"GHS av":3972.28,"Accepted":4738,"Rejected":64,"Hardware Errors":2}]}"#;
        let snap = parse_antminer_snapshot(stats, summary, "", r#"{"STATUS":[{"STATUS":"E","Msg":"Invalid command"}]}"#);
        assert_eq!(snap.identity.model, "Antminer E9 Pro");
        assert!((snap.hashrate.avg5s_ghs - 4.00863).abs() < 0.01);
        assert_eq!(snap.boards.len(), 2);
        assert_eq!(snap.fans.rpm.len(), 4);
        assert_eq!(snap.hw_errors, Some(2));
        assert_eq!(snap.shares_accepted, Some(4738));
        assert!(!snap.thermal.per_chip_c.is_empty());
    }

    #[test]
    fn splits_combined_antminer_log() {
        let raw = r#"{"STATS":[{"Type":"Antminer L7"},{"GHS 5s":9500.0}]}
--- summary ---
{"SUMMARY":[{"GHS 5s":9500.0}]}
--- pools ---
{"POOLS":[{"URL":"stratum+tcp://pool","User":"worker","Status":"Alive","Accepted":1,"Rejected":0}]}"#;
        let (stats, summary, pools, devs) = split_antminer_log(raw);
        assert!(stats.contains("Antminer L7"));
        assert!(summary.contains("SUMMARY"));
        assert!(pools.contains("POOLS"));
        assert!(devs.is_empty());
    }
}
