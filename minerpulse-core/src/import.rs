use crate::drivers::antminer::{parse_antminer_snapshot, split_antminer_log, AntminerDriver};
use crate::drivers::avalon::{parse_avalon_estats_log, parse_estats, AvalonDriver};
use crate::drivers::avalon_cgminer::{is_avalon_cgminer_dump, parse_avalon_cgminer_dump};
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

fn normalize_import_content(content: &str) -> &str {
    content.trim_start_matches('\u{feff}').trim()
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
    let trimmed = normalize_import_content(content);

    if trimmed.is_empty() {
        return Err(MinerPulseError::with_code(
            crate::error::ErrorCode::ParseFailed,
        ));
    }

    if is_avalon_cgminer_dump(trimmed) {
        if let Some(snapshot) = parse_avalon_cgminer_dump(trimmed) {
            return Ok(ImportResult {
                snapshot,
                source_label: label,
                miner_ip: None,
            });
        }
    }

    if trimmed.starts_with('{') || trimmed.starts_with("{'") {
        if trimmed.contains("--- summary ---")
            || trimmed.contains("--- pools ---")
            || trimmed.contains("--- devs ---")
            || trimmed.contains("--- stats ---")
            || trimmed.contains("\"Type\":\"Antminer")
            || trimmed.contains("\"Type\": \"Antminer")
        {
            let (stats, summary, pools, devs) = split_antminer_log(trimmed);
            if AntminerDriver::detect(&stats)
                || AntminerDriver::detect(&summary)
                || stats.contains("Antminer")
                || summary.contains("Antminer")
            {
                return Ok(ImportResult {
                    snapshot: parse_antminer_snapshot(&stats, &summary, &pools, &devs),
                    source_label: label,
                    miner_ip: None,
                });
            }
        }
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
            snapshot: parse_whatsminer_snapshot(trimmed, "", "", "", "", "", "", Vec::new()),
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

    if is_avalon_cgminer_dump(trimmed) {
        if let Some(snapshot) = parse_avalon_cgminer_dump(trimmed) {
            return Ok(ImportResult {
                snapshot,
                source_label: label.to_string(),
                miner_ip: None,
            });
        }
    }

    Err(MinerPulseError::with_code(
        crate::error::ErrorCode::ParseFailed,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::drivers::whatsminer::classify_whatsminer;
    use std::fs;
    use std::path::PathBuf;

    #[test]
    fn rejects_oversized_import() {
        let huge = "x".repeat(MAX_IMPORT_BYTES + 1);
        assert!(import_file_content(&huge, Some("big.txt")).is_err());
    }

    #[test]
    fn rejects_antminer_summary_as_whatsminer() {
        let sample = r#"{"STATUS":[{"Msg":"Summary","STATUS":"S"}],"SUMMARY":[{"GHS 5s":2863.31,"GHS av":3949.71}]}"#;
        assert!(classify_whatsminer(sample).is_none());
    }

    #[test]
    fn imports_e9_pro_combined_console_log() {
        let raw = r#"{"STATUS":[{"STATUS":"S","Msg":"CGMiner stats"}],"STATS":[{"Type":"Antminer E9 Pro"},{"STATS":2,"GHS 5s":2863.31,"GHS av":3949.71,"total_rateideal":3800.0,"miner_count":2,"fan1":5760,"fan2":5760,"fan3":5760,"fan4":5880,"temp1":58,"temp2":59,"temp2_1":74,"temp2_2":74,"temp_in_chip_1":"66-65-74-73","temp_in_chip_2":"66-65-72-72","chain_rate1":1.990750336,"chain_rate2":1.97701632,"chain_acs1":"oooooooo","chain_acs2":"oooooooo","chain_acn1":8,"chain_acn2":8}]}
--- pools ---
{"POOLS":[{"URL":"stratum+tcp://gate.emcd.network:7878","User":"subbob.6x2x1","Status":"Alive","Accepted":5207,"Rejected":74}]}
--- summary ---
{"SUMMARY":[{"Elapsed":13196,"GHS 5s":2863.31,"GHS av":3949.71,"Accepted":5239,"Rejected":74,"Hardware Errors":2}]}
--- devs ---
{"STATUS":[{"STATUS":"E","Msg":"Invalid command"}]}"#;
        let result = import_file_content(raw, Some("e9.txt")).expect("import");
        assert_eq!(result.snapshot.identity.model, "Antminer E9 Pro");
        assert_eq!(result.snapshot.boards.len(), 2);
        assert_eq!(result.snapshot.fans.rpm.len(), 4);
        assert_eq!(result.snapshot.pools.len(), 1);
        assert_eq!(result.snapshot.hw_errors, Some(2));
        assert!((result.snapshot.hashrate.avg5s_ghs - 2.86331).abs() < 0.01);
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

    #[test]
    fn imports_avalon_1326_cgminer_dump() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../OldProject/txt/1326.txt");
        if !path.exists() {
            return;
        }
        let content = fs::read_to_string(path).expect("read 1326 sample");
        let result = import_file_content(&content, Some("1326.txt")).expect("import 1326");
        assert!(result.snapshot.identity.model.contains("1326"));
        assert_eq!(result.snapshot.boards.len(), 3);
        assert_eq!(result.snapshot.board_chips[0].matrix_id.as_deref(), Some("Matrix_1326"));
    }

    #[test]
    fn imports_avalon_1346_cgminer_dump() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../OldProject/txt/1346.txt");
        if !path.exists() {
            return;
        }
        let content = fs::read_to_string(path).expect("read 1346 sample");
        let result = import_file_content(&content, Some("1346.txt")).expect("import 1346");
        assert!(result.snapshot.identity.model.contains("1346"));
        assert_eq!(result.snapshot.boards.len(), 3);
        assert_eq!(result.snapshot.board_chips[0].matrix_id.as_deref(), Some("Matrix_1346"));
    }
}
