use super::json_util::{array_items, json_f64, json_str, json_u64};
use super::MinerDriver;
use crate::error::MinerPulseError;
use crate::model::{
    BoardStats, FanStats, HashrateStats, MinerIdentity, MinerSnapshot, MinerVendor, PoolInfo,
    PowerStats, ThermalStats,
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
        let devs = client
            .send_payload(host, port, r#"{"cmd":"devs"}"#)
            .unwrap_or_default();

        Ok(parse_whatsminer_snapshot(&summary, &pools, &devs))
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

fn parse_whatsminer_devs(raw: &str) -> Vec<BoardStats> {
    let trimmed = raw.trim();
    if !trimmed.starts_with('{') {
        return Vec::new();
    }

    let value: Value = match serde_json::from_str(trimmed) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };

    let Some(devs) = array_items(&value, "DEVS") else {
        return Vec::new();
    };

    devs
        .iter()
        .enumerate()
        .map(|(index, dev)| {
            let label = json_str(dev, "Name")
                .or_else(|| json_str(dev, "ID"))
                .unwrap_or("Board")
                .to_string();

            BoardStats {
                label: if label == "Board" {
                    format!("Board {}", index + 1)
                } else {
                    label
                },
                hashrate_ghs: json_f64(dev, "MHS 5s")
                    .or_else(|| json_f64(dev, "MHS av"))
                    .map(mhs_to_ghs)
                    .or_else(|| json_f64(dev, "GHS 5s")),
                temp_c: json_f64(dev, "Temperature")
                    .or_else(|| json_f64(dev, "Chip Temp Avg")),
                fan_rpm: json_u64(dev, "Fan Speed").map(|rpm| rpm as u32),
                status: json_str(dev, "Status").unwrap_or("").to_string(),
            }
        })
        .collect()
}

pub fn parse_whatsminer_snapshot(summary_raw: &str, pools_raw: &str, devs_raw: &str) -> MinerSnapshot {
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
        .and_then(|item| {
            item.get("Firmware Version")
                .or_else(|| item.get("Btminer Version"))
                .and_then(|v| v.as_str())
        })
        .unwrap_or("")
        .to_string();

    let mut hashrate = HashrateStats::default();
    if let Some(item) = summary {
        hashrate.avg5s_ghs = json_f64(item, "MHS 5s").map(mhs_to_ghs).unwrap_or(0.0);
        hashrate.avg_ghs = json_f64(item, "MHS av")
            .or_else(|| json_f64(item, "MHS 1m"))
            .or_else(|| json_f64(item, "MHS 15m"))
            .map(mhs_to_ghs)
            .unwrap_or(0.0);
        hashrate.current_ghs = hashrate.avg5s_ghs;
    }

    let mut thermal = ThermalStats::default();
    if let Some(item) = summary {
        thermal.inlet_c = json_f64(item, "Temperature")
            .or_else(|| json_f64(item, "Env Temp"))
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
            .or_else(|| json_f64(item, "Power Rate"));
        power.voltage = json_f64(item, "Voltage");
    }

    let status = summary
        .and_then(|item| item.get("Status").and_then(|v| v.as_str()))
        .unwrap_or("Unknown")
        .to_string();

    let uptime_sec = summary.and_then(|item| {
        json_u64(item, "Uptime").or_else(|| json_u64(item, "Elapsed"))
    });

    let shares_accepted = summary.and_then(|item| json_u64(item, "Accepted"));
    let shares_rejected = summary.and_then(|item| json_u64(item, "Rejected"));
    let hw_errors = summary.and_then(|item| json_u64(item, "Hardware Errors"));

    let mut boards = parse_whatsminer_devs(devs_raw);
    if boards.is_empty() {
        boards = (0..hashrate.per_board_ghs.len())
            .map(|index| BoardStats {
                label: format!("Board {}", index + 1),
                hashrate_ghs: hashrate.per_board_ghs.get(index).copied(),
                temp_c: thermal.per_board_max_c.get(index).copied(),
                fan_rpm: fans.rpm.get(index).copied(),
                status: String::new(),
            })
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
        boards,
        pools: parse_pools_json(pools_raw),
        shares_accepted,
        shares_rejected,
        hw_errors,
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
        let sample = r#"{"SUMMARY":[{"Miner Type":"M50","MHS 5s":70668114.52,"MHS av":70000000.0,"Power":3500}]}"#;
        let snap = parse_whatsminer_snapshot(sample, "", "");
        assert!((snap.hashrate.avg5s_ghs - 70668.11452).abs() < 0.01);
        assert_eq!(snap.power.watts, Some(3500.0));
    }
}
