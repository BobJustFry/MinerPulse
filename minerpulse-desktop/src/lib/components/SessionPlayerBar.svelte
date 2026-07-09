<script lang="ts">
  import { t, type Locale, type MessageKey } from "$lib/i18n";
  import ToolbarBtn from "$lib/components/ToolbarBtn.svelte";
  import type { Density } from "$lib/types";
  import {
    formatTimelineMs,
    PLAYBACK_SPEEDS,
    type PlaybackSpeed,
  } from "$lib/sessionPlayer";

  let {
    locale,
    density = "comfortable",
    visible,
    playing,
    speed,
    frameIndex,
    frameTotal,
    currentMs,
    durationMs,
    fileLabel,
    onPlay,
    onPause,
    onStop,
    onClose,
    onSeek,
    onSpeed,
  }: {
    locale: Locale;
    density?: Density;
    visible: boolean;
    playing: boolean;
    speed: PlaybackSpeed;
    frameIndex: number;
    frameTotal: number;
    currentMs: number;
    durationMs: number;
    fileLabel: string;
    onPlay: () => void;
    onPause: () => void;
    onStop: () => void;
    onClose: () => void;
    onSeek: (progress: number) => void;
    onSpeed: (speed: PlaybackSpeed) => void;
  } = $props();

  let scrubbing = $state(false);
  let scrubProgress = $state(0);
  let resumeAfterScrub = $state(false);

  function msg(key: MessageKey, args?: Record<string, string | number>) {
    return t(locale, key, args);
  }

  let progress = $derived(
    durationMs > 0 ? currentMs / durationMs : frameTotal > 1 ? frameIndex / (frameTotal - 1) : 0,
  );

  let sliderValue = $derived(
    scrubbing ? scrubProgress : Math.round(Math.max(0, Math.min(progress, 1)) * 1000),
  );

  function handleScrubStart() {
    resumeAfterScrub = playing;
    if (playing) onPause();
    scrubbing = true;
  }

  function handleScrubInput(event: Event) {
    const value = Number((event.currentTarget as HTMLInputElement).value);
    scrubProgress = value;
    onSeek(value / 1000);
  }

  function handleScrubRelease() {
    scrubbing = false;
    if (resumeAfterScrub) onPlay();
    resumeAfterScrub = false;
  }
</script>

{#if visible}
  <section class="session-player" aria-label={msg("player.title")}>
    <div class="session-player-top">
      <div class="session-player-meta">
        <span class="session-player-label">{msg("player.title")}</span>
        {#if fileLabel}
          <span class="session-player-file">{fileLabel}</span>
        {/if}
      </div>
      <ToolbarBtn
        class="ghost session-player-close"
        label={msg("player.close")}
        icon="close"
        {density}
        onclick={onClose}
      />
    </div>

    <div class="session-player-controls">
      {#if playing}
        <ToolbarBtn label={msg("toolbar.pause")} icon="pause" {density} onclick={onPause} />
      {:else}
        <ToolbarBtn
          class="primary"
          label={msg("toolbar.play")}
          icon="play"
          {density}
          onclick={onPlay}
        />
      {/if}
      <ToolbarBtn label={msg("player.rewind")} icon="rewind" {density} onclick={onStop} />
    </div>

    <div class="session-player-timeline">
      <span class="session-player-time">{formatTimelineMs(currentMs)}</span>
      <input
        class="session-player-range"
        type="range"
        min="0"
        max="1000"
        step="1"
        value={sliderValue}
        aria-label={msg("player.timeline")}
        onpointerdown={handleScrubStart}
        oninput={handleScrubInput}
        onchange={handleScrubRelease}
        onmouseup={handleScrubRelease}
        ontouchend={handleScrubRelease}
      />
      <span class="session-player-time">{formatTimelineMs(durationMs)}</span>
    </div>

    <div class="session-player-footer">
      <span class="session-player-frame">
        {msg("player.frame", { current: frameIndex + 1, total: frameTotal })}
      </span>
      <div class="session-player-speeds" role="group" aria-label={msg("player.speed")}>
        {#each PLAYBACK_SPEEDS as option (option)}
          <button
            type="button"
            class="session-player-speed"
            class:active={speed === option}
            onclick={() => onSpeed(option)}
          >
            {option}x
          </button>
        {/each}
      </div>
    </div>
  </section>
{/if}
