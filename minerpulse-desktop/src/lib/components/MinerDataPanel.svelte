<script lang="ts">
  import { onMount } from "svelte";
  import { t, type Locale, type MessageKey } from "$lib/i18n";
  import WhatsminerErrorModal from "$lib/components/WhatsminerErrorModal.svelte";
  import {
    formatEfficiency,
    formatHashrate,
    formatNumber,
    formatPercent,
    formatUptime,
    shortPoolUrl,
    statusMessageKey,
    statusTone,
    vendorLabel,
  } from "$lib/formatMiner";
  import {
    loadWhatsminerErrorCatalog,
    lookupWhatsminerError,
    pickLocalizedText,
    type WhatsminerErrorCatalog,
  } from "$lib/whatsminerErrors";
  import type { MinerSnapshot } from "$lib/types";

  let {
    snapshot,
    locale,
    density = "comfortable",
  }: { snapshot: MinerSnapshot; locale: Locale; density?: "comfortable" | "compact" } = $props();

  let errorCatalog = $state<WhatsminerErrorCatalog | null>(null);
  let errorModalOpen = $state(false);
  let selectedErrorCode = $state("");

  onMount(() => {
    if (snapshot.identity.vendor === "whatsminer" && (snapshot.faults?.length ?? 0) > 0) {
      loadWhatsminerErrorCatalog()
        .then((catalog) => {
          errorCatalog = catalog;
        })
        .catch(() => {
          errorCatalog = null;
        });
    }
  });

  function msg(key: MessageKey, args?: Record<string, string | number>) {
    return t(locale, key, args);
  }

  function isWhatsminer() {
    return snapshot.identity.vendor === "whatsminer";
  }

  function isAvalon() {
    return snapshot.identity.vendor === "avalon";
  }

  function formatFreqList(values?: number[]): string {
    if (!values || values.length === 0) return "—";
    return values.join(" / ");
  }

  function faultLabel(code: string): string {
    if (!errorCatalog) return code;
    const entry = lookupWhatsminerError(errorCatalog, code);
    return entry ? pickLocalizedText(locale, entry.name, code) : code;
  }

  function openError(code: string) {
    selectedErrorCode = code;
    errorModalOpen = true;
  }

  function hasHashrateData() {
    return (
      snapshot.hashrate.avg5s_ghs > 0 ||
      snapshot.hashrate.avg_ghs > 0 ||
      snapshot.hashrate.current_ghs > 0
    );
  }

  function rejectRate(): number | null {
    const accepted = snapshot.shares_accepted;
    const rejected = snapshot.shares_rejected;
    if (accepted == null || rejected == null) return null;
    const total = accepted + rejected;
    if (total <= 0) return null;
    return rejected;
  }

  function coolingLabel(mode: string): string {
    if (mode === "air") return msg("data.param.coolingAir");
    if (mode === "liquid") return msg("data.param.coolingLiquid");
    return mode;
  }

  type ParamItem = { label: string; value: string };

  function paramItems(): ParamItem[] {
    const p = snapshot.params;
    if (!p) return [];
    const items: ParamItem[] = [];
    const add = (label: MessageKey, value: string | null | undefined) => {
      if (value != null && value !== "") items.push({ label: msg(label), value });
    };
    const n = (v: number | null | undefined, d: number, s: string) =>
      v == null ? null : formatNumber(v, d, s);
    add("data.param.powerMode", p.power_mode ?? null);
    add("data.param.frequency", n(p.frequency_mhz, 0, " MHz"));
    add("data.param.rated", p.rated_ghs != null ? formatHashrate(p.rated_ghs) : null);
    add("data.param.powerLimit", n(p.power_limit_w, 0, " W"));
    add("data.param.cooling", p.cooling_mode ? coolingLabel(p.cooling_mode) : null);
    add("data.param.envTemp", n(p.env_temp_c, 1, " °C"));
    add("data.param.chipMin", n(p.chip_temp_min_c, 1, " °C"));
    add("data.param.chipAvg", n(p.chip_temp_avg_c, 1, " °C"));
    add("data.param.chipMax", n(p.chip_temp_max_c, 1, " °C"));
    add("data.param.hwErrPct", n(p.device_hardware_pct, 2, "%"));
    add("data.param.rejectPct", n(p.device_reject_pct, 2, "%"));
    add("data.param.psuModel", p.psu_model ?? null);
    add("data.param.psuInV", n(p.psu_input_voltage, 1, " V"));
    add("data.param.psuInA", n(p.psu_input_current, 1, " A"));
    add("data.param.psuOutV", n(p.psu_output_voltage, 2, " V"));
    add("data.param.psuWatts", n(p.psu_watts, 0, " W"));
    add("data.param.psuTemp", n(p.psu_temp_c, 1, " °C"));
    add("data.param.psuFan", n(p.psu_fan_rpm, 0, " RPM"));
    return items;
  }
</script>

<div class="data-dashboard">
  <section class="data-hero">
    <div class="data-hero-main">
      <div class="data-hero-vendor">{vendorLabel(snapshot.identity.vendor)}</div>
      <h2 class="data-hero-title">{snapshot.identity.model}</h2>
      <div class="data-hero-meta">
        {#if snapshot.identity.firmware}
          <span>{msg("data.firmware")}: {snapshot.identity.firmware}</span>
        {/if}
        {#if snapshot.identity.core_chip}
          <span>{msg("data.coreChip")}: {snapshot.identity.core_chip}</span>
        {/if}
        {#if snapshot.work_mode != null}
          <span>{msg("data.workMode")}: {snapshot.work_mode}</span>
        {/if}
        {#if snapshot.ecmm != null}
          <span>{msg("data.ecmm")}: {snapshot.ecmm}</span>
        {/if}
        {#if snapshot.uptime_sec != null}
          <span>{msg("data.uptime")}: {formatUptime(snapshot.uptime_sec)}</span>
        {/if}
      </div>
    </div>
    <div class="data-hero-side">
      <span class="data-status-pill tone-{statusTone(snapshot.status)}">
        {#if statusMessageKey(snapshot.status)}
          {msg(statusMessageKey(snapshot.status) as MessageKey)}
        {:else}
          {snapshot.status || "—"}
        {/if}
      </span>
      {#if hasHashrateData()}
        <div class="data-hero-hash">{formatHashrate(snapshot.hashrate.avg5s_ghs)}</div>
        <div class="data-hero-hash-label">{msg("data.hashrate5s")}</div>
      {/if}
    </div>
  </section>

  <div class="data-sections">
    {#if paramItems().length > 0}
      {#if density === "compact"}
        <details class="data-section span-2 data-params-details">
          <summary class="data-section-head data-params-summary">{msg("data.group.params")}</summary>
          <div class="data-metrics">
            {#each paramItems() as item}
              <div class="data-metric">
                <div class="data-metric-label">{item.label}</div>
                <div class="data-metric-value">{item.value}</div>
              </div>
            {/each}
          </div>
        </details>
      {:else}
        <section class="data-section span-2">
          <header class="data-section-head">{msg("data.group.params")}</header>
          <div class="data-metrics">
            {#each paramItems() as item}
              <div class="data-metric">
                <div class="data-metric-label">{item.label}</div>
                <div class="data-metric-value">{item.value}</div>
              </div>
            {/each}
          </div>
        </section>
      {/if}
    {/if}

    {#if hasHashrateData()}
      <section class="data-section span-2">
        <header class="data-section-head">{msg("data.group.hashrate")}</header>
        <div class="data-metrics">
          <div class="data-metric highlight">
            <div class="data-metric-label">{msg("data.hashrate5s")}</div>
            <div class="data-metric-value">{formatHashrate(snapshot.hashrate.avg5s_ghs)}</div>
          </div>
          <div class="data-metric">
            <div class="data-metric-label">{msg("data.hashrateAvg")}</div>
            <div class="data-metric-value">{formatHashrate(snapshot.hashrate.avg_ghs)}</div>
          </div>
          <div class="data-metric">
            <div class="data-metric-label">{msg("data.hashrateCurrent")}</div>
            <div class="data-metric-value">{formatHashrate(snapshot.hashrate.current_ghs)}</div>
          </div>
          {#if snapshot.power.watts != null && snapshot.hashrate.avg5s_ghs > 0}
            <div class="data-metric">
              <div class="data-metric-label">{msg("data.efficiency")}</div>
              <div class="data-metric-value">
                {formatEfficiency(snapshot.power.watts, snapshot.hashrate.avg5s_ghs)}
              </div>
            </div>
          {/if}
        </div>
      </section>
    {/if}

    {#if snapshot.thermal.inlet_c != null || snapshot.thermal.per_board_max_c.length > 0 || snapshot.thermal.per_chip_c.length > 0}
      <section class="data-section">
        <header class="data-section-head">{msg("data.group.thermal")}</header>
        <div class="data-metrics">
          {#if snapshot.thermal.inlet_c != null}
            <div class="data-metric">
              <div class="data-metric-label">{msg("data.tempIn")}</div>
              <div class="data-metric-value">
                {formatNumber(snapshot.thermal.inlet_c, 1, " °C")}
              </div>
            </div>
          {/if}
          {#if snapshot.thermal.per_board_max_c.length > 0}
            <div class="data-metric">
              <div class="data-metric-label">{msg("data.tempBoardMax")}</div>
              <div class="data-metric-value">
                {formatNumber(Math.max(...snapshot.thermal.per_board_max_c), 1, " °C")}
              </div>
            </div>
          {/if}
          {#if snapshot.thermal.per_chip_c.length > 0}
            <div class="data-metric">
              <div class="data-metric-label">{msg("data.tempChipAvg")}</div>
              <div class="data-metric-value">
                {formatNumber(
                  snapshot.thermal.per_chip_c.reduce((sum, value) => sum + value, 0) /
                    snapshot.thermal.per_chip_c.length,
                  0,
                  " °C",
                )}
              </div>
            </div>
          {/if}
        </div>
      </section>
    {/if}

    {#if snapshot.fans.rpm.length > 0}
      <section class="data-section">
        <header class="data-section-head">{msg("data.group.cooling")}</header>
        <div class="data-metrics">
          {#each snapshot.fans.rpm as rpm, index (index)}
            <div class="data-metric">
              <div class="data-metric-label">{msg("data.fan")} {index + 1}</div>
              <div class="data-metric-value">{formatNumber(rpm, 0, " RPM")}</div>
            </div>
          {/each}
        </div>
      </section>
    {/if}

    {#if snapshot.power.watts != null || snapshot.power.voltage != null}
      <section class="data-section">
        <header class="data-section-head">{msg("data.group.power")}</header>
        <div class="data-metrics">
          {#if snapshot.power.watts != null}
            <div class="data-metric">
              <div class="data-metric-label">{msg("data.power")}</div>
              <div class="data-metric-value">
                {formatNumber(snapshot.power.watts, 0, " W")}
              </div>
            </div>
          {/if}
          {#if snapshot.power.voltage != null}
            <div class="data-metric">
              <div class="data-metric-label">{msg("data.voltage")}</div>
              <div class="data-metric-value">
                {formatNumber(snapshot.power.voltage, 1, " V")}
              </div>
            </div>
          {/if}
        </div>
      </section>
    {/if}

    {#if snapshot.shares_accepted != null || snapshot.shares_rejected != null || snapshot.hw_errors != null}
      <section class="data-section">
        <header class="data-section-head">{msg("data.group.shares")}</header>
        <div class="data-metrics">
          {#if snapshot.shares_accepted != null}
            <div class="data-metric">
              <div class="data-metric-label">{msg("data.accepted")}</div>
              <div class="data-metric-value">{snapshot.shares_accepted.toLocaleString()}</div>
            </div>
          {/if}
          {#if snapshot.shares_rejected != null}
            <div class="data-metric">
              <div class="data-metric-label">{msg("data.rejected")}</div>
              <div class="data-metric-value">{snapshot.shares_rejected.toLocaleString()}</div>
            </div>
          {/if}
          {#if rejectRate() != null}
            <div class="data-metric">
              <div class="data-metric-label">{msg("data.rejectRate")}</div>
              <div class="data-metric-value">
                {formatPercent(rejectRate(), (snapshot.shares_accepted ?? 0) + (snapshot.shares_rejected ?? 0))}
              </div>
            </div>
          {/if}
          {#if snapshot.hw_errors != null}
            <div class="data-metric">
              <div class="data-metric-label">{msg("data.hwErrors")}</div>
              <div class="data-metric-value">{snapshot.hw_errors.toLocaleString()}</div>
            </div>
          {/if}
        </div>
      </section>
    {/if}

    {#if isWhatsminer() && (snapshot.faults?.length ?? 0) > 0}
      <section class="data-section span-full">
        <header class="data-section-head">{msg("data.group.errors")}</header>
        <div class="fault-list">
          {#each snapshot.faults ?? [] as fault (fault.code + (fault.occurred_at ?? ""))}
            <button
              type="button"
              class="fault-link"
              title={msg("errors.openHint")}
              onclick={() => openError(fault.code)}
            >
              <span class="fault-code">{fault.code}</span>
              <span class="fault-name">{faultLabel(fault.code)}</span>
              {#if fault.occurred_at}
                <span class="fault-time">{fault.occurred_at}</span>
              {/if}
            </button>
          {/each}
        </div>
      </section>
    {/if}

    {#if snapshot.boards.length > 0}
      <section class="data-section span-full">
        <header class="data-section-head">{msg("data.group.boards")}</header>
        <div class="data-boards-grid">
          {#each snapshot.boards as board, index (index)}
            <article class="data-board-card">
              <div class="data-board-head">
                <span class="data-board-label">{board.label}</span>
                {#if board.status}
                  <span class="data-board-status tone-{statusTone(board.status)}">
                    {board.status}
                  </span>
                {/if}
              </div>
              <div class="data-board-metrics">
                {#if board.hashrate_ghs != null}
                  <div>
                    <span>{msg("data.hashrate5s")}</span>
                    <strong>{formatHashrate(board.hashrate_ghs)}</strong>
                  </div>
                {/if}
                {#if board.temp_c != null}
                  <div>
                    <span>{msg("data.temp")}</span>
                    <strong>{formatNumber(board.temp_c, 1, " °C")}</strong>
                  </div>
                {/if}
                {#if board.chip_temp_min_c != null || board.chip_temp_avg_c != null || board.chip_temp_max_c != null}
                  <div>
                    <span>{msg("data.chipTempRange")}</span>
                    <strong>
                      {formatNumber(board.chip_temp_min_c, 0, "")}–{formatNumber(
                        board.chip_temp_max_c,
                        0,
                        " °C",
                      )}
                    </strong>
                  </div>
                {/if}
                {#if board.effective_chips != null}
                  <div>
                    <span>{msg("data.chips")}</span>
                    <strong>{board.effective_chips}</strong>
                  </div>
                {/if}
                {#if board.fan_rpm != null}
                  <div>
                    <span>{msg("data.fan")}</span>
                    <strong>{formatNumber(board.fan_rpm, 0, " RPM")}</strong>
                  </div>
                {/if}
                {#if isAvalon() && board.freq_domains_mhz && board.freq_domains_mhz.length > 0}
                  <div>
                    <span>{msg("data.freqDomains")}</span>
                    <strong>{formatFreqList(board.freq_domains_mhz)} MHz</strong>
                  </div>
                {/if}
                {#if isAvalon() && board.freq_bands_mhz && board.freq_bands_mhz.length > 0}
                  <div>
                    <span>{msg("data.freqBands")}</span>
                    <strong>{formatFreqList(board.freq_bands_mhz)} MHz</strong>
                  </div>
                {/if}
                {#if isAvalon() && board.voltage_level != null}
                  <div>
                    <span>{msg("data.voltageLevel")}</span>
                    <strong>{board.voltage_level}</strong>
                  </div>
                {/if}
              </div>
            </article>
          {/each}
        </div>
      </section>
    {/if}

    {#if snapshot.pools.length > 0}
      <section class="data-section span-full">
        <header class="data-section-head">{msg("data.group.pools")}</header>
        <div class="data-pools-grid">
          {#each snapshot.pools as pool, index (index)}
            <article class="data-pool-card">
              <div class="data-pool-head">
                <span class="data-pool-status tone-{statusTone(pool.status)}">{pool.status}</span>
                <span class="data-pool-stats">
                  {pool.accepted.toLocaleString()} / {pool.rejected.toLocaleString()}
                </span>
              </div>
              <div class="data-pool-url">{shortPoolUrl(pool.url)}</div>
              {#if pool.worker}
                <div class="data-pool-worker">{pool.worker}</div>
              {/if}
            </article>
          {/each}
        </div>
      </section>
    {/if}
  </div>
</div>

<WhatsminerErrorModal bind:open={errorModalOpen} bind:code={selectedErrorCode} {locale} />
