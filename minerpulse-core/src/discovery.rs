use crate::drivers::antminer::{detect_antminer_summary, AntminerDriver};
use crate::drivers::registry::{detect_vendor, model_from_stats};
use crate::drivers::MinerDriver;
use crate::drivers::whatsminer::{classify_for_discovery, classify_whatsminer};
use crate::model::MinerVendor;
use crate::tcp::TcpCgminerClient;
use rayon::prelude::*;
use rayon::ThreadPoolBuilder;
use serde::{Deserialize, Serialize};
use std::net::{IpAddr, Ipv4Addr};
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

const DEFAULT_PORT: u16 = 4028;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanSubnet {
    pub id: String,
    pub label: String,
    pub start_ip: String,
    pub end_ip: String,
    pub source_ip: Option<String>,
}

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

pub fn list_scan_subnets() -> Vec<ScanSubnet> {
    let mut subnets = Vec::new();

    if let Ok(interfaces) = local_ip_address::list_afinet_netifas() {
        for (name, ip) in interfaces {
            let IpAddr::V4(ip_v4) = ip else {
                continue;
            };
            if ip_v4.is_loopback() || ip_v4.is_link_local() || ip_v4.is_unspecified() {
                continue;
            }
            let octets = ip_v4.octets();
            let id = format!("{}.{}.{}", octets[0], octets[1], octets[2]);
            if subnets.iter().any(|s: &ScanSubnet| s.id == id) {
                continue;
            }
            let start = Ipv4Addr::new(octets[0], octets[1], octets[2], 1);
            let end = Ipv4Addr::new(octets[0], octets[1], octets[2], 254);
            subnets.push(ScanSubnet {
                id: id.clone(),
                label: format!(
                    "{} — {}.{}.{}.0/24 ({ip_v4})",
                    name, octets[0], octets[1], octets[2]
                ),
                start_ip: start.to_string(),
                end_ip: end.to_string(),
                source_ip: Some(ip_v4.to_string()),
            });
        }
    }

    if subnets.is_empty() {
        subnets.push(ScanSubnet {
            id: "192.168.0".into(),
            label: "192.168.0.0/24".into(),
            start_ip: "192.168.0.1".into(),
            end_ip: "192.168.0.254".into(),
            source_ip: None,
        });
    }

    subnets
}

pub fn preview_scan_ranges() -> String {
    list_scan_subnets()
        .iter()
        .map(|subnet| format!("{}-{}", subnet.start_ip, subnet.end_ip))
        .collect::<Vec<_>>()
        .join(", ")
}

pub fn scan_network(request: ScanRequest) -> Result<ScanResult, crate::error::MinerPulseError> {
    let cancel = Arc::new(AtomicBool::new(false));
    scan_network_streaming(request, cancel, |_, _, _, _| {}, |_| {})
}

pub fn scan_network_streaming<P, F>(
    request: ScanRequest,
    cancel: Arc<AtomicBool>,
    on_progress: P,
    on_found: F,
) -> Result<ScanResult, crate::error::MinerPulseError>
where
    P: FnMut(u32, u32, u32, &str) + Send + Sync,
    F: FnMut(DiscoveredMiner) + Send + Sync,
{
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

    let total = all_ips.len() as u32;
    let range_label = range_labels.join(", ");
    let scanned_counter = AtomicU32::new(0);
    let found_counter = AtomicU32::new(0);
    let miners_store: Mutex<Vec<DiscoveredMiner>> = Mutex::new(Vec::new());
    let on_progress = Arc::new(Mutex::new(on_progress));
    let on_found = Arc::new(Mutex::new(on_found));

    let thread_count = std::thread::available_parallelism()
        .map(|n| n.get() * 2)
        .unwrap_or(4);

    let pool = ThreadPoolBuilder::new()
        .num_threads(thread_count)
        .build()
        .map_err(|_| crate::error::MinerPulseError::with_code(crate::error::ErrorCode::IoError))?;

    pool.install(|| {
        all_ips.par_iter().for_each(|ip| {
            if cancel.load(Ordering::Relaxed) {
                return;
            }

            if let Some(miner) = probe_miner(&client, &ip.to_string(), port) {
                found_counter.fetch_add(1, Ordering::Relaxed);
                if let Ok(mut miners) = miners_store.lock() {
                    miners.push(miner.clone());
                }
                if let Ok(mut found) = on_found.lock() {
                    found(miner);
                }
            }

            let scanned = scanned_counter.fetch_add(1, Ordering::Relaxed) + 1;
            if scanned % 4 == 0 || scanned == total {
                let found = found_counter.load(Ordering::Relaxed);
                if let Ok(mut progress) = on_progress.lock() {
                    progress(scanned, total, found, &range_label);
                }
            }
        });
    });

    let final_scanned = scanned_counter.load(Ordering::Relaxed);
    let final_found = found_counter.load(Ordering::Relaxed);
    if let Ok(mut progress) = on_progress.lock() {
        progress(final_scanned, total, final_found, &range_label);
    }

    let mut miners = miners_store.into_inner().unwrap_or_default();
    miners.sort_by(|a, b| ipv4_sort_key(&a.ip).cmp(&ipv4_sort_key(&b.ip)));

    Ok(ScanResult {
        miners,
        scanned: final_scanned,
        range_label,
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
    list_scan_subnets()
        .into_iter()
        .filter_map(|subnet| {
            let start = Ipv4Addr::from_str(&subnet.start_ip).ok()?;
            let end = Ipv4Addr::from_str(&subnet.end_ip).ok()?;
            Some((start, end))
        })
        .collect()
}

fn probe_miner(client: &TcpCgminerClient, ip: &str, port: u16) -> Option<DiscoveredMiner> {
    if let Ok(stats) = client.send_command(ip, port, "stats") {
        if let Some((vendor, model)) = classify_probe_response(&stats) {
            return Some(make_discovered(ip, port, vendor, model));
        }
    }

    if let Ok(stats) = client.send_receive(ip, port, "stats", "", true) {
        if let Some((vendor, model)) = classify_probe_response(&stats) {
            return Some(make_discovered(ip, port, vendor, model));
        }
    }

    if let Ok(summary) = client.send_receive(ip, port, "summary", "", true) {
        if let Some((vendor, model)) = classify_for_discovery(&summary) {
            return Some(make_discovered(ip, port, vendor, model));
        }
        if detect_antminer_summary(&summary)
            || AntminerDriver::detect(&summary)
            || summary.contains("Antminer")
        {
            let model = model_from_stats(&summary);
            let model = if model.is_empty() {
                "Antminer".to_string()
            } else {
                model
            };
            return Some(make_discovered(ip, port, MinerVendor::Antminer, model));
        }
    }

    if let Ok(summary) = client.send_payload(ip, port, r#"{"cmd":"summary"}"#) {
        if let Some((vendor, model)) = classify_whatsminer(&summary) {
            return Some(make_discovered(ip, port, vendor, model));
        }
    }

    None
}

fn classify_probe_response(response: &str) -> Option<(MinerVendor, String)> {
    if let Some(result) = classify_for_discovery(response) {
        return Some(result);
    }
    classify_cgminer_response(response)
}

fn make_discovered(ip: &str, port: u16, vendor: MinerVendor, model: String) -> DiscoveredMiner {
    DiscoveredMiner {
        ip: ip.to_string(),
        port,
        vendor,
        model,
        supported: crate::drivers::registry::driver_available(vendor),
    }
}

fn classify_cgminer_response(response: &str) -> Option<(MinerVendor, String)> {
    if !is_miner_response(response) {
        return None;
    }

    if let Some(result) = classify_cgminer_json(response) {
        return Some(result);
    }

    let vendor = detect_vendor(response);
    if vendor == MinerVendor::Unknown {
        return None;
    }

    let model = model_from_stats(response);
    let model = if model.is_empty() {
        default_model_for_vendor(vendor)
    } else {
        model
    };

    Some((vendor, model))
}

fn classify_cgminer_json(response: &str) -> Option<(MinerVendor, String)> {
    let trimmed = response.trim();
    if !trimmed.starts_with('{') {
        return None;
    }

    let value: serde_json::Value = serde_json::from_str(trimmed).ok()?;
    let stats = value.get("STATS")?.as_array()?.first()?;

    if let Some(kind) = stats.get("Type").and_then(|v| v.as_str()) {
        if kind.contains("Antminer") {
            if stats
                .get("ID")
                .and_then(|v| v.as_str())
                .map(|id| id.contains("BTM"))
                .unwrap_or(false)
            {
                return None;
            }
            return Some((MinerVendor::Antminer, kind.to_string()));
        }
    }

    if let Some(id) = stats.get("ID").and_then(|v| v.as_str()) {
        if id.contains("AV") {
            let model = stats
                .get("Ver")
                .or_else(|| stats.get("Version"))
                .and_then(|v| v.as_str())
                .map(|ver| format!("Avalon-{ver}"))
                .unwrap_or_else(|| "Avalon".to_string());
            return Some((MinerVendor::Avalon, model));
        }
        if id.contains("DT") {
            return Some((MinerVendor::Innosilicon, "Innosilicon".to_string()));
        }
    }

    None
}

fn default_model_for_vendor(vendor: MinerVendor) -> String {
    match vendor {
        MinerVendor::Avalon => "Avalon".to_string(),
        MinerVendor::Antminer => "Antminer".to_string(),
        MinerVendor::Innosilicon => "Innosilicon".to_string(),
        MinerVendor::Whatsminer => "WhatsMiner".to_string(),
        MinerVendor::Generic => "CGMiner".to_string(),
        MinerVendor::Unknown => "Unknown".to_string(),
    }
}

fn is_miner_response(response: &str) -> bool {
    !response.is_empty()
        && !response.contains("Connection failed")
        && !response.contains("Connection timeout")
        && !response.contains("Stream broken")
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
    use crate::drivers::whatsminer::{classify_for_discovery, classify_whatsminer};

    #[test]
    fn classifies_whatsminer_summary_json() {
        let sample = r#"{"STATUS":[{"Msg":"Summary","STATUS":"S"}],"SUMMARY":[{"Miner Type":"M50","MHS 5s":70668114.52}]}"#;
        let (vendor, model) = classify_whatsminer(sample).unwrap();
        assert_eq!(vendor, MinerVendor::Whatsminer);
        assert_eq!(model, "M50");
    }

    #[test]
    fn classifies_antminer_json_stats() {
        let sample = r#"{"STATUS":[{"STATUS":"S"}],"STATS":[{"Type":"Antminer L7","GHS 5s":"9500.00"}]}"#;
        let (vendor, model) = classify_cgminer_json(sample).unwrap();
        assert_eq!(vendor, MinerVendor::Antminer);
        assert_eq!(model, "Antminer L7");
    }

    #[test]
    fn classifies_whatsminer_btm_stats_as_whatsminer_not_antminer() {
        let sample = r#"{"STATUS":[{"STATUS":"S","Msg":"CGMiner stats"}],"STATS":[{"BMMiner":"2.12","Miner":"81.0-1.0.0","Type":"Antminer L7"},{"STATS":3,"ID":"BTM_SOC3","GHS 5s":9529.45,"GHS av":9301.18}]}"#;
        let (vendor, model) = classify_for_discovery(sample).unwrap();
        assert_eq!(vendor, MinerVendor::Whatsminer);
        assert!(!model.is_empty());
    }

    #[test]
    fn classifies_whatsminer_rt_hashrate_summary() {
        let sample = r#"{"SUMMARY":[{"Elapsed":105715,"GHS 5s":9529.45,"GHS av":9301.18,"RT HASHRATE":"9529.46 MH/s"}]}"#;
        let (vendor, _) = classify_for_discovery(sample).unwrap();
        assert_eq!(vendor, MinerVendor::Whatsminer);
    }

    #[test]
    fn rejects_empty_probe_response() {
        assert!(!is_miner_response("Connection failed"));
    }
}
