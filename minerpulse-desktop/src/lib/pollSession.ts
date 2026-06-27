import { invoke } from "@tauri-apps/api/core";
import type { MpulseFile } from "$lib/sessionPlayer";

export const POLL_RATES_HZ = [1, 3, 5, 10, 15] as const;
export type PollRateHz = (typeof POLL_RATES_HZ)[number];
export const DEFAULT_POLL_RATE_HZ: PollRateHz = 1;

export interface PollSnapshotEvent {
  snapshot: import("$lib/types").MinerSnapshot;
  t_ms: number;
  frame_index: number;
  recording: boolean;
}

export interface PollFinishedEvent {
  reason: string;
  saved_path?: string | null;
  frame_count: number;
  error?: string | null;
}

export interface PollStatus {
  running: boolean;
  recording: boolean;
}

export function isPollRateHz(value: number): value is PollRateHz {
  return (POLL_RATES_HZ as readonly number[]).includes(value);
}

export async function startPoll(options: {
  ip: string;
  port: number;
  recordPath?: string | null;
  pollRateHz?: PollRateHz;
}): Promise<void> {
  await invoke("start_poll", {
    request: {
      ip: options.ip,
      port: options.port,
      poll_rate_hz: options.pollRateHz ?? DEFAULT_POLL_RATE_HZ,
      record_path: options.recordPath ?? null,
    },
  });
}

export async function stopPoll(): Promise<void> {
  await invoke("stop_poll");
}

export async function getPollStatus(): Promise<PollStatus> {
  return invoke<PollStatus>("get_poll_status");
}

export async function loadSessionFile(path: string): Promise<MpulseFile> {
  return invoke<MpulseFile>("load_session_file", { path });
}
