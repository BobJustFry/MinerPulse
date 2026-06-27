use crate::error::MinerPulseError;
use crate::tcp::TcpCgminerClient;

pub fn ascset_payload(parameter: &str) -> String {
    format!(r#"{{"command":"ascset","parameter":"{parameter}"}}"#)
}

pub fn send_ascset(
    client: &TcpCgminerClient,
    host: &str,
    port: u16,
    parameter: &str,
) -> Result<String, MinerPulseError> {
    client.send_payload(host, port, &ascset_payload(parameter))
}

pub fn reboot_parameter() -> &'static str {
    "0,reboot,0"
}

pub fn workmode_parameter(mode: u32) -> String {
    format!("0,workmode,{mode}")
}

pub fn target_temp_parameter(temp: u32) -> String {
    format!("0,target-temp,{temp}")
}

pub fn voltage_level_parameter(level: u32) -> String {
    format!("0,voltage-level,{level}")
}

pub fn frequency_parameter(freqs: [u32; 4], board: u32, flags: u32) -> String {
    format!(
        "0,frequency,{}:{}:{}:{}-0-{}-{}",
        freqs[0], freqs[1], freqs[2], freqs[3], board, flags
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_ascset_json() {
        let payload = ascset_payload("0,reboot,0");
        assert!(payload.contains(r#""command":"ascset""#));
        assert!(payload.contains("0,reboot,0"));
    }

    #[test]
    fn formats_frequency_command() {
        assert_eq!(
            frequency_parameter([464, 484, 504, 524], 2, 0),
            "0,frequency,464:484:504:524-0-2-0"
        );
    }
}
