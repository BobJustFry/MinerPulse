export function parseFreqDomains(value: string): [number, number, number, number] | null {
  const parts = value
    .split(":")
    .map((part) => part.trim())
    .filter(Boolean);
  if (parts.length !== 4) return null;
  const nums = parts.map((part) => Number.parseInt(part, 10));
  if (nums.some((num) => Number.isNaN(num) || num <= 0)) return null;
  return nums as [number, number, number, number];
}

export function formatFreqDomains(values: number[]): string {
  if (values.length !== 4) return "";
  return values.join(":");
}
