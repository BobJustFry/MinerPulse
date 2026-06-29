import { t, type Locale, type MessageKey } from "$lib/i18n";

export interface WhatsminerChipInput {
  index: number;
  temp_c: number;
  freq_mhz?: number | null;
  voltage?: number | null;
  /** PLL / splash error count (`error` / `err` in btminer log). */
  pll_errors?: number | null;
  crc_errors?: number | null;
  nonce?: number | null;
  repeat_count?: number | null;
  performance_pct?: [number, number] | null;
}

export type WhatsminerChipStatus =
  | "healthy"
  | "ramping"
  | "underperforming"
  | "pllElevated"
  | "pllHigh"
  | "crcFault"
  | "repeatHigh"
  | "tempHigh"
  | "tempCritical"
  | "deadChip"
  | "mixed";

export interface WhatsminerBoardContext {
  label: string;
  chips: WhatsminerChipInput[];
}

export interface WhatsminerChipAnalysis {
  status: WhatsminerChipStatus;
  flags: WhatsminerChipStatus[];
  nonceDeficitPct: number;
  pllRatio: number;
  pctMin: number | null;
}

const PLL_ELEVATED = 150;
const PLL_HIGH = 300;
const REPEAT_HIGH = 10;
const REPEAT_ELEVATED = 5;
const PCT_RAMPING = 90;
const PCT_UNDERPERFORM = 85;
const PCT_HEALTHY_MIN = 95;
const NONCE_DEAD_DEFICIT = 50;
const NONCE_WARN_DEFICIT = 25;
const TEMP_CRITICAL = 100;
const TEMP_HIGH = 95;
const TEMP_DELTA_WARN = 8;

function boardAverage(values: number[]): number {
  if (values.length === 0) return 0;
  return values.reduce((sum, value) => sum + value, 0) / values.length;
}

function pctMin(chip: WhatsminerChipInput): number | null {
  if (!chip.performance_pct) return null;
  return Math.min(chip.performance_pct[0], chip.performance_pct[1]);
}

function nonceDeficit(chip: WhatsminerChipInput, board: WhatsminerBoardContext): number {
  if (chip.nonce == null) return 0;
  const nonces = board.chips
    .map((entry) => entry.nonce)
    .filter((value): value is number => value != null);
  const avg = boardAverage(nonces);
  if (avg <= 0) return 0;
  if (chip.nonce >= avg) return 0;
  return ((avg - chip.nonce) / avg) * 100;
}

function collectFlags(
  chip: WhatsminerChipInput,
  board: WhatsminerBoardContext,
  deficit: number,
): WhatsminerChipStatus[] {
  const flags: WhatsminerChipStatus[] = [];
  const pll = chip.pll_errors ?? 0;
  const crc = chip.crc_errors ?? 0;
  const repeat = chip.repeat_count ?? 0;
  const minPct = pctMin(chip);
  const pllValues = board.chips
    .map((entry) => entry.pll_errors ?? 0)
    .filter((value) => value > 0);
  const pllAvg = boardAverage(pllValues);
  const tempValues = board.chips.map((entry) => entry.temp_c);
  const tempAvg = boardAverage(tempValues);

  if (chip.nonce != null && deficit >= NONCE_DEAD_DEFICIT) {
    flags.push("deadChip");
  }
  if (crc > 0) {
    flags.push("crcFault");
  }
  if (chip.temp_c >= TEMP_CRITICAL) {
    flags.push("tempCritical");
  } else if (chip.temp_c >= TEMP_HIGH || chip.temp_c >= tempAvg + TEMP_DELTA_WARN) {
    flags.push("tempHigh");
  }
  if (pll >= PLL_HIGH || (pllAvg > 0 && pll >= pllAvg * 2.5 && pll >= PLL_ELEVATED)) {
    flags.push("pllHigh");
  } else if (pll >= PLL_ELEVATED || (pllAvg > 0 && pll >= pllAvg * 2 && pll >= 80)) {
    flags.push("pllElevated");
  }
  if (repeat >= REPEAT_HIGH || (repeat >= REPEAT_ELEVATED && pll >= 80)) {
    flags.push("repeatHigh");
  }
  if (
    (minPct != null && minPct < PCT_UNDERPERFORM) ||
    deficit >= NONCE_WARN_DEFICIT
  ) {
    flags.push("underperforming");
  } else if (minPct != null && minPct < PCT_RAMPING) {
    flags.push("ramping");
  }

  return flags;
}

function pickPrimaryStatus(flags: WhatsminerChipStatus[]): WhatsminerChipStatus {
  if (flags.length === 0) return "healthy";
  if (flags.length >= 2) {
    const severe = flags.filter(
      (flag) =>
        flag !== "ramping" &&
        flag !== "pllElevated" &&
        flag !== "tempHigh" &&
        flag !== "repeatHigh",
    );
    if (severe.length >= 2) return "mixed";
    if (severe.length === 1 && flags.length >= 2) return "mixed";
  }

  const priority: WhatsminerChipStatus[] = [
    "deadChip",
    "crcFault",
    "tempCritical",
    "underperforming",
    "pllHigh",
    "mixed",
    "pllElevated",
    "repeatHigh",
    "tempHigh",
    "ramping",
  ];
  for (const status of priority) {
    if (flags.includes(status)) return status;
  }
  return flags[0] ?? "healthy";
}

export function analyzeWhatsminerChip(
  chip: WhatsminerChipInput,
  board: WhatsminerBoardContext,
): WhatsminerChipAnalysis {
  const deficit = nonceDeficit(chip, board);
  const flags = collectFlags(chip, board, deficit);
  const pll = chip.pll_errors ?? 0;
  const pllValues = board.chips.map((entry) => entry.pll_errors ?? 0);
  const pllAvg = boardAverage(pllValues.filter((value) => value > 0));
  const minPct = pctMin(chip);

  let status = pickPrimaryStatus(flags);
  if (
    status === "healthy" &&
    minPct != null &&
    minPct >= PCT_HEALTHY_MIN &&
    (chip.pll_errors ?? 0) <= PLL_ELEVATED &&
    (chip.crc_errors ?? 0) === 0
  ) {
    status = "healthy";
  } else if (status === "healthy" && flags.length > 0) {
    status = flags[0];
  }

  return {
    status,
    flags,
    nonceDeficitPct: deficit,
    pllRatio: pllAvg > 0 ? pll / pllAvg : pll,
    pctMin: minPct,
  };
}

function formatPctPair(value: [number, number] | null | undefined): string {
  if (!value) return "—";
  return `${value[0].toFixed(1)}% / ${value[1].toFixed(1)}%`;
}

function statusBodyKey(status: WhatsminerChipStatus): MessageKey {
  return `chips.wm.status.${status}.body` as MessageKey;
}

function statusTitleKey(status: WhatsminerChipStatus): MessageKey {
  return `chips.wm.status.${status}.title` as MessageKey;
}

function flagLabel(locale: Locale, flag: WhatsminerChipStatus): string {
  return t(locale, `chips.wm.flag.${flag}` as MessageKey);
}

export function buildWhatsminerChipTooltip(
  locale: Locale,
  chip: WhatsminerChipInput,
  board: WhatsminerBoardContext,
  analysis: WhatsminerChipAnalysis,
): string {
  const lines: string[] = [];
  lines.push(
    t(locale, "chips.wm.header", {
      index: chip.index,
      board: board.label,
    }),
  );
  lines.push("");

  const metricParts: string[] = [];
  if (chip.freq_mhz != null) {
    metricParts.push(
      t(locale, "chips.wm.metric.freq", { value: chip.freq_mhz }),
    );
  }
  if (chip.voltage != null) {
    metricParts.push(
      t(locale, "chips.wm.metric.vol", { value: chip.voltage }),
    );
  }
  metricParts.push(
    t(locale, "chips.wm.metric.temp", { value: chip.temp_c }),
  );
  lines.push(metricParts.join(" · "));

  const counterParts: string[] = [];
  if (chip.nonce != null) {
    counterParts.push(
      t(locale, "chips.wm.metric.nonce", {
        value: chip.nonce.toLocaleString(locale === "zh-CN" ? "zh-CN" : locale),
      }),
    );
  }
  if (chip.pll_errors != null) {
    counterParts.push(
      t(locale, "chips.wm.metric.pll", { value: chip.pll_errors }),
    );
  }
  if (chip.crc_errors != null) {
    counterParts.push(
      t(locale, "chips.wm.metric.crc", { value: chip.crc_errors }),
    );
  }
  if (chip.repeat_count != null) {
    counterParts.push(
      t(locale, "chips.wm.metric.repeat", { value: chip.repeat_count }),
    );
  }
  if (counterParts.length > 0) {
    lines.push(counterParts.join(" · "));
  }

  if (chip.performance_pct) {
    lines.push(
      t(locale, "chips.wm.metric.pct", {
        value: formatPctPair(chip.performance_pct),
      }),
    );
  }

  lines.push("");
  lines.push(t(locale, statusTitleKey(analysis.status)));

  const bodyArgs: Record<string, string | number> = {
    pll: chip.pll_errors ?? 0,
    crc: chip.crc_errors ?? 0,
    repeat: chip.repeat_count ?? 0,
    temp: chip.temp_c,
    pct: analysis.pctMin != null ? analysis.pctMin.toFixed(1) : "—",
    deficit: analysis.nonceDeficitPct.toFixed(0),
    issues: analysis.flags.map((flag) => flagLabel(locale, flag)).join(", "),
  };
  lines.push(t(locale, statusBodyKey(analysis.status), bodyArgs));

  return lines.join("\n");
}

export function chipToWhatsminerInput(
  chip: {
    index: number;
    temp_c: number;
    freq_mhz?: number | null;
    voltage?: number | null;
    errors?: number | null;
    crc_errors?: number | null;
    nonce?: number | null;
    repeat_count?: number | null;
    performance_pct?: [number, number] | null;
  },
): WhatsminerChipInput {
  return {
    index: chip.index,
    temp_c: chip.temp_c,
    freq_mhz: chip.freq_mhz,
    voltage: chip.voltage,
    pll_errors: chip.errors,
    crc_errors: chip.crc_errors,
    nonce: chip.nonce,
    repeat_count: chip.repeat_count,
    performance_pct: chip.performance_pct,
  };
}
