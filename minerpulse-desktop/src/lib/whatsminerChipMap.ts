import type { ChipCell } from "$lib/chipMatrix";

/**
 * WhatsMiner hashboard physical layout (MicroBT series-parallel matrix).
 *
 * References:
 * - M20S: 35 voltage domains × 3 chips/domain = 105 chips per board (Zeus/MicroBT docs)
 * - Firmware: chip_num + chip_column_num (parallel rows per domain)
 * - HashSource/whatsminer_chip_map snake pattern matching PCB airflow
 *
 * Index mapping: chipIndex = domainIndex * chipsPerDomain + rowIndex
 */

export interface DomainChipInput {
  index: number;
  temp_c: number;
}

export interface WhatsminerLayoutMeta {
  domains: number;
  chipsPerDomain: number;
  bottomDomains: number;
  topDomains: number;
  sectionBreakRow: number;
}

function sortedChips(chips: DomainChipInput[]): DomainChipInput[] {
  return [...chips].sort((a, b) => a.index - b.index);
}

function cell(chip: DomainChipInput): ChipCell {
  return { index: chip.index, temp: chip.temp_c, empty: false };
}

function snakeSections(numDomains: number) {
  const remaining = Math.max(0, numDomains - 1);
  const bottomDomains = 1 + Math.floor(remaining / 2);
  const topDomains = remaining - Math.floor(remaining / 2);
  return { bottomDomains, topDomains };
}

export function whatsminerLayoutMeta(
  chipCount: number,
  chipsPerDomain: number,
): WhatsminerLayoutMeta {
  const domains = chipsPerDomain > 0 ? Math.ceil(chipCount / chipsPerDomain) : 1;
  const { bottomDomains, topDomains } = snakeSections(domains);
  return {
    domains,
    chipsPerDomain,
    bottomDomains,
    topDomains,
    sectionBreakRow: topDomains > 0 ? topDomains * chipsPerDomain : -1,
  };
}

function renderSection(
  ordered: DomainChipInput[],
  chipsPerDomain: number,
  startDomain: number,
  endDomain: number,
  reversed: boolean,
): ChipCell[][] {
  const domainCount = endDomain - startDomain;
  const rows: ChipCell[][] = [];

  for (let rowIdx = 0; rowIdx < chipsPerDomain; rowIdx += 1) {
    const row: ChipCell[] = [];
    for (let i = 0; i < domainCount; i += 1) {
      const domainIdx = reversed ? endDomain - 1 - i : startDomain + i;
      const chipIdx = domainIdx * chipsPerDomain + rowIdx;
      if (chipIdx < ordered.length) {
        row.push(cell(ordered[chipIdx]));
      }
    }
    if (row.length > 0) {
      rows.push(row);
    }
  }

  return rows;
}

/**
 * Snake-pattern chip map used on WhatsMiner hashboards.
 * Top half: domains left→right. Bottom half: domains right→left (D0/C0 at bottom-right).
 */
export function buildWhatsminerChipGrid(
  chips: DomainChipInput[],
  chipsPerDomain: number,
): ChipCell[][] {
  if (chips.length === 0 || chipsPerDomain <= 0) {
    return [];
  }

  const ordered = sortedChips(chips);
  const numDomains = Math.ceil(ordered.length / chipsPerDomain);
  const { bottomDomains, topDomains } = snakeSections(numDomains);
  const grid: ChipCell[][] = [];

  if (topDomains > 0) {
    grid.push(
      ...renderSection(
        ordered,
        chipsPerDomain,
        bottomDomains,
        numDomains,
        false,
      ),
    );
  }

  grid.push(
    ...renderSection(ordered, chipsPerDomain, 0, bottomDomains, true),
  );

  return grid;
}
