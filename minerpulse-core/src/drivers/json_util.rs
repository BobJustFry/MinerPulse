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
