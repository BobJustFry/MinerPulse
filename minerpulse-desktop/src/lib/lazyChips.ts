export const LAZY_CHIP_ACTIVATION_MIN = 100;
export const LAZY_CHIP_MAX_RATIO = 0.5;

export interface ChipSolutionsInput {
  index: number;
  solutions?: number | null;
}

export function isLazyChipDetectionActive(chips: ChipSolutionsInput[]): boolean {
  return chips.some((chip) => (chip.solutions ?? 0) >= LAZY_CHIP_ACTIVATION_MIN);
}

/** Lazy when any chip has >= 100 solutions and this chip has < 50% of board max solutions. */
export function lazyChipIndexes(chips: ChipSolutionsInput[]): Set<number> {
  if (!isLazyChipDetectionActive(chips)) {
    return new Set();
  }

  const values = chips
    .map((chip) => chip.solutions)
    .filter((value): value is number => value != null);
  if (values.length === 0) {
    return new Set();
  }

  const maxSolutions = Math.max(...values);
  const threshold = maxSolutions * LAZY_CHIP_MAX_RATIO;
  const lazy = new Set<number>();

  for (const chip of chips) {
    if (chip.solutions != null && chip.solutions < threshold) {
      lazy.add(chip.index);
    }
  }

  return lazy;
}

export function lazyChipCount(chips: ChipSolutionsInput[]): number {
  return lazyChipIndexes(chips).size;
}
