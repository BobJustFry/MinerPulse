pub mod discovery;
pub mod drivers;
pub mod entitlements;
pub mod error;
pub mod model;
pub mod mpulse;
pub mod rate_limit;
pub mod tcp;

pub use discovery::{scan_network, DiscoveredMiner, ScanRequest, ScanResult};
pub use drivers::registry::{detect_driver, DriverRegistry};
pub use entitlements::{EntitlementGate, SubscriptionTier};
pub use error::{ErrorCode, ErrorResponse, MinerPulseError};
pub use model::MinerSnapshot;
pub use mpulse::{save_snapshot, MpulseFile, MpulseKind};
pub use rate_limit::RateLimiter;
pub use tcp::TcpCgminerClient;
