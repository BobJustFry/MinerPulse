<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { emit } from "@tauri-apps/api/event";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { onMount } from "svelte";
  import { t, type Locale, type MessageKey } from "$lib/i18n";
  import type {
    DiscoveredMiner,
    ErrorResponse,
    ScanResult,
    ScanSubnet,
  } from "$lib/types";

  const CUSTOM_SUBNET_ID = "custom";

  let locale = $state<Locale>("ru");
  let subnets = $state<ScanSubnet[]>([]);
  let selectedSubnetId = $state("");
  let customStart = $state("192.168.0.1");
  let customEnd = $state("192.168.0.254");
  let port = $state("4028");
  let scanning = $state(false);
  let statusText = $state("");
  let discovered = $state<DiscoveredMiner[]>([]);

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

  async function runScan() {
    scanning = true;
    discovered = [];
    statusText = msg("status.scanning");
    try {
      const request = buildScanRequest();
      const result = await invoke<ScanResult>("scan_miners", { request });
      discovered = result.miners;
      localStorage.setItem("minerpulse.scan", JSON.stringify(discovered));
      if (result.miners.length === 0) {
        statusText = msg("status.scanEmpty", { range: result.range_label });
      } else {
        statusText = msg("status.scanDone", {
          count: result.miners.length,
          scanned: result.scanned,
        });
      }
    } catch (err) {
      statusText = formatError(err);
    } finally {
      scanning = false;
    }
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
    statusText = msg("scan.hint");
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

    <button class="btn primary" disabled={scanning} onclick={runScan}>
      {scanning ? msg("toolbar.scanning") : msg("toolbar.scan")}
    </button>
  </section>

  <section class="scan-results-wrap">
    <div class="scan-status">{statusText}</div>
    {#if discovered.length > 0}
      <div class="scan-results" role="listbox">
        {#each discovered as miner (miner.ip)}
          <button
            type="button"
            class="scan-item"
            class:unsupported={!miner.supported}
            onclick={() => pickMiner(miner)}
          >
            <span class="scan-item-ip">{miner.ip}</span>
            <span class="scan-item-model">
              {miner.model || vendorLabel(miner.vendor)}
            </span>
            {#if !miner.supported}
              <span class="scan-item-badge">{msg("scan.preview")}</span>
            {/if}
          </button>
        {/each}
      </div>
      <p class="scan-footnote">{msg("scan.pickHint")}</p>
    {:else if !scanning && statusText && !statusText.includes("…")}
      <div class="scan-empty">{msg("scan.noMiners")}</div>
    {/if}
  </section>
</div>
