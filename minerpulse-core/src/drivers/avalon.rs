use super::avalon_cgminer::{is_avalon_cgminer_dump, parse_avalon_cgminer_dump};
use super::avalon_chips::{parse_avalon_board_chips, parse_bracket_u32s};
use super::parse::{get_parameter, get_parameter_bracket, parse_f64, parse_i32, parse_u64};
use super::MinerDriver;
use crate::error::MinerPulseError;
use crate::model::{
    BoardChipMap, BoardStats, FanStats, HashrateStats, MinerIdentity, MinerSnapshot, MinerVendor,
    PoolInfo, PowerStats, ThermalStats,
};
use crate::tcp::TcpCgminerClient;

pub struct AvalonDriver;

impl MinerDriver for AvalonDriver {
    fn id(&self) -> &'static str {
        "avalon"
    }

    fn detect(stats_response: &str) -> bool
    where
        Self: Sized,
    {
        if is_avalon_estats_log(stats_response) || is_avalon_cgminer_dump(stats_response) {
            return true;
        }
        if let Some(id) = get_parameter(stats_response, "ID") {
            if id.contains("AV") {
                return true;
            }
        }
        get_parameter(stats_response, "Ver").is_some()
            || get_parameter_bracket(stats_response, "Ver[").is_some()
    }

    fn fetch_snapshot(
        &self,
        client: &TcpCgminerClient,
        host: &str,
        port: u16,
        _options: &crate::fetch_options::FetchOptions,
    ) -> Result<MinerSnapshot, MinerPulseError> {
        let pools_raw = client
            .send_receive(host, port, "pools", "", true)
            .or_else(|_| client.send_receive(host, port, "pools", "", false))
            .unwrap_or_default();

        let raw = client.send_receive(host, port, "estats+lcd", "", false)?;
        if is_avalon_estats_log(&raw) {
            return Ok(parse_estats(&raw, &pools_raw));
        }

        let stats = client
            .send_receive(host, port, "stats", "", true)
            .or_else(|_| client.send_receive(host, port, "stats", "", false))
            .unwrap_or_default();
        if is_avalon_cgminer_dump(&stats) {
            if let Some(mut snapshot) = parse_avalon_cgminer_dump(&stats) {
                if snapshot.pools.is_empty() && !pools_raw.is_empty() {
                    snapshot.pools = parse_avalon_pools(&pools_raw);
                    snapshot.raw_log.push_str("\n--- pools ---\n");
                    snapshot.raw_log.push_str(&pools_raw);
                }
                return Ok(snapshot);
            }
        }

        Ok(parse_estats(&raw, &pools_raw))
    }
}

fn is_avalon_estats_log(raw: &str) -> bool {
    raw.contains("CMD=estats")
        || raw.contains("MM ID0=")
        || raw.contains("'MM ID0':")
        || raw.contains("\"MM ID0\":")
        || raw.contains("ID=AVA")
}

fn flatten_log(raw: &str) -> String {
    raw.replace('\r', "")
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn parse_bracket_floats(text: &str, key: &str) -> Vec<f64> {
    get_parameter_bracket(text, key)
        .map(|value| {
            value
                .split_whitespace()
                .filter_map(|part| parse_f64(part))
                .collect()
        })
        .unwrap_or_default()
}

fn parse_ataopts_voltage_level(raw: &str) -> Option<u32> {
    let marker = "--avalon10-voltage-level";
    let rest = raw.split(marker).nth(1)?;
    rest.split_whitespace()
        .next()
        .and_then(|part| parse_u64(part).map(|v| v as u32))
}

fn avalon_board_tuning(text: &str, slot: usize) -> (Vec<u32>, Vec<u32>, Option<u32>) {
    let freq_domains = parse_bracket_u32s(text, &format!("SF{slot}["));
    let freq_bands = parse_bracket_u32s(text, &format!("ATABD{slot}["));
    let voltage_level = get_parameter_bracket(text, &format!("ATAOPTS{slot}["))
        .and_then(|raw| parse_ataopts_voltage_level(&raw));
    (freq_domains, freq_bands, voltage_level)
}

fn build_avalon_boards(
    flattened: &str,
    board_count: usize,
    hashrate: &HashrateStats,
    thermal: &ThermalStats,
    fans: &FanStats,
    fan_count_matches_boards: bool,
) -> Vec<BoardStats> {
    if board_count == 0 {
        return Vec::new();
    }

    (0..board_count)
        .map(|index| {
            let (freq_domains, freq_bands, voltage_level) = avalon_board_tuning(flattened, index);
            BoardStats {
                label: format!("Board {}", index + 1),
                hashrate_ghs: hashrate.per_board_ghs.get(index).copied(),
                temp_c: thermal
                    .per_board_max_c
                    .get(index)
                    .copied()
                    .or_else(|| {
                        thermal
                            .per_chip_c
                            .get(index)
                            .map(|value| *value as f64)
                    }),
                fan_rpm: if fan_count_matches_boards {
                    fans.rpm.get(index).copied()
                } else {
                    None
                },
                status: String::new(),
                freq_domains_mhz: freq_domains,
                freq_bands_mhz: freq_bands,
                voltage_level,
                ..Default::default()
            }
        })
        .collect()
}

fn enrich_avalon_boards(
    boards: &mut [BoardStats],
    board_chips: &[BoardChipMap],
    eeprom_counts: &[u32],
) {
    for (index, board) in boards.iter_mut().enumerate() {
        if let Some(count) = eeprom_counts.get(index).copied() {
            if count > 0 {
                board.effective_chips = Some(count);
            }
        }
        if let Some(chip_board) = board_chips.iter().find(|item| item.slot as usize == index) {
            if chip_board.chips.is_empty() {
                continue;
            }
            let mut min_temp = i32::MAX;
            let mut max_temp = i32::MIN;
            let mut sum = 0i64;
            for chip in &chip_board.chips {
                min_temp = min_temp.min(chip.temp_c);
                max_temp = max_temp.max(chip.temp_c);
                sum += chip.temp_c as i64;
            }
            let count = chip_board.chips.len() as i64;
            board.chip_temp_min_c = Some(min_temp as f64);
            board.chip_temp_max_c = Some(max_temp as f64);
            board.chip_temp_avg_c = Some(sum as f64 / count as f64);
        }
    }
}

fn avalon_model_from_firmware(firmware: &str) -> String {
    if firmware.is_empty() {
        return "Avalon".to_string();
    }
    if firmware.contains("Pro") || firmware.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        return format!("Avalon {firmware}");
    }
    format!("Avalon-{firmware}")
}

fn parse_avalon_status(text: &str) -> String {
    if let Some(raw) = get_parameter_bracket(text, "SYSTEMSTATU[") {
        let first_line = raw.lines().next().unwrap_or(&raw).trim();
        if let Some(work) = first_line.strip_prefix("Work:") {
            return work.trim().to_string();
        }
        if !first_line.is_empty() {
            return first_line.to_string();
        }
    }
    get_parameter(text, "Work").unwrap_or_else(|| "Unknown".to_string())
}

fn parse_avalon_pools_from_lcd(raw: &str) -> Vec<PoolInfo> {
    let url = get_parameter(raw, "Current Pool");
    let worker = get_parameter(raw, "User");
    url.map(|pool_url| {
        vec![PoolInfo {
            url: pool_url,
            worker: worker.unwrap_or_default(),
            status: "Active".to_string(),
            accepted: 0,
            rejected: 0,
        }]
    })
    .unwrap_or_default()
}

pub fn parse_avalon_estats_log(raw: &str) -> MinerSnapshot {
    let flattened = flatten_log(raw);
    let firmware = get_parameter_bracket(&flattened, "Ver[")
        .or_else(|| get_parameter(&flattened, "Ver"))
        .unwrap_or_default();
    let model = avalon_model_from_firmware(&firmware);

    let mut hashrate = HashrateStats::default();
    hashrate.avg5s_ghs = get_parameter(raw, "GHS 5s")
        .and_then(|s| parse_f64(&s))
        .or_else(|| parse_f64(&get_parameter_bracket(&flattened, "GHSspd[").unwrap_or_default()))
        .unwrap_or(0.0);
    hashrate.avg_ghs = get_parameter(raw, "GHS av")
        .and_then(|s| parse_f64(&s))
        .or_else(|| {
            get_parameter_bracket(&flattened, "GHSavg[")
                .and_then(|s| parse_f64(&s))
        })
        .unwrap_or(0.0);
    hashrate.current_ghs = hashrate.avg5s_ghs;
    hashrate.per_board_ghs = parse_bracket_floats(&flattened, "MGHS[");

    let mut thermal = ThermalStats::default();
    thermal.inlet_c = get_parameter_bracket(&flattened, "Temp[")
        .and_then(|s| parse_f64(&s))
        .or_else(|| get_parameter(raw, "Temperature").and_then(|s| parse_f64(&s)));
    thermal.per_board_max_c = parse_bracket_floats(&flattened, "MTmax[");
    if thermal.per_board_max_c.is_empty() {
        if let Some(tmax) = get_parameter_bracket(&flattened, "TMax[")
            .or_else(|| get_parameter(&flattened, "TMax"))
            .and_then(|s| parse_f64(&s))
        {
            thermal.per_board_max_c.push(tmax);
        }
    }
    let board_avg = parse_bracket_floats(&flattened, "MTavg[");
    thermal.per_chip_c = board_avg
        .into_iter()
        .map(|value| value.round() as i32)
        .collect();

    let mut fans = FanStats::default();
    for index in 1..=8 {
        let key = format!("Fan{index}[");
        if let Some(raw_rpm) = get_parameter_bracket(&flattened, &key) {
            if let Some(rpm) = parse_u64(&raw_rpm) {
                fans.rpm.push(rpm as u32);
            }
        }
    }

    let mut power = PowerStats::default();
    power.voltage = get_parameter_bracket(&flattened, "Vo[")
        .and_then(|s| parse_f64(&s))
        .or_else(|| get_parameter(&flattened, "Voltage").and_then(|s| parse_f64(&s)));
    power.watts = get_parameter_bracket(&flattened, "MPO[")
        .and_then(|s| parse_f64(&s))
        .or_else(|| get_parameter(&flattened, "Power").and_then(|s| parse_f64(&s)));

    let status = parse_avalon_status(&flattened);
    let uptime_sec = get_parameter(&flattened, "Elapsed")
        .and_then(|s| parse_u64(&s))
        .or_else(|| get_parameter(raw, "Elapsed").and_then(|s| parse_u64(&s)));

    let hw_errors = get_parameter_bracket(&flattened, "HW[")
        .and_then(|s| parse_u64(&s))
        .or_else(|| get_parameter(&flattened, "Hardware Errors").and_then(|s| parse_u64(&s)));

    let core_chip = get_parameter_bracket(&flattened, "Core[");
    let work_mode = get_parameter_bracket(&flattened, "WORKMODE[")
        .and_then(|s| parse_u64(&s))
        .map(|v| v as u32);
    let ecmm = get_parameter_bracket(&flattened, "ECMM[")
        .and_then(|s| parse_u64(&s));

    let board_chips = parse_avalon_board_chips(
        &flattened,
        &firmware,
        core_chip.as_deref(),
    );

    let board_count = hashrate
        .per_board_ghs
        .len()
        .max(thermal.per_board_max_c.len())
        .max(thermal.per_chip_c.len())
        .max(board_chips.len());
    let fan_count_matches_boards = !fans.rpm.is_empty() && fans.rpm.len() == board_count;

    let boards = build_avalon_boards(
        &flattened,
        board_count,
        &hashrate,
        &thermal,
        &fans,
        fan_count_matches_boards,
    );

    let eeprom_counts = parse_bracket_u32s(&flattened, "EEPROM[");
    let mut boards = boards;
    enrich_avalon_boards(&mut boards, &board_chips, &eeprom_counts);

    let pools = parse_avalon_pools_from_lcd(raw);

    MinerSnapshot {
        identity: MinerIdentity {
            vendor: MinerVendor::Avalon,
            model,
            firmware: firmware.clone(),
            driver_id: "avalon".to_string(),
            core_chip: core_chip.clone(),
        },
        hashrate,
        thermal,
        fans,
        power,
        boards,
        pools,
        shares_accepted: None,
        shares_rejected: None,
        hw_errors,
        raw_log: raw.to_string(),
        status,
        uptime_sec,
        work_mode,
        ecmm,
        board_chips,
        ..Default::default()
    }
}

/// Re-parse Avalon chip maps from embedded log text (e.g. when loading older `.mpulse` snapshots).
pub fn refresh_avalon_board_chips_from_raw_log(
    snapshot: &mut MinerSnapshot,
    frame_raw_log: Option<&str>,
) {
    if snapshot.identity.driver_id != "avalon"
        && snapshot.identity.vendor != MinerVendor::Avalon
    {
        return;
    }

    let raw = frame_raw_log
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            if snapshot.raw_log.trim().is_empty() {
                None
            } else {
                Some(snapshot.raw_log.as_str())
            }
        });

    let Some(raw) = raw else {
        return;
    };

    let fresh = parse_avalon_cgminer_dump(raw).or_else(|| {
        if raw.contains("CMD=estats")
            || raw.contains("MM ID0=")
            || raw.contains("ID=AVA")
        {
            Some(parse_avalon_estats_log(raw))
        } else {
            None
        }
    });

    let Some(fresh) = fresh else {
        return;
    };

    if fresh.board_chips.is_empty() {
        return;
    }

    snapshot.board_chips = fresh.board_chips;
    sync_avalon_board_chip_stats(snapshot, &fresh.boards);
}

fn sync_avalon_board_chip_stats(snapshot: &mut MinerSnapshot, fresh_boards: &[BoardStats]) {
    for (index, board) in snapshot.boards.iter_mut().enumerate() {
        let Some(fresh) = fresh_boards.get(index) else {
            continue;
        };
        if let Some(value) = fresh.effective_chips {
            board.effective_chips = Some(value);
        }
        if fresh.chip_temp_min_c.is_some() {
            board.chip_temp_min_c = fresh.chip_temp_min_c;
            board.chip_temp_max_c = fresh.chip_temp_max_c;
            board.chip_temp_avg_c = fresh.chip_temp_avg_c;
        }
    }
}

fn parse_avalon_pools(raw: &str) -> Vec<PoolInfo> {
    let trimmed = raw.trim();
    if trimmed.starts_with('{') {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) {
            if let Some(pools) = value.get("POOLS").and_then(|p| p.as_array()) {
                return pools
                    .iter()
                    .filter_map(|pool| {
                        Some(PoolInfo {
                            url: pool.get("URL")?.as_str()?.to_string(),
                            worker: pool
                                .get("User")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string(),
                            status: pool
                                .get("Status")
                                .and_then(|v| v.as_str())
                                .unwrap_or("Unknown")
                                .to_string(),
                            accepted: pool
                                .get("Accepted")
                                .and_then(|v| v.as_u64())
                                .unwrap_or(0),
                            rejected: pool
                                .get("Rejected")
                                .and_then(|v| v.as_u64())
                                .unwrap_or(0),
                        })
                    })
                    .collect();
            }
        }
    }

    Vec::new()
}

pub fn parse_estats(raw: &str, pools_raw: &str) -> MinerSnapshot {
    if is_avalon_estats_log(raw) {
        let mut snapshot = parse_avalon_estats_log(raw);
        if snapshot.pools.is_empty() {
            snapshot.pools = parse_avalon_pools(pools_raw);
        }
        if !pools_raw.is_empty() && !snapshot.raw_log.contains("--- pools ---") {
            snapshot.raw_log.push_str("\n--- pools ---\n");
            snapshot.raw_log.push_str(pools_raw);
        }
        return snapshot;
    }

    let cleaned = raw.replace('\'', "").replace("  ", " ");

    let firmware = get_parameter(&cleaned, "Ver").unwrap_or_default();
    let model = avalon_model_from_firmware(&firmware);

    let mut hashrate = HashrateStats::default();
    if let Some(v) = get_parameter(&cleaned, "GHS 5s").and_then(|s| parse_f64(&s)) {
        hashrate.avg5s_ghs = v;
        hashrate.current_ghs = v;
    }
    if let Some(v) = get_parameter(&cleaned, "GHS av").and_then(|s| parse_f64(&s)) {
        hashrate.avg_ghs = v;
    }

    for i in 0..8 {
        let key = format!("GHS {i}");
        if let Some(v) = get_parameter(&cleaned, &key).and_then(|s| parse_f64(&s)) {
            hashrate.per_board_ghs.push(v);
        }
    }

    let mut thermal = ThermalStats::default();
    thermal.inlet_c = get_parameter(&cleaned, "Temp").and_then(|s| parse_f64(&s));
    if let Some(tmax) = get_parameter(&cleaned, "TMax").and_then(|s| parse_f64(&s)) {
        thermal.per_board_max_c.push(tmax);
    }
    for i in 0..8 {
        let key = format!("TMax[{i}]");
        if let Some(v) = get_parameter(&cleaned, &key).and_then(|s| parse_f64(&s)) {
            if i >= thermal.per_board_max_c.len() {
                thermal.per_board_max_c.push(v);
            }
        }
        let chip_key = format!("TAvg[{i}]");
        if let Some(v) = get_parameter(&cleaned, &chip_key).and_then(|s| parse_i32(&s)) {
            thermal.per_chip_c.push(v);
        }
    }

    let mut fans = FanStats::default();
    for i in 1..=8 {
        let key = format!("Fan{i}");
        if let Some(rpm) = get_parameter(&cleaned, &key).and_then(|s| parse_u64(&s)) {
            fans.rpm.push(rpm as u32);
        }
    }

    let mut power = PowerStats::default();
    power.watts = get_parameter(&cleaned, "Power").and_then(|s| parse_f64(&s));
    power.voltage = get_parameter(&cleaned, "Voltage").and_then(|s| parse_f64(&s));

    let status = get_parameter(&cleaned, "Work").unwrap_or_else(|| "Unknown".to_string());

    let board_count = hashrate
        .per_board_ghs
        .len()
        .max(thermal.per_board_max_c.len())
        .max(fans.rpm.len());

    let boards = (0..board_count)
        .map(|index| BoardStats {
            label: format!("Board {}", index + 1),
            hashrate_ghs: hashrate.per_board_ghs.get(index).copied(),
            temp_c: thermal.per_board_max_c.get(index).copied(),
            fan_rpm: fans.rpm.get(index).copied(),
            status: String::new(),
            ..Default::default()
        })
        .collect();

    let mut raw_log = raw.to_string();
    if !pools_raw.is_empty() {
        raw_log.push_str("\n--- pools ---\n");
        raw_log.push_str(pools_raw);
    }

    MinerSnapshot {
        identity: MinerIdentity {
            vendor: MinerVendor::Avalon,
            model,
            firmware: firmware.clone(),
            driver_id: "avalon".to_string(),
            ..Default::default()
        },
        hashrate,
        thermal,
        fans,
        power,
        boards,
        pools: parse_avalon_pools(pools_raw),
        shares_accepted: get_parameter(&cleaned, "Accepted").and_then(|s| parse_u64(&s)),
        shares_rejected: get_parameter(&cleaned, "Rejected").and_then(|s| parse_u64(&s)),
        hw_errors: get_parameter(&cleaned, "Hardware Errors").and_then(|s| parse_u64(&s)),
        raw_log,
        status,
        uptime_sec: get_parameter(&cleaned, "Elapsed").and_then(|s| parse_u64(&s)),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    #[test]
    fn detects_avalon_by_ver() {
        assert!(AvalonDriver::detect("Ver=1346|Temp=30"));
    }

    #[test]
    fn builds_avalon_board_rows() {
        let sample = "Ver=1346|GHS 0=1200|GHS 1=1180|TMax=72|Fan1=4200|Fan2=4300|Temp=28";
        let snap = parse_estats(sample, "");
        assert_eq!(snap.boards.len(), 2);
        assert_eq!(snap.fans.rpm.len(), 2);
    }

    #[test]
    fn parses_avalon_estats_log_sample() {
        let sample = r#"CMD=estats
ID=AVA100
Elapsed=1797
MM ID0=Ver[1126Pro-S-64-21071301_test] Temp[27] TMax[135] Fan1[1000] Fan2[1100] MGHS[26208.88 27221.62] MTmax[87 135] MTavg[69 68] Vo[358] HW[14] SYSTEMSTATU[Work: In Work]
CMD=lcd
Elapsed=1797
GHS av=53466.99
GHS 5s=57320.03
Current Pool=stratum+tcp://pool.example:443
User=worker1"#;
        let snap = parse_avalon_estats_log(sample);
        assert!((snap.hashrate.avg5s_ghs - 57320.03).abs() < 0.01);
        assert_eq!(snap.boards.len(), 2);
        assert_eq!(snap.fans.rpm.len(), 2);
        assert_eq!(snap.hw_errors, Some(14));
        assert_eq!(snap.pools.len(), 1);
        assert_eq!(snap.status, "In Work");
    }

    #[test]
    fn parses_avalon_chip_map_from_estats_log() {
        let sample = "MM ID0=Ver[1126Pro-S-64-test] Core[A3201] WORKMODE[1] ECMM[4] \
                      SF0[496 516 536 556] ATABD0[496 516 536 556] \
                      ATAOPTS0[--avalon10-freq 496:516:536:556 --avalon10-voltage-level 48 ] \
                      PVT_T0[66 71 87] PVT_V0[326 332 329] MW0[46 52 11] ASICCRC0[0 1 0] \
                      PVT_T1[68 71 69] PVT_V1[327 335 342] MW1[41 37 56] ASICCRC1[0 0 2]";
        let snap = parse_avalon_estats_log(sample);
        assert_eq!(snap.identity.core_chip.as_deref(), Some("A3201"));
        assert_eq!(snap.work_mode, Some(1));
        assert_eq!(snap.ecmm, Some(4));
        assert_eq!(snap.board_chips.len(), 2);
        assert_eq!(snap.boards.len(), 2);
        assert_eq!(snap.boards[0].freq_domains_mhz, vec![496, 516, 536, 556]);
        assert_eq!(snap.boards[0].voltage_level, Some(48));
        assert_eq!(snap.board_chips[0].matrix_id.as_deref(), Some("Matrix_11x"));
        assert_eq!(snap.board_chips[0].chips[1].errors, Some(1));
    }

    #[test]
    fn parses_ataopts_voltage_level() {
        let raw = "--avalon10-freq 464:484:504:524 --avalon10-voltage-level 69 ";
        assert_eq!(parse_ataopts_voltage_level(raw), Some(69));
    }

    #[test]
    fn parses_avalon_1466_board_details() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../OldProject/txt/minerpulse-1782981279760.mpulse");
        if !path.exists() {
            return;
        }
        let content = fs::read_to_string(path).expect("read 1466 sample");
        let mpulse: crate::mpulse::MpulseFile =
            serde_json::from_str(&content).expect("parse mpulse");
        let raw = &mpulse.frames[0].snapshot.raw_log;
        let snap = parse_avalon_estats_log(raw);
        assert!(snap.identity.model.contains("1466"));
        assert_eq!(snap.identity.core_chip.as_deref(), Some("A3198S"));
        assert_eq!(snap.board_chips.len(), 3);
        assert_eq!(snap.board_chips[0].matrix_id.as_deref(), Some("Matrix_176"));
        assert_eq!(snap.board_chips[0].chips.len(), 176);
        assert_eq!(snap.boards[0].effective_chips, Some(176));
        assert!(snap.boards[0].chip_temp_min_c.is_some());
        assert!(snap.boards[0].chip_temp_max_c.is_some());
        assert!(snap.boards[0].chip_temp_avg_c.is_some());
        assert!(
            snap.boards[0].chip_temp_min_c.unwrap()
                <= snap.boards[0].chip_temp_max_c.unwrap()
        );
        let board2 = snap
            .board_chips
            .iter()
            .find(|board| board.slot == 2)
            .expect("board 2 chips");
        assert!(board2.chips.iter().any(|chip| chip.temp_c == -26));
        assert!(board2.chips.iter().any(|chip| chip.temp_c == -273));
    }
}
