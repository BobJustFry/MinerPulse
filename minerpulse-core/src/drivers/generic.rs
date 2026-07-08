//! Generic cgminer/BMMiner fallback driver.
//!
//! Handles miners that speak the cgminer JSON API on 4028 but are not matched by
//! a vendor-specific driver (LuxOS, BraiinsOS, other cgminer forks). Read-only:
//! extracts identity, hashrate, pools, temps and status from standard fields.
//! Vendor-neutral — no per-vendor branches (see driver isolation rules).

use super::json_util::{
    array_items, collect_fan_rpms, collect_temp_boards, first_in_array, json_f64, json_str,
    json_u64, merge_stats_objects,
};
use super::parse::derive_run_status;
use crate::model::{
    FanStats, HashrateStats, MinerIdentity, MinerSnapshot, MinerVendor, PoolInfo, ThermalStats,
};
use serde_json::Value;

const DRIVER_ID: &str = "cgminer";

fn parse_doc(raw: &str) -> Option<Value> {
    let trimmed = raw.trim();
    if trimmed.starts_with('{') {
        serde_json::from_str(trimmed).ok()
    } else {
        None
    }
}

/// GHS-family fields are already GH/s; MHS-family need /1000.
fn hashrate_from(summary: &Value) -> HashrateStats {
    let mut hr = HashrateStats::default();
    let ghs_5s = json_f64(summary, "GHS 5s");
    let ghs_av = json_f64(summary, "GHS av").or_else(|| json_f64(summary, "GHS 30m"));
    let mhs_5s = json_f64(summary, "MHS 5s").map(|v| v / 1000.0);
    let mhs_av = json_f64(summary, "MHS av").map(|v| v / 1000.0);
    hr.avg5s_ghs = ghs_5s.or(mhs_5s).unwrap_or(0.0);
    hr.avg_ghs = ghs_av.or(mhs_av).unwrap_or(0.0);
    hr.current_ghs = hr.avg5s_ghs;
    hr
}

fn model_from(model_hint: Option<&str>, stats: Option<&Value>, summary: Option<&Value>) -> String {
    if let Some(hint) = model_hint {
        if !hint.trim().is_empty() {
            return hint.trim().to_string();
        }
    }
    let from_stats = stats
        .and_then(|s| json_str(s, "Type").or_else(|| json_str(s, "Model")))
        .map(str::to_string);
    let from_summary = summary
        .and_then(|s| json_str(s, "Type"))
        .map(str::to_string);
    from_stats
        .or(from_summary)
        .filter(|m| !m.is_empty())
        .unwrap_or_else(|| "CGMiner".to_string())
}

fn pools_from(pools_raw: &str) -> Vec<PoolInfo> {
    let Some(value) = parse_doc(pools_raw) else {
        return Vec::new();
    };
    let Some(items) = array_items(&value, "POOLS") else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|pool| {
            Some(PoolInfo {
                url: json_str(pool, "URL")?.to_string(),
                worker: json_str(pool, "User").unwrap_or("").to_string(),
                status: json_str(pool, "Status").unwrap_or("Unknown").to_string(),
                accepted: json_u64(pool, "Accepted").unwrap_or(0),
                rejected: json_u64(pool, "Rejected").unwrap_or(0),
                ..Default::default()
            })
        })
        .collect()
}

/// Build a best-effort snapshot from cgminer responses. `model_hint` may come
/// from a `devdetails`/`version` probe done by the registry.
pub fn parse_generic_cgminer_snapshot(
    model_hint: Option<&str>,
    stats_raw: &str,
    summary_raw: &str,
    pools_raw: &str,
) -> MinerSnapshot {
    let stats_doc = parse_doc(stats_raw);
    let stats = stats_doc.as_ref().and_then(merge_stats_objects);
    let summary_doc = parse_doc(summary_raw);
    let summary = summary_doc.as_ref().and_then(|d| first_in_array(d, "SUMMARY").cloned());

    let model = model_from(model_hint, stats.as_ref(), summary.as_ref());

    let hashrate = summary
        .as_ref()
        .map(hashrate_from)
        .unwrap_or_default();

    let uptime_sec = summary
        .as_ref()
        .and_then(|s| json_u64(s, "Elapsed"))
        .or_else(|| stats.as_ref().and_then(|s| json_u64(s, "Elapsed")));

    let shares_accepted = summary.as_ref().and_then(|s| json_u64(s, "Accepted"));
    let shares_rejected = summary.as_ref().and_then(|s| json_u64(s, "Rejected"));
    let hw_errors = summary.as_ref().and_then(|s| json_u64(s, "Hardware Errors"));

    let mut thermal = ThermalStats::default();
    let mut fans = FanStats::default();
    if let Some(stats) = &stats {
        thermal.per_board_max_c = collect_temp_boards(stats);
        fans.rpm = collect_fan_rpms(stats);
    }

    let has_hash = hashrate.avg5s_ghs > 0.0 || hashrate.avg_ghs > 0.0;
    let has_telemetry = has_hash || uptime_sec.unwrap_or(0) > 0 || !thermal.per_board_max_c.is_empty();
    let status = derive_run_status(has_hash, has_telemetry).to_string();

    let mut raw_log = String::new();
    for (label, raw) in [
        ("summary", summary_raw),
        ("stats", stats_raw),
        ("pools", pools_raw),
    ] {
        if !raw.trim().is_empty() {
            raw_log.push_str(&format!("--- {label} ---\n{raw}\n"));
        }
    }

    MinerSnapshot {
        identity: MinerIdentity {
            vendor: MinerVendor::Generic,
            model,
            firmware: String::new(),
            driver_id: DRIVER_ID.to_string(),
            ..Default::default()
        },
        hashrate,
        thermal,
        fans,
        pools: pools_from(pools_raw),
        shares_accepted,
        shares_rejected,
        hw_errors,
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
    fn parses_luxos_style_summary() {
        let summary = r#"{"STATUS":[{"STATUS":"S"}],"SUMMARY":[{"Elapsed":2330,"GHS 5s":13084.18,"GHS av":13701.74,"Accepted":179,"Rejected":0,"Hardware Errors":19}]}"#;
        let stats = r#"{"STATS":[{"Type":"Antminer S9i"},{"temp1":56,"fan1":3200,"Elapsed":2330}]}"#;
        let snap = parse_generic_cgminer_snapshot(None, stats, summary, "");
        assert_eq!(snap.identity.vendor, MinerVendor::Generic);
        assert_eq!(snap.identity.driver_id, "cgminer");
        assert_eq!(snap.identity.model, "Antminer S9i");
        assert!((snap.hashrate.avg5s_ghs - 13084.18).abs() < 0.01);
        assert_eq!(snap.status, "mining");
        assert_eq!(snap.uptime_sec, Some(2330));
    }

    #[test]
    fn model_hint_takes_priority() {
        let summary = r#"{"SUMMARY":[{"GHS 5s":0,"GHS av":0}]}"#;
        let snap = parse_generic_cgminer_snapshot(Some("LuxOS T21"), "", summary, "");
        assert_eq!(snap.identity.model, "LuxOS T21");
        assert_eq!(snap.status, "offline");
    }

    #[test]
    fn parses_pools() {
        let pools = r#"{"POOLS":[{"URL":"stratum+tcp://pool:3333","User":"worker","Status":"Alive","Accepted":10,"Rejected":1}]}"#;
        let snap = parse_generic_cgminer_snapshot(Some("X"), "", "", pools);
        assert_eq!(snap.pools.len(), 1);
        assert_eq!(snap.pools[0].status, "Alive");
        assert_eq!(snap.pools[0].accepted, 10);
    }
}
