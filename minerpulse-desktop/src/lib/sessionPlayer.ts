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

export type SessionFrameLoader = (index: number) => Promise<MpulseFrame>;

export interface SessionTimeline {
  frame_count: number;
  timeline_ms: number[];
}

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
  private timeline: SessionTimeline | null = null;
  private frameLoader: SessionFrameLoader | null = null;
  private index = 0;
  private timer: ReturnType<typeof setTimeout> | null = null;
  private playing = false;
  private playbackSpeed: PlaybackSpeed = 1;
  private emitSerial = 0;
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
    this.timeline = null;
    this.frameLoader = null;
    this.frames = frames;
    this.index = 0;
    this.playbackSpeed = 1;
    if (frames.length > 0) {
      void this.emitFrame(false);
    } else {
      this.emitState();
    }
  }

  loadRemote(timeline: SessionTimeline, loader: SessionFrameLoader) {
    this.pause();
    this.frames = [];
    this.timeline = timeline;
    this.frameLoader = loader;
    this.index = 0;
    this.playbackSpeed = 1;
    if (timeline.frame_count > 0) {
      void this.emitFrame(false);
    } else {
      this.emitState();
    }
  }

  getState(): SessionPlayerState {
    const origin = this.originMs();
    const last = this.timelineMsAt(this.frameCount() - 1) ?? origin;
    const current = this.timelineMsAt(this.index) ?? origin;
    return {
      index: this.index,
      total: this.frameCount(),
      current_ms: Math.max(0, current - origin),
      duration_ms: Math.max(0, last - origin),
      playing: this.playing,
      speed: this.playbackSpeed,
    };
  }

  private frameCount(): number {
    return this.timeline?.frame_count ?? this.frames.length;
  }

  private originMs(): number {
    return this.timeline?.timeline_ms[0] ?? this.frames[0]?.t_ms ?? 0;
  }

  private timelineMsAt(index: number): number | null {
    if (this.timeline) {
      return this.timeline.timeline_ms[index] ?? null;
    }
    return this.frames[index]?.t_ms ?? null;
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
    if (this.frameCount() === 0) return;
    this.index = Math.max(0, Math.min(index, this.frameCount() - 1));
    void this.emitFrame(false);
  }

  seekToProgress(progress: number) {
    if (this.frameCount() === 0) return;
    const clamped = Math.max(0, Math.min(progress, 1));
    const origin = this.originMs();
    const duration = this.getState().duration_ms;
    const targetMs = origin + duration * clamped;
    let bestIndex = 0;
    const count = this.frameCount();
    for (let i = 0; i < count; i += 1) {
      const t = this.timelineMsAt(i) ?? origin;
      if (t <= targetMs) {
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
    if (this.frameCount() === 0) return;
    this.pause();
    this.playing = true;
    this.index = Math.max(0, Math.min(fromIndex, this.frameCount() - 1));
    void this.emitFrame(false);
    if (this.index >= this.frameCount() - 1) {
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
    if (this.frameCount() > 0) {
      this.index = 0;
      void this.emitFrame(false);
    }
  }

  unload() {
    this.pause();
    this.frames = [];
    this.timeline = null;
    this.frameLoader = null;
    this.index = 0;
    this.playbackSpeed = 1;
    this.emitState();
  }

  private scheduleAdvance() {
    if (!this.playing || this.frameCount() < 2) return;
    const currentMs = this.timelineMsAt(this.index);
    const nextMs = this.timelineMsAt(this.index + 1);
    if (currentMs == null || nextMs == null) return;
    const gap = Math.max(0, nextMs - currentMs);
    const delay = gap / this.playbackSpeed;
    this.timer = setTimeout(() => {
      if (!this.playing) return;
      this.index += 1;
      void this.emitFrame(false);
      if (this.index >= this.frameCount() - 1) {
        this.pause();
        this.onEnd();
        return;
      }
      this.scheduleAdvance();
    }, delay);
  }

  private async resolveFrame(index: number): Promise<MpulseFrame | null> {
    if (this.frameLoader) {
      return this.frameLoader(index);
    }
    return this.frames[index] ?? null;
  }

  private async emitFrame(notifyPlayingState: boolean) {
    const serial = ++this.emitSerial;
    const frame = await this.resolveFrame(this.index);
    if (serial !== this.emitSerial || frame == null) return;
    this.onFrame(frame, this.index, this.frameCount());
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
