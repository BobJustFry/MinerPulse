use super::parse::{get_parameter_bracket, parse_i32, parse_u64};
use crate::model::{BoardChipMap, ChipStats};

pub fn parse_avalon_board_chips(
    text: &str,
    firmware: &str,
    core_chip: Option<&str>,
) -> Vec<BoardChipMap> {
    let mut boards = Vec::new();

    for slot in 0..8u32 {
        let temps = parse_bracket_i32s(text, &format!("PVT_T{slot}["));
        if temps.is_empty() {
            break;
        }

        let voltages = parse_bracket_u32s(text, &format!("PVT_V{slot}["));
        let solutions = parse_bracket_u32s(text, &format!("MW{slot}["));
        let crc_errors = parse_bracket_u32s(text, &format!("ASICCRC{slot}["));
        let matrix_id =
            select_avalon_matrix_id(firmware, core_chip, temps.len()).to_string();

        let chips = (0..temps.len())
            .map(|idx| {
                let index = (idx + 1) as u32;
                ChipStats {
                    index,
                    temp_c: temps[idx],
                    voltage: voltages.get(idx).copied(),
                    solutions: solutions.get(idx).copied(),
                    errors: crc_errors.get(idx).copied(),
                    ..Default::default()
                }
            })
            .collect();

        boards.push(BoardChipMap {
            slot,
            label: format!("Board {}", slot + 1),
            chips_per_domain: 0,
            matrix_id: Some(matrix_id),
            chips,
        });
    }

    boards
}

pub fn select_avalon_matrix_id(
    firmware: &str,
    core_chip: Option<&str>,
    chip_count: usize,
) -> &'static str {
    if chip_count == 176 {
        return "Matrix_176";
    }

    if let Some(model) = avalon_model_digits(firmware) {
        if model == 1466 {
            return "Matrix_176";
        }
        if model == 1346 {
            return "Matrix_1346";
        }
        if model == 1326 {
            return "Matrix_1326";
        }
        if (1500..=1599).contains(&model) {
            return "Matrix_15x160";
        }
        if model == 1246 && chip_count <= 114 {
            return "Matrix_1326";
        }
        if (1100..=1199).contains(&model) {
            return "Matrix_11x";
        }
        if (1000..=1099).contains(&model) {
            if chip_count > 114 {
                return "Matrix_11x";
            }
            return "Matrix_10x";
        }
    }

    if let Some(core) = core_chip {
        let core = core.trim().to_ascii_uppercase();
        if core.starts_with("A3198") {
            return "Matrix_176";
        }
        if core.starts_with("A3200") {
            return if chip_count >= 160 {
                "Matrix_15x160"
            } else {
                "Matrix_1346"
            };
        }
        if core.starts_with("A3201") {
            return if chip_count <= 114 {
                "Matrix_1326"
            } else {
                "Matrix_11x"
            };
        }
        if core.starts_with("A3202") {
            return "Matrix_11x";
        }
        if core.starts_with("A3203") || core.starts_with("A3204") || core.starts_with("A3205") {
            return if chip_count > 114 {
                "Matrix_11x"
            } else {
                "Matrix_10x"
            };
        }
    }

    match chip_count {
        n if n >= 176 => "Matrix_176",
        n if n >= 160 => "Matrix_15x160",
        n if n >= 120 => "Matrix_11x",
        n if n >= 114 => "Matrix_10x",
        _ => "Matrix_10x",
    }
}

fn avalon_model_digits(firmware: &str) -> Option<u32> {
    let digits: String = firmware
        .chars()
        .take_while(|ch| ch.is_ascii_digit())
        .collect();
    if digits.len() < 4 {
        return None;
    }
    digits.parse().ok()
}

pub fn parse_bracket_i32s(text: &str, key: &str) -> Vec<i32> {
    get_parameter_bracket(text, key)
        .map(|value| {
            value
                .split_whitespace()
                .filter_map(|part| parse_i32(part))
                .collect()
        })
        .unwrap_or_default()
}

pub fn parse_bracket_u32s(text: &str, key: &str) -> Vec<u32> {
    get_parameter_bracket(text, key)
        .map(|value| {
            value
                .split_whitespace()
                .filter_map(|part| {
                    parse_i32(part)
                        .map(|v| v.max(0) as u32)
                        .or_else(|| parse_u64(part).map(|v| v as u32))
                })
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selects_matrix_by_model() {
        assert_eq!(
            select_avalon_matrix_id("1126Pro-S-64", Some("A3201"), 120),
            "Matrix_11x"
        );
        assert_eq!(select_avalon_matrix_id("1346", None, 120), "Matrix_1346");
        assert_eq!(
            select_avalon_matrix_id("1466-156-24061401", None, 176),
            "Matrix_176"
        );
        assert_eq!(
            select_avalon_matrix_id("1246", Some("A3201"), 114),
            "Matrix_1326"
        );
        assert_eq!(
            select_avalon_matrix_id("1047-20093002", None, 120),
            "Matrix_11x"
        );
        assert_eq!(
            select_avalon_matrix_id("1047-20093002", Some("A3205"), 114),
            "Matrix_10x"
        );
    }

    #[test]
    fn selects_matrix_by_core_chip() {
        assert_eq!(
            select_avalon_matrix_id("unknown", Some("A3198S"), 176),
            "Matrix_176"
        );
        assert_eq!(
            select_avalon_matrix_id("unknown", Some("A3200"), 120),
            "Matrix_1346"
        );
        assert_eq!(
            select_avalon_matrix_id("unknown", Some("A3201"), 114),
            "Matrix_1326"
        );
    }

    #[test]
    fn parses_avalon_chip_arrays() {
        let sample = "PVT_T0[66 71 87] PVT_V0[326 332 329] MW0[46 52 11] ASICCRC0[0 1 0] \
                      PVT_T1[68 71 69] PVT_V1[327 335 342] MW1[41 37 56] ASICCRC1[0 0 2]";
        let boards = parse_avalon_board_chips(sample, "1126Pro", Some("A3201"));
        assert_eq!(boards.len(), 2);
        assert_eq!(boards[0].matrix_id.as_deref(), Some("Matrix_11x"));
        assert_eq!(boards[0].chips.len(), 3);
        assert_eq!(boards[0].chips[0].index, 1);
        assert_eq!(boards[0].chips[0].temp_c, 66);
        assert_eq!(boards[0].chips[0].voltage, Some(326));
        assert_eq!(boards[0].chips[0].solutions, Some(46));
        assert_eq!(boards[0].chips[1].errors, Some(1));
        assert_eq!(boards[1].chips[2].errors, Some(2));
    }

    #[test]
    fn parses_negative_avalon_chip_temps() {
        let sample = "PVT_T0[66 -26 -34 -273] PVT_V0[326 332 329 300] MW0[1 2 3 4]";
        let boards = parse_avalon_board_chips(sample, "1466", Some("A3198S"));
        assert_eq!(boards.len(), 1);
        assert_eq!(boards[0].chips[1].temp_c, -26);
        assert_eq!(boards[0].chips[2].temp_c, -34);
        assert_eq!(boards[0].chips[3].temp_c, -273);
    }
}
