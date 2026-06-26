import type { Locale } from "$lib/i18n";

export interface WhatsminerErrorL10n {
  en: string;
  ru: string;
  "zh-CN": string;
}

export interface WhatsminerErrorEntry {
  code: string;
  category: string;
  name: WhatsminerErrorL10n;
  description: WhatsminerErrorL10n;
  action: WhatsminerErrorL10n;
}

export interface WhatsminerErrorCatalog {
  version: string;
  source: string;
  errors: WhatsminerErrorEntry[];
}

let catalogPromise: Promise<WhatsminerErrorCatalog> | null = null;

export function loadWhatsminerErrorCatalog(): Promise<WhatsminerErrorCatalog> {
  if (!catalogPromise) {
    catalogPromise = fetch("/whatsminer-errors.json").then((response) => {
      if (!response.ok) {
        throw new Error(`Failed to load error catalog: ${response.status}`);
      }
      return response.json() as Promise<WhatsminerErrorCatalog>;
    });
  }
  return catalogPromise;
}

export function pickLocalizedText(
  locale: Locale,
  text: WhatsminerErrorL10n | undefined,
  fallback = "",
): string {
  if (!text) return fallback;
  return text[locale] ?? text.en ?? text.ru ?? fallback;
}

export function lookupWhatsminerError(
  catalog: WhatsminerErrorCatalog,
  code: string,
): WhatsminerErrorEntry | undefined {
  const normalized = code.trim().toLowerCase();
  return catalog.errors.find((entry) => entry.code.toLowerCase() === normalized);
}

export function chipTempColor(temp: number, min = 60, max = 100): string {
  const ratio = Math.max(0, Math.min(1, (temp - min) / Math.max(1, max - min)));
  const hue = 120 - ratio * 120;
  return `hsl(${hue} 72% 42%)`;
}

export interface ChipCell {
  index: number;
  temp: number;
}

export function buildChipGrid(
  chips: Array<{ index: number; temp_c: number }>,
  chipsPerDomain: number,
): ChipCell[][] {
  if (chips.length === 0 || chipsPerDomain <= 0) {
    return [];
  }

  const numDomains = Math.ceil(chips.length / chipsPerDomain);
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
        const domainIdx = section.reversed
          ? section.end - 1 - i
          : section.start + i;
        const chipIdx = domainIdx * chipsPerDomain + rowIdx;
        if (chipIdx < chips.length) {
          row.push({
            index: chips[chipIdx].index,
            temp: chips[chipIdx].temp_c,
          });
        }
      }
      if (row.length > 0) {
        rows.push(row);
      }
    }
  }

  return rows;
}
