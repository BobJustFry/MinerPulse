<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { save } from "@tauri-apps/plugin-dialog";
  import { check } from "@tauri-apps/plugin-updater";
  import { relaunch } from "@tauri-apps/plugin-process";
  import { onMount } from "svelte";
  import { locales, t, type Locale, type MessageKey } from "$lib/i18n";
  import { openScanWindow } from "$lib/openScanWindow";
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
      });
    })();

    return () => {
      unlisten?.();
    };
  });
</script>

<div class="app-shell">
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
        <div class="card-grid">
          <div class="metric-card">
            <div class="metric-label">{msg("data.hashrate")}</div>
            <div class="metric-value">
              {snapshot.hashrate.avg5s_ghs.toFixed(2)} GH/s
            </div>
          </div>
          <div class="metric-card">
            <div class="metric-label">{msg("data.tempIn")}</div>
            <div class="metric-value">
              {snapshot.thermal.inlet_c ?? "—"} °C
            </div>
          </div>
          <div class="metric-card">
            <div class="metric-label">{msg("data.firmware")}</div>
            <div class="metric-value">{snapshot.identity.firmware || "—"}</div>
          </div>
          <div class="metric-card">
            <div class="metric-label">{msg("data.model")}</div>
            <div class="metric-value">{snapshot.identity.model}</div>
          </div>
        </div>
      {:else}
        <div class="locked-panel">{msg("status.noData")}</div>
      {/if}
    {:else if activeTab === "console"}
      <div class="console-box">{snapshot?.raw_log || msg("status.noData")}</div>
    {:else if activeTab === "pools"}
      {#if snapshot?.pools?.length}
        <div class="card-grid">
          {#each snapshot.pools as pool}
            <div class="metric-card">
              <div class="metric-label">{pool.status}</div>
              <div class="metric-value" style="font-size:14px">{pool.url}</div>
            </div>
          {/each}
        </div>
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
