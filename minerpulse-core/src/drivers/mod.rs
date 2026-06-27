use crate::error::MinerPulseError;
use crate::fetch_options::FetchOptions;
use crate::model::MinerSnapshot;
use crate::tcp::TcpCgminerClient;

pub mod antminer;
pub mod avalon;
pub mod avalon_cgminer;
pub mod avalon_chips;
pub mod avalon_commands;
pub mod json_util;
pub mod parse;
pub mod registry;
pub mod whatsminer;

pub trait MinerDriver: Send + Sync {
    fn id(&self) -> &'static str;

    fn detect(stats_response: &str) -> bool
    where
        Self: Sized;

    fn fetch_snapshot(
        &self,
        client: &TcpCgminerClient,
        host: &str,
        port: u16,
        options: &FetchOptions,
    ) -> Result<MinerSnapshot, MinerPulseError>;
}
