import zones from "./whatsminerTimezones.json";

/** IANA zone names from WhatsMiner LuCI (OpenWrt) system page. */
export const WHATSMINER_TIMEZONES: readonly string[] = zones;

/** POSIX TZ string for API `timezone` field (e.g. `<+05>-5`). */
export function posixTimezoneForZonename(zonename: string, at = new Date()): string {
  if (!zonename || zonename === "UTC") {
    return "UTC0";
  }
  try {
    const offsetMin = timezoneOffsetMinutes(zonename, at);
    const absH = Math.floor(Math.abs(offsetMin) / 60);
    const sign = offsetMin >= 0 ? "+" : "-";
    const hh = String(absH).padStart(2, "0");
    return `<${sign}${hh}>-${absH}`;
  } catch {
    return "UTC0";
  }
}

function timezoneOffsetMinutes(zonename: string, at: Date): number {
  const dtf = new Intl.DateTimeFormat("en-US", {
    timeZone: zonename,
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    hourCycle: "h23",
  });
  const parts = Object.fromEntries(dtf.formatToParts(at).map((p) => [p.type, p.value]));
  const asUtc = Date.UTC(
    Number(parts.year),
    Number(parts.month) - 1,
    Number(parts.day),
    Number(parts.hour),
    Number(parts.minute),
    Number(parts.second),
  );
  return Math.round((asUtc - at.getTime()) / 60_000);
}

export function normalizeZonename(value: string): string {
  return value.trim();
}

export function zonenameInList(value: string): boolean {
  const z = normalizeZonename(value);
  return WHATSMINER_TIMEZONES.includes(z);
}
