<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { emit, listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { onMount } from "svelte";
  import { t, type Locale, type MessageKey } from "$lib/i18n";
  import type {
    DiscoveredMiner,
    ErrorResponse,
    ScanSubnet,
  } from "$lib/types";

  const CUSTOM_SUBNET_ID = "custom";
  const SCAN_CONFIG_KEY = "minerpulse.scanConfig";
  const SCAN_RESULTS_KEY = "minerpulse.scan";

  interface ScanConfig {
    selectedSubnetId: string;
    customStart: string;
    customEnd: string;
    port: string;
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
  let subnets = $state<ScanSubnet[]>([]);
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
      document.documentElement.dataset.theme = parsed.theme ?? "light";
      document.documentElement.dataset.density = parsed.density ?? "comfortable";
      document.documentElement.lang = locale === "zh-CN" ? "zh-CN" : locale;
    } catch {
      /* ignore */
    }
  }

  function loadScanPrefs(subnetList: ScanSubnet[]) {
    const savedConfig = localStorage.getItem(SCAN_CONFIG_KEY);
    if (savedConfig) {
      try {
        const cfg = JSON.parse(savedConfig) as ScanConfig;
        if (
          cfg.selectedSubnetId === CUSTOM_SUBNET_ID ||
          subnetList.some((subnet) => subnet.id === cfg.selectedSubnetId)
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

  function buildScanRequest() {
    const scanPort = Number(port) || 4028;
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

<div class="scan-window">
  <header class="scan-window-head">
    <h1>{msg("scan.title")}</h1>
    <p>{msg("scan.subtitle")}</p>
  </header>

  <section class="scan-form">
    <label class="scan-field">
      <span>{msg("scan.subnet")}</span>
      <select bind:value={selectedSubnetId} disabled={scanning}>
        {#each subnets as subnet (subnet.id)}
          <option value={subnet.id}>{subnet.label}</option>
        {/each}
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
