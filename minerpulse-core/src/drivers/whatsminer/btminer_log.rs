use crate::model::{BoardChipMap, ChipStats};

use super::layout;

#[derive(Debug, Default)]
struct ParsedSlot {
    id: u32,
    chips: Vec<ChipStats>,
}

pub fn parse_btminer_log(text: &str) -> Vec<BoardChipMap> {
    let mut slots = Vec::new();
    let mut current: Option<ParsedSlot> = None;

    for line in text.lines().map(str::trim) {
        if line.is_empty() || line == "(" || line == ")" {
            continue;
        }
        if let Some(slot) = parse_slot_line(line) {
            if let Some(prev) = current.take() {
                if !prev.chips.is_empty() {
                    slots.push(prev);
                }
            }
            current = Some(slot);
        } else if line.starts_with('C') && line.contains("freq:") && line.contains("temp:") {
            if let Some(slot) = &mut current {
                if let Some(chip) = parse_chip_line(line) {
                    slot.chips.push(chip);
                }
            }
        }
    }

    if let Some(slot) = current {
        if !slot.chips.is_empty() {
            slots.push(slot);
        }
    }

    slots
        .into_iter()
        .map(|slot| {
            let chips_per_domain =
                layout::infer_chips_per_domain(slot.chips.len());
            BoardChipMap {
                slot: slot.id,
                label: format!("SM{}", slot.id),
                chips_per_domain,
                matrix_id: None,
                chips: slot.chips,
            }
        })
        .collect()
}

pub fn parse_btminer_html(html: &str) -> Option<String> {
    let start = html.find(r#"id="syslog">"#)? + 12;
    let end = start + html[start..].find("</textarea>")?;
    Some(html[start..end].to_string())
}

fn parse_slot_line(line: &str) -> Option<ParsedSlot> {
    if line.starts_with("slot:") {
        return Some(parse_slot_header(line));
    }

    let rest = line.strip_prefix("slot")?;
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        return None;
    }
    if !rest[digits.len()..].starts_with(':') {
        return None;
    }
    Some(ParsedSlot {
        id: digits.parse().ok()?,
        chips: Vec::new(),
    })
}

fn parse_slot_header(line: &str) -> ParsedSlot {
    let mut slot = ParsedSlot::default();
    for part in line.split(',').map(str::trim) {
        let Some((key, val)) = part.split_once(':') else {
            continue;
        };
        if key.trim() == "slot" {
            slot.id = val.trim().parse().unwrap_or_default();
        }
    }
    slot
}

fn parse_chip_line(line: &str) -> Option<ChipStats> {
    let id_end = line.find(char::is_whitespace)?;
    let index: u32 = line[1..id_end].parse().ok()?;

    let mut chip = ChipStats {
        index,
        temp_c: 0,
        freq_mhz: None,
        voltage: None,
        errors: None,
        solutions: None,
        crc_errors: None,
        nonce: None,
        repeat_count: None,
        performance_pct: None,
    };

    if let Some(pct_str) = line.split("pct:").nth(1) {
        let parts: Vec<_> = pct_str.split('/').collect();
        let primary = parts
            .first()
            .and_then(|value| value.trim().trim_end_matches('%').parse().ok());
        let secondary = parts
            .get(1)
            .and_then(|value| value.trim().trim_end_matches('%').parse().ok());
        if let (Some(primary), Some(secondary)) = (primary, secondary) {
            chip.performance_pct = Some([primary, secondary]);
        }
    }

    for part in line.split_whitespace() {
        let Some((key, val)) = part.split_once(':') else {
            continue;
        };
        match key {
            "freq" => chip.freq_mhz = val.parse().ok(),
            "vol" => chip.voltage = val.parse().ok(),
            "temp" => chip.temp_c = val.parse().unwrap_or(0),
            "err" | "error" => chip.errors = val.parse().ok(),
            "crc" => chip.crc_errors = val.parse().ok(),
            "nonce" => chip.nonce = val.parse().ok(),
            "repeat" => chip.repeat_count = val.parse().ok(),
            _ => {}
        }
    }

    Some(chip)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_luci_btminerapi_log_format() {
        let sample = r#"
slot0:
(
   slot: 0
   temp: 69.56
   C0  : freq:609  vol:325 temp:60  nonce:2418565  error:40   crc:0
   C1  : freq:635  vol:322 temp:67  nonce:2500039  error:52   crc:0
)
slot1:
(
   slot: 1
   temp: 68.10
   C0  : freq:610  vol:324 temp:61  nonce:2400000  error:41   crc:0
)
"#;
        let boards = parse_btminer_log(sample);
        assert_eq!(boards.len(), 2);
        assert_eq!(boards[0].slot, 0);
        assert_eq!(boards[0].chips.len(), 2);
        assert_eq!(boards[0].chips[0].temp_c, 60);
        assert_eq!(boards[0].chips[0].errors, Some(40));
        assert_eq!(boards[1].slot, 1);
    }

    #[test]
    fn parses_chip_lines_from_btminer_log() {
        let sample = r#"
slot:0, freq:785, temp:92.2, step:1
C0 freq:785 vol:320 temp:85 nonce:100 err:0 crc:0 x:0 repeat:0 pct:98.8%/94.1%
C1 freq:785 vol:320 temp:86 nonce:101 err:0 crc:0 x:0 repeat:0 pct:98.8%/94.1%
slot:1, freq:782, temp:91.1, step:1
C0 freq:782 vol:318 temp:84 nonce:99 err:0 crc:0 x:0 repeat:0 pct:98.8%/94.1%
"#;
        let boards = parse_btminer_log(sample);
        assert_eq!(boards.len(), 2);
        assert_eq!(boards[0].chips.len(), 2);
        assert_eq!(boards[0].chips[0].temp_c, 85);
        assert_eq!(boards[1].slot, 1);
    }

    #[test]
    fn parses_extended_chip_fields_from_btminer_log() {
        let sample = r#"
slot:0, freq:785, temp:92.2, step:1
C110 freq:609 vol:326 temp:60 nonce:42321590 error:878 crc:0 x:0 repeat:31 pct:101.3%/101.4%
"#;
        let boards = parse_btminer_log(sample);
        let chip = &boards[0].chips[0];
        assert_eq!(chip.index, 110);
        assert_eq!(chip.freq_mhz, Some(609));
        assert_eq!(chip.voltage, Some(326));
        assert_eq!(chip.temp_c, 60);
        assert_eq!(chip.nonce, Some(42_321_590));
        assert_eq!(chip.errors, Some(878));
        assert_eq!(chip.crc_errors, Some(0));
        assert_eq!(chip.repeat_count, Some(31));
        assert_eq!(chip.performance_pct, Some([101.3, 101.4]));
    }
}
