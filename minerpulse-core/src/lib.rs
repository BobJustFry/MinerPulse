pub mod discovery;
pub mod drivers;
pub mod entitlements;
pub mod error;
pub mod model;
pub mod mpulse;
pub mod rate_limit;
pub mod tcp;

pub use discovery::{
    list_scan_subnets, preview_scan_ranges, scan_network, scan_network_streaming, DiscoveredMiner,
    ScanRequest, ScanResult, ScanSubnet,
};
pub use drivers::registry::{
    detect_driver, detect_vendor, driver_available, fetch_with_detect, model_from_stats,
    DriverRegistry,
};
pub use entitlements::{EntitlementGate, SubscriptionTier};
pub use error::{ErrorCode, ErrorResponse, MinerPulseError};
pub use model::MinerSnapshot;
pub use mpulse::{save_snapshot, MpulseFile, MpulseKind};
pub use rate_limit::RateLimiter;
pub use tcp::TcpCgminerClient;
