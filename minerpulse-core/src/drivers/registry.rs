use super::antminer::{detect_antminer_summary, parse_antminer_snapshot, AntminerDriver};
use super::avalon::AvalonDriver;
use super::whatsminer::options::WhatsminerFetchOptions;
use super::whatsminer::WhatsminerDriver;
use super::MinerDriver;
use crate::error::MinerPulseError;
use crate::model::{MinerSnapshot, MinerVendor};
use crate::tcp::TcpCgminerClient;
use crate::trace;

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

fn ensure_not_cancelled(options: &WhatsminerFetchOptions) -> Result<(), MinerPulseError> {
    if options.is_cancelled() {
        return Err(MinerPulseError::operation_cancelled());
    }
    Ok(())
}

/// Detect vendor on 4028, then delegate to **one** driver. `wm_options` is WhatsMiner-only.
pub fn fetch_with_detect(
    client: &TcpCgminerClient,
    host: &str,
    port: u16,
    wm_options: &WhatsminerFetchOptions,
) -> Result<MinerSnapshot, MinerPulseError> {
    ensure_not_cancelled(wm_options)?;

    if wm_options.fast_poll {
        trace("detect", "fast_start", host);
        match fetch_with_detect_fast(client, host, port, wm_options) {
            Ok(snapshot) => {
                trace(
                    "detect",
                    "fast_ok",
                    &format!("{host} vendor={:?}", snapshot.identity.vendor),
                );
                return Ok(snapshot);
            }
            Err(err) if err.code() == crate::error::ErrorCode::NotSupported => {
                trace("detect", "fast_fallback", host);
            }
            Err(err) => return Err(err),
        }
    }

    trace("detect", "full_start", host);
    fetch_with_detect_full(client, host, port, wm_options)
}

/// One-shot read: WhatsMiner JSON summary first, then a single Antminer/Avalon probe.
fn fetch_with_detect_fast(
    client: &TcpCgminerClient,
    host: &str,
    port: u16,
    wm_options: &WhatsminerFetchOptions,
) -> Result<MinerSnapshot, MinerPulseError> {
    let mut last_conn_err: Option<MinerPulseError> = None;

    match client.send_payload(host, port, r#"{"cmd":"summary"}"#) {
        Ok(summary) if is_meaningful_response(&summary) => {
            ensure_not_cancelled(wm_options)?;
            if WhatsminerDriver::detect(&summary) {
                trace("detect", "whatsminer_payload", host);
                return WhatsminerDriver::fetch_with_options(client, host, port, wm_options);
            }
        }
        Ok(_) => {}
        Err(err) => {
            if err.code() == crate::error::ErrorCode::OperationCancelled {
                return Err(err);
            }
            last_conn_err = Some(err);
        }
    }

    ensure_not_cancelled(wm_options)?;

    let stats = match client.send_receive(host, port, "stats", "", true) {
        Ok(value) if is_meaningful_response(&value) => {
            ensure_not_cancelled(wm_options)?;
            Some(value)
        }
        Ok(_) => None,
        Err(err) => {
            if err.code() == crate::error::ErrorCode::OperationCancelled {
                return Err(err);
            }
            last_conn_err = Some(err);
            None
        }
    };

    if let Some(ref stats) = stats {
        if let Some(driver) = detect_driver(stats) {
            return driver.fetch_snapshot(client, host, port);
        }
    }

    let summary = match client.send_receive(host, port, "summary", "", true) {
        Ok(value) if is_meaningful_response(&value) => {
            ensure_not_cancelled(wm_options)?;
            value
        }
        Ok(_) => String::new(),
        Err(err) => {
            if err.code() == crate::error::ErrorCode::OperationCancelled {
                return Err(err);
            }
            last_conn_err = Some(err);
            String::new()
        }
    };

    if WhatsminerDriver::detect(&summary) {
        return WhatsminerDriver::fetch_with_options(client, host, port, wm_options);
    }

    if AntminerDriver::detect(stats.as_deref().unwrap_or_default())
        || detect_antminer_summary(&summary)
    {
        let stats = stats.unwrap_or_default();
        let pools_raw = client
            .send_receive(host, port, "pools", "", true)
            .unwrap_or_default();
        let devs_raw = client
            .send_receive(host, port, "devs", "", true)
            .unwrap_or_default();
        return Ok(parse_antminer_snapshot(&stats, &summary, &pools_raw, &devs_raw));
    }

    if let Some(err) = last_conn_err {
        return Err(err);
    }

    Err(MinerPulseError::with_code(crate::error::ErrorCode::NotSupported))
}

fn fetch_with_detect_full(
    client: &TcpCgminerClient,
    host: &str,
    port: u16,
    wm_options: &WhatsminerFetchOptions,
) -> Result<MinerSnapshot, MinerPulseError> {
    let mut last_stats = String::new();
    let mut saw_response = false;
    let mut last_conn_err: Option<MinerPulseError> = None;

    for json_mode in [true, false] {
        ensure_not_cancelled(wm_options)?;
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

    ensure_not_cancelled(wm_options)?;

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

    ensure_not_cancelled(wm_options)?;

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
                Err(_) => String::new(),
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
