import { chipTempColor } from "$lib/whatsminerErrors";

export type ChipMatrix = number[][];

export type ChipDisplayMetric = "temp" | "voltage" | "solutions" | "crc";

export interface ChipCellData {
  index: number;
  temp: number;
  voltage?: number | null;
  errors?: number | null;
  solutions?: number | null;
}

export interface ChipCell {
  index: number;
  temp: number;
  voltage?: number | null;
  errors?: number | null;
  solutions?: number | null;
  empty: boolean;
}

export interface ChipMetricRange {
  voltageMin: number;
  voltageMax: number;
  solutionsMax: number;
}

export type ChipVoltageUnit = "millivolts" | "centivolts";

export function chipVoltageUnitForVendor(vendor: string | undefined): ChipVoltageUnit {
  return vendor?.toLowerCase() === "avalon" ? "millivolts" : "centivolts";
}

const matrixCache = new Map<string, Promise<ChipMatrix>>();

export const CHIP_DISPLAY_METRICS: ChipDisplayMetric[] = [
  "temp",
  "voltage",
  "solutions",
  "crc",
];

export function loadChipMatrix(matrixId: string): Promise<ChipMatrix> {
  let pending = matrixCache.get(matrixId);
  if (!pending) {
    pending = fetch(`/matrix/${matrixId}.json`).then((response) => {
      if (!response.ok) {
        throw new Error(`Failed to load matrix ${matrixId}: ${response.status}`);
      }
      return response.json() as Promise<ChipMatrix>;
    });
    matrixCache.set(matrixId, pending);
  }
  return pending;
}

export function chipLookup(
  chips: ChipCellData[],
): Map<number, ChipCellData> {
  const map = new Map<number, ChipCellData>();
  for (const chip of chips) {
    map.set(chip.index, chip);
  }
  return map;
}

interface DomainChipInput {
  index: number;
  temp_c: number;
}

function domainGridCell(chip: DomainChipInput): ChipCell {
  return {
    index: chip.index,
    temp: chip.temp_c,
    empty: false,
  };
}

function sortedDomainChips(chips: DomainChipInput[]): DomainChipInput[] {
  return [...chips].sort((a, b) => a.index - b.index);
}

/** Wide boards: domains left-to-right, few rows (WhatsMiner-style). */
function buildHorizontalDomainGrid(
  chips: DomainChipInput[],
  numDomains: number,
  chipsPerDomain: number,
): ChipCell[][] {
  const ordered = sortedDomainChips(chips);
  const rows: ChipCell[][] = [];

  for (let rowIdx = 0; rowIdx < chipsPerDomain; rowIdx += 1) {
    const row: ChipCell[] = [];
    for (let domainIdx = 0; domainIdx < numDomains; domainIdx += 1) {
      const chipIdx = domainIdx * chipsPerDomain + rowIdx;
      if (chipIdx < ordered.length) {
        row.push(domainGridCell(ordered[chipIdx]));
      }
    }
    if (row.length > 0) {
      rows.push(row);
    }
  }

  return rows;
}

/** Tall boards: Avalon-style serpentine domain columns. */
function buildVerticalDomainGrid(
  chips: DomainChipInput[],
  numDomains: number,
  chipsPerDomain: number,
): ChipCell[][] {
  const ordered = sortedDomainChips(chips);
  const remaining = Math.max(0, numDomains - 1);
  const bottomDomains = 1 + Math.floor(remaining / 2);
  const topDomains = remaining - Math.floor(remaining / 2);

  const sections: Array<{ start: number; end: number; reversed: boolean }> = [];
  if (topDomains > 0) {
    sections.push({ start: bottomDomains, end: numDomains, reversed: false });
  }
  sections.push({ start: 0, end: bottomDomains, reversed: true });

  const rows: ChipCell[][] = [];

  for (const section of sections) {
    for (let rowIdx = 0; rowIdx < chipsPerDomain; rowIdx += 1) {
      const row: ChipCell[] = [];
      const domainCount = section.end - section.start;
      for (let i = 0; i < domainCount; i += 1) {
        const domainIdx = section.reversed ? section.end - 1 - i : section.start + i;
        const chipIdx = domainIdx * chipsPerDomain + rowIdx;
        if (chipIdx < ordered.length) {
          row.push(domainGridCell(ordered[chipIdx]));
        }
      }
      if (row.length > 0) {
        rows.push(row);
      }
    }
  }

  return rows;
}

export function buildChipGrid(
  chips: DomainChipInput[],
  chipsPerDomain: number,
): ChipCell[][] {
  if (chips.length === 0 || chipsPerDomain <= 0) {
    return [];
  }

  const numDomains = Math.ceil(chips.length / chipsPerDomain);
  if (numDomains > chipsPerDomain) {
    return buildHorizontalDomainGrid(chips, numDomains, chipsPerDomain);
  }
  return buildVerticalDomainGrid(chips, numDomains, chipsPerDomain);
}

export function buildMatrixGrid(
  matrix: ChipMatrix,
  chips: ChipCellData[],
): ChipCell[][] {
  const byIndex = chipLookup(chips);
  return matrix
    .map((row) =>
      row.map((slotIndex) => {
        if (slotIndex <= 0) {
          return { index: 0, temp: 0, empty: true };
        }
        const chip = byIndex.get(slotIndex);
        if (!chip) {
          return { index: slotIndex, temp: 0, empty: true };
        }
        return {
          index: chip.index,
          temp: chip.temp,
          voltage: chip.voltage,
          errors: chip.errors,
          solutions: chip.solutions,
          empty: false,
        };
      }),
    )
    .filter((row) => row.some((cell) => !cell.empty || cell.index > 0));
}

export { chipTempColor };

export function chipMetricAvailable(
  boards: Array<{ chips: ChipCellData[] }>,
  metric: ChipDisplayMetric,
): boolean {
  if (metric === "temp") {
    return boards.some((board) => board.chips.length > 0);
  }
  return boards.some((board) =>
    board.chips.some((chip) => {
      switch (metric) {
        case "voltage":
          return chip.voltage != null;
        case "solutions":
          return chip.solutions != null;
        case "crc":
          return chip.errors != null;
        default:
          return false;
      }
    }),
  );
}

export function availableChipMetrics(
  boards: Array<{ chips: ChipCellData[] }>,
): ChipDisplayMetric[] {
  return CHIP_DISPLAY_METRICS.filter((metric) => chipMetricAvailable(boards, metric));
}

export function chipMetricRange(
  boards: Array<{ chips: ChipCellData[] }>,
): ChipMetricRange {
  let voltageMin = Number.POSITIVE_INFINITY;
  let voltageMax = Number.NEGATIVE_INFINITY;
  let solutionsMax = 1;

  for (const board of boards) {
    for (const chip of board.chips) {
      if (chip.voltage != null) {
        voltageMin = Math.min(voltageMin, chip.voltage);
        voltageMax = Math.max(voltageMax, chip.voltage);
      }
      if (chip.solutions != null) {
        solutionsMax = Math.max(solutionsMax, chip.solutions);
      }
    }
  }

  if (!Number.isFinite(voltageMin)) {
    voltageMin = 300;
    voltageMax = 350;
  } else if (voltageMin === voltageMax) {
    voltageMax = voltageMin + 1;
  }

  return { voltageMin, voltageMax, solutionsMax };
}

export function chipVoltageColor(
  voltage: number,
  min: number,
  max: number,
): string {
  const ratio = Math.max(0, Math.min(1, (voltage - min) / Math.max(1, max - min)));
  const hue = 215 - ratio * 35;
  return `hsl(${hue} 68% 44%)`;
}

export function chipSolutionsColor(value: number, max: number): string {
  const ratio = Math.max(0, Math.min(1, value / Math.max(1, max)));
  const lightness = 34 + ratio * 18;
  return `hsl(142 58% ${lightness}%)`;
}

export function chipCrcColor(value: number): string {
  if (value <= 0) {
    return "hsl(142 58% 38%)";
  }
  return "hsl(0 72% 42%)";
}

export function chipCellBackground(
  cell: ChipCell,
  metric: ChipDisplayMetric,
  range: ChipMetricRange,
): string {
  switch (metric) {
    case "temp":
      return chipTempColor(cell.temp);
    case "voltage":
      return cell.voltage != null
        ? chipVoltageColor(cell.voltage, range.voltageMin, range.voltageMax)
        : "hsl(220 8% 42%)";
    case "solutions":
      return cell.solutions != null
        ? chipSolutionsColor(cell.solutions, range.solutionsMax)
        : "hsl(220 8% 42%)";
    case "crc":
      return chipCrcColor(cell.errors ?? 0);
  }
}

export function chipCellDisplayValue(
  cell: ChipCell,
  metric: ChipDisplayMetric,
  voltageUnit: ChipVoltageUnit = "centivolts",
): string {
  switch (metric) {
    case "temp":
      return `${cell.temp}°`;
    case "voltage":
      return formatChipVoltageCompact(cell.voltage, voltageUnit);
    case "solutions":
      return formatChipCount(cell.solutions);
    case "crc":
      return formatChipCount(cell.errors);
  }
}

export function formatChipVoltage(
  value: number | null | undefined,
  unit: ChipVoltageUnit = "centivolts",
): string {
  if (value == null) return "—";
  if (unit === "millivolts") {
    return `${value.toLocaleString()} mV`;
  }
  return `${(value / 100).toFixed(2)} V`;
}

export function formatChipVoltageCompact(
  value: number | null | undefined,
  unit: ChipVoltageUnit = "centivolts",
): string {
  if (value == null) return "—";
  if (unit === "millivolts") {
    return value.toLocaleString();
  }
  return (value / 100).toFixed(1);
}

export function formatChipCount(value: number | null | undefined): string {
  if (value == null) return "—";
  return String(value);
}

export function formatChipMetric(
  value: number | null | undefined,
): string {
  if (value == null) return "—";
  return value.toLocaleString();
}
