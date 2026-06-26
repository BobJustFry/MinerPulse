import type { MinerSnapshot } from "$lib/types";

export const MAX_IMPORT_BYTES = 1024 * 1024;

export interface ParseImportResponse {
  snapshot: MinerSnapshot;
  source_label: string;
  miner_ip?: string | null;
}

export function isImportCandidate(file: File | undefined | null): file is File {
  if (!file) return false;
  if (file.size <= 0 || file.size > MAX_IMPORT_BYTES) return false;
  return true;
}
