export type HashrateUnit = "GH/s" | "TH/s" | "PH/s";

export function hashrateChartScale(maxGhs: number): { unit: HashrateUnit; divisor: number } {
  const abs = Math.abs(maxGhs);
  if (abs >= 1_000_000) return { unit: "PH/s", divisor: 1_000_000 };
  if (abs >= 1000) return { unit: "TH/s", divisor: 1000 };
  return { unit: "GH/s", divisor: 1 };
}

export function formatHashrateAxis(value: number): string {
  const abs = Math.abs(value);
  if (abs >= 100) return value.toFixed(0);
  if (abs >= 10) return value.toFixed(1);
  return value.toFixed(2);
}

export function pickSnapshotHashrateGhs(hashrate: {
  avg5s_ghs: number;
  current_ghs: number;
  avg_ghs: number;
  per_board_ghs: number[];
}): number {
  const candidates: number[] = [];
  for (const value of [hashrate.avg5s_ghs, hashrate.current_ghs, hashrate.avg_ghs]) {
    if (Number.isFinite(value) && value > 0) {
      candidates.push(value);
    }
  }
  const boardSum = hashrate.per_board_ghs.reduce((sum, value) => {
    return Number.isFinite(value) && value > 0 ? sum + value : sum;
  }, 0);
  if (boardSum > 0) {
    candidates.push(boardSum);
  }
  if (candidates.length === 0) return 0;
  return Math.max(...candidates);
}

export function formatHashrate(ghs: number | null | undefined): string {
  if (ghs == null || Number.isNaN(ghs)) return "—";
  const abs = Math.abs(ghs);
  if (abs >= 1_000_000) return `${(ghs / 1_000_000).toFixed(2)} PH/s`;
  if (abs >= 1000) return `${(ghs / 1000).toFixed(2)} TH/s`;
  return `${ghs.toFixed(2)} GH/s`;
}

export function formatNumber(
  value: number | null | undefined,
  digits = 0,
  suffix = "",
): string {
  if (value == null || Number.isNaN(value)) return "—";
  return `${value.toFixed(digits)}${suffix}`;
}

export function formatUptime(sec: number | null | undefined): string {
  if (sec == null) return "—";
  const days = Math.floor(sec / 86400);
  const hours = Math.floor((sec % 86400) / 3600);
  const minutes = Math.floor((sec % 3600) / 60);
  if (days > 0) return `${days}d ${hours}h ${minutes}m`;
  if (hours > 0) return `${hours}h ${minutes}m`;
  return `${minutes}m`;
}

export function formatEfficiency(
  watts: number | null | undefined,
  ghs: number | null | undefined,
): string {
  if (watts == null || ghs == null || ghs <= 0) return "—";
  const th = ghs / 1000;
  if (th <= 0) return "—";
  return `${(watts / th).toFixed(1)} J/TH`;
}

export function formatPercent(
  part: number | null | undefined,
  total: number | null | undefined,
): string {
  if (part == null || total == null || total <= 0) return "—";
  return `${((part / total) * 100).toFixed(2)}%`;
}

export function shortPoolUrl(url: string): string {
  if (!url) return "—";
  return url.replace(/^stratum\+(?:tcp|ssl):\/\//, "").replace(/\/$/, "");
}

export function vendorLabel(vendor: string): string {
  const map: Record<string, string> = {
    avalon: "Avalon",
    antminer: "Antminer",
    whatsminer: "WhatsMiner",
    innosilicon: "Innosilicon",
    generic: "CGMiner",
    unknown: "?",
  };
  return map[vendor] ?? vendor;
}

export function driverLabel(driverId: string): string {
  const map: Record<string, string> = {
    antminer: "Antminer",
    whatsminer: "WhatsMiner",
    avalon: "Avalon",
    innosilicon: "Innosilicon",
    generic: "CGMiner",
    cgminer: "CGMiner",
    unknown: "?",
  };
  return map[driverId] ?? driverId;
}

export function statusTone(status: string): "ok" | "warn" | "muted" {
  const value = status.trim().toLowerCase();
  if (!value) return "muted";
  if (
    value === "s" ||
    value === "alive" ||
    value === "ok" ||
    value === "mining" ||
    value.includes("running") ||
    value.includes("work") ||
    value.includes("active")
  ) {
    return "ok";
  }
  if (
    value === "offline" ||
    value === "unavailable" ||
    value.includes("dead") ||
    value.includes("fail") ||
    value.includes("error")
  ) {
    return "warn";
  }
  return "muted";
}

/// Canonical status tokens set by the WhatsMiner driver map to i18n keys.
export function statusMessageKey(status: string): string | null {
  switch (status.trim().toLowerCase()) {
    case "mining":
      return "minerStatus.mining";
    case "idle":
      return "minerStatus.idle";
    case "offline":
      return "minerStatus.offline";
    case "unavailable":
      return "minerStatus.unavailable";
    case "unknown":
    case "":
      return "minerStatus.unknown";
    default:
      return null;
  }
}
