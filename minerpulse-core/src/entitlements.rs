use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SubscriptionTier {
    #[default]
    Free,
    Client,
    Service,
}

pub struct EntitlementGate {
    pub tier: SubscriptionTier,
}

impl EntitlementGate {
    pub fn new(tier: SubscriptionTier) -> Self {
        Self { tier }
    }

    pub fn can_poll(&self) -> bool {
        matches!(self.tier, SubscriptionTier::Client | SubscriptionTier::Service)
    }

    pub fn can_record_session(&self) -> bool {
        matches!(self.tier, SubscriptionTier::Client | SubscriptionTier::Service)
    }

    pub fn can_play(&self) -> bool {
        matches!(self.tier, SubscriptionTier::Client | SubscriptionTier::Service)
    }

    pub fn can_show_charts(&self) -> bool {
        matches!(self.tier, SubscriptionTier::Service)
    }

    pub fn can_save_snapshot(&self) -> bool {
        true
    }

    pub fn min_read_interval_sec(&self) -> u64 {
        if self.can_poll() {
            0
        } else {
            10
        }
    }
}
