use crate::entitlements::SubscriptionTier;
use crate::error::MinerPulseError;
use crate::mpulse::{chart_point_from_snapshot, MpulseFile, MpulseFrame, MpulseKind};
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::path::Path;

pub const MAGIC_SESSION: &[u8; 4] = b"MPRS";
pub const MAGIC_SNAPSHOT: &[u8; 4] = b"MPSN";
pub const BINARY_FORMAT_VERSION: u32 = 2;
pub const EXT_SESSION: &str = "mprs";
pub const EXT_SNAPSHOT: &str = "mpsn";
pub const LEGACY_EXT_SESSION: &str = "mpulse-session";
pub const LEGACY_EXT_SNAPSHOT: &str = "mpulse-snap";

const HEADER_LEN: usize = 20;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredChartPoint {
    pub t_ms: u64,
    pub hashrate_ghs: f64,
    pub board_temps: Vec<f32>,
    pub power_w: Option<f32>,
    pub fan_rpm: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BinarySessionMeta {
    pub format_version: u32,
    pub kind: MpulseKind,
    pub miner_ip: String,
    pub driver_id: String,
    pub poll_rate_hz: Option<u32>,
    pub recorder_tier: SubscriptionTier,
    pub recorded_at: Option<String>,
    pub saved_at: Option<String>,
    pub frame_count: u32,
}

#[derive(Debug, Clone)]
pub struct LoadedBinarySession {
    pub meta: BinarySessionMeta,
    pub chart_points: Vec<StoredChartPoint>,
    pub timeline_ms: Vec<u64>,
    pub frames: Vec<MpulseFrame>,
}

pub fn is_binary_mpulse(bytes: &[u8]) -> bool {
    bytes.len() >= 4
        && (bytes.starts_with(MAGIC_SESSION) || bytes.starts_with(MAGIC_SNAPSHOT))
}

pub fn extension_for_kind(kind: MpulseKind) -> &'static str {
    match kind {
        MpulseKind::Session => EXT_SESSION,
        MpulseKind::Snapshot => EXT_SNAPSHOT,
    }
}

pub fn save_binary_mpulse(path: &Path, file: &MpulseFile) -> Result<(), MinerPulseError> {
    let chart_points = file
        .frames
        .iter()
        .map(|frame| chart_point_from_snapshot(&frame.snapshot, frame.t_ms))
        .collect::<Vec<_>>();
    let meta = BinarySessionMeta {
        format_version: BINARY_FORMAT_VERSION,
        kind: file.kind,
        miner_ip: file.miner_ip.clone(),
        driver_id: file.driver_id.clone(),
        poll_rate_hz: file.poll_rate_hz,
        recorder_tier: file.recorder_tier,
        recorded_at: file.recorded_at.clone(),
        saved_at: file.saved_at.clone(),
        frame_count: file.frames.len() as u32,
    };
    let frames = file.frames.clone();

    let magic = match file.kind {
        MpulseKind::Session => MAGIC_SESSION,
        MpulseKind::Snapshot => MAGIC_SNAPSHOT,
    };
    let meta_bytes = rmp_serde::to_vec_named(&meta).map_err(|_| io_error())?;
    let chart_bytes = rmp_serde::to_vec_named(&chart_points).map_err(|_| io_error())?;
    let frame_bytes = rmp_serde::to_vec_named(&frames).map_err(|_| io_error())?;
    let mut frames_gz = GzEncoder::new(Vec::new(), Compression::fast());
    frames_gz
        .write_all(&frame_bytes)
        .map_err(|_| io_error())?;
    let frames_gz = frames_gz.finish().map_err(|_| io_error())?;

    let mut payload = Vec::with_capacity(
        HEADER_LEN + meta_bytes.len() + chart_bytes.len() + frames_gz.len(),
    );
    payload.extend_from_slice(magic);
    payload.extend_from_slice(&BINARY_FORMAT_VERSION.to_le_bytes());
    payload.extend_from_slice(&(meta_bytes.len() as u32).to_le_bytes());
    payload.extend_from_slice(&(chart_bytes.len() as u32).to_le_bytes());
    payload.extend_from_slice(&(frames_gz.len() as u32).to_le_bytes());
    payload.extend_from_slice(&meta_bytes);
    payload.extend_from_slice(&chart_bytes);
    payload.extend_from_slice(&frames_gz);

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|_| io_error())?;
    }
    std::fs::write(path, payload).map_err(|_| io_error())
}

pub fn load_binary_mpulse(path: &Path) -> Result<LoadedBinarySession, MinerPulseError> {
    let bytes = std::fs::read(path).map_err(|_| io_error())?;
    load_binary_mpulse_bytes(&bytes)
}

pub fn load_binary_mpulse_bytes(bytes: &[u8]) -> Result<LoadedBinarySession, MinerPulseError> {
    if bytes.len() < HEADER_LEN || !is_binary_mpulse(bytes) {
        return Err(MinerPulseError::with_code(crate::error::ErrorCode::ParseFailed));
    }
    if bytes.len() > crate::mpulse::MAX_SESSION_FILE_BYTES {
        return Err(MinerPulseError::with_code(crate::error::ErrorCode::InvalidInput));
    }

    let version = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
    if version != BINARY_FORMAT_VERSION {
        return Err(MinerPulseError::with_code(crate::error::ErrorCode::ParseFailed));
    }
    let meta_len = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
    let chart_len = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    let frames_len = u32::from_le_bytes(bytes[16..20].try_into().unwrap()) as usize;
    let expected = HEADER_LEN
        .checked_add(meta_len)
        .and_then(|v| v.checked_add(chart_len))
        .and_then(|v| v.checked_add(frames_len))
        .ok_or_else(|| MinerPulseError::with_code(crate::error::ErrorCode::ParseFailed))?;
    if bytes.len() != expected {
        return Err(MinerPulseError::with_code(crate::error::ErrorCode::ParseFailed));
    }

    let meta_start = HEADER_LEN;
    let chart_start = meta_start + meta_len;
    let frames_start = chart_start + chart_len;
    let meta: BinarySessionMeta = rmp_serde::from_slice(&bytes[meta_start..chart_start])
        .map_err(|_| MinerPulseError::with_code(crate::error::ErrorCode::ParseFailed))?;
    let chart_points: Vec<StoredChartPoint> =
        rmp_serde::from_slice(&bytes[chart_start..frames_start])
            .map_err(|_| MinerPulseError::with_code(crate::error::ErrorCode::ParseFailed))?;

    let mut decoder = GzDecoder::new(&bytes[frames_start..]);
    let mut frame_bytes = Vec::new();
    decoder
        .read_to_end(&mut frame_bytes)
        .map_err(|_| MinerPulseError::with_code(crate::error::ErrorCode::ParseFailed))?;
    let mut frames: Vec<MpulseFrame> = rmp_serde::from_slice(&frame_bytes)
        .map_err(|_| MinerPulseError::with_code(crate::error::ErrorCode::ParseFailed))?;
    refresh_loaded_binary_frames(&mut frames);

    let timeline_ms = frames.iter().map(|frame| frame.t_ms).collect::<Vec<_>>();
    Ok(LoadedBinarySession {
        meta,
        chart_points,
        timeline_ms,
        frames,
    })
}

fn refresh_loaded_binary_frames(frames: &mut [MpulseFrame]) {
    use crate::drivers::avalon::refresh_avalon_board_chips_from_raw_log;
    for frame in frames {
        let raw_log = if !frame.raw_log.is_empty() {
            frame.raw_log.clone()
        } else if !frame.snapshot.raw_log.is_empty() {
            frame.snapshot.raw_log.clone()
        } else {
            continue;
        };
        refresh_avalon_board_chips_from_raw_log(&mut frame.snapshot, Some(&raw_log));
    }
}

fn io_error() -> MinerPulseError {
    MinerPulseError::with_code(crate::error::ErrorCode::IoError)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{HashrateStats, MinerIdentity, MinerSnapshot, MinerVendor};

    fn sample_snapshot() -> MinerSnapshot {
        MinerSnapshot {
            identity: MinerIdentity {
                vendor: MinerVendor::Antminer,
                model: "S19".into(),
                firmware: "stock".into(),
                driver_id: "antminer".into(),
                ..Default::default()
            },
            hashrate: HashrateStats {
                current_ghs: 110_000.0,
                avg_ghs: 109_000.0,
                avg5s_ghs: 110_000.0,
                per_board_ghs: vec![55_000.0, 55_000.0],
            },
            thermal: crate::model::ThermalStats {
                per_board_max_c: vec![68.0, 70.0],
                ..Default::default()
            },
            power: crate::model::PowerStats {
                watts: Some(3250.0),
                ..Default::default()
            },
            status: "mining".into(),
            ..Default::default()
        }
    }

    #[test]
    fn roundtrips_binary_session() {
        let dir = std::env::temp_dir().join(format!("mpulse-bin-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(format!("test.{EXT_SESSION}"));

        let mut file = MpulseFile::new_session("192.168.0.35", "antminer", SubscriptionTier::Service, 1);
        file.push_recorded_frame(0, sample_snapshot());
        file.push_recorded_frame(1000, sample_snapshot());

        save_binary_mpulse(&path, &file).unwrap();
        let loaded = load_binary_mpulse(&path).unwrap();
        assert_eq!(loaded.meta.kind, MpulseKind::Session);
        assert_eq!(loaded.frames.len(), 2);
        assert_eq!(loaded.chart_points.len(), 2);
        assert_eq!(loaded.timeline_ms, vec![0, 1000]);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
