use super::antminer::{detect_antminer_summary, parse_antminer_snapshot, AntminerDriver};
use super::avalon::AvalonDriver;
use super::whatsminer::options::WhatsminerFetchOptions;
use super::whatsminer::WhatsminerDriver;
use super::MinerDriver;
use crate::error::MinerPulseError;
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

fn is_meaningful_response(response: &str) -> bool {
    !response.trim().is_empty() && !is_error_response(response)
}

/// Detect vendor on 4028, then delegate to **one** driver. `wm_options` is WhatsMiner-only.
pub fn fetch_with_detect(
    client: &TcpCgminerClient,
    host: &str,
    port: u16,
    wm_options: &WhatsminerFetchOptions,
) -> Result<MinerSnapshot, MinerPulseError> {
    let mut last_stats = String::new();
    let mut saw_response = false;
    let mut last_conn_err: Option<MinerPulseError> = None;

    for json_mode in [true, false] {
        match client.send_receive(host, port, "stats", "", json_mode) {
            Ok(stats) if is_meaningful_response(&stats) => {
                saw_response = true;
                last_stats = stats.clone();
                if let Some(driver) = detect_driver(&stats) {
                    return driver.fetch_snapshot(client, host, port);
                }
            }
            Ok(_) => {}
            Err(err) => last_conn_err = Some(err),
        }
    }

    let summary = match client.send_receive(host, port, "summary", "", true) {
        Ok(value) if is_meaningful_response(&value) => {
            saw_response = true;
            value
        }
        Ok(_) => String::new(),
        Err(err) => {
            last_conn_err = Some(err);
            String::new()
        }
    };

    let pools_raw = match client.send_receive(host, port, "pools", "", true) {
        Ok(value) if is_meaningful_response(&value) => {
            saw_response = true;
            value
        }
        Ok(_) => String::new(),
        Err(err) => {
            last_conn_err = Some(err);
            String::new()
        }
    };

    let devs_raw = match client.send_receive(host, port, "devs", "", true) {
        Ok(value) if is_meaningful_response(&value) => {
            saw_response = true;
            value
        }
        Ok(_) => String::new(),
        Err(err) => {
            last_conn_err = Some(err);
            String::new()
        }
    };

    if WhatsminerDriver::detect(&summary) || WhatsminerDriver::detect(&last_stats) {
        return WhatsminerDriver::fetch_with_options(client, host, port, wm_options);
    }

    match client.send_payload(host, port, r#"{"cmd":"summary"}"#) {
        Ok(payload_summary) if is_meaningful_response(&payload_summary) => {
            saw_response = true;
            if WhatsminerDriver::detect(&payload_summary) {
                return WhatsminerDriver::fetch_with_options(client, host, port, wm_options);
            }
        }
        Err(err) => last_conn_err = Some(err),
        _ => {}
    }

    if AntminerDriver::detect(&last_stats)
        || detect_antminer_summary(&summary)
        || last_stats.contains("Antminer")
    {
        let stats = if last_stats.is_empty() {
            match client.send_receive(host, port, "stats", "", true) {
                Ok(value) if is_meaningful_response(&value) => value,
                Ok(_) => String::new(),
                Err(err) => {
                    last_conn_err = Some(err);
                    String::new()
                }
            }
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

    if !saw_response {
        if let Some(err) = last_conn_err {
            return Err(err);
        }
        return Err(MinerPulseError::no_port_response(port));
    }

    Err(MinerPulseError::with_code(
        crate::error::ErrorCode::NotSupported,
    ))
}

pub fn fetch_whatsminer(
    client: &TcpCgminerClient,
    host: &str,
    port: u16,
    wm_options: &WhatsminerFetchOptions,
) -> Result<MinerSnapshot, MinerPulseError> {
    WhatsminerDriver::fetch_with_options(client, host, port, wm_options)
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
