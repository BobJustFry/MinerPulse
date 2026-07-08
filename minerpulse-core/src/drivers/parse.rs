/// Parse CGMiner/Avalon pipe-delimited response fields.
pub fn get_parameter(text: &str, key: &str) -> Option<String> {
    let idx = text.find(key)?;
    let rest = &text[idx + key.len()..];
    let value = if let Some(stripped) = rest.strip_prefix('=') {
        stripped
    } else if key.ends_with('[') {
        rest
    } else {
        return None;
    };

    let end = value
        .find([',', '|', '\t', ']', '\n', '\r'])
        .unwrap_or(value.len());
    let trimmed = value[..end].trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

pub fn get_parameter_bracket(text: &str, key: &str) -> Option<String> {
    let idx = text.find(key)?;
    let rest = &text[idx + key.len()..];
    let end = rest.find(']')?;
    let value = rest[..end].trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

pub fn parse_f64(value: &str) -> Option<f64> {
    value.trim().parse().ok()
}

pub fn parse_u64(value: &str) -> Option<u64> {
    value.trim().parse().ok()
}

pub fn parse_i32(value: &str) -> Option<i32> {
    value.trim().parse().ok()
}

/// Vendor-neutral MAC string normalization (uppercase, colon-separated).
pub fn normalize_mac_address(raw: &str) -> String {
    raw.trim()
        .to_uppercase()
        .replace('-', ":")
}

/// True when a parsed status is missing or the placeholder "Unknown".
pub fn status_is_unknown(status: &str) -> bool {
    status.trim().is_empty() || status.trim().eq_ignore_ascii_case("unknown")
}

/// Canonical run-status token derived from telemetry when the miner reports none.
/// cgminer/BMMiner `summary` has no top-level status field, so we infer it.
pub fn derive_run_status(has_hashrate: bool, has_telemetry: bool) -> &'static str {
    if has_hashrate {
        "mining"
    } else if has_telemetry {
        "idle"
    } else {
        "offline"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ver_field() {
        let sample = "STATUS=S,When=1,Code=22,Msg=CGMiner stats,Ver=1346|Temp=32";
        assert_eq!(get_parameter(sample, "Ver").as_deref(), Some("1346"));
        assert_eq!(get_parameter(sample, "Temp").as_deref(), Some("32"));
    }
}
