use crate::drivers::antminer::{parse_antminer_snapshot, AntminerDriver};
use crate::drivers::avalon::{parse_avalon_estats_log, parse_estats, AvalonDriver};
use crate::drivers::whatsminer::{classify_whatsminer, parse_whatsminer_snapshot};
use crate::drivers::MinerDriver;
use crate::error::MinerPulseError;
use crate::model::MinerSnapshot;
use crate::mpulse::MpulseFile;

pub const MAX_IMPORT_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone)]
pub struct ImportResult {
    pub snapshot: MinerSnapshot,
    pub source_label: String,
    pub miner_ip: Option<String>,
}

pub fn import_file_content(content: &str, filename: Option<&str>) -> Result<ImportResult, MinerPulseError> {
    if content.len() > MAX_IMPORT_BYTES {
        return Err(MinerPulseError::with_code(
            crate::error::ErrorCode::InvalidInput,
        ));
    }

    let label = filename
        .filter(|name| !name.is_empty())
        .unwrap_or("import.txt")
        .to_string();
    let trimmed = content.trim();

    if trimmed.is_empty() {
        return Err(MinerPulseError::with_code(
            crate::error::ErrorCode::ParseFailed,
        ));
    }

    if trimmed.starts_with('{') {
        return import_json(trimmed, &label);
    }

    if trimmed.contains("CMD=estats")
        || trimmed.contains("MM ID0=")
        || trimmed.contains("ID=AVA")
    {
        return Ok(ImportResult {
            snapshot: parse_avalon_estats_log(trimmed),
            source_label: label,
            miner_ip: None,
        });
    }

    if AntminerDriver::detect(trimmed) {
        return Ok(ImportResult {
            snapshot: parse_antminer_snapshot(trimmed, "", "", ""),
            source_label: label,
            miner_ip: None,
        });
    }

    if classify_whatsminer(trimmed).is_some() {
        return Ok(ImportResult {
            snapshot: parse_whatsminer_snapshot(trimmed, "", ""),
            source_label: label,
            miner_ip: None,
        });
    }

    if AvalonDriver::detect(trimmed) {
        return Ok(ImportResult {
            snapshot: parse_estats(trimmed, ""),
            source_label: label,
            miner_ip: None,
        });
    }

    Err(MinerPulseError::with_code(
        crate::error::ErrorCode::ParseFailed,
    ))
}

fn import_json(trimmed: &str, label: &str) -> Result<ImportResult, MinerPulseError> {
    if let Ok(file) = serde_json::from_str::<MpulseFile>(trimmed) {
        let frame = file
            .frames
            .last()
            .or_else(|| file.frames.first())
            .ok_or(MinerPulseError::with_code(
                crate::error::ErrorCode::ParseFailed,
            ))?;

        return Ok(ImportResult {
            snapshot: frame.snapshot.clone(),
            source_label: label.to_string(),
            miner_ip: Some(file.miner_ip),
        });
    }

    if let Ok(snapshot) = serde_json::from_str::<MinerSnapshot>(trimmed) {
        return Ok(ImportResult {
            snapshot,
            source_label: label.to_string(),
            miner_ip: None,
        });
    }

    if AntminerDriver::detect(trimmed) {
        return Ok(ImportResult {
            snapshot: parse_antminer_snapshot(trimmed, "", "", ""),
            source_label: label.to_string(),
            miner_ip: None,
        });
    }

    if classify_whatsminer(trimmed).is_some() {
        return Ok(ImportResult {
            snapshot: parse_whatsminer_snapshot(trimmed, "", ""),
            source_label: label.to_string(),
            miner_ip: None,
        });
    }

    Err(MinerPulseError::with_code(
        crate::error::ErrorCode::ParseFailed,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    #[test]
    fn rejects_oversized_import() {
        let huge = "x".repeat(MAX_IMPORT_BYTES + 1);
        assert!(import_file_content(&huge, Some("big.txt")).is_err());
    }

    #[test]
    fn imports_avalon_estats_log_by_filename() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../OldProject/txt/amv_20250713 - 224007.txt");
        if !path.exists() {
            return;
        }
        let content = fs::read_to_string(path).expect("read sample log");
        let result = import_file_content(&content, Some("amv.txt")).expect("import");
        assert!(result.snapshot.hashrate.avg5s_ghs > 1000.0);
        assert_eq!(result.snapshot.boards.len(), 2);
        assert_eq!(result.snapshot.fans.rpm.len(), 4);
        assert!(!result.snapshot.pools.is_empty());
    }
}
