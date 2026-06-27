<script lang="ts">
  import { isTauri } from "@tauri-apps/api/core";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { onMount } from "svelte";
  import { t, type Locale, type MessageKey } from "$lib/i18n";
  import { formatWindowTitle, syncWindowChrome } from "$lib/windowChrome";
  import type { Theme } from "$lib/types";

  let {
    locale,
    theme,
    product,
    version,
    build,
  }: {
    locale: Locale;
    theme: Theme;
    product: string;
    version: string;
    build: number;
  } = $props();

  let enabled = $state(false);
  let maximized = $state(false);

  const titleText = $derived(formatWindowTitle(product, version, build));

  function msg(key: MessageKey) {
    return t(locale, key);
  }

  onMount(() => {
    enabled = isTauri();
    if (!enabled) return;

    const window = getCurrentWindow();
    let unlisten: (() => void) | undefined;

    void (async () => {
      maximized = await window.isMaximized();
      unlisten = await window.onResized(async () => {
        maximized = await window.isMaximized();
      });
    })();

    return () => {
      unlisten?.();
    };
  });

  $effect(() => {
    if (!enabled) return;
    titleText;
    theme;
    void syncWindowChrome({ title: titleText, theme }).catch(() => {
      /* web preview / unsupported */
    });
  });

  async function minimizeWindow() {
    await getCurrentWindow().minimize();
  }

  async function toggleMaximizeWindow() {
    await getCurrentWindow().toggleMaximize();
    maximized = await getCurrentWindow().isMaximized();
  }

  async function closeWindow() {
    await getCurrentWindow().close();
  }

  function onCaptionDoubleClick(event: MouseEvent) {
    if ((event.target as HTMLElement).closest(".window-caption-controls")) return;
    void toggleMaximizeWindow();
  }
</script>

{#if enabled}
  <header class="window-caption">
    <div
      class="window-caption-brand"
      data-tauri-drag-region
      ondblclick={onCaptionDoubleClick}
    >
      <img class="window-caption-icon" src="/logo.png" width="16" height="16" alt="" />
      <span class="window-caption-name">{product}</span>
      <span class="window-caption-version">{version} ({build})</span>
    </div>

    <div class="window-caption-controls">
      <button
        type="button"
        class="window-caption-btn"
        aria-label={msg("window.minimize")}
        onclick={minimizeWindow}
      >
        <svg viewBox="0 0 12 12" aria-hidden="true"><path d="M2 6h8" /></svg>
      </button>
      <button
        type="button"
        class="window-caption-btn"
        aria-label={maximized ? msg("window.restore") : msg("window.maximize")}
        onclick={toggleMaximizeWindow}
      >
        {#if maximized}
          <svg viewBox="0 0 12 12" aria-hidden="true">
            <path d="M3.5 3.5h5v5h-5z M2 4.5V2h2.5" fill="none" />
          </svg>
        {:else}
          <svg viewBox="0 0 12 12" aria-hidden="true">
            <path d="M2.5 2.5h7v7h-7z" fill="none" />
          </svg>
        {/if}
      </button>
      <button
        type="button"
        class="window-caption-btn window-caption-btn-close"
        aria-label={msg("window.close")}
        onclick={closeWindow}
      >
        <svg viewBox="0 0 12 12" aria-hidden="true">
          <path d="M2.5 2.5l7 7M9.5 2.5l-7 7" />
        </svg>
      </button>
    </div>
  </header>
{/if}
