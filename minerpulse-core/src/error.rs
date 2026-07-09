use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    ConnFailed,
    ConnTimeout,
    StreamBroken,
    NotSupported,
    RateLimit,
    ParseFailed,
    InvalidInput,
    IoError,
    NoSnapshot,
    OperationCancelled,
}

#[derive(Debug, Error)]
pub enum MinerPulseError {
    #[error("{code:?}")]
    Coded {
        code: ErrorCode,
        message: Option<String>,
    },
}

impl MinerPulseError {
    pub fn code(&self) -> ErrorCode {
        match self {
            MinerPulseError::Coded { code, .. } => *code,
        }
    }

    pub fn with_code(code: ErrorCode) -> Self {
        MinerPulseError::Coded {
            code,
            message: None,
        }
    }

    pub fn conn_failed() -> Self {
        Self::with_code(ErrorCode::ConnFailed)
    }

    pub fn conn_timeout() -> Self {
        Self::with_code(ErrorCode::ConnTimeout)
    }

    pub fn stream_broken() -> Self {
        Self::with_code(ErrorCode::StreamBroken)
    }

    pub fn rate_limit(secs: u64) -> Self {
        MinerPulseError::Coded {
            code: ErrorCode::RateLimit,
            message: Some(secs.to_string()),
        }
    }

    pub fn operation_cancelled() -> Self {
        Self::with_code(ErrorCode::OperationCancelled)
    }

    pub fn no_port_response(port: u16) -> Self {
        MinerPulseError::Coded {
            code: ErrorCode::ConnFailed,
            message: Some(port.to_string()),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub code: ErrorCode,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub args: Option<serde_json::Value>,
}

impl From<&MinerPulseError> for ErrorResponse {
    fn from(err: &MinerPulseError) -> Self {
        match err {
            MinerPulseError::Coded { code, message } => {
                let args = if *code == ErrorCode::ConnFailed {
                    message
                        .as_ref()
                        .and_then(|m| m.parse::<u16>().ok())
                        .map(|port| serde_json::json!({ "port": port }))
                } else if let Some(text) = message.as_ref() {
                    if let Ok(sec) = text.parse::<u64>() {
                        Some(serde_json::json!({ "sec": sec }))
                    } else {
                        Some(serde_json::json!({ "message": text }))
                    }
                } else {
                    None
                };
                ErrorResponse { code: *code, args }
            }
        }
    }
}
