<script lang="ts">
  import LineChart from "$lib/components/LineChart.svelte";
  import {
    buildBoardTempSeries,
    buildFanSeries,
    buildHashrateSeries,
    buildPowerSeries,
    formatChartDuration,
    hasFanSeries,
    hasPowerSeries,
    resolveHashrateChart,
    type ChartPoint,
  } from "$lib/chartHistory";
  import { formatHashrateAxis } from "$lib/formatMiner";
  import { t, type Locale, type MessageKey } from "$lib/i18n";
  import type { ChartsLayout } from "$lib/types";

  let {
    points,
    cursorMs = null,
    locale,
    live = false,
    layout = $bindable("tile" as ChartsLayout),
  }: {
    points: ChartPoint[];
    cursorMs?: number | null;
    locale: Locale;
    live?: boolean;
    layout?: ChartsLayout;
  } = $props();

  function msg(key: MessageKey, args?: Record<string, string | number>) {
    return t(locale, key, args);
  }

  let hashrateChart = $derived(resolveHashrateChart(points));
  let hashrateSeries = $derived([
    buildHashrateSeries(points, msg("charts.series.hashrate"), hashrateChart.divisor),
  ]);
  let boardSeries = $derived(buildBoardTempSeries(points));
  let powerSeries = $derived([buildPowerSeries(points, msg("charts.series.power"))]);
  let fanSeries = $derived([buildFanSeries(points, msg("charts.series.fan"))]);
  let showPower = $derived(hasPowerSeries(points));
  let showFan = $derived(hasFanSeries(points));
  let durationLabel = $derived.by(() => {
    if (points.length < 2) return "";
    return formatChartDuration(points[points.length - 1].t_ms, points[0].t_ms);
  });
</script>

<section class="charts-panel" class:charts-panel-list={layout === "list"}>
  <div class="charts-head">
    <div>
      <h2 class="charts-title">{msg("charts.title")}</h2>
      {#if points.length > 0}
        <p class="charts-meta">
          {msg("charts.points", { count: points.length })}
          {#if durationLabel}
            · {msg("charts.duration", { value: durationLabel })}
          {/if}
          {#if live}
            · {msg("charts.live")}
          {/if}
        </p>
      {/if}
    </div>
    <div class="charts-layout-toggle" role="group" aria-label={msg("charts.layout")}>
      <button
        type="button"
        class="charts-layout-btn"
        class:active={layout === "tile"}
        onclick={() => {
          layout = "tile";
        }}
      >
        {msg("charts.layout.tile")}
      </button>
      <button
        type="button"
        class="charts-layout-btn"
        class:active={layout === "list"}
        onclick={() => {
          layout = "list";
        }}
      >
        {msg("charts.layout.list")}
      </button>
    </div>
  </div>

  {#if points.length === 0}
    <div class="locked-panel">{msg("charts.empty")}</div>
  {:else}
    <div class="charts-grid" class:layout-tile={layout === "tile"} class:layout-list={layout === "list"}>
      <LineChart
        title={msg("charts.hashrate")}
        unit={hashrateChart.unit}
        formatY={formatHashrateAxis}
        compact={layout === "list"}
        {points}
        series={hashrateSeries}
        {cursorMs}
      />

      {#if boardSeries.length > 0}
        <LineChart
          title={msg("charts.boardTemp")}
          unit="°C"
          compact={layout === "list"}
          {points}
          series={boardSeries}
          {cursorMs}
        />
      {/if}

      {#if showPower}
        <LineChart
          title={msg("charts.power")}
          unit="W"
          compact={layout === "list"}
          {points}
          series={powerSeries}
          {cursorMs}
        />
      {/if}

      {#if showFan}
        <LineChart
          title={msg("charts.fan")}
          unit="RPM"
          compact={layout === "list"}
          {points}
          series={fanSeries}
          {cursorMs}
        />
      {/if}
    </div>
  {/if}
</section>
