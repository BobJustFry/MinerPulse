use crate::drivers::parse::normalize_mac_address;
use digest_auth::{parse, AuthContext};
use reqwest::blocking::Client;
use reqwest::header::{AUTHORIZATION, WWW_AUTHENTICATE};
use serde_json::Value;
use std::time::Duration;

const HTTP_TIMEOUT: Duration = Duration::from_secs(4);

/// Antminer-only: HTTP Digest `get_system_info.cgi` → `macaddr`.
pub fn fetch_mac_address(host: &str) -> Option<String> {
    for (user, pass) in [("root", "root"), ("admin", "root")] {
        if let Some(mac) = fetch_mac_with_creds(host, user, pass) {
            return Some(mac);
        }
    }
    None
}

fn fetch_mac_with_creds(host: &str, username: &str, password: &str) -> Option<String> {
    let client = build_http_client()?;
    let path = "/cgi-bin/get_system_info.cgi";
    let url = format!("http://{host}{path}");
    let response = client.post(&url).send().ok()?;
    let www = response
        .headers()
        .get(WWW_AUTHENTICATE)
        .and_then(|value| value.to_str().ok())?;
    let mut challenge = parse(www).ok()?;
    let auth = AuthContext::new_post(username, password, path, None::<&[u8]>);
    let authorization = challenge.respond(&auth).ok()?.to_header_string();
    let body = client
        .post(&url)
        .header(AUTHORIZATION, authorization)
        .send()
        .ok()?
        .text()
        .ok()?;
    parse_mac_body(&body)
}

fn parse_mac_body(body: &str) -> Option<String> {
    let value: Value = serde_json::from_str(body).ok()?;
    value
        .get("macaddr")
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
    fn parses_system_info_mac() {
        let body = r#"{"macaddr":"aa:bb:cc:dd:ee:ff","minertype":"Antminer L7"}"#;
        assert_eq!(
            parse_mac_body(body).as_deref(),
            Some("AA:BB:CC:DD:EE:FF")
        );
    }
}
