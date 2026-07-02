use super::avalon::parse_avalon_estats_log;
use super::json_util::{json_f64, json_u64};
use crate::model::MinerSnapshot;
use serde_json::Value;

pub fn is_avalon_cgminer_dump(raw: &str) -> bool {
    let has_avalon = raw.contains("AVA100")
        || raw.contains("'ID':'AVA")
        || raw.contains("\"ID\":\"AVA")
        || raw.contains("'ID': 'AVA")
        || raw.contains("\"ID\": \"AVA");

    let has_payload = raw.contains("MM ID0")
        || raw.contains("GHSmm[")
        || raw.contains("GHSspd[")
        || raw.contains("Ver[1326")
        || raw.contains("Ver[1346")
        || raw.contains("Ver[1466");

    has_avalon && has_payload
}

fn normalize_python_dict(line: &str) -> String {
    line.replace('\'', "\"")
        .replace("True", "true")
        .replace("False", "false")
        .replace("None", "null")
}

fn merge_summary_object(summary: &Value) -> Option<Value> {
    let items = summary.get("SUMMARY")?.as_array()?;
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

fn extract_mm_payload(line: &Value) -> Option<String> {
    let items = line.get("STATS")?.as_array()?;
    for item in items {
        if let Some(mm) = item.get("MM ID0").and_then(|v| v.as_str()) {
            if !mm.is_empty() {
                return Some(mm.to_string());
            }
        }
    }
    None
}

fn enrich_from_cgminer_summary(snapshot: &mut MinerSnapshot, summary: &Value) {
    if let Some(mhs) = json_f64(summary, "MHS 30s").or_else(|| json_f64(summary, "MHS 5s")) {
        snapshot.hashrate.avg5s_ghs = mhs / 1000.0;
        snapshot.hashrate.current_ghs = snapshot.hashrate.avg5s_ghs;
    }
    if let Some(mhs) = json_f64(summary, "MHS av") {
        snapshot.hashrate.avg_ghs = mhs / 1000.0;
    }
    if snapshot.uptime_sec.is_none() {
        snapshot.uptime_sec = json_u64(summary, "Elapsed");
    }
    if snapshot.shares_accepted.is_none() {
        snapshot.shares_accepted = json_u64(summary, "Accepted");
    }
    if snapshot.shares_rejected.is_none() {
        snapshot.shares_rejected = json_u64(summary, "Rejected");
    }
    if snapshot.hw_errors.is_none() {
        snapshot.hw_errors = json_u64(summary, "Hardware Errors");
    }
}

pub fn parse_avalon_cgminer_dump(raw: &str) -> Option<MinerSnapshot> {
    let mut mm_payload = String::new();
    let mut summary_value = None;

    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() || !line.starts_with('{') {
            continue;
        }

        let json_line = if line.contains('\'') {
            normalize_python_dict(line)
        } else {
            line.to_string()
        };

        let value: Value = serde_json::from_str(&json_line).ok()?;

        if value.get("SUMMARY").is_some() {
            summary_value = merge_summary_object(&value);
        }
        if let Some(mm) = extract_mm_payload(&value) {
            mm_payload = mm;
        }
    }

    if mm_payload.is_empty() {
        return None;
    }

    let mut snapshot = parse_avalon_estats_log(&format!("MM ID0={mm_payload}"));
    if let Some(summary) = summary_value {
        enrich_from_cgminer_summary(&mut snapshot, &summary);
    }
    snapshot.raw_log = raw.to_string();
    Some(snapshot)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    #[test]
    fn detects_avalon_cgminer_dump() {
        let sample = r#"{'SUMMARY':[{'MHS av':98896251.30}]}
{'STATS':[{'ID':'AVA100','MM ID0':'Ver[1326-test] GHSspd[1000.0] MGHS[100 200] PVT_T0[1 2 3]'}]}"#;
        assert!(is_avalon_cgminer_dump(sample));
    }

    #[test]
    fn parses_embedded_1326_sample() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../OldProject/txt/1326.txt");
        if !path.exists() {
            return;
        }
        let raw = fs::read_to_string(path).expect("read 1326 sample");
        let snapshot = parse_avalon_cgminer_dump(&raw).expect("parse 1326");
        assert!(snapshot.identity.model.contains("1326"));
        assert_eq!(snapshot.identity.core_chip.as_deref(), Some("A3200C"));
        assert_eq!(snapshot.boards.len(), 3);
        assert_eq!(snapshot.board_chips.len(), 3);
        assert_eq!(snapshot.board_chips[0].matrix_id.as_deref(), Some("Matrix_1326"));
        assert!(snapshot.hashrate.avg_ghs > 90_000.0);
        assert_eq!(snapshot.board_chips[0].chips.len(), 114);
    }

    #[test]
    fn parses_embedded_1346_sample() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../OldProject/txt/1346.txt");
        if !path.exists() {
            return;
        }
        let raw = fs::read_to_string(path).expect("read 1346 sample");
        let snapshot = parse_avalon_cgminer_dump(&raw).expect("parse 1346");
        assert!(snapshot.identity.model.contains("1346"));
        assert_eq!(snapshot.board_chips.len(), 3);
        assert_eq!(snapshot.board_chips[0].matrix_id.as_deref(), Some("Matrix_1346"));
        assert!(snapshot.hashrate.avg_ghs > 100_000.0);
        assert_eq!(snapshot.board_chips[0].chips.len(), 120);
    }
}
