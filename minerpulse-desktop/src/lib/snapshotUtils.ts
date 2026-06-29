import type { MinerSnapshot } from "$lib/types";

/** True when the miner response carries no usable telemetry. */
export function isSnapshotEmpty(snapshot: MinerSnapshot): boolean {
  const { hashrate, boards, pools, board_chips, raw_log } = snapshot;
  if (
    hashrate.current_ghs > 0 ||
    hashrate.avg_ghs > 0 ||
    hashrate.avg5s_ghs > 0
  ) {
    return false;
  }
  if ((boards?.length ?? 0) > 0) return false;
  if ((pools?.length ?? 0) > 0) return false;
  if ((board_chips?.length ?? 0) > 0) return false;
  if ((raw_log?.trim().length ?? 0) > 0) return false;
  return true;
}
