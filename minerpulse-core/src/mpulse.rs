use crate::entitlements::SubscriptionTier;
use crate::error::MinerPulseError;
use crate::model::MinerSnapshot;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MpulseKind {
    Snapshot,
    Session,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MpulseFrame {
    pub t_ms: u64,
    pub snapshot: MinerSnapshot,
    pub raw_log: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MpulseFile {
    pub format_version: u32,
    pub kind: MpulseKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub saved_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recorded_at: Option<String>,
    pub recorder_tier: SubscriptionTier,
    pub miner_ip: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hash_map_id: Option<String>,
    pub driver_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interval_sec: Option<u32>,
    pub frames: Vec<MpulseFrame>,
}

impl MpulseFile {
    pub fn snapshot(
        snapshot: MinerSnapshot,
        miner_ip: &str,
        tier: SubscriptionTier,
    ) -> Self {
        Self {
            format_version: 1,
            kind: MpulseKind::Snapshot,
            saved_at: Some(Utc::now().to_rfc3339()),
            recorded_at: None,
            recorder_tier: tier,
            miner_ip: miner_ip.to_string(),
            hash_map_id: None,
            driver_id: snapshot.identity.driver_id.clone(),
            interval_sec: None,
            frames: vec![MpulseFrame {
                t_ms: 0,
                raw_log: snapshot.raw_log.clone(),
                snapshot,
            }],
        }
    }
}

pub fn save_snapshot(path: &Path, file: &MpulseFile) -> Result<(), MinerPulseError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|_| {
            MinerPulseError::with_code(crate::error::ErrorCode::IoError)
        })?;
    }

    let json = serde_json::to_string_pretty(file).map_err(|_| {
        MinerPulseError::with_code(crate::error::ErrorCode::IoError)
    })?;

    fs::write(path, json).map_err(|_| MinerPulseError::with_code(crate::error::ErrorCode::IoError))?;
    Ok(())
}
