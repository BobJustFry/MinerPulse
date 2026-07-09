use minerpulse_core::ErrorCode;
use serde::Serialize;
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use tauri::{AppHandle, Manager};

use crate::license::LicenseState;
use crate::ErrorResponse;

const LOG_FILE: &str = "minerpulse-diagnostic.log";
const MAX_LOG_BYTES: u64 = 2 * 1024 * 1024;

static APP_HANDLE: OnceLock<AppHandle> = OnceLock::new();

pub struct DiagnosticLogState {
    path: PathBuf,
    file: Mutex<()>,
}

impl DiagnosticLogState {
    pub fn new(app: &AppHandle) -> Result<Self, String> {
        let dir = app
            .path()
            .app_data_dir()
            .map_err(|e| e.to_string())?
            .join("diagnostics");
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        Ok(Self {
            path: dir.join(LOG_FILE),
            file: Mutex::new(()),
        })
    }

    fn write_line(&self, line: &str) {
        let _guard = self.file.lock().unwrap_or_else(|e| e.into_inner());
        if self.path.metadata().map(|m| m.len()).unwrap_or(0) > MAX_LOG_BYTES {
            let backup = self.path.with_extension("log.1");
            let _ = fs::rename(&self.path, backup);
        }
        if let Ok(mut file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
        {
            let _ = writeln!(file, "{line}");
        }
    }
}

pub fn trace_hook(category: &str, event: &str, detail: &str) {
    if let Some(app) = APP_HANDLE.get() {
        if let Some(log) = app.try_state::<DiagnosticLogState>() {
            log.write_line(&format_line("TRACE", category, event, detail));
        }
    }
}

fn format_line(level: &str, category: &str, event: &str, detail: &str) -> String {
    let ts = chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
    if detail.is_empty() {
        format!("{ts} [{level}] {category} {event}")
    } else {
        format!("{ts} [{level}] {category} {event} | {detail}")
    }
}

pub fn event(app: &AppHandle, level: &str, category: &str, event: &str, detail: &str) {
    if let Some(state) = app.try_state::<DiagnosticLogState>() {
        state.write_line(&format_line(level, category, event, detail));
    }
}

pub fn init(app: &AppHandle) -> Result<(), String> {
    let state = DiagnosticLogState::new(app)?;
    let path = state.path.display().to_string();
    app.manage(state);
    minerpulse_core::set_trace_hook(trace_hook);
    let _ = APP_HANDLE.set(app.clone());
    event(app, "INFO", "app", "diag_init", &path);
    Ok(())
}

fn build_zip(
    log_path: &Path,
    archive_path: &Path,
    manifest: &serde_json::Value,
) -> Result<u64, String> {
    let file = fs::File::create(archive_path).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipWriter::new(std::io::BufWriter::new(file));
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    zip.start_file(LOG_FILE, options)
        .map_err(|e| e.to_string())?;
    let mut log_bytes = Vec::new();
    if log_path.exists() {
        fs::File::open(log_path)
            .and_then(|mut f| f.read_to_end(&mut log_bytes))
            .map_err(|e| e.to_string())?;
    }
    zip.write_all(&log_bytes).map_err(|e| e.to_string())?;

    zip.start_file("manifest.json", options)
        .map_err(|e| e.to_string())?;
    let manifest_bytes = serde_json::to_vec_pretty(manifest).map_err(|e| e.to_string())?;
    zip.write_all(&manifest_bytes).map_err(|e| e.to_string())?;

    zip.finish().map_err(|e| e.to_string())?;
    fs::metadata(archive_path)
        .map(|m| m.len())
        .map_err(|e| e.to_string())
}

fn api_base() -> &'static str {
    option_env!("MINERPULSE_LICENSE_API_URL").unwrap_or("https://api.mpulse.bob4.fun")
}

#[derive(Debug, Serialize)]
pub struct UploadDiagnosticLogResponse {
    pub id: String,
    pub filename: String,
}

#[tauri::command]
pub async fn upload_diagnostic_log(
    app: AppHandle,
    local_filename: String,
    timezone: String,
) -> Result<UploadDiagnosticLogResponse, ErrorResponse> {
    let token = app
        .state::<LicenseState>()
        .access_token()
        .ok_or_else(|| ErrorResponse {
            code: ErrorCode::InvalidInput,
            args: None,
        })?;

    let log_state = app.state::<DiagnosticLogState>();
    let log_path = log_state.path.clone();
    let hwid = app.state::<LicenseState>().hwid();
    let (app_version, app_build) = app_version_meta();

    event(
        &app,
        "INFO",
        "upload",
        "start",
        &format!("file={local_filename} tz={timezone}"),
    );

    let safe_name = local_filename
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>();
    let archive_name = if safe_name.ends_with(".zip") {
        safe_name
    } else {
        format!("{safe_name}.zip")
    };

    let temp_dir = app.path().temp_dir().map_err(|_| ErrorResponse {
        code: ErrorCode::IoError,
        args: None,
    })?;
    let archive_path = temp_dir.join(&archive_name);

    let manifest = serde_json::json!({
        "hwid": hwid,
        "app_version": app_version,
        "app_build": app_build,
        "timezone": timezone,
        "uploaded_at_utc": chrono::Utc::now().to_rfc3339(),
    });

    let _size_bytes =
        build_zip(&log_path, &archive_path, &manifest).map_err(|_| ErrorResponse {
            code: ErrorCode::IoError,
            args: None,
        })?;

    let bytes = fs::read(&archive_path).map_err(|_| ErrorResponse {
        code: ErrorCode::IoError,
        args: None,
    })?;
    let _ = fs::remove_file(&archive_path);

    let part = reqwest::multipart::Part::bytes(bytes)
        .file_name(archive_name.clone())
        .mime_str("application/zip")
        .map_err(|_| ErrorResponse {
            code: ErrorCode::InvalidInput,
            args: None,
        })?;

    let form = reqwest::multipart::Form::new()
        .part("file", part)
        .text("filename", archive_name.clone())
        .text("timezone", timezone)
        .text("hwid", hwid.clone())
        .text("app_version", app_version)
        .text("app_build", app_build.to_string());

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|_| ErrorResponse {
            code: ErrorCode::InvalidInput,
            args: None,
        })?;

    let response = client
        .post(format!("{}/v1/account/logs", api_base()))
        .bearer_auth(token)
        .multipart(form)
        .send()
        .await
        .map_err(|e| {
            event(&app, "ERROR", "upload", "http_fail", &e.to_string());
            ErrorResponse {
                code: ErrorCode::ConnFailed,
                args: None,
            }
        })?;

    if !response.status().is_success() {
        let status = response.status().to_string();
        let body = response.text().await.unwrap_or_default();
        event(
            &app,
            "ERROR",
            "upload",
            "api_fail",
            &format!("status={status} body={body}"),
        );
        return Err(ErrorResponse {
            code: ErrorCode::InvalidInput,
            args: None,
        });
    }

    #[derive(serde::Deserialize)]
    struct ApiResponse {
        id: String,
        filename: String,
    }

    let parsed: ApiResponse = response.json().await.map_err(|_| ErrorResponse {
        code: ErrorCode::ParseFailed,
        args: None,
    })?;

    event(
        &app,
        "INFO",
        "upload",
        "ok",
        &format!("id={} file={}", parsed.id, parsed.filename),
    );

    Ok(UploadDiagnosticLogResponse {
        id: parsed.id,
        filename: parsed.filename,
    })
}

fn app_version_meta() -> (String, u32) {
    let meta: serde_json::Value =
        serde_json::from_str(env!("MINERPULSE_VERSION_JSON")).unwrap_or(serde_json::json!({}));
    let version = meta
        .get("version")
        .and_then(|v| v.as_str())
        .unwrap_or("0.0.0")
        .to_string();
    let build = meta.get("build").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
    (version, build)
}
