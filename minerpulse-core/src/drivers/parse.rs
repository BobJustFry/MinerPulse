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
