use super::btminer_log::{parse_btminer_html, parse_btminer_log};
use crate::fetch_options::FetchOptions;
use crate::model::BoardChipMap;
use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, SET_COOKIE};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

const LUCI_SESSION_TTL: Duration = Duration::from_secs(600);
const HTTP_TIMEOUT: Duration = Duration::from_secs(4);
const LUCI_LOGIN_TIMEOUT: Duration = Duration::from_secs(3);

struct LuciSession {
    cookie: String,
    expires: Instant,
}

fn luci_sessions() -> &'static Mutex<HashMap<String, LuciSession>> {
    static SESSIONS: OnceLock<Mutex<HashMap<String, LuciSession>>> = OnceLock::new();
    SESSIONS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn cache_key(host: &str, base: &str) -> String {
    format!("{base}|{host}")
}

fn store_session(host: &str, base: &str, cookie: &str) {
    if let Ok(mut sessions) = luci_sessions().lock() {
        sessions.insert(
            cache_key(host, base),
            LuciSession {
                cookie: cookie.to_string(),
                expires: Instant::now() + LUCI_SESSION_TTL,
            },
        );
    }
}

fn cached_log(client: &Client, host: &str, base: &str) -> Option<String> {
    let key = cache_key(host, base);
    let cookie = {
        let mut sessions = luci_sessions().lock().ok()?;
        let session = sessions.get(&key)?;
        if session.expires <= Instant::now() {
            sessions.remove(&key);
            return None;
        }
        session.cookie.clone()
    };

    let url = format!("{base}/cgi-bin/luci/admin/status/btminerapi");
    let response = client
        .get(&url)
        .header("Cookie", cookie)
        .send()
        .ok()?;
    if !response.status().is_success() {
        luci_sessions().lock().ok()?.remove(&key);
        return None;
    }
    extract_chip_log(&response.text().ok()?)
}

pub fn fetch_btminer_chip_data(host: &str, options: &FetchOptions) -> (Vec<BoardChipMap>, String) {
    let client = match build_luci_client() {
        Ok(client) => client,
        Err(_) => return (Vec::new(), String::new()),
    };

    for scheme in ["https", "http"] {
        let base = format!("{scheme}://{host}");
        if let Some(log) = cached_log(&client, host, &base) {
            return (parse_btminer_log(&log), log);
        }
        if let Some(log) = fetch_log_anonymous(&client, &base) {
            return (parse_btminer_log(&log), log);
        }

        for (username, password) in options.luci_credential_pairs() {
            if let Some(log) = fetch_log_authenticated(&client, host, &base, &username, &password) {
                return (parse_btminer_log(&log), log);
            }
        }
    }

    (Vec::new(), String::new())
}

pub fn luci_reachable(host: &str) -> bool {
    let client = match build_luci_client() {
        Ok(client) => client,
        Err(_) => return false,
    };
    for scheme in ["https", "http"] {
        let base = format!("{scheme}://{host}");
        let url = format!("{base}/cgi-bin/luci");
        if client.get(&url).send().map(|r| r.status().is_success() || r.status().as_u16() == 302).unwrap_or(false) {
            return true;
        }
    }
    false
}

pub fn verify_luci_login(host: &str, username: &str, password: &str) -> bool {
    let client = match build_luci_client_with_timeout(LUCI_LOGIN_TIMEOUT) {
        Ok(client) => client,
        Err(_) => return false,
    };
    for scheme in ["https", "http"] {
        let base = format!("{scheme}://{host}");
        if luci_login_with_client(&client, &base, username, password).is_some() {
            return true;
        }
    }
    false
}

/// LuCI network status — `macaddr` from `/admin/network/iface_status/lan` (amazingFarm).
pub fn fetch_lan_macaddr(host: &str, username: &str, password: &str) -> Option<String> {
    use crate::drivers::whatsminer::access::normalize_mac;

    let client = build_luci_client_with_timeout(LUCI_LOGIN_TIMEOUT).ok()?;
    for scheme in ["https", "http"] {
        let base = format!("{scheme}://{host}");
        let cookie = luci_login_with_client(&client, &base, username, password)?;
        let url = format!("{base}/cgi-bin/luci/admin/network/iface_status/lan");
        let response = client.get(&url).header("Cookie", cookie).send().ok()?;
        if !response.status().is_success() {
            continue;
        }
        let body = response.text().ok()?;
        let value: Value = serde_json::from_str(&body).ok()?;
        let mac = value
            .as_array()
            .and_then(|items| items.first())
            .and_then(|item| item.get("macaddr"))
            .and_then(|v| v.as_str())
            .map(normalize_mac)?;
        return Some(mac);
    }
    None
}

pub fn test_luci_credentials(host: &str, username: &str, password: &str) -> bool {
    let client = match build_luci_client() {
        Ok(client) => client,
        Err(_) => return false,
    };
    for scheme in ["https", "http"] {
        let base = format!("{scheme}://{host}");
        let cookie = match luci_login(&client, &base, username, password) {
            Some(cookie) => cookie,
            None => continue,
        };
        let url = format!("{base}/cgi-bin/luci/admin/status/btminerapi");
        if let Ok(response) = client.get(&url).header("Cookie", cookie).send() {
            if response.status().is_success() {
                if let Ok(body) = response.text() {
                    return extract_chip_log(&body).is_some();
                }
            }
        }
    }
    false
}

/// Attempt to enable the WhatsMiner TCP API switch via LuCI web UI.
pub fn enable_api_switch_luci(host: &str, username: &str, password: &str) -> bool {
    let client = match build_luci_client() {
        Ok(client) => client,
        Err(_) => return false,
    };

    for scheme in ["https", "http"] {
        let base = format!("{scheme}://{host}");
        let cookie = match luci_login(&client, &base, username, password) {
            Some(cookie) => cookie,
            None => continue,
        };

        let endpoints = [
            (
                format!("{base}/cgi-bin/luci/admin/system/api"),
                vec![
                    ("cbi.submit", "1"),
                    ("cbid.system.apiswitch", "1"),
                    ("cbid.system.api_switch", "1"),
                    ("apiswitch", "1"),
                ],
            ),
            (
                format!("{base}/cgi-bin/luci/admin/network/api_access"),
                vec![("cbi.submit", "1"), ("api_switch", "1"), ("apiswitch", "1")],
            ),
            (
                format!("{base}/cgi-bin/luci/admin/status/btminerapi"),
                vec![("api_switch", "1"), ("apiswitch", "1")],
            ),
        ];

        for (url, fields) in endpoints {
            let response = client
                .post(&url)
                .header("Cookie", &cookie)
                .header("Referer", &url)
                .form(&fields)
                .send();
            if let Ok(resp) = response {
                if resp.status().is_success() || resp.status().as_u16() == 302 {
                    return true;
                }
            }
        }
    }

    false
}

fn build_luci_client() -> Result<Client, ()> {
    build_luci_client_with_timeout(HTTP_TIMEOUT)
}

fn build_luci_client_with_timeout(timeout: Duration) -> Result<Client, ()> {
    Client::builder()
        .danger_accept_invalid_certs(true)
        .timeout(timeout)
        .redirect(reqwest::redirect::Policy::limited(4))
        .build()
        .map_err(|_| ())
}

fn fetch_log_anonymous(client: &Client, base: &str) -> Option<String> {
    let url = format!("{base}/cgi-bin/luci/admin/status/btminerapi");
    let response = client.get(&url).send().ok()?;
    if !response.status().is_success() {
        return None;
    }
    extract_chip_log(&response.text().ok()?)
}

fn fetch_log_authenticated(
    client: &Client,
    host: &str,
    base: &str,
    username: &str,
    password: &str,
) -> Option<String> {
    let cookie = luci_login(client, base, username, password)?;
    store_session(host, base, &cookie);
    let url = format!("{base}/cgi-bin/luci/admin/status/btminerapi");
    let response = client
        .get(&url)
        .header("Cookie", cookie)
        .send()
        .ok()?;
    if !response.status().is_success() {
        return None;
    }
    extract_chip_log(&response.text().ok()?)
}

fn luci_login(_client: &Client, base: &str, username: &str, password: &str) -> Option<String> {
    let login_client = Client::builder()
        .danger_accept_invalid_certs(true)
        .timeout(HTTP_TIMEOUT)
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .ok()?;
    luci_login_with_client(&login_client, base, username, password)
}

fn luci_login_with_client(
    login_client: &Client,
    base: &str,
    username: &str,
    password: &str,
) -> Option<String> {
    let login_url = format!("{base}/cgi-bin/luci");
    let response = login_client
        .post(&login_url)
        .header("Referer", format!("{base}/cgi-bin/luci"))
        .form(&[("luci_username", username), ("luci_password", password)])
        .send()
        .ok()?;

    let status = response.status();
    if !status.is_success() && status.as_u16() != 302 {
        return None;
    }

    extract_sysauth_cookie(response.headers())
}

fn extract_sysauth_cookie(headers: &HeaderMap) -> Option<String> {
    for value in headers.get_all(SET_COOKIE) {
        let raw = value.to_str().ok()?;
        for part in raw.split(',') {
            let trimmed = part.trim();
            let name_value = trimmed.split(';').next()?;
            if name_value.starts_with("sysauth=") || name_value.starts_with("sysauth_http=") {
                return Some(name_value.to_string());
            }
        }
    }
    None
}

fn extract_chip_log(html: &str) -> Option<String> {
    let log = parse_btminer_html(html)?;
    if log.contains("slot:") && log.contains("temp:") {
        Some(log)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::header::HeaderValue;

    #[test]
    fn parses_sysauth_cookie() {
        let mut headers = HeaderMap::new();
        headers.insert(
            SET_COOKIE,
            HeaderValue::from_static("sysauth_http=deadbeef; path=/"),
        );
        assert_eq!(
            extract_sysauth_cookie(&headers).as_deref(),
            Some("sysauth_http=deadbeef")
        );
    }

    #[test]
    fn rejects_html_without_chip_dump() {
        assert!(extract_chip_log("<html><body>login</body></html>").is_none());
    }

    #[test]
    #[ignore = "requires WhatsMiner on local network"]
    fn fetches_chip_data_from_live_whatsminer() {
        let options = FetchOptions::default();
        let (boards, log) = fetch_btminer_chip_data("192.168.35.35", &options);
        assert!(!log.is_empty(), "expected btminer log text");
        assert!(!boards.is_empty(), "expected parsed chip boards");
        assert!(boards.iter().any(|board| !board.chips.is_empty()));
    }
}
