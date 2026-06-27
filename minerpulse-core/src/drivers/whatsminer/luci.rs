use super::btminer_log::{parse_btminer_html, parse_btminer_log};
use crate::fetch_options::FetchOptions;
use crate::model::BoardChipMap;
use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, SET_COOKIE};
use std::time::Duration;

pub fn fetch_btminer_chip_data(host: &str, options: &FetchOptions) -> (Vec<BoardChipMap>, String) {
    let client = match Client::builder()
        .danger_accept_invalid_certs(true)
        .timeout(Duration::from_secs(8))
        .redirect(reqwest::redirect::Policy::limited(4))
        .build()
    {
        Ok(client) => client,
        Err(_) => return (Vec::new(), String::new()),
    };

    for scheme in ["https", "http"] {
        let base = format!("{scheme}://{host}");
        if let Some(log) = fetch_log_anonymous(&client, &base) {
            return (parse_btminer_log(&log), log);
        }

        for (username, password) in options.luci_credential_pairs() {
            if let Some(log) = fetch_log_authenticated(&client, &base, &username, &password) {
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
    base: &str,
    username: &str,
    password: &str,
) -> Option<String> {
    let cookie = luci_login(client, base, username, password)?;
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

fn luci_login(client: &Client, base: &str, username: &str, password: &str) -> Option<String> {
    let login_url = format!("{base}/cgi-bin/luci");
    let response = client
        .post(&login_url)
        .header("Referer", format!("{base}/cgi-bin/luci"))
        .form(&[("luci_username", username), ("luci_password", password)])
        .send()
        .ok()?;

    let cookie = extract_sysauth_cookie(response.headers())?;
    Some(cookie)
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
}
