use crate::model::MinerFault;
use serde_json::Value;

pub fn parse_error_entries(raw: &str) -> Vec<MinerFault> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }

    let Ok(value) = serde_json::from_str::<Value>(trimmed) else {
        return Vec::new();
    };

    let mut faults = Vec::new();
    collect_from_value(&value, &mut faults);
    faults.sort_by(|a, b| a.code.cmp(&b.code));
    faults.dedup_by(|a, b| a.code == b.code);
    faults
}

fn collect_from_value(value: &Value, faults: &mut Vec<MinerFault>) {
    if let Some(items) = value.get("error-code").or_else(|| value.get("error_code")) {
        collect_error_array(items, faults);
    }

    if let Some(msg) = value.get("Msg") {
        if let Some(items) = msg.get("error_code").or_else(|| msg.get("error-code")) {
            collect_error_array(items, faults);
        }
        if let Some(obj) = msg.as_object() {
            for (key, val) in obj {
                if key.contains("error") {
                    collect_error_array(val, faults);
                }
            }
        }
    }

    if let Some(msg) = value.get("msg") {
        if let Some(items) = msg.get("error-code").or_else(|| msg.get("error_code")) {
            collect_error_array(items, faults);
        }
    }
}

fn collect_error_array(value: &Value, faults: &mut Vec<MinerFault>) {
    match value {
        Value::Array(items) => {
            for item in items {
                collect_error_object(item, faults);
            }
        }
        Value::Object(map) => {
            for (code, occurred_at) in map {
                faults.push(MinerFault {
                    code: code.clone(),
                    occurred_at: value_to_time(occurred_at),
                });
            }
        }
        _ => {}
    }
}

fn collect_error_object(value: &Value, faults: &mut Vec<MinerFault>) {
    match value {
        Value::Object(map) => {
            for (code, occurred_at) in map {
                faults.push(MinerFault {
                    code: code.clone(),
                    occurred_at: value_to_time(occurred_at),
                });
            }
        }
        Value::String(code) => {
            faults.push(MinerFault {
                code: code.clone(),
                occurred_at: None,
            });
        }
        _ => {}
    }
}

fn value_to_time(value: &Value) -> Option<String> {
    value.as_str().map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_v2_error_code_response() {
        let raw = r#"{"STATUS":[{"STATUS":"S"}],"Msg":{"error_code":[{"329":"2022-01-17 11:28:11"},{"530":"2022-01-17 11:30:00"}]}}"#;
        let faults = parse_error_entries(raw);
        assert_eq!(faults.len(), 2);
        assert_eq!(faults[0].code, "329");
    }

    #[test]
    fn parses_v3_device_info_errors() {
        let raw = r#"{"code":0,"msg":{"error-code":[{"300":"1970-01-02 02:00:06"},{"301":"1970-01-02 02:00:06"}]}}"#;
        let faults = parse_error_entries(raw);
        assert_eq!(faults.len(), 2);
        assert!(faults.iter().any(|f| f.code == "301"));
    }
}
