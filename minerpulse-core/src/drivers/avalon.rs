use super::parse::{get_parameter, parse_f64, parse_u64};
use super::MinerDriver;
use crate::error::MinerPulseError;
use crate::model::{
    FanStats, HashrateStats, MinerIdentity, MinerSnapshot, MinerVendor, PowerStats, ThermalStats,
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
        if let Some(id) = get_parameter(stats_response, "ID") {
            if id.contains("AV") {
                return true;
            }
        }
        get_parameter(stats_response, "Ver").is_some()
    }

    fn fetch_snapshot(
        &self,
        client: &TcpCgminerClient,
        host: &str,
        port: u16,
    ) -> Result<MinerSnapshot, MinerPulseError> {
        let raw = client.send_receive(host, port, "estats+lcd", "", false)?;
        Ok(parse_estats(&raw))
    }
}

pub fn parse_estats(raw: &str) -> MinerSnapshot {
    let cleaned = raw.replace('\'', "").replace("  ", " ");

    let firmware = get_parameter(&cleaned, "Ver").unwrap_or_default();
    let model = if firmware.is_empty() {
        "Unknown".to_string()
    } else {
        format!("Avalon-{firmware}")
    };

    let mut hashrate = HashrateStats::default();
    if let Some(v) = get_parameter(&cleaned, "GHS 5s").and_then(|s| parse_f64(&s)) {
        hashrate.avg5s_ghs = v;
    }
    if let Some(v) = get_parameter(&cleaned, "GHS av").and_then(|s| parse_f64(&s)) {
        hashrate.avg_ghs = v;
    }
    if let Some(v) = get_parameter(&cleaned, "GHS 5s").and_then(|s| parse_f64(&s)) {
        hashrate.current_ghs = v;
    }

    for i in 0..3 {
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

    let mut fans = FanStats::default();
    for i in 1..=4 {
        let key = format!("Fan{i}");
        if let Some(rpm) = get_parameter(&cleaned, &key).and_then(|s| parse_u64(&s)) {
            fans.rpm.push(rpm as u32);
        }
    }

    let mut power = PowerStats::default();
    power.watts = get_parameter(&cleaned, "Power").and_then(|s| parse_f64(&s));
    power.voltage = get_parameter(&cleaned, "Voltage").and_then(|s| parse_f64(&s));

    let status = get_parameter(&cleaned, "Work").unwrap_or_else(|| "Unknown".to_string());

    MinerSnapshot {
        identity: MinerIdentity {
            vendor: MinerVendor::Avalon,
            model,
            firmware: firmware.clone(),
            driver_id: "avalon".to_string(),
        },
        hashrate,
        thermal,
        fans,
        power,
        pools: Vec::new(),
        raw_log: raw.to_string(),
        status,
        uptime_sec: get_parameter(&cleaned, "Elapsed").and_then(|s| parse_u64(&s)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_avalon_by_ver() {
        assert!(AvalonDriver::detect("Ver=1346|Temp=30"));
    }
}
