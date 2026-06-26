use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum MinerVendor {
    #[default]
    Unknown,
    Avalon,
    Antminer,
    Innosilicon,
    Whatsminer,
    Generic,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MinerIdentity {
    pub vendor: MinerVendor,
    pub model: String,
    pub firmware: String,
    pub driver_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HashrateStats {
    pub current_ghs: f64,
    pub avg_ghs: f64,
    pub avg5s_ghs: f64,
    pub per_board_ghs: Vec<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ThermalStats {
    pub inlet_c: Option<f64>,
    pub per_board_max_c: Vec<f64>,
    pub per_chip_c: Vec<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FanStats {
    pub rpm: Vec<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PowerStats {
    pub watts: Option<f64>,
    pub voltage: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BoardStats {
    pub label: String,
    pub hashrate_ghs: Option<f64>,
    pub temp_c: Option<f64>,
    pub fan_rpm: Option<u32>,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PoolInfo {
    pub url: String,
    pub worker: String,
    pub status: String,
    pub accepted: u64,
    pub rejected: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinerSnapshot {
    pub identity: MinerIdentity,
    pub hashrate: HashrateStats,
    pub thermal: ThermalStats,
    pub fans: FanStats,
    pub power: PowerStats,
    #[serde(default)]
    pub boards: Vec<BoardStats>,
    pub pools: Vec<PoolInfo>,
    #[serde(default)]
    pub shares_accepted: Option<u64>,
    #[serde(default)]
    pub shares_rejected: Option<u64>,
    #[serde(default)]
    pub hw_errors: Option<u64>,
    pub raw_log: String,
    pub status: String,
    pub uptime_sec: Option<u64>,
}

impl Default for MinerSnapshot {
    fn default() -> Self {
        Self {
            identity: MinerIdentity::default(),
            hashrate: HashrateStats::default(),
            thermal: ThermalStats::default(),
            fans: FanStats::default(),
            power: PowerStats::default(),
            boards: Vec::new(),
            pools: Vec::new(),
            shares_accepted: None,
            shares_rejected: None,
            hw_errors: None,
            raw_log: String::new(),
            status: String::new(),
            uptime_sec: None,
        }
    }
}
