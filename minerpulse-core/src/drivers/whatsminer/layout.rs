/// WhatsMiner hashboard layout parameters (MicroBT series-parallel matrix).
///
/// Physical layout: voltage domains in series, `chips_per_domain` ASICs in parallel
/// per domain. Typical air-cooled boards: 35×3 (M20S), 37×3 (111-chip boards).

pub fn resolve_chips_per_domain(model: &str, chip_count: usize) -> u32 {
    if let Some(cpd) = lookup_by_model(model) {
        return cpd;
    }
    if let Some(cpd) = lookup_by_chip_count(chip_count) {
        return cpd;
    }
    infer_chips_per_domain(chip_count)
}

fn lookup_by_model(model: &str) -> Option<u32> {
    let upper = model.to_ascii_uppercase();
    if upper.contains("M30S++") || upper.contains("M31S++") || upper.contains("M32S++") {
        return Some(5);
    }
    if upper.contains("M30L") {
        return Some(4);
    }
    None
}

fn lookup_by_chip_count(chip_count: usize) -> Option<u32> {
    match chip_count {
        144 | 156 => Some(4),
        170 | 175 | 180 | 185 | 190 | 195 | 200 | 205 | 210 | 215 | 220 | 225 | 230 | 235
        | 240 | 250 | 255 => Some(5),
        63 | 66 | 69 | 72 | 75 | 78 | 81 | 84 | 87 | 90 | 93 | 96 | 99 | 102 | 105 | 108
        | 111 | 114 | 117 | 120 | 123 | 126 | 129 | 132 | 135 | 138 | 141 | 147 | 153 | 159
        | 165 | 171 | 177 | 183 | 189 => Some(3),
        _ => None,
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_m20s_board_as_three_per_domain() {
        assert_eq!(resolve_chips_per_domain("WhatsMiner M20S", 105), 3);
    }

    #[test]
    fn resolves_m50_class_board_as_three_per_domain() {
        assert_eq!(resolve_chips_per_domain("WhatsMiner M50S_VH55", 111), 3);
    }

    #[test]
    fn resolves_m30s_plus_plus_as_five_per_domain() {
        assert_eq!(resolve_chips_per_domain("WhatsMiner M30S++_V10", 85), 5);
    }
}
