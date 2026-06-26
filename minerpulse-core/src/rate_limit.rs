use crate::error::MinerPulseError;
use std::time::{Duration, Instant};

pub struct RateLimiter {
    min_interval: Duration,
    last: Option<Instant>,
}

impl RateLimiter {
    pub fn new(min_interval_secs: u64) -> Self {
        Self {
            min_interval: Duration::from_secs(min_interval_secs),
            last: None,
        }
    }

    pub fn try_acquire(&mut self) -> Result<(), MinerPulseError> {
        let now = Instant::now();
        if let Some(last) = self.last {
            let elapsed = now.duration_since(last);
            if elapsed < self.min_interval {
                let remaining = self.min_interval - elapsed;
                return Err(MinerPulseError::rate_limit(remaining.as_secs().max(1)));
            }
        }
        self.last = Some(now);
        Ok(())
    }

    pub fn reset(&mut self) {
        self.last = None;
    }
}
