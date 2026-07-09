use super::btminer_log::{parse_btminer_html, parse_btminer_log};
use super::options::WhatsminerFetchOptions;
use crate::model::BoardChipMap;
use crate::trace;
use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, SET_COOKIE};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

const LUCI_SESSION_TTL: Duration = Duration::from_secs(600);
const HTTP_TIMEOUT: Duration = Duration::from_secs(4);
const LUCI_FAST_TIMEOUT: Duration = Duration::from_secs(2);
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
    extract_btminer_syslog(&response.text().ok()?)
}

pub fn fetch_btminer_chip_data(host: &str, options: &WhatsminerFetchOptions) -> (Vec<BoardChipMap>, String) {
    if options.is_cancelled() {
        return (Vec::new(), String::new());
    }

    let full_luci = options.fetch_chips || !options.fast_poll;
    let timeout = if full_luci {
        HTTP_TIMEOUT
    } else {
        LUCI_FAST_TIMEOUT
    };
    let client = match build_luci_client_with_timeout(timeout) {
        Ok(client) => client,
        Err(_) => return (Vec::new(), String::new()),
    };

    let schemes: &[&str] = if full_luci {
        &["https", "http"]
    } else {
        &["https"]
    };

    let cred_count = options.luci_credential_pairs().len();
    trace(
        "luci",
        "chips_begin",
        &format!("{host} fast={} creds={cred_count}", options.fast_poll),
    );

    for scheme in schemes {
        let base = format!("{scheme}://{host}");
        if let Some(log) = cached_log(&client, host, &base) {
            trace("luci", "chips_cached_ok", &base);
            return (parse_btminer_log(&log), log);
        }
        if full_luci {
            if let Some(log) = fetch_log_anonymous(&client, &base) {
                trace("luci", "chips_anon_ok", &base);
                return (parse_btminer_log(&log), log);
            }
        }

        for (username, password) in options.luci_credential_pairs() {
            if options.is_cancelled() {
                return (Vec::new(), String::new());
            }
            if let Some(log) = fetch_log_authenticated(&client, host, &base, &username, &password) {
                trace(
                    "luci",
                    "chips_auth_ok",
                    &format!("{base} user={username}"),
                );
                return (parse_btminer_log(&log), log);
            }
        }
    }

    trace("luci", "chips_empty", host);
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
                    return extract_btminer_syslog(&body).is_some();
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

/// Change super password via LuCI web UI (fallback when API 4433 blocks default password).
pub fn change_super_password_luci(
    host: &str,
    username: &str,
    _old_password: &str,
    new_password: &str,
) -> bool {
    let client = match build_luci_client() {
        Ok(client) => client,
        Err(_) => return false,
    };

    for scheme in ["https", "http"] {
        let base = format!("{scheme}://{host}");
        let cookie = match luci_login(&client, &base, username, _old_password) {
            Some(cookie) => cookie,
            None => continue,
        };

        let endpoints = [
            format!("{base}/cgi-bin/luci/admin/system/passwd"),
            format!("{base}/cgi-bin/luci/admin/system/admin"),
        ];
        let field_sets: [&[(&str, &str)]; 2] = [
            &[
                ("cbi.submit", "1"),
                ("pwd1", new_password),
                ("pwd2", new_password),
            ],
            &[
                ("cbi.submit", "1"),
                ("password", new_password),
                ("password_confirm", new_password),
            ],
        ];

        for url in endpoints {
            for fields in field_sets {
                let response = client
                    .post(&url)
                    .header("Cookie", &cookie)
                    .header("Referer", &url)
                    .form(fields)
                    .send();
                if let Ok(resp) = response {
                    let code = resp.status().as_u16();
                    if code == 200 || code == 302 {
                        return verify_luci_login(host, username, new_password);
                    }
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
    extract_btminer_syslog(&response.text().ok()?)
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
    let response = match client.get(&url).header("Cookie", cookie).send() {
        Ok(resp) => resp,
        Err(err) => {
            trace(
                "luci",
                "chips_get_err",
                &format!("{base} timeout_or_conn err={err}"),
            );
            return None;
        }
    };
    let status = response.status().as_u16();
    if !response.status().is_success() {
        trace("luci", "chips_get_status", &format!("{base} code={status}"));
        return None;
    }
    let body = response.text().ok()?;
    let log = extract_btminer_syslog(&body);
    trace(
        "luci",
        "chips_get_ok",
        &format!("{base} code={status} parsed={}", log.is_some()),
    );
    log
}

fn extract_luci_form_token(html: &str) -> Option<String> {
    for pattern in [
        r#"name="token" value=""#,
        r#"name='token' value='"#,
        r#"name="luci_token" value=""#,
    ] {
        let start = html.find(pattern)? + pattern.len();
        let rest = &html[start..];
        let end = rest.find('"').or_else(|| rest.find('\''))?;
        let token = rest[..end].trim();
        if !token.is_empty() {
            return Some(token.to_string());
        }
    }
    None
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
    let token = login_client
        .get(&login_url)
        .send()
        .ok()
        .and_then(|resp| resp.text().ok())
        .and_then(|html| extract_luci_form_token(&html));

    let mut form = vec![
        ("luci_username", username),
        ("luci_password", password),
    ];
    if let Some(ref t) = token {
        form.push(("token", t.as_str()));
    }

    let response = match login_client
        .post(&login_url)
        .header("Referer", &login_url)
        .form(&form)
        .send()
    {
        Ok(resp) => resp,
        Err(err) => {
            trace(
                "luci",
                "login_err",
                &format!("{base} user={username} timeout_or_conn err={err}"),
            );
            return None;
        }
    };

    let status = response.status();
    let code = status.as_u16();
    if !status.is_success() && code != 302 {
        trace(
            "luci",
            "login_rejected",
            &format!("{base} user={username} code={code}"),
        );
        return None;
    }

    let cookie = extract_sysauth_cookie(response.headers());
    trace(
        "luci",
        "login_result",
        &format!(
            "{base} user={username} code={code} cookie={}",
            cookie.is_some()
        ),
    );
    cookie
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

/// Syslog textarea from LuCI btminerapi (authenticated page, may lack per-chip lines).
fn extract_btminer_syslog(html: &str) -> Option<String> {
    let log = parse_btminer_html(html)?;
    if log.contains("slot:") || log.lines().any(|line| line.trim().starts_with("slot")) {
        Some(log)
    } else {
        None
    }
}

/// Strict: syslog must include per-chip `C0`/`C1`… lines for chip map parsing.
fn extract_chip_log(html: &str) -> Option<String> {
    let log = extract_btminer_syslog(html)?;
    let has_chip_lines = log.lines().any(|line| {
        let t = line.trim();
        t.starts_with('C')
            && t.chars().nth(1).is_some_and(|c| c.is_ascii_digit())
            && t.contains("temp:")
    });
    if has_chip_lines {
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
    fn accepts_slot_summary_for_luci_login() {
        let summary_only = r#"
slot: 0, freq: 0, temp: 21.2, step: 5
slot: 1, freq: 0, temp: 21.2, step: 5
"#;
        let html = format!(r#"<textarea id="syslog">{summary_only}</textarea>"#);
        assert!(extract_btminer_syslog(&html).is_some());
        assert!(extract_chip_log(&html).is_none());
    }

    #[test]
    fn rejects_slot_summary_without_per_chip_lines() {
        let summary_only = r#"
slot: 0, freq: 0, temp: 21.2, step: 5
slot: 1, freq: 0, temp: 21.2, step: 5
"#;
        assert!(extract_chip_log(&format!(r#"<textarea id="syslog">{summary_only}</textarea>"#)).is_none());
    }

    #[test]
    fn rejects_html_without_chip_dump() {
        assert!(extract_chip_log("<html><body>login</body></html>").is_none());
    }

    #[test]
    #[ignore = "requires WhatsMiner on local network"]
    fn read_path_chip_fetch_uses_full_luci() {
        use super::super::options::{WhatsminerFetchOptions, WhatsminerLuciAuth};

        let host = "192.168.35.35";
        let auth = WhatsminerFetchOptions::read_once(Some(WhatsminerLuciAuth {
            username: "admin".into(),
            password: "admin".into(),
        }));
        let (boards, log) = fetch_btminer_chip_data(host, &auth);
        eprintln!("boards={} log_bytes={}", boards.len(), log.len());
        if boards.is_empty() && !log.is_empty() {
            eprintln!("log_preview:\n{}", &log[..log.len().min(1200)]);
        }
        assert!(!log.is_empty(), "expected btminer log on read path options");
        if boards.is_empty() {
            eprintln!("no chip boards parsed (miner may be in protect/idle)");
        }
    }

    #[test]
    #[ignore = "requires WhatsMiner on local network"]
    fn fetches_chip_data_from_live_whatsminer() {
        let options = WhatsminerFetchOptions::default();
        let (boards, log) = fetch_btminer_chip_data("192.168.35.35", &options);
        assert!(!log.is_empty(), "expected btminer log text");
        assert!(!boards.is_empty(), "expected parsed chip boards");
        assert!(boards.iter().any(|board| !board.chips.is_empty()));
    }
}
