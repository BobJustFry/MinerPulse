export type Theme = "light" | "dark";
export type Density = "compact" | "comfortable";
export type Locale = "ru" | "en" | "zh-CN";
export type TabId = "data" | "chips" | "console" | "pools" | "charts" | "commands";
export type SubscriptionTier = "free" | "client" | "service";

export interface Entitlements {
  tier: SubscriptionTier;
  can_poll: boolean;
  can_record_session: boolean;
  can_play: boolean;
  can_show_charts: boolean;
  can_save_snapshot: boolean;
  min_read_interval_sec: number;
}

export interface LicenseInfo {
  tier: SubscriptionTier;
  plan_name?: string | null;
  expires_at?: string | null;
  user_email?: string | null;
  user_nickname?: string | null;
  licensed: boolean;
}

export interface MinerSnapshot {
  identity: {
    vendor: string;
    model: string;
    firmware: string;
    driver_id: string;
    core_chip?: string | null;
  };
  hashrate: {
    current_ghs: number;
    avg_ghs: number;
    avg5s_ghs: number;
    per_board_ghs: number[];
  };
  thermal: {
    inlet_c?: number | null;
    per_board_max_c: number[];
    per_chip_c: number[];
  };
  fans: { rpm: number[] };
  power: { watts?: number | null; voltage?: number | null };
  boards: Array<{
    label: string;
    hashrate_ghs?: number | null;
    temp_c?: number | null;
    fan_rpm?: number | null;
    status: string;
    chip_temp_min_c?: number | null;
    chip_temp_avg_c?: number | null;
    chip_temp_max_c?: number | null;
    effective_chips?: number | null;
    freq_domains_mhz?: number[];
    freq_bands_mhz?: number[];
    voltage_level?: number | null;
  }>;
  board_chips?: BoardChipMap[];
  faults?: MinerFault[];
  pools: Array<{
    url: string;
    worker: string;
    status: string;
    accepted: number;
    rejected: number;
  }>;
  shares_accepted?: number | null;
  shares_rejected?: number | null;
  hw_errors?: number | null;
  raw_log: string;
  status: string;
  uptime_sec?: number | null;
  work_mode?: number | null;
  ecmm?: number | null;
}

export interface BoardChipMap {
  slot: number;
  label: string;
  chips_per_domain: number;
  matrix_id?: string | null;
  chips: Array<{
    index: number;
    temp_c: number;
    freq_mhz?: number | null;
    voltage?: number | null;
    errors?: number | null;
    solutions?: number | null;
    crc_errors?: number | null;
    nonce?: number | null;
    repeat_count?: number | null;
    performance_pct?: [number, number] | null;
  }>;
}

export interface MinerFault {
  code: string;
  occurred_at?: string | null;
}

export interface ScanSubnet {
  id: string;
  label: string;
  start_ip: string;
  end_ip: string;
  source_ip?: string | null;
}

export interface DiscoveredMiner {
  ip: string;
  port: number;
  vendor: string;
  model: string;
  supported: boolean;
}

export interface ScanResult {
  miners: DiscoveredMiner[];
  scanned: number;
  range_label: string;
}

export interface ErrorResponse {
  code: string;
  args?: { sec?: number };
}
