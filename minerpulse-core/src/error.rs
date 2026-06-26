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
            MinerPulseError::Coded { code, message } => ErrorResponse {
                code: *code,
                args: message
                    .as_ref()
                    .map(|s| serde_json::json!({ "sec": s.parse::<u64>().unwrap_or(0) })),
            },
        }
    }
}
