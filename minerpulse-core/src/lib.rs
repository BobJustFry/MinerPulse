pub mod discovery;
pub mod drivers;
pub mod entitlements;
pub mod error;
pub mod fetch_options;
pub mod import;
pub mod model;
pub mod mpulse;
pub mod rate_limit;
pub mod tcp;

pub use discovery::{
    list_scan_subnets, preview_scan_ranges, scan_network, scan_network_streaming, DiscoveredMiner,
    ScanRequest, ScanResult, ScanSubnet,
};
pub use drivers::registry::{
    detect_driver, detect_vendor, driver_available, fetch_whatsminer, fetch_with_detect,
    model_from_stats, DriverRegistry,
};
pub use drivers::whatsminer::access::{
    compute_needs_setup, enable_api_switch, probe_whatsminer_access, WhatsminerAccessStatus,
};
pub use drivers::whatsminer::luci::{test_luci_credentials, verify_luci_login};
pub use entitlements::{EntitlementGate, SubscriptionTier};
pub use error::{ErrorCode, ErrorResponse, MinerPulseError};
pub use fetch_options::{FetchOptions, WhatsminerLuciAuth};
pub use model::{MinerSnapshot, WhatsminerAccessInfo};
pub use import::{import_file_content, ImportResult, MAX_IMPORT_BYTES};
pub use mpulse::{
    decode_mpulse_bytes, load_mpulse, save_session, save_snapshot, MpulseFile, MpulseFrame,
    MpulseKind, DEFAULT_POLL_INTERVAL_SEC, DEFAULT_POLL_RATE_HZ, MAX_SESSION_DURATION_SEC,
    MAX_SESSION_FILE_BYTES, POLL_RATES_HZ, normalize_poll_rate_hz, poll_interval_ms,
    poll_wait_after_tick,
};
pub use rate_limit::RateLimiter;
pub use tcp::TcpCgminerClient;
pub use drivers::avalon_commands::send_ascset;
