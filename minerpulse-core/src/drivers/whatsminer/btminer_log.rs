use crate::model::{BoardChipMap, ChipStats};

#[derive(Debug, Default)]
struct ParsedSlot {
    id: u32,
    chips: Vec<ChipStats>,
}

pub fn parse_btminer_log(text: &str) -> Vec<BoardChipMap> {
    let mut slots = Vec::new();
    let mut current: Option<ParsedSlot> = None;

    for line in text.lines().map(str::trim) {
        if line.starts_with("slot:") {
            if let Some(slot) = current.take() {
                if !slot.chips.is_empty() {
                    slots.push(slot);
                }
            }
            current = Some(parse_slot_header(line));
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
            let chips_per_domain = infer_chips_per_domain(slot.chips.len());
            BoardChipMap {
                slot: slot.id,
                label: format!("SM{}", slot.id),
                chips_per_domain,
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

pub fn infer_chips_per_domain(chip_count: usize) -> u32 {
    if chip_count == 0 {
        return 3;
    }
    for cpd in [3u32, 2, 4, 5, 6] {
        if chip_count.is_multiple_of(cpd as usize) {
            let domains = chip_count / cpd as usize;
            if (20..=100).contains(&domains) {
                return cpd;
            }
        }
    }
    for cpd in [2u32, 3, 4, 5, 6] {
        if chip_count.is_multiple_of(cpd as usize) {
            return cpd;
        }
    }
    3
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
    };

    for part in line.split_whitespace() {
        let Some((key, val)) = part.split_once(':') else {
            continue;
        };
        match key {
            "freq" => chip.freq_mhz = val.parse().ok(),
            "vol" => chip.voltage = val.parse().ok(),
            "temp" => chip.temp_c = val.parse().unwrap_or(0),
            "err" => chip.errors = val.parse().ok(),
            _ => {}
        }
    }

    Some(chip)
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
