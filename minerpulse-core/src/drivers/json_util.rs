use super::parse::parse_f64;
use serde_json::Value;

pub fn json_f64(obj: &Value, key: &str) -> Option<f64> {
    obj.get(key).and_then(|v| {
        v.as_f64()
            .or_else(|| v.as_str().and_then(parse_f64))
            .or_else(|| v.as_i64().map(|n| n as f64))
    })
}

pub fn json_u64(obj: &Value, key: &str) -> Option<u64> {
    obj.get(key).and_then(|v| {
        v.as_u64()
            .or_else(|| v.as_i64().map(|n| n.max(0) as u64))
            .or_else(|| v.as_str().and_then(|s| s.trim().parse().ok()))
    })
}

pub fn json_str<'a>(obj: &'a Value, key: &str) -> Option<&'a str> {
    obj.get(key).and_then(|v| v.as_str())
}

pub fn first_in_array<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
    value.get(key)?.as_array()?.first()
}

pub fn array_items<'a>(value: &'a Value, key: &str) -> Option<&'a Vec<Value>> {
    value.get(key)?.as_array()
}

/// CGMiner JSON often puts metadata in STATS[0] and live metrics in STATS[1+].
pub fn merge_stats_objects(value: &Value) -> Option<Value> {
    let items = array_items(value, "STATS")?;
    let mut merged = serde_json::Map::new();

    for item in items {
        let Some(obj) = item.as_object() else {
            continue;
        };
        for (key, val) in obj {
            if val.is_null() {
                continue;
            }
            merged.insert(key.clone(), val.clone());
        }
    }

    if merged.is_empty() {
        None
    } else {
        Some(Value::Object(merged))
    }
}

pub fn json_hashrate_ghs(obj: &Value, keys: &[&str]) -> Option<f64> {
    for key in keys {
        let Some(raw) = json_f64(obj, key) else {
            continue;
        };
        if key.contains("MHS") || key.contains("mhs") {
            return Some(raw / 1000.0);
        }
        if (key.contains("HS") || key.contains("hs")) && raw > 1_000_000.0 {
            return Some(raw / 1_000_000.0);
        }
        return Some(raw);
    }
    None
}

pub fn json_temp_c(obj: &Value) -> Option<f64> {
    const KEYS: [&str; 6] = [
        "Temperature",
        "temp_avg",
        "temp1",
        "Temp",
        "Chip Temp Avg",
        "Env Temp",
    ];
    for key in KEYS {
        if let Some(value) = json_f64(obj, key) {
            if value > 0.0 {
                return Some(value);
            }
        }
    }
    None
}

pub fn collect_temp_boards(obj: &Value) -> Vec<f64> {
    let mut temps = Vec::new();
    for index in 1..=16 {
        for key in [
            format!("temp{index}"),
            format!("temp2_{index}"),
            format!("Temperature{index}"),
        ] {
            if let Some(value) = json_f64(obj, &key) {
                if value > 0.0 {
                    temps.push(value);
                    break;
                }
            }
        }
    }
    temps
}

pub fn collect_fan_rpms(obj: &Value) -> Vec<u32> {
    let mut fans = Vec::new();
    for key in ["Fan Speed In", "Fan Speed Out", "Fan Speed", "Fan1", "Fan2", "Fan3", "Fan4"] {
        if let Some(rpm) = json_u64(obj, key) {
            fans.push(rpm as u32);
        }
    }
    for index in 1..=8 {
        for key in [format!("Fan{index}"), format!("fan{index}")] {
            if let Some(rpm) = json_u64(obj, &key) {
                fans.push(rpm as u32);
                break;
            }
        }
    }
    fans
}
