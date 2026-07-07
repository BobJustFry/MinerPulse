use super::access::fetch_device_info;
use super::luci::fetch_lan_macaddr;
use super::options::WhatsminerLuciAuth;

/// WhatsMiner-only MAC resolution (4433 API, then LuCI iface).
pub fn fetch_mac_address(host: &str, luci_auth: Option<&WhatsminerLuciAuth>) -> Option<String> {
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
