pub mod discovery;
pub mod drivers;
pub mod entitlements;
pub mod error;
pub mod fetch_options;
pub mod import;
pub mod model;
pub mod mpulse;
pub mod mpulse_binary;
pub mod rate_limit;
pub mod tcp;
pub mod trace;

pub use discovery::{
    list_scan_subnets, preview_scan_ranges, scan_network, scan_network_streaming, DiscoveredMiner,
    ScanRequest, ScanResult, ScanSubnet,
};
pub use drivers::registry::{
    detect_driver, detect_vendor, driver_available, fetch_whatsminer, fetch_with_detect,
    model_from_stats, DriverRegistry,
};
pub use drivers::whatsminer::control::{
    apply_control_action, export_miner_log, read_control_state, WhatsminerControlAction,
    WhatsminerControlApplyResult, WhatsminerControlState,
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
    decode_mpulse_bytes, load_mpulse, open_mpulse_file, save_session, save_snapshot, MpulseFile,
    MpulseFrame, MpulseKind, DEFAULT_POLL_INTERVAL_SEC, DEFAULT_POLL_RATE_HZ,
    MAX_SESSION_DURATION_SEC, MAX_SESSION_FILE_BYTES, POLL_RATES_HZ, normalize_poll_rate_hz,
    poll_interval_ms, poll_wait_after_tick,
};
pub use mpulse_binary::{
    extension_for_kind, is_binary_mpulse, load_binary_mpulse, save_binary_mpulse,
    BinarySessionMeta, LoadedBinarySession, StoredChartPoint, EXT_SESSION, EXT_SNAPSHOT,
    LEGACY_EXT_SESSION, LEGACY_EXT_SNAPSHOT,
};
pub use rate_limit::RateLimiter;
pub use tcp::TcpCgminerClient;
pub use trace::{set_trace_hook, trace};
pub use drivers::avalon_commands::send_ascset;
