use minerpulse_core::{
    import_file_content, open_mpulse_file, ErrorResponse, LoadedBinarySession, MpulseFrame,
    MpulseKind, StoredChartPoint, EXT_SESSION, EXT_SNAPSHOT, LEGACY_EXT_SESSION,
    LEGACY_EXT_SNAPSHOT,
};
use serde::Serialize;
use std::fs;
use std::path::Path;
use std::sync::Mutex;

pub struct SessionStore {
    inner: Mutex<Option<LoadedBinarySession>>,
    path: Mutex<Option<String>>,
}

impl SessionStore {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(None),
            path: Mutex::new(None),
        }
    }

    pub fn close(&self) {
        *self.inner.lock().unwrap() = None;
        *self.path.lock().unwrap() = None;
    }

    pub fn get_frame(&self, index: usize) -> Result<MpulseFrame, ErrorResponse> {
        let guard = self.inner.lock().unwrap();
        let session = guard.as_ref().ok_or_else(session_not_open)?;
        session
            .frames
            .get(index)
            .cloned()
            .ok_or_else(|| ErrorResponse {
                code: minerpulse_core::ErrorCode::InvalidInput,
                args: None,
            })
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct OpenedSessionPayload {
    pub miner_ip: String,
    pub driver_id: String,
    pub poll_rate_hz: Option<u32>,
    pub frame_count: u32,
    pub timeline_ms: Vec<u64>,
    pub chart_points: Vec<StoredChartPoint>,
    pub file_label: String,
}

impl OpenedSessionPayload {
    fn from_loaded(opened: &LoadedBinarySession, path: &Path) -> Self {
        Self {
            miner_ip: opened.meta.miner_ip.clone(),
            driver_id: opened.meta.driver_id.clone(),
            poll_rate_hz: opened.meta.poll_rate_hz,
            frame_count: opened.meta.frame_count,
            timeline_ms: opened.timeline_ms.clone(),
            chart_points: opened.chart_points.clone(),
            file_label: path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("session")
                .to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum OpenMinerFileResponse {
    Session {
        miner_ip: String,
        driver_id: String,
        poll_rate_hz: Option<u32>,
        frame_count: u32,
        timeline_ms: Vec<u64>,
        chart_points: Vec<StoredChartPoint>,
        file_label: String,
    },
    Snapshot {
        snapshot: minerpulse_core::MinerSnapshot,
        source_label: String,
        miner_ip: Option<String>,
    },
    Log {
        snapshot: minerpulse_core::MinerSnapshot,
        source_label: String,
        miner_ip: Option<String>,
    },
}

pub fn parse_miner_file(path: &Path) -> Result<ParseMinerFileResult, ErrorResponse> {
    let extension = path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase());

    if matches!(
        extension.as_deref(),
        Some(ext) if ext == EXT_SESSION || ext == LEGACY_EXT_SESSION || ext == "mpulse"
    ) {
        return parse_mpulse_path(path);
    }

    if matches!(
        extension.as_deref(),
        Some(ext) if ext == EXT_SNAPSHOT || ext == LEGACY_EXT_SNAPSHOT
    ) {
        return parse_snapshot_path(path);
    }

    if is_log_extension(extension.as_deref()) {
        return parse_log_file(path);
    }

    let bytes = fs::read(path).map_err(|_| io_error())?;
    if minerpulse_core::is_binary_mpulse(&bytes) {
        return parse_mpulse_path(path);
    }

    parse_log_file(path)
}

pub enum ParseMinerFileResult {
    Session(LoadedBinarySession),
    Snapshot {
        snapshot: minerpulse_core::MinerSnapshot,
        source_label: String,
        miner_ip: Option<String>,
    },
    Log {
        snapshot: minerpulse_core::MinerSnapshot,
        source_label: String,
        miner_ip: Option<String>,
    },
}

impl SessionStore {
    pub fn install_session(
        &self,
        opened: LoadedBinarySession,
        path: &Path,
    ) -> OpenedSessionPayload {
        let payload = OpenedSessionPayload::from_loaded(&opened, path);
        *self.inner.lock().unwrap() = Some(opened);
        *self.path.lock().unwrap() = Some(path.to_string_lossy().to_string());
        payload
    }
}

fn parse_mpulse_path(path: &Path) -> Result<ParseMinerFileResult, ErrorResponse> {
    let opened = open_mpulse_file(path).map_err(|e| ErrorResponse::from(&e))?;
    if opened.meta.kind == MpulseKind::Session && !opened.frames.is_empty() {
        return Ok(ParseMinerFileResult::Session(opened));
    }
    parse_snapshot_from_opened(path, &opened)
}

fn parse_snapshot_path(path: &Path) -> Result<ParseMinerFileResult, ErrorResponse> {
    let opened = open_mpulse_file(path).map_err(|e| ErrorResponse::from(&e))?;
    parse_snapshot_from_opened(path, &opened)
}

fn parse_snapshot_from_opened(
    path: &Path,
    opened: &LoadedBinarySession,
) -> Result<ParseMinerFileResult, ErrorResponse> {
    let frame = opened.frames.first().ok_or_else(|| ErrorResponse {
        code: minerpulse_core::ErrorCode::ParseFailed,
        args: None,
    })?;
    Ok(ParseMinerFileResult::Snapshot {
        snapshot: frame.snapshot.clone(),
        source_label: path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("snapshot")
            .to_string(),
        miner_ip: Some(opened.meta.miner_ip.clone()),
    })
}

fn parse_log_file(path: &Path) -> Result<ParseMinerFileResult, ErrorResponse> {
    let meta = fs::metadata(path).map_err(|_| io_error())?;
    if meta.len() as usize > minerpulse_core::MAX_IMPORT_BYTES {
        return Err(ErrorResponse {
            code: minerpulse_core::ErrorCode::InvalidInput,
            args: None,
        });
    }
    let content = fs::read_to_string(path).map_err(|_| io_error())?;
    let filename = path
        .file_name()
        .and_then(|name| name.to_str())
        .map(str::to_string);
    let result =
        import_file_content(&content, filename.as_deref()).map_err(|e| ErrorResponse::from(&e))?;
    Ok(ParseMinerFileResult::Log {
        snapshot: result.snapshot,
        source_label: result.source_label,
        miner_ip: result.miner_ip,
    })
}

fn is_log_extension(extension: Option<&str>) -> bool {
    matches!(extension, Some("txt" | "log" | "json"))
}

fn session_not_open() -> ErrorResponse {
    ErrorResponse {
        code: minerpulse_core::ErrorCode::InvalidInput,
        args: None,
    }
}

fn io_error() -> ErrorResponse {
    ErrorResponse {
        code: minerpulse_core::ErrorCode::IoError,
        args: None,
    }
}
