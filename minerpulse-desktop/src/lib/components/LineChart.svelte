<script lang="ts">
  import {
    buildLinePath,
    chartBounds,
    COMPACT_CHART_LAYOUT,
    cursorX,
    DEFAULT_CHART_LAYOUT,
    formatChartDuration,
    scaleChartLayout,
    type ChartPoint,
    type LineChartSeries,
  } from "$lib/chartHistory";

  let {
    title,
    unit,
    points,
    series,
    cursorMs = null,
    formatY,
    compact = false,
  }: {
    title: string;
    unit: string;
    points: ChartPoint[];
    series: LineChartSeries[];
    cursorMs?: number | null;
    formatY?: (value: number) => string;
    compact?: boolean;
  } = $props();

  let svgWrap: HTMLDivElement | undefined = $state();
  let measuredSize = $state({ width: 0, height: 0 });

  let layout = $derived.by(() => {
    if (!compact) return DEFAULT_CHART_LAYOUT;
    if (measuredSize.width > 0 && measuredSize.height > 0) {
      return scaleChartLayout(
        COMPACT_CHART_LAYOUT,
        measuredSize.width,
        measuredSize.height,
      );
    }
    return COMPACT_CHART_LAYOUT;
  });

  $effect(() => {
    if (!compact || !svgWrap) return;
    const observer = new ResizeObserver((entries) => {
      const rect = entries[0]?.contentRect;
      if (!rect) return;
      measuredSize = {
        width: Math.max(1, Math.round(rect.width)),
        height: Math.max(1, Math.round(rect.height)),
      };
    });
    observer.observe(svgWrap);
    return () => observer.disconnect();
  });

  function allValues(): number[] {
    return series.flatMap((item) => item.values).filter((value) => Number.isFinite(value));
  }

  function yTicks(min: number, max: number): number[] {
    const mid = (min + max) / 2;
    return [max, mid, min];
  }

  function xLabel(index: number): string {
    if (points.length === 0) return "";
    const origin = points[0].t_ms;
    if (index === 0) return formatChartDuration(points[0].t_ms, origin);
    if (index === points.length - 1) {
      return formatChartDuration(points[points.length - 1].t_ms, origin);
    }
    return "";
  }

  let bounds = $derived(chartBounds(allValues()));
  let cursorIndex = $derived.by(() => {
    if (cursorMs == null || points.length === 0) return null;
    let bestIndex = 0;
    let bestDelta = Math.abs(points[0].t_ms - cursorMs);
    for (let index = 1; index < points.length; index += 1) {
      const delta = Math.abs(points[index].t_ms - cursorMs);
      if (delta < bestDelta) {
        bestDelta = delta;
        bestIndex = index;
      }
    }
    return bestIndex;
  });
  let cursorLineX = $derived(cursorX(cursorIndex, points.length, layout));
  let plotHeight = $derived(layout.height - layout.padTop - layout.padBottom);

  function pointXY(
    value: number,
    index: number,
    count: number,
    minY: number,
    maxY: number,
  ): { x: number; y: number } {
    const plotW = layout.width - layout.padLeft - layout.padRight;
    const stepX = count > 1 ? plotW / (count - 1) : 0;
    const x = layout.padLeft + stepX * index;
    const y =
      layout.padTop + plotHeight - ((value - minY) / (maxY - minY)) * plotHeight;
    return { x, y };
  }
  function formatTick(value: number): string {
    if (formatY) return formatY(value);
    return value.toFixed(value >= 100 ? 0 : 1);
  }
</script>

<article class="chart-card" class:chart-card-compact={compact}>
  <div class="chart-head">
    <h3 class="chart-title">{title}</h3>
    {#if unit}
      <span class="chart-unit">{unit}</span>
    {/if}
  </div>
  <div class="chart-svg-wrap" class:chart-svg-wrap-compact={compact} bind:this={svgWrap}>
    <svg
      class="chart-svg"
      class:chart-svg-compact={compact}
      viewBox={`0 0 ${layout.width} ${layout.height}`}
      preserveAspectRatio="xMidYMid meet"
      role="img"
      aria-label={title}
    >
    {#each yTicks(bounds.min, bounds.max) as tick, index (index)}
      {@const y =
        layout.padTop +
        plotHeight -
        ((tick - bounds.min) / (bounds.max - bounds.min)) * plotHeight}
      <line
        x1={layout.padLeft}
        x2={layout.width - layout.padRight}
        y1={y}
        y2={y}
        class="chart-grid-line"
      />
      <text x={layout.padLeft - 8} y={y + 4} class="chart-axis-label" text-anchor="end">
        {formatTick(tick)}
      </text>
    {/each}

    {#each series as item (item.id)}
      <path
        d={buildLinePath(item.values, layout, bounds.min, bounds.max)}
        class="chart-line"
        class:chart-line-accent={item.id === "hashrate"}
        stroke={item.id === "hashrate" ? undefined : item.color}
        fill="none"
      />
      {#each item.values as value, index (index)}
        {#if Number.isFinite(value) && item.values.length === 1}
          {@const pos = pointXY(value, index, item.values.length, bounds.min, bounds.max)}
          <circle
            cx={pos.x}
            cy={pos.y}
            r="4"
            class="chart-point"
            class:chart-line-accent={item.id === "hashrate"}
            fill={item.id === "hashrate" ? undefined : item.color}
          />
        {/if}
      {/each}
    {/each}

    {#if cursorLineX != null}
      <line
        x1={cursorLineX}
        x2={cursorLineX}
        y1={layout.padTop}
        y2={layout.height - layout.padBottom}
        class="chart-cursor"
      />
    {/if}

    {#if points.length > 0}
      <text x={layout.padLeft} y={layout.height - 8} class="chart-axis-label">
        {xLabel(0)}
      </text>
      <text
        x={layout.width - layout.padRight}
        y={layout.height - 8}
        class="chart-axis-label"
        text-anchor="end"
      >
        {xLabel(points.length - 1)}
      </text>
    {/if}
    </svg>
  </div>

  {#if series.length > 1}
    <div class="chart-legend">
      {#each series as item (item.id)}
        <span class="chart-legend-item">
          <span class="chart-legend-swatch" style={`background:${item.color}`}></span>
          {item.label}
        </span>
      {/each}
    </div>
  {/if}
</article>
