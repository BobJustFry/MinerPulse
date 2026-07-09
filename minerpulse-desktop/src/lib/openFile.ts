import { invoke } from "@tauri-apps/api/core";
import type { MinerSnapshot } from "$lib/types";
import type { ChartPoint } from "$lib/chartHistory";
import type { MpulseFrame } from "$lib/sessionPlayer";

export interface StoredChartPoint {
  t_ms: number;
  hashrate_ghs: number;
  board_temps: number[];
  power_w: number | null;
  fan_rpm: number | null;
}

export interface OpenedSessionPayload {
  miner_ip: string;
  driver_id: string;
  poll_rate_hz?: number | null;
  frame_count: number;
  timeline_ms: number[];
  chart_points: StoredChartPoint[];
  file_label: string;
}

export type OpenMinerFileResponse =
  | ({ kind: "session" } & OpenedSessionPayload)
  | {
      kind: "snapshot";
      snapshot: MinerSnapshot;
      source_label: string;
      miner_ip?: string | null;
    }
  | {
      kind: "log";
      snapshot: MinerSnapshot;
      source_label: string;
      miner_ip?: string | null;
    };

export const MINER_FILE_EXTENSIONS = [
  "mprs",
  "mpsn",
  "mpulse-session",
  "mpulse-snap",
  "mpulse",
  "txt",
  "log",
  "json",
] as const;

export function chartPointsFromStored(points: StoredChartPoint[]): ChartPoint[] {
  return points.map((point) => ({
    t_ms: point.t_ms,
    hashrate_ghs: point.hashrate_ghs,
    board_temps: point.board_temps,
    power_w: point.power_w,
    fan_rpm: point.fan_rpm,
  }));
}

export async function openMinerFile(path: string): Promise<OpenMinerFileResponse> {
  return invoke<OpenMinerFileResponse>("open_miner_file", { path });
}

export async function getSessionFrame(index: number): Promise<MpulseFrame> {
  return invoke<MpulseFrame>("get_session_frame", { index });
}

export async function closeOpenedSession(): Promise<void> {
  await invoke("close_opened_session");
}

export function defaultSessionSaveName(): string {
  return `minerpulse-${Date.now()}.mprs`;
}

export function defaultSnapshotSaveName(): string {
  return `minerpulse-${Date.now()}.mpsn`;
}
