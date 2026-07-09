<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { emit, listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { onMount } from "svelte";
  import WindowCaption from "$lib/components/WindowCaption.svelte";
  import { t, type Locale, type MessageKey } from "$lib/i18n";
  import type { Theme } from "$lib/types";
  import type {
    DiscoveredMiner,
    ErrorResponse,
    ScanSubnet,
  } from "$lib/types";

  const CUSTOM_SUBNET_ID = "custom";
  const FAVORITE_PREFIX = "fav:";
  const SCAN_CONFIG_KEY = "minerpulse.scanConfig";
  const SCAN_FAVORITES_KEY = "minerpulse.scanFavorites";
  const SCAN_RESULTS_KEY = "minerpulse.scan";

  interface ScanConfig {
    selectedSubnetId: string;
    customStart: string;
    customEnd: string;
    port: string;
  }

  interface FavoriteRange {
    id: string;
    start_ip: string;
    end_ip: string;
    label: string;
  }

  interface ScanProgressPayload {
    scanned: number;
    total: number;
    found_count: number;
    range_label: string;
  }

  interface ScanFinishedPayload {
    cancelled: boolean;
    error?: string | null;
    scanned: number;
    total: number;
    found_count: number;
    range_label: string;
  }

  let locale = $state<Locale>("ru");
  let theme = $state<Theme>("dark");
  let appProduct = $state("Miner Pulse");
  let appVersionNumber = $state("0.0.0");
  let appBuild = $state(0);
  let subnets = $state<ScanSubnet[]>([]);
  let favorites = $state<FavoriteRange[]>([]);
  let selectedSubnetId = $state("");
  let customStart = $state("192.168.0.1");
  let customEnd = $state("192.168.0.254");
  let port = $state("4028");
  let scanning = $state(false);
  let statusText = $state("");
  let discovered = $state<DiscoveredMiner[]>([]);
  let progressScanned = $state(0);
  let progressTotal = $state(0);
  let progressFound = $state(0);
  let progressRange = $state("");
  let hasScanned = $state(false);
  let prefsLoaded = $state(false);

  let unlistenProgress: UnlistenFn | undefined;
  let unlistenFound: UnlistenFn | undefined;
  let unlistenFinished: UnlistenFn | undefined;

  function msg(key: MessageKey, args?: Record<string, string | number>) {
    return t(locale, key, args);
  }

  function applyUiPrefs() {
    const saved = localStorage.getItem("minerpulse.ui");
    if (!saved) return;
    try {
      const parsed = JSON.parse(saved);
      locale = parsed.locale ?? locale;
      theme = parsed.theme === "light" ? "light" : "dark";
      document.documentElement.dataset.theme = theme;
      document.documentElement.dataset.density = parsed.density ?? "comfortable";
      document.documentElement.lang = locale === "zh-CN" ? "zh-CN" : locale;
    } catch {
      /* ignore */
    }
  }

  async function closeWindow() {
    await getCurrentWindow().close();
  }

  function isFavoriteId(id: string): boolean {
    return id.startsWith(FAVORITE_PREFIX);
  }

  function favoriteId(start: string, end: string): string {
    return `${FAVORITE_PREFIX}${start}-${end}`;
  }

  function isValidIpv4(ip: string): boolean {
    const parts = ip.trim().split(".");
    if (parts.length !== 4) return false;
    return parts.every((part) => {
      if (!/^\d{1,3}$/.test(part)) return false;
      const value = Number(part);
      return value >= 0 && value <= 255;
    });
  }

  function isValidRange(start: string, end: string): boolean {
    if (!isValidIpv4(start) || !isValidIpv4(end)) return false;
    return compareIp(start.trim(), end.trim()) <= 0;
  }

  function loadFavorites(): FavoriteRange[] {
    const saved = localStorage.getItem(SCAN_FAVORITES_KEY);
    if (!saved) return [];
    try {
      const parsed = JSON.parse(saved) as FavoriteRange[];
      if (!Array.isArray(parsed)) return [];
      return parsed.filter(
        (item) =>
          isFavoriteId(item.id) &&
          isValidRange(item.start_ip, item.end_ip) &&
          item.label.trim().length > 0,
      );
    } catch {
      return [];
    }
  }

  function saveFavorites() {
    if (!prefsLoaded) return;
    localStorage.setItem(SCAN_FAVORITES_KEY, JSON.stringify(favorites));
  }

  function addFavoriteRange() {
    const start = customStart.trim();
    const end = customEnd.trim();
    if (!isValidRange(start, end)) {
      statusText = msg("scan.favoriteInvalid");
      return;
    }
    const id = favoriteId(start, end);
    if (favorites.some((item) => item.id === id)) {
      statusText = msg("scan.favoriteDuplicate");
      selectedSubnetId = id;
      return;
    }
    const next: FavoriteRange = {
      id,
      start_ip: start,
      end_ip: end,
      label: `${start} — ${end}`,
    };
    favorites = [...favorites, next].sort((a, b) => compareIp(a.start_ip, b.start_ip));
    selectedSubnetId = id;
    statusText = msg("scan.favoriteAdded");
    saveFavorites();
  }

  function removeSelectedFavorite() {
    if (!isFavoriteId(selectedSubnetId)) return;
    favorites = favorites.filter((item) => item.id !== selectedSubnetId);
    selectedSubnetId = CUSTOM_SUBNET_ID;
    saveFavorites();
    statusText = msg("scan.favoriteRemoved");
  }

  function loadScanPrefs(subnetList: ScanSubnet[]) {
    favorites = loadFavorites();
    const savedConfig = localStorage.getItem(SCAN_CONFIG_KEY);
    if (savedConfig) {
      try {
        const cfg = JSON.parse(savedConfig) as ScanConfig;
        if (
          cfg.selectedSubnetId === CUSTOM_SUBNET_ID ||
          subnetList.some((subnet) => subnet.id === cfg.selectedSubnetId) ||
          favorites.some((favorite) => favorite.id === cfg.selectedSubnetId)
        ) {
          selectedSubnetId = cfg.selectedSubnetId;
        }
        customStart = cfg.customStart ?? customStart;
        customEnd = cfg.customEnd ?? customEnd;
        port = cfg.port ?? port;
      } catch {
        /* ignore */
      }
    }

    const savedResults = localStorage.getItem(SCAN_RESULTS_KEY);
    if (savedResults) {
      try {
        const miners = JSON.parse(savedResults) as DiscoveredMiner[];
        if (Array.isArray(miners) && miners.length > 0) {
          discovered = miners;
          progressFound = miners.length;
        }
      } catch {
        /* ignore */
      }
    }
  }

  function saveScanConfig() {
    if (!prefsLoaded) return;
    localStorage.setItem(
      SCAN_CONFIG_KEY,
      JSON.stringify({
        selectedSubnetId,
        customStart,
        customEnd,
        port,
      } satisfies ScanConfig),
    );
  }

  $effect(() => {
    if (!prefsLoaded) return;
    selectedSubnetId;
    customStart;
    customEnd;
    port;
    saveScanConfig();
  });

  function formatError(err: unknown): string {
    const e = err as ErrorResponse;
    if (e?.code) {
      const key = `error.${e.code}` as MessageKey;
      return msg(key);
    }
    return String(err);
  }

  function vendorLabel(vendor: string) {
    const map: Record<string, string> = {
      avalon: "Avalon",
      antminer: "Antminer",
      whatsminer: "WhatsMiner",
      innosilicon: "Innosilicon",
      generic: "CGMiner",
      unknown: "?",
    };
    return map[vendor] ?? vendor;
  }

  const KNOWN_SCAN_VENDORS = new Set(["avalon", "antminer", "whatsminer"]);

  function isKnownMiner(miner: DiscoveredMiner): boolean {
    if (miner.supported) return true;
    return KNOWN_SCAN_VENDORS.has(miner.vendor.toLowerCase());
  }

  function ipSortKey(ip: string): number[] {
    return ip.split(".").map((part) => Number(part) || 0);
  }

  function compareIp(a: string, b: string): number {
    const ka = ipSortKey(a);
    const kb = ipSortKey(b);
    for (let i = 0; i < 4; i += 1) {
      if (ka[i] !== kb[i]) return ka[i] - kb[i];
    }
    return 0;
  }

  function addDiscoveredMiner(miner: DiscoveredMiner) {
    if (discovered.some((item) => item.ip === miner.ip)) return;
    discovered = [...discovered, miner].sort((a, b) => compareIp(a.ip, b.ip));
  }

  function progressPercent(): number {
    if (progressTotal <= 0) return 0;
    return Math.min(100, Math.round((progressScanned / progressTotal) * 100));
  }

  function selectedSubnet(): ScanSubnet | undefined {
    return subnets.find((s) => s.id === selectedSubnetId);
  }

  function selectedFavorite(): FavoriteRange | undefined {
    return favorites.find((item) => item.id === selectedSubnetId);
  }

  function buildScanRequest() {
    const scanPort = Number(port) || 4028;
    const favorite = selectedFavorite();
    if (favorite) {
      return {
        port: scanPort,
        start_ip: favorite.start_ip,
        end_ip: favorite.end_ip,
      };
    }
    if (selectedSubnetId === CUSTOM_SUBNET_ID) {
      return {
        port: scanPort,
        start_ip: customStart.trim(),
        end_ip: customEnd.trim(),
      };
    }
    const subnet = selectedSubnet();
    if (!subnet) {
      return { port: scanPort };
    }
    return {
      port: scanPort,
      start_ip: subnet.start_ip,
      end_ip: subnet.end_ip,
    };
  }

  function applyProgress(payload: ScanProgressPayload) {
    progressScanned = payload.scanned;
    progressTotal = payload.total;
    progressFound = payload.found_count;
    progressRange = payload.range_label;
    statusText = msg("scan.progress", {
      scanned: payload.scanned,
      total: payload.total,
      found: payload.found_count,
      percent: progressPercent(),
    });
  }

  function finishScan(payload: ScanFinishedPayload) {
    scanning = false;
    hasScanned = true;
    progressScanned = payload.scanned;
    progressTotal = payload.total;
    progressFound = payload.found_count;
    progressRange = payload.range_label;

    if (payload.error) {
      statusText = formatError({ code: payload.error });
      return;
    }

    localStorage.setItem("minerpulse.scan", JSON.stringify(discovered));
    saveScanConfig();

    if (payload.cancelled) {
      statusText =
        payload.found_count > 0
          ? msg("scan.cancelledWithResults", { count: payload.found_count })
          : msg("scan.cancelled");
      return;
    }

    if (payload.found_count === 0) {
      statusText = msg("status.scanEmpty", { range: payload.range_label });
    } else {
      statusText = msg("status.scanDone", {
        count: payload.found_count,
        scanned: payload.scanned,
      });
    }
  }

  async function runScan() {
    scanning = true;
    hasScanned = false;
    discovered = [];
    progressScanned = 0;
    progressTotal = 0;
    progressFound = 0;
    progressRange = "";
    statusText = msg("status.scanning");

    try {
      const request = buildScanRequest();
      await invoke("start_scan", { request });
    } catch (err) {
      scanning = false;
      statusText = formatError(err);
    }
  }

  async function cancelScan() {
    if (!scanning) return;
    statusText = msg("scan.cancelling");
    await invoke("cancel_scan");
  }

  async function pickMiner(miner: DiscoveredMiner) {
    await emit("miner-selected", {
      ip: miner.ip,
      port: miner.port,
      model: miner.model || vendorLabel(miner.vendor),
    });
    await getCurrentWindow().close();
  }

  onMount(async () => {
    applyUiPrefs();
    try {
      const v = await invoke<{ display: string; version: string; build: number; product: string }>(
        "get_app_version",
      );
      appProduct = v.product;
      appVersionNumber = v.version;
      appBuild = v.build;
    } catch {
      /* ignore in web preview */
    }
    subnets = await invoke<ScanSubnet[]>("list_scan_subnets");
    selectedSubnetId = subnets[0]?.id ?? CUSTOM_SUBNET_ID;
    loadScanPrefs(subnets);
    prefsLoaded = true;
    statusText = msg("scan.hint");

    unlistenProgress = await listen<ScanProgressPayload>("scan://progress", (event) => {
      applyProgress(event.payload);
    });
    unlistenFound = await listen<DiscoveredMiner>("scan://found", (event) => {
      addDiscoveredMiner(event.payload);
      progressFound = discovered.length;
    });
    unlistenFinished = await listen<ScanFinishedPayload>("scan://finished", (event) => {
      finishScan(event.payload);
    });

    return () => {
      unlistenProgress?.();
      unlistenFound?.();
      unlistenFinished?.();
    };
  });
</script>

<div class="app-shell">
  <WindowCaption {locale} {theme} product={appProduct} version={appVersionNumber} build={appBuild} />

  <div class="scan-window">
    <header class="modal-head scan-window-head">
      <div>
        <div class="modal-kicker">{appProduct}</div>
        <h1 class="modal-title">{msg("scan.title")}</h1>
        <p class="scan-subtitle">{msg("scan.subtitle")}</p>
      </div>
      <button
        type="button"
        class="modal-close"
        onclick={closeWindow}
        aria-label={msg("scan.close")}
      >
        ×
      </button>
    </header>

  <section class="scan-form">
    <label class="scan-field">
      <span>{msg("scan.subnet")}</span>
      <select bind:value={selectedSubnetId} disabled={scanning}>
        <optgroup label={msg("scan.networkGroup")}>
          {#each subnets as subnet (subnet.id)}
            <option value={subnet.id}>{subnet.label}</option>
          {/each}
        </optgroup>
        {#if favorites.length > 0}
          <optgroup label={msg("scan.favoritesGroup")}>
            {#each favorites as favorite (favorite.id)}
              <option value={favorite.id}>{favorite.label}</option>
            {/each}
          </optgroup>
        {/if}
        <option value={CUSTOM_SUBNET_ID}>{msg("scan.customSubnet")}</option>
      </select>
    </label>

    {#if selectedSubnetId === CUSTOM_SUBNET_ID}
      <div class="scan-range-row">
        <label class="scan-field">
          <span>{msg("scan.from")}</span>
          <input bind:value={customStart} disabled={scanning} />
        </label>
        <label class="scan-field">
          <span>{msg("scan.to")}</span>
          <input bind:value={customEnd} disabled={scanning} />
        </label>
        <button
          type="button"
          class="btn scan-favorite-add"
          disabled={scanning}
          title={msg("scan.addFavorite")}
          aria-label={msg("scan.addFavorite")}
          onclick={addFavoriteRange}
        >
          ★
        </button>
      </div>
    {:else if isFavoriteId(selectedSubnetId)}
      <div class="scan-favorite-meta">
        <span class="scan-favorite-label">{selectedFavorite()?.label}</span>
        <button
          type="button"
          class="btn danger scan-favorite-remove"
          disabled={scanning}
          onclick={removeSelectedFavorite}
        >
          {msg("scan.removeFavorite")}
        </button>
      </div>
    {/if}

    <label class="scan-field">
      <span>{msg("toolbar.port")}</span>
      <input class="port" bind:value={port} disabled={scanning} />
    </label>

    <div class="scan-actions">
      <button class="btn primary" disabled={scanning} onclick={runScan}>
        {scanning ? msg("toolbar.scanning") : msg("toolbar.scan")}
      </button>
      {#if scanning}
        <button class="btn" onclick={cancelScan}>{msg("scan.cancel")}</button>
      {/if}
    </div>
  </section>

  <section class="scan-results-wrap">
    {#if scanning}
      <div class="scan-progress-panel">
        <div class="scan-progress-track" role="progressbar" aria-valuemin="0" aria-valuemax="100" aria-valuenow={progressPercent()}>
          <div class="scan-progress-fill" style:width="{progressPercent()}%"></div>
        </div>
        <div class="scan-progress">{statusText}</div>
        {#if progressRange}
          <div class="scan-panel-range">{progressRange}</div>
        {/if}
      </div>
    {:else}
      <div class="scan-status">{statusText}</div>
    {/if}

    {#if discovered.length > 0}
      <div class="scan-results" role="listbox">
        {#each discovered as miner (miner.ip)}
          <button
            type="button"
            class="scan-item"
            class:unsupported={!isKnownMiner(miner)}
            onclick={() => pickMiner(miner)}
          >
            <span class="scan-item-ip">{miner.ip}</span>
            <span class="scan-item-model">
              {miner.model || vendorLabel(miner.vendor)}
            </span>
            {#if isKnownMiner(miner)}
              <span class="scan-item-badge scan-item-badge-select">{msg("scan.select")}</span>
            {:else}
              <span class="scan-item-badge">{msg("scan.preview")}</span>
            {/if}
          </button>
        {/each}
      </div>
      <p class="scan-footnote">{msg("scan.pickHint")}</p>
    {:else if hasScanned && !scanning && discovered.length === 0}
      <div class="scan-empty">{msg("scan.noMiners")}</div>
    {/if}
  </section>
  </div>
</div>
