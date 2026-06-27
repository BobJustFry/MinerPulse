import type { MinerSnapshot } from "$lib/types";
import {
  hashrateChartScale,
  pickSnapshotHashrateGhs,
  type HashrateUnit,
} from "$lib/formatMiner";

export const MAX_CHART_WINDOW_MS = 30 * 60 * 1000;

export interface ChartPoint {
  t_ms: number;
  hashrate_ghs: number;
  board_temps: number[];
  power_w: number | null;
  fan_rpm: number | null;
}

export interface LineChartSeries {
  id: string;
  label: string;
  color: string;
  values: number[];
}

const BOARD_COLORS = ["#0066cc", "#16a34a", "#d97706", "#9333ea", "#0891b2", "#dc2626"];

export function pointFromSnapshot(snapshot: MinerSnapshot, t_ms: number): ChartPoint {
  let board_temps = [...snapshot.thermal.per_board_max_c];
  if (board_temps.length === 0) {
    board_temps = snapshot.boards
      .map((board) => board.chip_temp_max_c ?? board.temp_c)
      .filter((value): value is number => value != null);
  }

  const fan_rpm =
    snapshot.fans.rpm.length > 0 ? Math.max(...snapshot.fans.rpm) : null;

  return {
    t_ms,
    hashrate_ghs: pickSnapshotHashrateGhs(snapshot.hashrate),
    board_temps,
    power_w: snapshot.power.watts ?? null,
    fan_rpm,
  };
}

export function appendChartPoint(
  history: ChartPoint[],
  point: ChartPoint,
  maxWindowMs = MAX_CHART_WINDOW_MS,
): ChartPoint[] {
  const next = [...history, point];
  const cutoff = Math.max(0, point.t_ms - maxWindowMs);
  return next.filter((item) => item.t_ms >= cutoff);
}

export function chartPointsFromFrames(
  frames: Array<{ t_ms: number; snapshot: MinerSnapshot }>,
): ChartPoint[] {
  return frames.map((frame) => pointFromSnapshot(frame.snapshot, frame.t_ms));
}

export function formatChartDuration(t_ms: number, originMs: number): string {
  const sec = Math.max(0, Math.floor((t_ms - originMs) / 1000));
  const minutes = Math.floor(sec / 60);
  const seconds = sec % 60;
  return `${minutes}:${seconds.toString().padStart(2, "0")}`;
}

export function hasPowerSeries(points: ChartPoint[]): boolean {
  return points.some((point) => point.power_w != null);
}

export function hasFanSeries(points: ChartPoint[]): boolean {
  return points.some((point) => point.fan_rpm != null);
}

export function maxBoardCount(points: ChartPoint[]): number {
  return points.reduce((max, point) => Math.max(max, point.board_temps.length), 0);
}

export function resolveHashrateChart(points: ChartPoint[]): {
  unit: HashrateUnit;
  divisor: number;
} {
  const maxGhs = Math.max(0, ...points.map((point) => point.hashrate_ghs));
  return hashrateChartScale(maxGhs);
}

export function buildHashrateSeries(
  points: ChartPoint[],
  label: string,
  divisor: number,
): LineChartSeries {
  return {
    id: "hashrate",
    label,
    color: "#0066cc",
    values: points.map((point) => (divisor > 0 ? point.hashrate_ghs / divisor : 0)),
  };
}

export function buildBoardTempSeries(points: ChartPoint[]): LineChartSeries[] {
  const count = maxBoardCount(points);
  return Array.from({ length: count }, (_, index) => ({
    id: `board-${index}`,
    label: `B${index + 1}`,
    color: BOARD_COLORS[index % BOARD_COLORS.length],
    values: points.map((point) => point.board_temps[index] ?? NaN),
  }));
}

export function buildPowerSeries(points: ChartPoint[], label: string): LineChartSeries {
  return {
    id: "power",
    label,
    color: "#7c3aed",
    values: points.map((point) => point.power_w ?? NaN),
  };
}

export function buildFanSeries(points: ChartPoint[], label: string): LineChartSeries {
  return {
    id: "fan",
    label,
    color: "#0891b2",
    values: points.map((point) => point.fan_rpm ?? NaN),
  };
}

export interface ChartLayout {
  width: number;
  height: number;
  padTop: number;
  padRight: number;
  padBottom: number;
  padLeft: number;
}

export const DEFAULT_CHART_LAYOUT: ChartLayout = {
  width: 640,
  height: 180,
  padTop: 14,
  padRight: 16,
  padBottom: 28,
  padLeft: 52,
};

export const COMPACT_CHART_LAYOUT: ChartLayout = {
  width: 1000,
  height: 96,
  padTop: 8,
  padRight: 4,
  padBottom: 20,
  padLeft: 36,
};

export function scaleChartLayout(
  base: ChartLayout,
  width: number,
  height: number,
): ChartLayout {
  if (width <= 0 || height <= 0) {
    return base;
  }
  const sx = width / base.width;
  const sy = height / base.height;
  return {
    width,
    height,
    padTop: base.padTop * sy,
    padRight: base.padRight * sx,
    padBottom: base.padBottom * sy,
    padLeft: base.padLeft * sx,
  };
}

export function chartBounds(values: number[]): { min: number; max: number } {
  const filtered = values.filter((value) => Number.isFinite(value));
  if (filtered.length === 0) return { min: 0, max: 1 };
  let min = Math.min(...filtered);
  let max = Math.max(...filtered);
  if (min === max) {
    const pad = Math.max(Math.abs(min) * 0.05, 0.5);
    min -= pad;
    max += pad;
  } else {
    const pad = (max - min) * 0.08;
    min -= pad;
    max += pad;
  }
  return { min, max };
}

export function buildLinePath(
  values: number[],
  layout: ChartLayout,
  minY: number,
  maxY: number,
): string {
  const plotW = layout.width - layout.padLeft - layout.padRight;
  const plotH = layout.height - layout.padTop - layout.padBottom;
  const stepX = values.length > 1 ? plotW / (values.length - 1) : 0;

  let path = "";
  let started = false;
  values.forEach((value, index) => {
    if (!Number.isFinite(value)) {
      started = false;
      return;
    }
    const x = layout.padLeft + stepX * index;
    const y = layout.padTop + plotH - ((value - minY) / (maxY - minY)) * plotH;
    path += started ? ` L ${x} ${y}` : `M ${x} ${y}`;
    started = true;
  });
  return path;
}

export function cursorIndex(points: ChartPoint[], cursorMs: number | null): number | null {
  if (cursorMs == null || points.length === 0) return null;
  let bestIndex = 0;
  let bestDelta = Math.abs(points[0].t_ms - cursorMs);
  for (let index = 1; index < points.length; index += 1) {
    const delta = Math.abs(points[index].t_ms - cursorMs);
    if (delta < bestDelta) {
      bestDelta = delta;
      bestIndex = index;
    }
  }
  return bestIndex;
}

export function cursorX(
  index: number | null,
  count: number,
  layout: ChartLayout,
): number | null {
  if (index == null || count <= 0) return null;
  const plotW = layout.width - layout.padLeft - layout.padRight;
  const stepX = count > 1 ? plotW / (count - 1) : 0;
  return layout.padLeft + stepX * index;
}
