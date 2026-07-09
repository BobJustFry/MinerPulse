/// WhatsMiner hashboard layout parameters (MicroBT series-parallel matrix).
///
/// Physical layout: voltage domains in series, `chips_per_domain` ASICs in parallel
/// per domain. Model tables imported from HashSource/whatsminer_chip_map firmware data.

use serde::Deserialize;
use std::sync::OnceLock;

#[derive(Debug, Clone, Deserialize)]
struct LayoutConfigEntry {
    model: String,
    chip_num: u16,
    chips_per_domain: u8,
}

#[derive(Debug, Deserialize)]
struct LayoutConfigFile {
    configs: Vec<LayoutConfigEntry>,
}

static LAYOUT_CONFIGS: OnceLock<Vec<LayoutConfigEntry>> = OnceLock::new();

fn layout_configs() -> &'static [LayoutConfigEntry] {
    LAYOUT_CONFIGS.get_or_init(|| {
        let raw = include_str!("layout_configs.json");
        serde_json::from_str::<LayoutConfigFile>(raw)
            .expect("layout_configs.json parse")
            .configs
    })
}

pub fn layout_config_count() -> usize {
    layout_configs().len()
}

pub fn resolve_chips_per_domain(model: &str, chip_count: usize) -> u32 {
    if let Some(cpd) = lookup_by_model_suffix(model) {
        return cpd;
    }
    if let Some(cpd) = lookup_imported_layout(model, chip_count) {
        return cpd;
    }
    if let Some(cpd) = lookup_by_chip_count(chip_count) {
        return cpd;
    }
    infer_chips_per_domain(chip_count)
}

/// M30S++/M31S++/M32S++ always use 5 chips per domain regardless of import table order.
fn lookup_by_model_suffix(model: &str) -> Option<u32> {
    let upper = model.to_ascii_uppercase();
    if upper.contains("M30S++") || upper.contains("M31S++") || upper.contains("M32S++") {
        return Some(5);
    }
    if upper.contains("M30L") {
        return Some(4);
    }
    None
}

fn normalize_model(model: &str) -> String {
    let upper = model.to_ascii_uppercase();
    let filtered: String = upper
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '+')
        .collect();
    filtered
        .strip_prefix("WHATSMINER")
        .unwrap_or(&filtered)
        .to_string()
}

/// Lookup from imported firmware layout table (HashSource/whatsminer_chip_map).
fn lookup_imported_layout(model: &str, chip_count: usize) -> Option<u32> {
    let normalized = normalize_model(model);
    if normalized.is_empty() {
        return None;
    }
    let configs = layout_configs();

    if let Some(cfg) = configs.iter().find(|c| normalized.contains(&c.model)) {
        return Some(cfg.chips_per_domain as u32);
    }

    for prefix_len in (4..=normalized.len()).rev() {
        let prefix = &normalized[..prefix_len];
        if let Some(cfg) = configs.iter().find(|c| c.model.starts_with(prefix)) {
            return Some(cfg.chips_per_domain as u32);
        }
    }

    if let Some(series_end) = normalized.find(['V', '+']) {
        let series = &normalized[..series_end];
        if let Some(cfg) = configs.iter().find(|c| c.model.starts_with(series)) {
            return Some(cfg.chips_per_domain as u32);
        }
    }

    if chip_count > 0 && normalized.len() >= 2 {
        let mut matches: Vec<&LayoutConfigEntry> = configs
            .iter()
            .filter(|c| c.chip_num as usize == chip_count && c.model.starts_with(&normalized))
            .collect();
        if matches.is_empty() {
            matches = configs
                .iter()
                .filter(|c| c.chip_num as usize == chip_count && normalized.starts_with(&c.model[..c.model.len().min(3)]))
                .filter(|c| c.model.starts_with(&normalized[..normalized.len().min(3)]))
                .collect();
        }
        if let Some(cfg) = matches.iter().min_by_key(|c| c.model.len()) {
            return Some(cfg.chips_per_domain as u32);
        }
    }

    None
}

fn lookup_by_chip_count(chip_count: usize) -> Option<u32> {
    match chip_count {
        144 | 156 | 164 | 172 | 176 | 180 | 184 | 188 | 196 | 204 => Some(4),
        170 | 175 | 185 | 190 | 195 | 200 | 205 | 210 | 215 | 220 | 225 | 230 | 235
        | 240 | 245 | 250 | 255 | 294 | 306 => Some(5),
        63 | 66 | 69 | 72 | 75 | 78 | 81 | 84 | 87 | 90 | 93 | 96 | 99 | 102 | 105 | 108
        | 111 | 114 | 117 | 120 | 123 | 126 | 129 | 132 | 135 | 138 | 141 | 147 | 153 | 159
        | 165 | 171 | 177 | 183 | 189 => Some(3),
        82 | 86 => Some(2),
        _ => None,
    }
}

pub fn infer_chips_per_domain(chip_count: usize) -> u32 {
    if chip_count == 0 {
        return 3;
    }
    if chip_count.is_multiple_of(4) && chip_count.is_multiple_of(2) {
        let domains_if_2 = chip_count / 2;
        let domains_if_4 = chip_count / 4;
        if domains_if_2 > 50 && (20..=100).contains(&domains_if_4) {
            return 4;
        }
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
    fn loads_imported_layout_table() {
        assert!(layout_config_count() >= 400);
    }

    #[test]
    fn resolves_m20s_board_as_three_per_domain() {
        assert_eq!(resolve_chips_per_domain("WhatsMiner M20S", 105), 3);
    }

    #[test]
    fn resolves_m50_class_board_as_three_per_domain() {
        assert_eq!(resolve_chips_per_domain("WhatsMiner M50S_VH55", 111), 3);
    }

    #[test]
    fn resolves_m50s_from_imported_table() {
        assert_eq!(
            resolve_chips_per_domain("WhatsMiner M50S++_VK40", 111),
            3
        );
    }

    #[test]
    fn resolves_m30s_plus_plus_as_five_per_domain() {
        assert_eq!(resolve_chips_per_domain("WhatsMiner M30S++_V10", 85), 5);
    }

    #[test]
    fn resolves_m60_board_as_four_per_domain() {
        assert_eq!(resolve_chips_per_domain("M60", 172), 4);
        assert_eq!(resolve_chips_per_domain("WhatsMiner M60", 172), 4);
        assert_eq!(resolve_chips_per_domain("WhatsMiner M60VK20", 172), 4);
    }

    #[test]
    fn infers_four_per_domain_for_172_chip_boards() {
        assert_eq!(infer_chips_per_domain(172), 4);
    }

    #[test]
    fn keeps_two_per_domain_for_100_chip_legacy_boards() {
        assert_eq!(infer_chips_per_domain(100), 2);
        assert_eq!(resolve_chips_per_domain("WhatsMiner M50", 100), 2);
    }

    #[test]
    fn resolves_m60_vl10_as_three_per_domain() {
        assert_eq!(resolve_chips_per_domain("M60VL10", 111), 3);
    }
}
