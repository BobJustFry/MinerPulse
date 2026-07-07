use crate::drivers::whatsminer::access::{fetch_device_info, normalize_mac};
use crate::drivers::whatsminer::luci::fetch_lan_macaddr;
use crate::fetch_options::WhatsminerLuciAuth;
use crate::model::{MinerSnapshot, MinerVendor};
use digest_auth::{parse, AuthContext};
use reqwest::blocking::Client;
use reqwest::header::{AUTHORIZATION, WWW_AUTHENTICATE};
use serde_json::Value;
use std::time::Duration;

const HTTP_TIMEOUT: Duration = Duration::from_secs(4);

/// Resolve MAC for a known miner vendor (methods ported from amazingFarm).
pub fn fetch_miner_mac(
    vendor: MinerVendor,
    host: &str,
    whatsminer_luci: Option<&WhatsminerLuciAuth>,
) -> Option<String> {
    match vendor {
        MinerVendor::Antminer => fetch_antminer_mac(host),
        MinerVendor::Avalon => fetch_avalon_mac(host),
        MinerVendor::Whatsminer => fetch_whatsminer_mac(host, whatsminer_luci),
        _ => None,
    }
}

pub fn enrich_snapshot_mac(
    snapshot: &mut MinerSnapshot,
    host: &str,
    whatsminer_luci: Option<&WhatsminerLuciAuth>,
) {
    if snapshot.identity.mac.is_some() {
        return;
    }
    let Some(mac) = fetch_miner_mac(snapshot.identity.vendor, host, whatsminer_luci) else {
        return;
    };
    snapshot.identity.mac = Some(mac.clone());
    if snapshot.identity.vendor == MinerVendor::Whatsminer {
        if let Some(access) = snapshot.whatsminer_access.as_mut() {
            if access.mac.is_none() {
                access.mac = Some(mac);
            }
        }
    }
}

fn fetch_antminer_mac(host: &str) -> Option<String> {
    for (user, pass) in [("root", "root"), ("admin", "root")] {
        if let Some(mac) = fetch_antminer_mac_with_creds(host, user, pass) {
            return Some(mac);
        }
    }
    None
}

fn fetch_antminer_mac_with_creds(host: &str, username: &str, password: &str) -> Option<String> {
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
    parse_antminer_mac(&body)
}

fn parse_antminer_mac(body: &str) -> Option<String> {
    let value: Value = serde_json::from_str(body).ok()?;
    value
        .get("macaddr")
        .and_then(|v| v.as_str())
        .map(normalize_mac)
}

fn fetch_avalon_mac(host: &str) -> Option<String> {
    let client = build_http_client()?;
    let url = format!("http://{host}/get_minerinfo.cgi");
    let body = client.get(&url).send().ok()?.text().ok()?;
    parse_avalon_mac(&body)
}

fn parse_avalon_mac(body: &str) -> Option<String> {
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
        .map(normalize_mac)
}

fn fetch_whatsminer_mac(host: &str, luci_auth: Option<&WhatsminerLuciAuth>) -> Option<String> {
    if let Some(info) = fetch_device_info(host) {
        if let Some(mac) = info.mac {
            return Some(mac);
        }
    }

    let pairs: Vec<(&str, &str)> = if let Some(auth) = luci_auth {
        vec![(auth.username.as_str(), auth.password.as_str())]
    } else {
        vec![("admin", "admin"), ("root", "root")]
    };

    for (username, password) in pairs {
        if let Some(mac) = fetch_lan_macaddr(host, username, password) {
            return Some(mac);
        }
    }
    None
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
    fn parses_antminer_system_info_mac() {
        let body = r#"{"macaddr":"aa:bb:cc:dd:ee:ff","minertype":"Antminer L7"}"#;
        assert_eq!(
            parse_antminer_mac(body).as_deref(),
            Some("AA:BB:CC:DD:EE:FF")
        );
    }

    #[test]
    fn parses_avalon_minerinfo_callback_mac() {
        let body = r#"minerinfoCallback({"mac":"11:22:33:44:55:66","hwtype":"AvalonMiner 1346"});"#;
        assert_eq!(
            parse_avalon_mac(body).as_deref(),
            Some("11:22:33:44:55:66")
        );
    }
}
