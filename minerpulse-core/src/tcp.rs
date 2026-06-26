use crate::error::MinerPulseError;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::Duration;

const BUFFER_SIZE: usize = 8192;

pub struct TcpCgminerClient {
    connect_timeout: Duration,
    io_timeout: Duration,
    try_count: u32,
}

impl Default for TcpCgminerClient {
    fn default() -> Self {
        Self {
            connect_timeout: Duration::from_millis(5000),
            io_timeout: Duration::from_millis(3000),
            try_count: 3,
        }
    }
}

impl TcpCgminerClient {
    pub fn for_discovery() -> Self {
        Self {
            connect_timeout: Duration::from_millis(600),
            io_timeout: Duration::from_millis(900),
            try_count: 1,
        }
    }

    pub fn send_command(
        &self,
        host: &str,
        port: u16,
        command: &str,
    ) -> Result<String, MinerPulseError> {
        self.send_receive(host, port, command, "", false)
    }

    pub fn send_payload(&self, host: &str, port: u16, payload: &str) -> Result<String, MinerPulseError> {
        self.transact(host, port, payload)
    }

    pub fn send_receive(
        &self,
        host: &str,
        port: u16,
        command: &str,
        parameter: &str,
        json_mode: bool,
    ) -> Result<String, MinerPulseError> {
        let payload = if json_mode {
            format!(
                r#"{{"command":"{}","parameter":"{}"}}"#,
                command, parameter
            )
        } else {
            command.to_string()
        };

        self.transact(host, port, &payload)
    }

    fn transact(&self, host: &str, port: u16, payload: &str) -> Result<String, MinerPulseError> {
        for _ in 0..self.try_count {
            match self.try_once(host, port, payload) {
                Ok(response) => return Ok(response),
                Err(MinerPulseError::Coded { code, .. })
                    if code == crate::error::ErrorCode::ConnTimeout =>
                {
                    return Err(MinerPulseError::conn_timeout());
                }
                Err(_) => continue,
            }
        }

        Err(MinerPulseError::conn_failed())
    }

    fn try_once(&self, host: &str, port: u16, payload: &str) -> Result<String, MinerPulseError> {
        let addr: SocketAddr = format!("{host}:{port}")
            .parse()
            .map_err(|_| MinerPulseError::with_code(crate::error::ErrorCode::InvalidInput))?;

        let mut stream = TcpStream::connect_timeout(&addr, self.connect_timeout)
            .map_err(|_| MinerPulseError::conn_failed())?;

        stream
            .set_read_timeout(Some(self.io_timeout))
            .map_err(|_| MinerPulseError::conn_failed())?;
        stream
            .set_write_timeout(Some(self.io_timeout))
            .map_err(|_| MinerPulseError::conn_failed())?;
        stream.set_nodelay(true).ok();

        stream
            .write_all(payload.as_bytes())
            .map_err(|_| MinerPulseError::stream_broken())?;

        let mut page = String::new();
        let mut buffer = [0u8; BUFFER_SIZE];

        loop {
            match stream.read(&mut buffer) {
                Ok(0) => break,
                Ok(n) => {
                    page.push_str(&String::from_utf8_lossy(&buffer[..n]));
                    if page.contains("<EOF>") {
                        break;
                    }
                }
                Err(_) => {
                    if page.is_empty() {
                        return Err(MinerPulseError::stream_broken());
                    }
                    break;
                }
            }
        }

        if page.is_empty() {
            return Err(MinerPulseError::conn_failed());
        }

        Ok(page.replace("<EOF>", "").trim().to_string())
    }
}
