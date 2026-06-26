use crate::drivers::registry::{detect_vendor, model_from_stats};
use crate::model::MinerVendor;
use crate::tcp::TcpCgminerClient;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::net::{IpAddr, Ipv4Addr};
use std::str::FromStr;

const DEFAULT_PORT: u16 = 4028;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanRequest {
    pub start_ip: Option<String>,
    pub end_ip: Option<String>,
    pub port: Option<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredMiner {
    pub ip: String,
    pub port: u16,
    pub vendor: MinerVendor,
    pub model: String,
    pub supported: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanResult {
    pub miners: Vec<DiscoveredMiner>,
    pub scanned: u32,
    pub range_label: String,
}

pub fn scan_network(request: ScanRequest) -> Result<ScanResult, crate::error::MinerPulseError> {
    let port = request.port.unwrap_or(DEFAULT_PORT);
    let ranges = resolve_ranges(request.start_ip.as_deref(), request.end_ip.as_deref())?;
    let client = TcpCgminerClient::for_discovery();

    let mut all_ips: Vec<Ipv4Addr> = Vec::new();
    let mut range_labels: Vec<String> = Vec::new();

    for (start, end) in ranges {
        range_labels.push(format!("{start}-{end}"));
        let mut current = start;
        loop {
            all_ips.push(current);
            if current == end {
                break;
            }
            current = next_ipv4(current);
        }
    }

    let scanned = all_ips.len() as u32;
    let mut miners: Vec<DiscoveredMiner> = all_ips
        .par_iter()
        .filter_map(|ip| probe_miner(&client, &ip.to_string(), port))
        .collect();

    miners.sort_by(|a, b| ipv4_sort_key(&a.ip).cmp(&ipv4_sort_key(&b.ip)));

    Ok(ScanResult {
        miners,
        scanned,
        range_label: range_labels.join(", "),
    })
}

fn resolve_ranges(
    start: Option<&str>,
    end: Option<&str>,
) -> Result<Vec<(Ipv4Addr, Ipv4Addr)>, crate::error::MinerPulseError> {
    match (start, end) {
        (Some(s), Some(e)) => {
            let start_ip = Ipv4Addr::from_str(s).map_err(|_| {
                crate::error::MinerPulseError::with_code(crate::error::ErrorCode::InvalidInput)
            })?;
            let end_ip = Ipv4Addr::from_str(e).map_err(|_| {
                crate::error::MinerPulseError::with_code(crate::error::ErrorCode::InvalidInput)
            })?;
            if ipv4_sort_key(&start_ip.to_string()) > ipv4_sort_key(&end_ip.to_string()) {
                return Err(crate::error::MinerPulseError::with_code(
                    crate::error::ErrorCode::InvalidInput,
                ));
            }
            Ok(vec![(start_ip, end_ip)])
        }
        _ => Ok(default_local_ranges()),
    }
}

fn default_local_ranges() -> Vec<(Ipv4Addr, Ipv4Addr)> {
    let mut ranges = Vec::new();

    if let Ok(interfaces) = local_ip_address::list_afinet_netifas() {
        for (_, ip) in interfaces {
            let IpAddr::V4(ip_v4) = ip else {
                continue;
            };
            if ip_v4.is_loopback() || ip_v4.is_link_local() || ip_v4.is_unspecified() {
                continue;
            }
            let octets = ip_v4.octets();
            let start = Ipv4Addr::new(octets[0], octets[1], octets[2], 1);
            let end = Ipv4Addr::new(octets[0], octets[1], octets[2], 254);
            let pair = (start, end);
            if !ranges.contains(&pair) {
                ranges.push(pair);
            }
        }
    }

    if ranges.is_empty() {
        ranges.push((
            Ipv4Addr::new(192, 168, 0, 1),
            Ipv4Addr::new(192, 168, 0, 254),
        ));
    }

    ranges
}

fn probe_miner(client: &TcpCgminerClient, ip: &str, port: u16) -> Option<DiscoveredMiner> {
    if let Ok(stats) = client.send_command(ip, port, "stats") {
        if is_miner_response(&stats) {
            let vendor = detect_vendor(&stats);
            let model = model_from_stats(&stats);
            if vendor != MinerVendor::Unknown && !model.is_empty() {
                return Some(DiscoveredMiner {
                    ip: ip.to_string(),
                    port,
                    vendor,
                    model,
                    supported: driver_available(vendor),
                });
            }
        }
    }

    if let Ok(summary) = client.send_payload(ip, port, r#"{"cmd":"summary"}"#) {
        if let Some((vendor, model)) = classify_whatsminer(&summary) {
            return Some(DiscoveredMiner {
                ip: ip.to_string(),
                port,
                vendor,
                model,
                supported: driver_available(vendor),
            });
        }
    }

    None
}

fn is_miner_response(response: &str) -> bool {
    !response.is_empty()
        && !response.contains("Connection failed")
        && !response.contains("Connection timeout")
        && !response.contains("Stream broken")
}

fn classify_whatsminer(json: &str) -> Option<(MinerVendor, String)> {
    let trimmed = json.trim();
    if !trimmed.starts_with('{') {
        return None;
    }

    let value: serde_json::Value = serde_json::from_str(trimmed).ok()?;
    let status_msg = value
        .get("STATUS")
        .and_then(|s| s.as_array())
        .and_then(|items| items.first())
        .and_then(|item| item.get("Msg"))
        .and_then(|msg| msg.as_str());

    let has_summary = value.get("SUMMARY").is_some();
    if !has_summary && status_msg != Some("Summary") {
        return None;
    }

    let model = value
        .get("SUMMARY")
        .and_then(|s| s.as_array())
        .and_then(|items| items.first())
        .and_then(|item| {
            item.get("Miner Type")
                .or_else(|| item.get("Type"))
                .and_then(|v| v.as_str())
                .map(str::to_string)
        })
        .unwrap_or_else(|| "WhatsMiner".to_string());

    Some((MinerVendor::Whatsminer, model))
}

pub fn driver_available(vendor: MinerVendor) -> bool {
    matches!(vendor, MinerVendor::Avalon)
}

fn next_ipv4(addr: Ipv4Addr) -> Ipv4Addr {
    u32::from(addr).checked_add(1).map(Ipv4Addr::from).unwrap_or(addr)
}

fn ipv4_sort_key(ip: &str) -> u32 {
    Ipv4Addr::from_str(ip).map(u32::from).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_whatsminer_summary_json() {
        let sample = r#"{"STATUS":[{"Msg":"Summary","STATUS":"S"}],"SUMMARY":[{"Miner Type":"M50","MHS 5s":70668114.52}]}"#;
        let (vendor, model) = classify_whatsminer(sample).unwrap();
        assert_eq!(vendor, MinerVendor::Whatsminer);
        assert_eq!(model, "M50");
    }

    #[test]
    fn rejects_empty_probe_response() {
        assert!(!is_miner_response("Connection failed"));
    }
}
