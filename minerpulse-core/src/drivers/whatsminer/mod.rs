mod btminer_log;
mod errors;
mod layout;
mod luci;

use super::json_util::{array_items, json_f64, json_str, json_u64};
use super::MinerDriver;
use crate::error::MinerPulseError;
use crate::fetch_options::FetchOptions;
use crate::model::{
    BoardStats, FanStats, HashrateStats, MinerIdentity, MinerSnapshot, MinerVendor, PoolInfo,
    PowerStats, ThermalStats,
};
use crate::tcp::TcpCgminerClient;
use btminer_log::parse_btminer_log;
use errors::parse_error_entries;
use luci::fetch_btminer_chip_data;
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
        classify_for_discovery(response).is_some()
    }

    fn fetch_snapshot(
        &self,
        client: &TcpCgminerClient,
        host: &str,
        port: u16,
        options: &FetchOptions,
    ) -> Result<MinerSnapshot, MinerPulseError> {
        let summary = client.send_payload(host, port, r#"{"cmd":"summary"}"#)?;
        let pools = if options.fast_poll {
            String::new()
        } else {
            client
                .send_payload(host, port, r#"{"cmd":"pools"}"#)
                .unwrap_or_default()
        };
        let devs = if options.fast_poll {
            String::new()
        } else {
            client
                .send_payload(host, port, r#"{"cmd":"devs"}"#)
                .unwrap_or_default()
        };
        let edevs = client
            .send_payload(host, port, r#"{"cmd":"edevs"}"#)
            .unwrap_or_default();
        let error_codes = if options.fast_poll {
            String::new()
        } else {
            client
                .send_payload(host, port, r#"{"cmd":"get_error_code"}"#)
                .unwrap_or_default()
        };
        let device_info = if options.fast_poll {
            String::new()
        } else {
            client
                .send_payload(
                    host,
                    port,
                    r#"{"cmd":"get.device.info","param":"error-code"}"#,
                )
                .unwrap_or_default()
        };

        let (board_chips, btminer_log) = if options.fast_poll {
            (Vec::new(), String::new())
        } else {
            fetch_btminer_chip_data(host, options)
        };

        Ok(parse_whatsminer_snapshot(
            &summary,
            &pools,
            &devs,
            &edevs,
            &error_codes,
            &device_info,
            &btminer_log,
            board_chips,
        ))
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

    if value.get("msg").and_then(|msg| msg.get("summary")).is_some() {
        let model = value
            .get("msg")
            .and_then(|msg| msg.get("summary"))
            .and_then(|summary| {
                summary
                    .get("type")
                    .or_else(|| summary.get("Miner Type"))
                    .and_then(|v| v.as_str())
            })
            .unwrap_or("WhatsMiner")
            .to_string();
        return Some((MinerVendor::Whatsminer, model));
    }

    let summary_item = value.get("SUMMARY").and_then(|s| s.as_array())?.first()?;

    let has_summary = value.get("SUMMARY").is_some();
    if !has_summary && status_msg != Some("Summary") {
        return None;
    }

    // CGMiner Antminer summary uses GHS fields; WhatsMiner uses MHS / Miner Type.
    let miner_type = summary_item
        .get("Miner Type")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let has_mhs = summary_item.get("MHS 5s").is_some()
        || summary_item.get("MHS av").is_some()
        || summary_item.get("MHS 1m").is_some();
    let has_whatsminer_hash = summary_item.get("hash-realtime").is_some()
        || summary_item.get("hash-average").is_some();

    if miner_type.is_none() && !has_mhs && !has_whatsminer_hash {
        return None;
    }

    if summary_item.get("GHS 5s").is_some() && miner_type.is_none() && !has_mhs {
        return None;
    }

    let model = miner_type.unwrap_or_else(|| "WhatsMiner".to_string());

    Some((MinerVendor::Whatsminer, model))
}

/// Discovery/read path: WhatsMiner often speaks cgminer JSON with compat fields
/// (`GHS 5s`, fake `Type: Antminer …`, `ID: BTM_SOC*`).
pub fn classify_for_discovery(response: &str) -> Option<(MinerVendor, String)> {
    if let Some(result) = classify_whatsminer(response) {
        return Some(result);
    }

    let trimmed = response.trim();
    if !trimmed.starts_with('{') {
        return None;
    }

    let value: Value = serde_json::from_str(trimmed).ok()?;

    if let Some(stats) = value.get("STATS").and_then(|s| s.as_array()) {
        for item in stats {
            if let Some(model) = whatsminer_model_from_cgminer_item(item) {
                return Some((MinerVendor::Whatsminer, model));
            }
        }
    }

    if let Some(summary) = value.get("SUMMARY").and_then(|s| s.as_array()) {
        for item in summary {
            if whatsminer_summary_markers(item) {
                let model = json_str(item, "Miner Type")
                    .unwrap_or("WhatsMiner")
                    .to_string();
                return Some((MinerVendor::Whatsminer, model));
            }
        }
    }

    None
}

fn whatsminer_model_from_cgminer_item(item: &Value) -> Option<String> {
    if json_str(item, "ID")
        .map(|id| id.contains("BTM"))
        .unwrap_or(false)
    {
        return Some(
            json_str(item, "Miner Type")
                .or_else(|| json_str(item, "Type"))
                .unwrap_or("WhatsMiner")
                .to_string(),
        );
    }

    if json_f64(item, "MHS 5s").is_some()
        || json_f64(item, "MHS av").is_some()
        || json_f64(item, "MHS 1m").is_some()
    {
        return Some(
            json_str(item, "Miner Type")
                .unwrap_or("WhatsMiner")
                .to_string(),
        );
    }

    None
}

fn whatsminer_summary_markers(item: &Value) -> bool {
    json_f64(item, "MHS 5s").is_some()
        || json_f64(item, "MHS av").is_some()
        || json_str(item, "RT HASHRATE").is_some()
        || json_str(item, "AV HASHRATE").is_some()
        || json_str(item, "THEORY HASHRATE").is_some()
        || json_str(item, "hash-realtime").is_some()
        || json_str(item, "hash-average").is_some()
}

fn mhs_to_ghs(mhs: f64) -> f64 {
    mhs / 1000.0
}

fn ths_to_ghs(ths: f64) -> f64 {
    ths * 1000.0
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

    if let Some(pools) = array_items(&value, "POOLS") {
        return pools
            .iter()
            .filter_map(|pool| {
                Some(PoolInfo {
                    url: json_str(pool, "URL")?.to_string(),
                    worker: json_str(pool, "User")
                        .or_else(|| json_str(pool, "account"))
                        .unwrap_or("")
                        .to_string(),
                    status: json_str(pool, "Status").unwrap_or("Unknown").to_string(),
                    accepted: json_u64(pool, "Accepted").unwrap_or(0),
                    rejected: json_u64(pool, "Rejected").unwrap_or(0),
                })
            })
            .collect();
    }

    value
        .get("msg")
        .and_then(|msg| msg.get("pools"))
        .and_then(|pools| pools.as_array())
        .map(|pools| {
            pools
                .iter()
                .filter_map(|pool| {
                    Some(PoolInfo {
                        url: json_str(pool, "url")?.to_string(),
                        worker: json_str(pool, "account").unwrap_or("").to_string(),
                        status: json_str(pool, "status").unwrap_or("Unknown").to_string(),
                        accepted: json_u64(pool, "accepted").unwrap_or(0),
                        rejected: json_u64(pool, "rejected").unwrap_or(0),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn board_chip_temp(dev: &Value, snake: &str, title: &str) -> Option<f64> {
    json_f64(dev, snake).or_else(|| json_f64(dev, title))
}

fn parse_whatsminer_devs(raw: &str, key: &str) -> Vec<BoardStats> {
    let trimmed = raw.trim();
    if !trimmed.starts_with('{') {
        return Vec::new();
    }

    let value: Value = match serde_json::from_str(trimmed) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };

    let devs = array_items(&value, key)
        .or_else(|| {
            value
                .get("msg")
                .and_then(|msg| msg.get(key))
                .and_then(|items| items.as_array())
        })
        .or_else(|| array_items(&value, "DEVS"));

    let Some(devs) = devs else {
        return Vec::new();
    };

    devs.iter()
        .enumerate()
        .map(|(index, dev)| {
            let slot = json_u64(dev, "Slot")
                .or_else(|| json_u64(dev, "slot"))
                .or_else(|| json_u64(dev, "ASC"))
                .or_else(|| json_u64(dev, "id"))
                .unwrap_or(index as u64) as u32;

            let label = json_str(dev, "Name")
                .or_else(|| json_str(dev, "ID"))
                .map(str::to_string)
                .unwrap_or_else(|| format!("SM{slot}"));

            let hashrate = json_f64(dev, "MHS 5s")
                .or_else(|| json_f64(dev, "MHS av"))
                .or_else(|| json_f64(dev, "HS RT"))
                .map(mhs_to_ghs)
                .or_else(|| json_f64(dev, "GHS 5s"))
                .or_else(|| json_f64(dev, "hash-average").map(ths_to_ghs))
                .or_else(|| json_f64(dev, "hash-realtime").map(ths_to_ghs));

            BoardStats {
                label,
                hashrate_ghs: hashrate,
                temp_c: json_f64(dev, "Temperature")
                    .or_else(|| board_chip_temp(dev, "chip-temp-avg", "Chip Temp Avg")),
                fan_rpm: json_u64(dev, "Fan Speed").map(|rpm| rpm as u32),
                status: json_str(dev, "Status")
                    .or_else(|| json_str(dev, "status"))
                    .unwrap_or("")
                    .to_string(),
                chip_temp_min_c: board_chip_temp(dev, "chip-temp-min", "Chip Temp Min"),
                chip_temp_avg_c: board_chip_temp(dev, "chip-temp-avg", "Chip Temp Avg"),
                chip_temp_max_c: board_chip_temp(dev, "chip-temp-max", "Chip Temp Max"),
                effective_chips: json_u64(dev, "Effective Chips")
                    .or_else(|| json_u64(dev, "effective-chips"))
                    .map(|value| value as u32),
                ..Default::default()
            }
        })
        .collect()
}

pub fn parse_whatsminer_snapshot(
    summary_raw: &str,
    pools_raw: &str,
    devs_raw: &str,
    edevs_raw: &str,
    error_codes_raw: &str,
    device_info_raw: &str,
    btminer_log_raw: &str,
    board_chips: Vec<crate::model::BoardChipMap>,
) -> MinerSnapshot {
    let value: Value = serde_json::from_str(summary_raw.trim()).unwrap_or(Value::Null);
    let summary = value
        .get("SUMMARY")
        .and_then(|s| s.as_array())
        .and_then(|items| items.first())
        .or_else(|| value.get("msg").and_then(|msg| msg.get("summary")));

    let model = summary
        .and_then(|item| {
            item.get("Miner Type")
                .or_else(|| item.get("Type"))
                .or_else(|| item.get("type"))
                .and_then(|v| v.as_str())
        })
        .unwrap_or("WhatsMiner")
        .to_string();

    let firmware = summary
        .and_then(|item| {
            item.get("Firmware Version")
                .or_else(|| item.get("Btminer Version"))
                .or_else(|| item.get("btminer-version"))
                .and_then(|v| v.as_str())
        })
        .unwrap_or("")
        .to_string();

    let mut hashrate = HashrateStats::default();
    if let Some(item) = summary {
        hashrate.avg5s_ghs = json_f64(item, "MHS 5s")
            .map(mhs_to_ghs)
            .or_else(|| json_f64(item, "hash-realtime").map(ths_to_ghs))
            .unwrap_or(0.0);
        hashrate.avg_ghs = json_f64(item, "MHS av")
            .or_else(|| json_f64(item, "MHS 1m"))
            .or_else(|| json_f64(item, "MHS 15m"))
            .or_else(|| json_f64(item, "hash-average"))
            .map(|value| {
                if item.get("MHS av").is_some()
                    || item.get("MHS 1m").is_some()
                    || item.get("MHS 15m").is_some()
                {
                    mhs_to_ghs(value)
                } else {
                    ths_to_ghs(value)
                }
            })
            .unwrap_or(0.0);
        hashrate.current_ghs = hashrate.avg5s_ghs;
    }

    let mut thermal = ThermalStats::default();
    if let Some(item) = summary {
        thermal.inlet_c = json_f64(item, "Temperature")
            .or_else(|| json_f64(item, "Env Temp"))
            .or_else(|| json_f64(item, "environment-temperature"))
            .or_else(|| board_chip_temp(item, "chip-temp-avg", "Chip Temp Avg"));

        if let Some(board_temps) = item.get("board-temperature").and_then(|v| v.as_array()) {
            thermal.per_board_max_c = board_temps
                .iter()
                .filter_map(|value| {
                    value
                        .as_f64()
                        .or_else(|| value.as_str().and_then(|s| s.parse().ok()))
                })
                .collect();
        }
    }

    let mut fans = FanStats::default();
    if let Some(item) = summary {
        if let Some(fan_in) = json_u64(item, "Fan Speed In").or_else(|| json_u64(item, "fan-speed-in"))
        {
            fans.rpm.push(fan_in as u32);
        }
        if let Some(fan_out) = json_u64(item, "Fan Speed Out").or_else(|| json_u64(item, "fan-speed-out"))
        {
            fans.rpm.push(fan_out as u32);
        }
        for index in 1..=4 {
            let key = format!("Fan Speed{index}");
            if let Some(rpm) = json_u64(item, &key) {
                fans.rpm.push(rpm as u32);
            }
        }
        if fans.rpm.is_empty() {
            if let Some(fan) = json_u64(item, "Fan Speed") {
                fans.rpm.push(fan as u32);
            }
        }
    }

    let mut power = PowerStats::default();
    if let Some(item) = summary {
        power.watts = json_f64(item, "Power")
            .or_else(|| json_f64(item, "Power Real"))
            .or_else(|| json_f64(item, "power-realtime"))
            .or_else(|| json_f64(item, "power-5min"))
            .or_else(|| json_f64(item, "Power Rate"));
        power.voltage = json_f64(item, "Voltage");
    }

    let status = summary
        .and_then(|item| item.get("Status").and_then(|v| v.as_str()))
        .unwrap_or("Unknown")
        .to_string();

    let uptime_sec = summary.and_then(|item| {
        json_u64(item, "Uptime")
            .or_else(|| json_u64(item, "Elapsed"))
            .or_else(|| json_u64(item, "elapsed"))
    });

    let shares_accepted = summary.and_then(|item| json_u64(item, "Accepted"));
    let shares_rejected = summary.and_then(|item| json_u64(item, "Rejected"));
    let hw_errors = summary.and_then(|item| json_u64(item, "Hardware Errors"));

    let mut boards = parse_whatsminer_devs(edevs_raw, "edevs");
    if boards.is_empty() {
        boards = parse_whatsminer_devs(devs_raw, "DEVS");
    }
    if boards.is_empty() {
        boards = (0..thermal.per_board_max_c.len())
            .map(|index| BoardStats {
                label: format!("SM{index}"),
                hashrate_ghs: hashrate.per_board_ghs.get(index).copied(),
                temp_c: thermal.per_board_max_c.get(index).copied(),
                fan_rpm: fans.rpm.get(index).copied(),
                status: String::new(),
                chip_temp_min_c: None,
                chip_temp_avg_c: None,
                chip_temp_max_c: None,
                effective_chips: None,
                ..Default::default()
            })
            .collect();
    }

    let mut faults = parse_error_entries(error_codes_raw);
    faults.extend(parse_error_entries(device_info_raw));
    faults.sort_by(|a, b| a.code.cmp(&b.code));
    faults.dedup_by(|a, b| a.code == b.code);

    let mut board_chips = if board_chips.is_empty() && !btminer_log_raw.is_empty() {
        parse_btminer_log(btminer_log_raw)
    } else {
        board_chips
    };
    for board in &mut board_chips {
        board.chips_per_domain = layout::resolve_chips_per_domain(&model, board.chips.len());
    }

    if !board_chips.is_empty() {
        thermal.per_chip_c = board_chips
            .iter()
            .flat_map(|board| board.chips.iter().map(|chip| chip.temp_c))
            .collect();
    }

    let mut raw_log = summary_raw.to_string();
    if !pools_raw.is_empty() {
        raw_log.push_str("\n--- pools ---\n");
        raw_log.push_str(pools_raw);
    }
    if !devs_raw.is_empty() {
        raw_log.push_str("\n--- devs ---\n");
        raw_log.push_str(devs_raw);
    }
    if !edevs_raw.is_empty() {
        raw_log.push_str("\n--- edevs ---\n");
        raw_log.push_str(edevs_raw);
    }
    if !error_codes_raw.is_empty() {
        raw_log.push_str("\n--- errors ---\n");
        raw_log.push_str(error_codes_raw);
    }
    if !btminer_log_raw.is_empty() {
        raw_log.push_str("\n--- btminer log ---\n");
        raw_log.push_str(btminer_log_raw);
    }

    MinerSnapshot {
        identity: MinerIdentity {
            vendor: MinerVendor::Whatsminer,
            model,
            firmware,
            driver_id: "whatsminer".to_string(),
            ..Default::default()
        },
        hashrate,
        thermal,
        fans,
        power,
        boards,
        pools: parse_pools_json(pools_raw),
        shares_accepted,
        shares_rejected,
        hw_errors,
        board_chips,
        faults,
        raw_log,
        status,
        uptime_sec,
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_antminer_summary_as_whatsminer() {
        let sample = r#"{"STATUS":[{"Msg":"Summary","STATUS":"S"}],"SUMMARY":[{"GHS 5s":2863.31,"GHS av":3949.71}]}"#;
        assert!(classify_whatsminer(sample).is_none());
    }

    #[test]
    fn classifies_whatsminer_summary_json() {
        let sample = r#"{"STATUS":[{"Msg":"Summary","STATUS":"S"}],"SUMMARY":[{"Miner Type":"M50","MHS 5s":70668114.52}]}"#;
        let (vendor, model) = classify_whatsminer(sample).unwrap();
        assert_eq!(vendor, MinerVendor::Whatsminer);
        assert_eq!(model, "M50");
    }

    #[test]
    fn converts_mhs_to_ghs() {
        let sample = r#"{"SUMMARY":[{"Miner Type":"M50","MHS 5s":70668114.52,"MHS av":70000000.0,"Power":3500}]}"#;
        let snap = parse_whatsminer_snapshot(sample, "", "", "", "", "", "", Vec::new());
        assert!((snap.hashrate.avg5s_ghs - 70668.11452).abs() < 0.01);
        assert_eq!(snap.power.watts, Some(3500.0));
    }

    #[test]
    fn parses_edevs_and_faults() {
        let edevs = r#"{"msg":{"edevs":[{"slot":0,"hash-average":33.9,"chip-temp-min":84.9,"chip-temp-avg":92.2,"chip-temp-max":97.4,"effective-chips":70}]}}"#;
        let errors = r#"{"Msg":{"error_code":[{"530":"2024-01-01 12:00:00"}]}}"#;
        let snap = parse_whatsminer_snapshot("", "", "", edevs, errors, "", "", Vec::new());
        assert_eq!(snap.boards.len(), 1);
        assert_eq!(snap.faults.len(), 1);
        assert_eq!(snap.faults[0].code, "530");
    }
}
