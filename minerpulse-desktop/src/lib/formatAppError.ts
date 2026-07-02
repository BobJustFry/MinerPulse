import { t, type Locale, type MessageKey } from "$lib/i18n";
import type { ErrorResponse } from "$lib/types";

export function formatAppError(
  locale: Locale,
  err: unknown,
  options?: { minReadIntervalSec?: number },
): string {
  const e = err as ErrorResponse;
  if (!e?.code) {
    return String(err);
  }

  const licenseMsg =
    typeof e.args?.message === "string" && e.args.message.length > 0
      ? e.args.message
      : null;

  if (licenseMsg) {
    const licenseKey = `subscription.error.${licenseMsg}` as MessageKey;
    const translated = t(locale, licenseKey);
    if (translated !== licenseKey) {
      return translated;
    }
    return licenseMsg;
  }

  const key = `error.${e.code}` as MessageKey;
  return t(locale, key, {
    sec: e.args?.sec ?? options?.minReadIntervalSec ?? 0,
  });
}
