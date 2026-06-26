use super::avalon::AvalonDriver;
use super::MinerDriver;
use crate::error::MinerPulseError;
use crate::model::{MinerSnapshot, MinerVendor};
use crate::tcp::TcpCgminerClient;

pub struct DriverRegistry;

impl DriverRegistry {
    pub fn all() -> Vec<Box<dyn MinerDriver>> {
        vec![Box::new(AvalonDriver) as Box<dyn MinerDriver>]
    }
}

pub fn detect_driver(stats_response: &str) -> Option<Box<dyn MinerDriver>> {
    if AvalonDriver::detect(stats_response) {
        return Some(Box::new(AvalonDriver) as Box<dyn MinerDriver>);
    }

    if let Some(id) = super::parse::get_parameter(stats_response, "ID") {
        if id.contains("DT") {
            return None; // Innosilicon stub for phase 2
        }
    }

    if let Some(kind) = super::parse::get_parameter(stats_response, "Type") {
        if kind.contains("Antminer") {
            return None; // Antminer stub for phase 2
        }
    }

    None
}

pub fn fetch_with_detect(
    client: &TcpCgminerClient,
    host: &str,
    port: u16,
) -> Result<MinerSnapshot, MinerPulseError> {
    let stats = client.send_receive(host, port, "stats", "", false)?;

    if stats.contains("Connection failed") || stats.contains("Stream broken") {
        return Err(MinerPulseError::conn_failed());
    }

    let driver = detect_driver(&stats).ok_or(MinerPulseError::with_code(
        crate::error::ErrorCode::NotSupported,
    ))?;

    driver.fetch_snapshot(client, host, port)
}

pub fn detect_vendor(stats_response: &str) -> MinerVendor {
    if AvalonDriver::detect(stats_response) {
        MinerVendor::Avalon
    } else if let Some(id) = super::parse::get_parameter(stats_response, "ID") {
        if id.contains("DT") {
            MinerVendor::Innosilicon
        } else {
            MinerVendor::Generic
        }
    } else if let Some(kind) = super::parse::get_parameter(stats_response, "Type") {
        if kind.contains("Antminer") {
            MinerVendor::Antminer
        } else {
            MinerVendor::Generic
        }
    } else {
        MinerVendor::Unknown
    }
}
