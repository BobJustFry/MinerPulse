import type { MinerSnapshot } from "$lib/types";

export interface MpulseFrame {
  t_ms: number;
  snapshot: MinerSnapshot;
  raw_log?: string;
}

export interface MpulseFile {
  format_version: number;
  kind: "snapshot" | "session";
  miner_ip: string;
  driver_id: string;
  interval_sec?: number | null;
  poll_rate_hz?: number | null;
  frames: MpulseFrame[];
}

export type PlaybackSpeed = 0.5 | 1 | 2 | 4 | 8;

export const PLAYBACK_SPEEDS: PlaybackSpeed[] = [0.5, 1, 2, 4, 8];

export interface SessionPlayerState {
  index: number;
  total: number;
  current_ms: number;
  duration_ms: number;
  playing: boolean;
  speed: PlaybackSpeed;
}

export type SessionPlayerListener = (
  frame: MpulseFrame,
  index: number,
  total: number,
) => void;

export function formatTimelineMs(ms: number): string {
  const totalSec = Math.max(0, Math.floor(ms / 1000));
  const hours = Math.floor(totalSec / 3600);
  const minutes = Math.floor((totalSec % 3600) / 60);
  const seconds = totalSec % 60;
  if (hours > 0) {
    return `${hours}:${minutes.toString().padStart(2, "0")}:${seconds
      .toString()
      .padStart(2, "0")}`;
  }
  return `${minutes}:${seconds.toString().padStart(2, "0")}`;
}

export class SessionPlayer {
  private frames: MpulseFrame[] = [];
  private index = 0;
  private timer: ReturnType<typeof setTimeout> | null = null;
  private playing = false;
  private playbackSpeed: PlaybackSpeed = 1;
  private onFrame: SessionPlayerListener;
  private onEnd: () => void;
  private onState: (state: SessionPlayerState) => void;

  constructor(
    onFrame: SessionPlayerListener,
    onEnd: () => void,
    onState: (state: SessionPlayerState) => void,
  ) {
    this.onFrame = onFrame;
    this.onEnd = onEnd;
    this.onState = onState;
  }

  load(frames: MpulseFrame[]) {
    this.pause();
    this.frames = frames;
    this.index = 0;
    this.playbackSpeed = 1;
    if (frames.length > 0) {
      this.emitFrame(false);
    } else {
      this.emitState();
    }
  }

  getState(): SessionPlayerState {
    const origin = this.frames[0]?.t_ms ?? 0;
    const last = this.frames[this.frames.length - 1]?.t_ms ?? origin;
    return {
      index: this.index,
      total: this.frames.length,
      current_ms: Math.max(0, (this.frames[this.index]?.t_ms ?? origin) - origin),
      duration_ms: Math.max(0, last - origin),
      playing: this.playing,
      speed: this.playbackSpeed,
    };
  }

  get frameCount(): number {
    return this.frames.length;
  }

  get currentIndex(): number {
    return this.index;
  }

  get isPlaying(): boolean {
    return this.playing;
  }

  get speed(): PlaybackSpeed {
    return this.playbackSpeed;
  }

  show(index: number) {
    if (this.frames.length === 0) return;
    this.index = Math.max(0, Math.min(index, this.frames.length - 1));
    this.emitFrame(false);
  }

  seekToProgress(progress: number) {
    if (this.frames.length === 0) return;
    const clamped = Math.max(0, Math.min(progress, 1));
    const origin = this.frames[0].t_ms;
    const duration = this.getState().duration_ms;
    const targetMs = origin + duration * clamped;
    let bestIndex = 0;
    for (let i = 0; i < this.frames.length; i += 1) {
      if (this.frames[i].t_ms <= targetMs) {
        bestIndex = i;
      } else {
        break;
      }
    }
    this.pause();
    this.show(bestIndex);
  }

  setSpeed(next: PlaybackSpeed) {
    const wasPlaying = this.playing;
    this.playbackSpeed = next;
    if (wasPlaying) {
      this.pause();
      this.play(this.index);
      return;
    }
    this.emitState();
  }

  play(fromIndex = this.index) {
    if (this.frames.length === 0) return;
    this.pause();
    this.playing = true;
    this.index = Math.max(0, Math.min(fromIndex, this.frames.length - 1));
    this.emitFrame(false);
    if (this.index >= this.frames.length - 1) {
      this.pause();
      this.onEnd();
      return;
    }
    this.scheduleAdvance();
  }

  pause() {
    this.playing = false;
    if (this.timer != null) {
      clearTimeout(this.timer);
      this.timer = null;
    }
    this.emitState();
  }

  stop() {
    this.pause();
    if (this.frames.length > 0) {
      this.index = 0;
      this.emitFrame(false);
    }
  }

  unload() {
    this.pause();
    this.frames = [];
    this.index = 0;
    this.playbackSpeed = 1;
    this.emitState();
  }

  private scheduleAdvance() {
    if (!this.playing || this.frames.length < 2) return;
    const current = this.frames[this.index];
    const next = this.frames[this.index + 1];
    const gap = Math.max(0, next.t_ms - current.t_ms);
    const delay = gap / this.playbackSpeed;
    this.timer = setTimeout(() => {
      if (!this.playing) return;
      this.index += 1;
      this.emitFrame(false);
      if (this.index >= this.frames.length - 1) {
        this.pause();
        this.onEnd();
        return;
      }
      this.scheduleAdvance();
    }, delay);
  }

  private emitFrame(notifyPlayingState: boolean) {
    this.onFrame(this.frames[this.index], this.index, this.frames.length);
    if (notifyPlayingState) {
      this.emitState();
    } else {
      this.onState(this.getState());
    }
  }

  private emitState() {
    this.onState(this.getState());
  }
}
