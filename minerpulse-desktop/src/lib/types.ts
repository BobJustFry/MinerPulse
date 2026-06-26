export type Theme = "light" | "dark";
export type Density = "compact" | "comfortable";
export type Locale = "ru" | "en" | "zh-CN";
export type TabId = "data" | "console" | "pools" | "charts" | "commands";
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

export interface MinerSnapshot {
  identity: {
    vendor: string;
    model: string;
    firmware: string;
    driver_id: string;
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
  pools: Array<{
    url: string;
    worker: string;
    status: string;
    accepted: number;
    rejected: number;
  }>;
  raw_log: string;
  status: string;
  uptime_sec?: number | null;
}

export interface ErrorResponse {
  code: string;
  args?: { sec?: number };
}
