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

export function statusTone(status: string): "ok" | "warn" | "muted" {
  const value = status.trim().toLowerCase();
  if (!value) return "muted";
  if (
    value === "s" ||
    value === "alive" ||
    value === "ok" ||
    value.includes("running") ||
    value.includes("work") ||
    value.includes("active")
  ) {
    return "ok";
  }
  if (value.includes("dead") || value.includes("fail") || value.includes("error")) {
    return "warn";
  }
  return "muted";
}
