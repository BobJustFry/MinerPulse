use crate::drivers::avalon::refresh_avalon_board_chips_from_raw_log;
use crate::drivers::whatsminer::refresh_whatsminer_board_chips_from_raw_log;
use crate::entitlements::SubscriptionTier;
use crate::error::MinerPulseError;
use crate::model::MinerSnapshot;
use crate::mpulse_binary::{is_binary_mpulse, save_binary_mpulse, LoadedBinarySession, StoredChartPoint};
use chrono::Utc;
use flate2::read::GzDecoder;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Read;
use std::path::Path;

pub const DEFAULT_POLL_RATE_HZ: u32 = 1;
pub const POLL_RATES_HZ: [u32; 5] = [1, 3, 5, 10, 15];
pub const DEFAULT_POLL_INTERVAL_SEC: u32 = 1;
pub const MAX_SESSION_DURATION_SEC: u64 = 30 * 60;
pub const MAX_SESSION_FILE_BYTES: usize = 16 * 1024 * 1024;

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
    #[serde(default, skip_serializing_if = "String::is_empty")]
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub poll_rate_hz: Option<u32>,
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
            poll_rate_hz: None,
            frames: vec![MpulseFrame {
                t_ms: 0,
                raw_log: snapshot.raw_log.clone(),
                snapshot,
            }],
        }
    }

    pub fn new_session(
        miner_ip: &str,
        driver_id: &str,
        tier: SubscriptionTier,
        poll_rate_hz: u32,
    ) -> Self {
        Self {
            format_version: 1,
            kind: MpulseKind::Session,
            saved_at: None,
            recorded_at: Some(Utc::now().to_rfc3339()),
            recorder_tier: tier,
            miner_ip: miner_ip.to_string(),
            hash_map_id: None,
            driver_id: driver_id.to_string(),
            interval_sec: None,
            poll_rate_hz: Some(poll_rate_hz),
            frames: Vec::new(),
        }
    }

    pub fn push_recorded_frame(&mut self, t_ms: u64, snapshot: MinerSnapshot) {
        let raw_log = if snapshot.raw_log.is_empty() {
            String::new()
        } else {
            snapshot.raw_log.clone()
        };
        self.frames.push(MpulseFrame {
            t_ms,
            snapshot,
            raw_log,
        });
    }

    pub fn trim_to_max_duration(&mut self, max_duration_ms: u64) {
        if max_duration_ms == 0 {
            return;
        }
        self.frames
            .retain(|frame| frame.t_ms <= max_duration_ms);
    }
}

pub fn chart_point_from_snapshot(snapshot: &MinerSnapshot, t_ms: u64) -> StoredChartPoint {
    let mut board_temps = snapshot
        .thermal
        .per_board_max_c
        .iter()
        .map(|value| *value as f32)
        .collect::<Vec<_>>();
    if board_temps.is_empty() {
        board_temps = snapshot
            .boards
            .iter()
            .filter_map(|board| board.chip_temp_max_c.or(board.temp_c))
            .map(|value| value as f32)
            .collect();
    }
    let fan_rpm = snapshot
        .fans
        .rpm
        .iter()
        .copied()
        .max()
        .or_else(|| snapshot.params.psu_fan_rpm);
    StoredChartPoint {
        t_ms,
        hashrate_ghs: snapshot.hashrate.current_ghs,
        board_temps,
        power_w: snapshot.power.watts.map(|value| value as f32),
        fan_rpm,
    }
}

pub fn open_mpulse_file(path: &Path) -> Result<LoadedBinarySession, MinerPulseError> {
    let bytes = fs::read(path).map_err(|_| {
        MinerPulseError::with_code(crate::error::ErrorCode::IoError)
    })?;
    if bytes.len() > MAX_SESSION_FILE_BYTES {
        return Err(MinerPulseError::with_code(
            crate::error::ErrorCode::InvalidInput,
        ));
    }
    if is_binary_mpulse(&bytes) {
        return load_binary_mpulse_bytes(&bytes);
    }
    let json = decode_mpulse_bytes(&bytes)?;
    let mut file: MpulseFile = serde_json::from_str(&json).map_err(|_| {
        MinerPulseError::with_code(crate::error::ErrorCode::ParseFailed)
    })?;
    refresh_loaded_mpulse_frames(&mut file);
    let chart_points = file
        .frames
        .iter()
        .map(|frame| chart_point_from_snapshot(&frame.snapshot, frame.t_ms))
        .collect::<Vec<_>>();
    let timeline_ms = file.frames.iter().map(|frame| frame.t_ms).collect::<Vec<_>>();
    Ok(LoadedBinarySession {
        meta: crate::mpulse_binary::BinarySessionMeta {
            format_version: file.format_version,
            kind: file.kind,
            miner_ip: file.miner_ip,
            driver_id: file.driver_id,
            poll_rate_hz: file.poll_rate_hz,
            recorder_tier: file.recorder_tier,
            recorded_at: file.recorded_at,
            saved_at: file.saved_at,
            frame_count: file.frames.len() as u32,
        },
        chart_points,
        timeline_ms,
        frames: file.frames,
    })
}

fn load_binary_mpulse_bytes(bytes: &[u8]) -> Result<LoadedBinarySession, MinerPulseError> {
    crate::mpulse_binary::load_binary_mpulse_bytes(bytes)
}

pub fn normalize_poll_rate_hz(rate: Option<u32>) -> u32 {
    let rate = rate.unwrap_or(DEFAULT_POLL_RATE_HZ);
    if POLL_RATES_HZ.contains(&rate) {
        rate
    } else {
        DEFAULT_POLL_RATE_HZ
    }
}

pub fn poll_interval_ms(poll_rate_hz: u32) -> u64 {
    let rate = normalize_poll_rate_hz(Some(poll_rate_hz)).max(1);
    (1000 / rate as u64).max(1)
}

/// How long to wait after a poll tick before starting the next one.
/// The interval is measured from `tick_start`, not from when the fetch finished.
pub fn poll_wait_after_tick(
    tick_start: std::time::Instant,
    interval: std::time::Duration,
    now: std::time::Instant,
) -> std::time::Duration {
    let deadline = tick_start + interval;
    if now >= deadline {
        std::time::Duration::ZERO
    } else {
        deadline - now
    }
}

pub fn is_gzip_bytes(bytes: &[u8]) -> bool {
    bytes.len() >= 2 && bytes[0] == 0x1f && bytes[1] == 0x8b
}

pub fn decode_mpulse_bytes(bytes: &[u8]) -> Result<String, MinerPulseError> {
    if is_gzip_bytes(bytes) {
        let mut decoder = GzDecoder::new(bytes);
        let mut json = String::new();
        decoder
            .read_to_string(&mut json)
            .map_err(|_| MinerPulseError::with_code(crate::error::ErrorCode::ParseFailed))?;
        return Ok(json);
    }

    String::from_utf8(bytes.to_vec()).map_err(|_| {
        MinerPulseError::with_code(crate::error::ErrorCode::ParseFailed)
    })
}

pub fn load_mpulse(path: &Path) -> Result<MpulseFile, MinerPulseError> {
    let opened = open_mpulse_file(path)?;
    Ok(MpulseFile {
        format_version: opened.meta.format_version,
        kind: opened.meta.kind,
        saved_at: opened.meta.saved_at,
        recorded_at: opened.meta.recorded_at,
        recorder_tier: opened.meta.recorder_tier,
        miner_ip: opened.meta.miner_ip,
        hash_map_id: None,
        driver_id: opened.meta.driver_id,
        interval_sec: None,
        poll_rate_hz: opened.meta.poll_rate_hz,
        frames: opened.frames,
    })
}

fn refresh_loaded_mpulse_frames(file: &mut MpulseFile) {
    for frame in &mut file.frames {
        let raw_log = if !frame.raw_log.is_empty() {
            frame.raw_log.clone()
        } else if !frame.snapshot.raw_log.is_empty() {
            frame.snapshot.raw_log.clone()
        } else {
            continue;
        };
        refresh_avalon_board_chips_from_raw_log(&mut frame.snapshot, Some(&raw_log));
        refresh_whatsminer_board_chips_from_raw_log(&mut frame.snapshot, Some(&raw_log));
    }
}

pub fn save_snapshot(path: &Path, file: &MpulseFile) -> Result<(), MinerPulseError> {
    save_binary_mpulse(path, file)
}

pub fn save_session(path: &Path, file: &MpulseFile) -> Result<(), MinerPulseError> {
    save_binary_mpulse(path, file)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{HashrateStats, MinerIdentity, MinerSnapshot, MinerVendor};

    fn sample_snapshot() -> MinerSnapshot {
        MinerSnapshot {
            identity: MinerIdentity {
                vendor: MinerVendor::Avalon,
                model: "Avalon 1326".into(),
                firmware: "1326".into(),
                driver_id: "avalon".into(),
                ..Default::default()
            },
            hashrate: HashrateStats {
                current_ghs: 1000.0,
                avg_ghs: 1000.0,
                avg5s_ghs: 1000.0,
                per_board_ghs: vec![],
            },
            raw_log: "sample log".into(),
            status: "OK".into(),
            ..Default::default()
        }
    }

    #[test]
    fn roundtrips_gzip_session_file() {
        let dir = std::env::temp_dir().join(format!("mpulse-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("session.mprs");

        let mut file = MpulseFile::new_session(
            "192.168.0.1",
            "avalon",
            SubscriptionTier::Service,
            DEFAULT_POLL_RATE_HZ,
        );
        file.push_recorded_frame(0, sample_snapshot());
        file.push_recorded_frame(5000, sample_snapshot());

        save_session(&path, &file).unwrap();
        let loaded = load_mpulse(&path).unwrap();
        assert_eq!(loaded.kind, MpulseKind::Session);
        assert_eq!(loaded.frames.len(), 2);
        assert_eq!(loaded.frames[0].snapshot.raw_log, "sample log");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn poll_wait_is_zero_when_fetch_exceeded_interval() {
        let start = std::time::Instant::now();
        let interval = std::time::Duration::from_millis(1000);
        let now = start + std::time::Duration::from_millis(1500);
        assert_eq!(
            poll_wait_after_tick(start, interval, now),
            std::time::Duration::ZERO
        );
    }

    #[test]
    fn poll_wait_waits_until_interval_from_tick_start() {
        let start = std::time::Instant::now();
        let interval = std::time::Duration::from_millis(1000);
        let now = start + std::time::Duration::from_millis(400);
        assert_eq!(
            poll_wait_after_tick(start, interval, now),
            std::time::Duration::from_millis(600)
        );
    }
}
