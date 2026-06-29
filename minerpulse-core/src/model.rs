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
    #[serde(default)]
    pub core_chip: Option<String>,
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
    #[serde(default)]
    pub chip_temp_min_c: Option<f64>,
    #[serde(default)]
    pub chip_temp_avg_c: Option<f64>,
    #[serde(default)]
    pub chip_temp_max_c: Option<f64>,
    #[serde(default)]
    pub effective_chips: Option<u32>,
    #[serde(default)]
    pub freq_domains_mhz: Vec<u32>,
    #[serde(default)]
    pub freq_bands_mhz: Vec<u32>,
    #[serde(default)]
    pub voltage_level: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ChipStats {
    pub index: u32,
    pub temp_c: i32,
    #[serde(default)]
    pub freq_mhz: Option<u32>,
    #[serde(default)]
    /// Per-chip voltage in millivolts (`PVT_V` on Avalon).
    pub voltage: Option<u32>,
    /// Per-chip CRC error count on Avalon (`ASICCRC`). PLL/splash count on WhatsMiner (`error`/`err`).
    #[serde(default)]
    pub errors: Option<u32>,
    /// Per-chip solution count (`MW` on Avalon).
    #[serde(default)]
    pub solutions: Option<u32>,
    /// WhatsMiner btminer log: CRC communication errors (`crc` field).
    #[serde(default)]
    pub crc_errors: Option<u32>,
    /// WhatsMiner btminer log: cumulative valid nonce count.
    #[serde(default)]
    pub nonce: Option<u64>,
    /// WhatsMiner btminer log: retry count (`repeat` field).
    #[serde(default)]
    pub repeat_count: Option<u32>,
    /// WhatsMiner btminer log: performance % (short-term / long-term from `pct:X%/Y%`).
    #[serde(default)]
    pub performance_pct: Option<[f32; 2]>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BoardChipMap {
    pub slot: u32,
    pub label: String,
    pub chips_per_domain: u32,
    #[serde(default)]
    pub matrix_id: Option<String>,
    pub chips: Vec<ChipStats>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MinerFault {
    pub code: String,
    #[serde(default)]
    pub occurred_at: Option<String>,
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
    #[serde(default)]
    pub board_chips: Vec<BoardChipMap>,
    #[serde(default)]
    pub faults: Vec<MinerFault>,
    pub raw_log: String,
    pub status: String,
    pub uptime_sec: Option<u64>,
    #[serde(default)]
    pub work_mode: Option<u32>,
    #[serde(default)]
    pub ecmm: Option<u64>,
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
            board_chips: Vec::new(),
            faults: Vec::new(),
            raw_log: String::new(),
            status: String::new(),
            uptime_sec: None,
            work_mode: None,
            ecmm: None,
        }
    }
}
