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
