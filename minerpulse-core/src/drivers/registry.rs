use super::antminer::{detect_antminer_summary, parse_antminer_snapshot, AntminerDriver};
use super::avalon::AvalonDriver;
use super::whatsminer::WhatsminerDriver;
use super::MinerDriver;
use crate::error::MinerPulseError;
use crate::fetch_options::FetchOptions;
use crate::model::{MinerSnapshot, MinerVendor};
use crate::tcp::TcpCgminerClient;

pub struct DriverRegistry;

impl DriverRegistry {
    pub fn all() -> Vec<Box<dyn MinerDriver>> {
        vec![
            Box::new(AvalonDriver) as Box<dyn MinerDriver>,
            Box::new(AntminerDriver) as Box<dyn MinerDriver>,
            Box::new(WhatsminerDriver) as Box<dyn MinerDriver>,
        ]
    }
}

pub fn driver_available(vendor: MinerVendor) -> bool {
    matches!(
        vendor,
        MinerVendor::Avalon | MinerVendor::Antminer | MinerVendor::Whatsminer
    )
}

pub fn model_from_stats(stats_response: &str) -> String {
    if AvalonDriver::detect(stats_response) {
        if let Some(ver) = super::parse::get_parameter(stats_response, "Ver") {
            return format!("Avalon-{ver}");
        }
        return "Avalon".to_string();
    }

    if let Some(kind) = super::parse::get_parameter(stats_response, "Type") {
        if kind.contains("Antminer") {
            return kind;
        }
    }

    if let Some(id) = super::parse::get_parameter(stats_response, "ID") {
        if id.contains("DT") {
            return "Innosilicon".to_string();
        }
    }

    String::new()
}

pub fn detect_driver(stats_response: &str) -> Option<Box<dyn MinerDriver>> {
    if AvalonDriver::detect(stats_response) {
        return Some(Box::new(AvalonDriver) as Box<dyn MinerDriver>);
    }

    if AntminerDriver::detect(stats_response) {
        return Some(Box::new(AntminerDriver) as Box<dyn MinerDriver>);
    }

    if let Some(id) = super::parse::get_parameter(stats_response, "ID") {
        if id.contains("DT") {
            return None; // Innosilicon stub for phase 2
        }
    }

    None
}

fn is_error_response(response: &str) -> bool {
    response.contains("Connection failed")
        || response.contains("Connection timeout")
        || response.contains("Stream broken")
}

pub fn fetch_with_detect(
    client: &TcpCgminerClient,
    host: &str,
    port: u16,
    options: &FetchOptions,
) -> Result<MinerSnapshot, MinerPulseError> {
    let mut last_stats = String::new();

    for json_mode in [true, false] {
        if let Ok(stats) = client.send_receive(host, port, "stats", "", json_mode) {
            if is_error_response(&stats) {
                continue;
            }
            last_stats = stats.clone();
            if let Some(driver) = detect_driver(&stats) {
                return driver.fetch_snapshot(client, host, port, options);
            }
        }
    }

    let summary = client
        .send_receive(host, port, "summary", "", true)
        .unwrap_or_default();
    let pools_raw = client
        .send_receive(host, port, "pools", "", true)
        .unwrap_or_default();
    let devs_raw = client
        .send_receive(host, port, "devs", "", true)
        .unwrap_or_default();

    if (AntminerDriver::detect(&last_stats)
        || detect_antminer_summary(&summary)
        || last_stats.contains("Antminer"))
        && crate::drivers::whatsminer::classify_for_discovery(&summary).is_none()
        && crate::drivers::whatsminer::classify_for_discovery(&last_stats).is_none()
    {
        let stats = if last_stats.is_empty() {
            client
                .send_receive(host, port, "stats", "", true)
                .or_else(|_| client.send_receive(host, port, "stats", "", false))
                .unwrap_or_default()
        } else {
            last_stats
        };
        return Ok(parse_antminer_snapshot(
            &stats,
            &summary,
            &pools_raw,
            &devs_raw,
        ));
    }

    if WhatsminerDriver::detect(&summary) || WhatsminerDriver::detect(&last_stats) {
        let driver = WhatsminerDriver;
        return driver.fetch_snapshot(client, host, port, options);
    }

    if let Ok(summary) = client.send_payload(host, port, r#"{"cmd":"summary"}"#) {
        if WhatsminerDriver::detect(&summary) {
            let driver = WhatsminerDriver;
            return driver.fetch_snapshot(client, host, port, options);
        }
    }

    Err(MinerPulseError::with_code(
        crate::error::ErrorCode::NotSupported,
    ))
}

pub fn fetch_whatsminer(
    client: &TcpCgminerClient,
    host: &str,
    port: u16,
    options: &FetchOptions,
) -> Result<MinerSnapshot, MinerPulseError> {
    WhatsminerDriver.fetch_snapshot(client, host, port, options)
}

pub fn detect_vendor(stats_response: &str) -> MinerVendor {
    if AvalonDriver::detect(stats_response) {
        MinerVendor::Avalon
    } else if AntminerDriver::detect(stats_response) {
        MinerVendor::Antminer
    } else if let Some(id) = super::parse::get_parameter(stats_response, "ID") {
        if id.contains("DT") {
            MinerVendor::Innosilicon
        } else {
            MinerVendor::Generic
        }
    } else {
        MinerVendor::Unknown
    }
}
