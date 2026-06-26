<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { save, ask } from "@tauri-apps/plugin-dialog";
  import { check } from "@tauri-apps/plugin-updater";
  import { relaunch } from "@tauri-apps/plugin-process";
  import { onMount } from "svelte";
  import { locales, t, type Locale, type MessageKey } from "$lib/i18n";
  import { openScanWindow } from "$lib/openScanWindow";
  import MinerDataPanel from "$lib/components/MinerDataPanel.svelte";
  import MinerPoolsPanel from "$lib/components/MinerPoolsPanel.svelte";
  import {
    isImportCandidate,
    MAX_IMPORT_BYTES,
    type ParseImportResponse,
  } from "$lib/importFile";
  import type {
    Entitlements,
    MinerSnapshot,
    SubscriptionTier,
    TabId,
    Theme,
    Density,
    ErrorResponse,
  } from "$lib/types";

  let theme = $state<Theme>("light");
  let density = $state<Density>("comfortable");
  let locale = $state<Locale>("ru");
  let activeTab = $state<TabId>("data");
  let ip = $state("192.168.0.100");
  let port = $state("4028");
  let busy = $state(false);
  let statusText = $state("");
  let snapshot = $state<MinerSnapshot | null>(null);
  let appVersion = $state("1.0.0 (build 1)");
  let connectionLoaded = $state(false);
  let dropActive = $state(false);
  let dropDepth = $state(0);
  let entitlements = $state<Entitlements>({
    tier: "free",
    can_poll: false,
    can_record_session: false,
    can_play: false,
    can_show_charts: false,
    can_save_snapshot: true,
    min_read_interval_sec: 10,
  });

  const tabs: TabId[] = ["data", "console", "pools", "charts", "commands"];
  const CONNECTION_KEY = "minerpulse.connection";

  function msg(key: MessageKey, args?: Record<string, string | number>) {
    return t(locale, key, args);
  }

  function applyUiPrefs() {
    document.documentElement.dataset.theme = theme;
    document.documentElement.dataset.density = density;
    document.documentElement.lang = locale === "zh-CN" ? "zh-CN" : locale;
    localStorage.setItem(
      "minerpulse.ui",
      JSON.stringify({ theme, density, locale }),
    );
  }

  function loadConnection() {
    const saved = localStorage.getItem(CONNECTION_KEY);
    if (!saved) return;
    try {
      const parsed = JSON.parse(saved) as { ip?: string; port?: string | number };
      ip = parsed.ip ?? ip;
      port = String(parsed.port ?? port);
    } catch {
      /* ignore */
    }
  }

  function saveConnection() {
    localStorage.setItem(CONNECTION_KEY, JSON.stringify({ ip, port }));
  }

  function tabLabel(tab: TabId) {
    const map: Record<TabId, MessageKey> = {
      data: "tabs.data",
      console: "tabs.console",
      pools: "tabs.pools",
      charts: "tabs.charts",
      commands: "tabs.commands",
    };
    return msg(map[tab]);
  }

  function tierLabel(tier: SubscriptionTier) {
    const map: Record<SubscriptionTier, MessageKey> = {
      free: "tier.free",
      client: "tier.client",
      service: "tier.service",
    };
    return msg(map[tier]);
  }

  function formatError(err: unknown): string {
    const e = err as ErrorResponse;
    if (e?.code) {
      const key = `error.${e.code}` as MessageKey;
      return msg(key, { sec: e.args?.sec ?? entitlements.min_read_interval_sec });
    }
    return String(err);
  }

  async function refreshEntitlements() {
    entitlements = await invoke<Entitlements>("get_entitlements");
  }

  async function openScan() {
    statusText = msg("scan.opening");
    try {
      await openScanWindow();
      statusText = msg("status.ready");
    } catch (err) {
      statusText = formatError(err);
    }
  }

  async function readMiner() {
    busy = true;
    statusText = msg("status.reading");
    try {
      const response = await invoke<{ snapshot: MinerSnapshot }>("read_miner", {
        request: { ip, port: Number(port) || 4028 },
      });
      snapshot = response.snapshot;
      statusText = `${snapshot.identity.model} · ${snapshot.status}`;
    } catch (err) {
      snapshot = null;
      statusText = formatError(err);
    } finally {
      busy = false;
    }
  }

  async function saveSnapshot() {
    if (!snapshot) {
      statusText = msg("error.NO_SNAPSHOT");
      return;
    }

    const path = await save({
      defaultPath: `minerpulse-${Date.now()}.mpulse`,
      filters: [{ name: "MinerPulse", extensions: ["mpulse"] }],
    });

    if (!path) return;

    try {
      const saved = await invoke<string>("save_snapshot_file", {
        request: { ip, path },
      });
      statusText = msg("status.saved", { path: saved });
    } catch (err) {
      statusText = formatError(err);
    }
  }

  function toggleTheme() {
    theme = theme === "light" ? "dark" : "light";
    applyUiPrefs();
  }

  function toggleDensity() {
    density = density === "comfortable" ? "compact" : "comfortable";
    applyUiPrefs();
  }

  function setLocale(next: Locale) {
    locale = next;
    applyUiPrefs();
    statusText = msg("status.ready");
  }

  async function checkForUpdates() {
    busy = true;
    statusText = msg("status.updating");
    try {
      const update = await check();
      if (!update) {
        statusText = msg("status.upToDate");
        return;
      }
      statusText = msg("status.updateAvailable", { version: update.version });
      await update.downloadAndInstall();
      await relaunch();
    } catch (err) {
      statusText = String(err);
    } finally {
      busy = false;
    }
  }

  async function cycleTier() {
    const order: SubscriptionTier[] = ["free", "client", "service"];
    const idx = order.indexOf(entitlements.tier);
    const next = order[(idx + 1) % order.length];
    await invoke("set_tier", { tier: next });
    await refreshEntitlements();
  }

  function handleDragEnter(event: DragEvent) {
    event.preventDefault();
    dropDepth += 1;
    dropActive = true;
  }

  function handleDragLeave(event: DragEvent) {
    event.preventDefault();
    dropDepth = Math.max(0, dropDepth - 1);
    if (dropDepth === 0) {
      dropActive = false;
    }
  }

  function handleDragOver(event: DragEvent) {
    event.preventDefault();
  }

  async function handleDrop(event: DragEvent) {
    event.preventDefault();
    dropDepth = 0;
    dropActive = false;

    const file = event.dataTransfer?.files?.[0];
    if (!file) return;

    if (file.size > MAX_IMPORT_BYTES) {
      statusText = msg("import.tooLarge");
      return;
    }

    if (!isImportCandidate(file)) return;

    try {
      const content = await file.text();
      const result = await invoke<ParseImportResponse>("parse_import_file", {
        content,
        filename: file.name,
      });

      const confirmed = await ask(
        msg("import.openPrompt", {
          name: file.name,
          model: result.snapshot.identity.model,
        }),
        {
          title: msg("import.title"),
          kind: "info",
        },
      );

      if (!confirmed) return;

      snapshot = result.snapshot;
      await invoke("remember_snapshot", { snapshot: result.snapshot });

      if (result.miner_ip) {
        ip = result.miner_ip;
        saveConnection();
      }

      activeTab = "data";
      statusText = msg("import.opened", { name: file.name });
    } catch (err) {
      statusText = formatError(err);
    }
  }

  function tabDisabled(tab: TabId): boolean {
    if (tab === "charts") return !entitlements.can_show_charts;
    if (tab === "commands") return !entitlements.can_poll;
    return false;
  }

  onMount(() => {
    let unlisten: (() => void) | undefined;

    void (async () => {
      const saved = localStorage.getItem("minerpulse.ui");
      if (saved) {
        try {
          const parsed = JSON.parse(saved);
          theme = parsed.theme ?? theme;
          density = parsed.density ?? density;
          locale = parsed.locale ?? locale;
        } catch {
          /* ignore */
        }
      }
      applyUiPrefs();
      loadConnection();
      connectionLoaded = true;
      await refreshEntitlements();
      try {
        const v = await invoke<{ display: string }>("get_app_version");
        appVersion = v.display;
      } catch {
        /* ignore in web preview */
      }
      statusText = msg("status.ready");

      unlisten = await listen<{
        ip: string;
        port: number;
        model: string;
      }>("miner-selected", (event) => {
        ip = event.payload.ip;
        port = String(event.payload.port);
        statusText = `${event.payload.model} · ${event.payload.ip}`;
        saveConnection();
      });
    })();

    return () => {
      unlisten?.();
    };
  });

  $effect(() => {
    if (!connectionLoaded) return;
    ip;
    port;
    saveConnection();
  });
</script>

<div
  class="app-shell"
  class:drop-active={dropActive}
  ondragenter={handleDragEnter}
  ondragleave={handleDragLeave}
  ondragover={handleDragOver}
  ondrop={handleDrop}
>
  {#if dropActive}
    <div class="drop-overlay" aria-hidden="true">
      <div class="drop-overlay-card">
        <div class="drop-overlay-title">{msg("import.dropTitle")}</div>
        <div class="drop-overlay-hint">{msg("import.dropHint")}</div>
      </div>
    </div>
  {/if}
  <header class="toolbar">
    <div class="brand">
      <div class="brand-mark"></div>
      <span>{msg("app.title")}</span>
    </div>

    <div class="field ip-field">
      <label for="ip">{msg("toolbar.ip")}</label>
      <input id="ip" bind:value={ip} />
      <button class="btn" disabled={busy} onclick={openScan}>
        {msg("toolbar.scan")}
      </button>
    </div>
    <div class="field">
      <label for="port">{msg("toolbar.port")}</label>
      <input id="port" class="port" bind:value={port} />
    </div>

    {#if entitlements.can_poll}
      <button class="btn primary" disabled={busy}>{msg("toolbar.start")}</button>
      <button class="btn" disabled>{msg("toolbar.stop")}</button>
    {:else}
      <button class="btn primary" disabled={busy} onclick={readMiner}>
        {msg("toolbar.read")}
      </button>
    {/if}

    <button class="btn" disabled={!snapshot || busy} onclick={saveSnapshot}>
      {msg("toolbar.save")}
    </button>

    <div class="spacer"></div>

    <button class="btn ghost" disabled={busy} onclick={checkForUpdates}>
      {msg("toolbar.update")}
    </button>

    <button class="btn ghost" onclick={toggleTheme}>
      {theme === "light" ? msg("toolbar.themeDark") : msg("toolbar.themeLight")}
    </button>
    <button class="btn ghost" onclick={toggleDensity}>
      {density === "comfortable"
        ? msg("toolbar.densityCompact")
        : msg("toolbar.densityComfortable")}
    </button>

    <select
      class="btn"
      value={locale}
      onchange={(e) => setLocale(e.currentTarget.value as Locale)}
    >
      {#each locales as item}
        <option value={item.id}>{item.label}</option>
      {/each}
    </select>

    <button
      class="tier-badge"
      class:service={entitlements.tier === "service"}
      onclick={cycleTier}
      title="Dev: cycle tier"
    >
      {tierLabel(entitlements.tier)}
    </button>
  </header>

  <nav class="tabs">
    {#each tabs as tab}
      <button
        class="tab"
        class:active={activeTab === tab}
        disabled={tabDisabled(tab)}
        onclick={() => (activeTab = tab)}
      >
        {tabLabel(tab)}
      </button>
    {/each}
  </nav>

  <main class="content">
    {#if activeTab === "data"}
      {#if snapshot}
        <MinerDataPanel {snapshot} {locale} />
      {:else}
        <div class="locked-panel">{msg("status.noData")}</div>
      {/if}
    {:else if activeTab === "console"}
      <div class="console-box">{snapshot?.raw_log || msg("status.noData")}</div>
    {:else if activeTab === "pools"}
      {#if snapshot}
        <MinerPoolsPanel {snapshot} {locale} />
      {:else}
        <div class="locked-panel">{msg("pools.empty")}</div>
      {/if}
    {:else if activeTab === "charts"}
      <div class="locked-panel">{msg("charts.locked")}</div>
    {:else if activeTab === "commands"}
      <div class="locked-panel">{msg("commands.locked")}</div>
    {/if}
  </main>

  <footer class="statusbar">
    <span class="status-dot" class:busy={busy}></span>
    <span>{statusText || msg("status.ready")}</span>
    <span class="spacer"></span>
    <span>{appVersion}</span>
  </footer>
</div>
