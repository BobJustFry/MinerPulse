use crate::drivers::parse::normalize_mac_address;
use reqwest::blocking::Client;
use serde_json::Value;
use std::time::Duration;

const HTTP_TIMEOUT: Duration = Duration::from_secs(4);

/// Avalon-only: `get_minerinfo.cgi` JSONP → `mac`.
pub fn fetch_mac_address(host: &str) -> Option<String> {
    let client = build_http_client()?;
    let url = format!("http://{host}/get_minerinfo.cgi");
    let body = client.get(&url).send().ok()?.text().ok()?;
    parse_mac_body(&body)
}

fn parse_mac_body(body: &str) -> Option<String> {
    let trimmed = body.trim();
    let json = trimmed
        .strip_prefix("minerinfoCallback(")
        .and_then(|inner| inner.strip_suffix(");"))
        .or_else(|| trimmed.strip_suffix(");"))
        .unwrap_or(trimmed);
    let value: Value = serde_json::from_str(json).ok()?;
    value
        .get("mac")
        .and_then(|v| v.as_str())
        .map(normalize_mac_address)
}

fn build_http_client() -> Option<Client> {
    Client::builder()
        .timeout(HTTP_TIMEOUT)
        .danger_accept_invalid_certs(true)
        .redirect(reqwest::redirect::Policy::limited(4))
        .build()
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_minerinfo_callback_mac() {
        let body = r#"minerinfoCallback({"mac":"11:22:33:44:55:66","hwtype":"AvalonMiner 1346"});"#;
        assert_eq!(
            parse_mac_body(body).as_deref(),
            Some("11:22:33:44:55:66")
        );
    }
}
