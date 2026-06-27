use super::btminer_log::{parse_btminer_html, parse_btminer_log};
use crate::fetch_options::FetchOptions;
use crate::model::BoardChipMap;
use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, SET_COOKIE};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

const LUCI_SESSION_TTL: Duration = Duration::from_secs(600);
const HTTP_TIMEOUT: Duration = Duration::from_secs(4);

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
    let client = match Client::builder()
        .danger_accept_invalid_certs(true)
        .timeout(HTTP_TIMEOUT)
        .redirect(reqwest::redirect::Policy::limited(4))
        .build()
    {
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
    let cookie = luci_login(base, username, password)?;
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

fn luci_login(base: &str, username: &str, password: &str) -> Option<String> {
    let login_client = Client::builder()
        .danger_accept_invalid_certs(true)
        .timeout(HTTP_TIMEOUT)
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .ok()?;

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
