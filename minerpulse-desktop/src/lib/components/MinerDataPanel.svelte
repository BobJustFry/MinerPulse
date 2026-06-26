<script lang="ts">
  import { t, type Locale, type MessageKey } from "$lib/i18n";
  import {
    formatEfficiency,
    formatHashrate,
    formatNumber,
    formatPercent,
    formatUptime,
    shortPoolUrl,
    statusTone,
    vendorLabel,
  } from "$lib/formatMiner";
  import type { MinerSnapshot } from "$lib/types";

  let { snapshot, locale }: { snapshot: MinerSnapshot; locale: Locale } = $props();

  function msg(key: MessageKey, args?: Record<string, string | number>) {
    return t(locale, key, args);
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
        {#if snapshot.uptime_sec != null}
          <span>{msg("data.uptime")}: {formatUptime(snapshot.uptime_sec)}</span>
        {/if}
      </div>
    </div>
    <div class="data-hero-side">
      <span class="data-status-pill tone-{statusTone(snapshot.status)}">
        {snapshot.status || "—"}
      </span>
      {#if hasHashrateData()}
        <div class="data-hero-hash">{formatHashrate(snapshot.hashrate.avg5s_ghs)}</div>
        <div class="data-hero-hash-label">{msg("data.hashrate5s")}</div>
      {/if}
    </div>
  </section>

  <div class="data-sections">
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
                {#if board.fan_rpm != null}
                  <div>
                    <span>{msg("data.fan")}</span>
                    <strong>{formatNumber(board.fan_rpm, 0, " RPM")}</strong>
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
