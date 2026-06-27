<script lang="ts">
  import { t, type Locale, type MessageKey } from "$lib/i18n";
  import {
    availableChipMetrics,
    buildChipGrid,
    buildMatrixGrid,
    chipCellBackground,
    chipCellDisplayValue,
    chipMetricRange,
    chipVoltageUnitForVendor,
    formatChipMetric,
    formatChipVoltage,
    loadChipMatrix,
    type ChipCell,
    type ChipDisplayMetric,
    type ChipMatrix,
  } from "$lib/chipMatrix";
  import {
    buildWhatsminerChipGrid,
    whatsminerLayoutMeta,
  } from "$lib/whatsminerChipMap";
  import type { BoardChipMap } from "$lib/types";

  let {
    boards,
    locale,
    vendor = "",
  }: {
    boards: BoardChipMap[];
    locale: Locale;
    vendor?: string;
  } = $props();

  let matrixLayouts = $state<Map<number, ChipMatrix>>(new Map());
  let matrixError = $state("");
  let displayMetric = $state<ChipDisplayMetric>("temp");

  const metricLabels: Record<ChipDisplayMetric, MessageKey> = {
    temp: "chips.metric.temp",
    voltage: "data.chipVoltage",
    solutions: "data.chipSolutions",
    crc: "data.chipCrc",
  };

  let voltageUnit = $derived(chipVoltageUnitForVendor(vendor));
  let boardsStacked = $derived(vendor.toLowerCase() === "whatsminer");

  function msg(key: MessageKey, args?: Record<string, string | number>) {
    return t(locale, key, args);
  }

  function usesMatrixLayout(board: BoardChipMap): boolean {
    return Boolean(board.matrix_id);
  }

  function chipFields(chip: BoardChipMap["chips"][number]) {
    return {
      index: chip.index,
      temp: chip.temp_c,
      voltage: chip.voltage,
      errors: chip.errors,
      solutions: chip.solutions,
    };
  }

  function enrichGridRows(
    grid: ChipCell[][],
    board: BoardChipMap,
  ): ChipCell[][] {
    const byIndex = new Map(board.chips.map((chip) => [chip.index, chip]));
    return grid.map((row) =>
      row.map((cell) => {
        const chip = byIndex.get(cell.index);
        return {
          index: cell.index,
          temp: cell.temp,
          voltage: chip?.voltage,
          errors: chip?.errors,
          solutions: chip?.solutions,
          empty: false,
        };
      }),
    );
  }

  function domainGrid(board: BoardChipMap): ChipCell[][] {
    const builder = boardsStacked ? buildWhatsminerChipGrid : buildChipGrid;
    return enrichGridRows(builder(board.chips, board.chips_per_domain), board);
  }

  function boardSectionBreak(board: BoardChipMap): number {
    if (!boardsStacked) return -1;
    return whatsminerLayoutMeta(board.chips.length, board.chips_per_domain).sectionBreakRow;
  }

  function boardLayoutLabel(board: BoardChipMap): string | null {
    if (!boardsStacked || board.chips_per_domain <= 0) return null;
    const meta = whatsminerLayoutMeta(board.chips.length, board.chips_per_domain);
    return `${meta.domains}d × ${meta.chipsPerDomain}c`;
  }

  function matrixGrid(board: BoardChipMap): ChipCell[][] {
    const matrix = matrixLayouts.get(board.slot);
    if (!matrix) return [];
    return buildMatrixGrid(
      matrix,
      board.chips.map((chip) => chipFields(chip)),
    );
  }

  function boardGrid(board: BoardChipMap): ChipCell[][] {
    return usesMatrixLayout(board) ? matrixGrid(board) : domainGrid(board);
  }

  function chipTitle(cell: ChipCell): string {
    if (cell.empty) return "";
    const parts = [`C${cell.index}: ${cell.temp}°C`];
    if (cell.voltage != null) {
      parts.push(`${msg("data.chipVoltage")}: ${formatChipVoltage(cell.voltage, voltageUnit)}`);
    }
    if (cell.solutions != null) {
      parts.push(`${msg("data.chipSolutions")}: ${formatChipMetric(cell.solutions)}`);
    }
    if (cell.errors != null) {
      parts.push(`${msg("data.chipCrc")}: ${formatChipMetric(cell.errors)}`);
    }
    return parts.join("\n");
  }

  function hasCrcFault(cell: ChipCell): boolean {
    return cell.errors != null && cell.errors > 0;
  }

  async function loadBoardMatrices(nextBoards: BoardChipMap[]) {
    matrixError = "";
    const next = new Map<number, ChipMatrix>();
    const pending = nextBoards
      .filter((board) => board.matrix_id)
      .map(async (board) => {
        try {
          const matrix = await loadChipMatrix(board.matrix_id!);
          next.set(board.slot, matrix);
        } catch {
          matrixError = board.matrix_id ?? "unknown";
        }
      });
    await Promise.all(pending);
    matrixLayouts = next;
  }

  $effect(() => {
    void loadBoardMatrices(boards);
  });

  let metricOptions = $derived(
    availableChipMetrics(
      boards.map((board) => ({
        chips: board.chips.map((chip) => chipFields(chip)),
      })),
    ),
  );

  let metricRange = $derived(
    chipMetricRange(
      boards.map((board) => ({
        chips: board.chips.map((chip) => chipFields(chip)),
      })),
    ),
  );

  $effect(() => {
    if (!metricOptions.includes(displayMetric)) {
      displayMetric = metricOptions[0] ?? "temp";
    }
  });
</script>

{#if boards.length > 0}
  <div class="chip-panel">
    {#if metricOptions.length > 1}
      <div class="chip-metric-bar">
        <span class="chip-metric-label">{msg("chips.displayMetric")}</span>
        <div class="chip-metric-options" role="group" aria-label={msg("chips.displayMetric")}>
          {#each metricOptions as metric (metric)}
            <button
              type="button"
              class="chip-metric-btn"
              class:active={displayMetric === metric}
              onclick={() => {
                displayMetric = metric;
              }}
            >
              {msg(metricLabels[metric])}
            </button>
          {/each}
        </div>
      </div>
    {/if}
    {#if matrixError}
      <p class="chip-matrix-error">{msg("data.chipMatrixLoadFailed", { id: matrixError })}</p>
    {/if}
    <div class="chip-boards" class:chip-boards-stacked={boardsStacked}>
      {#each boards as board (board.slot)}
        {@const grid = boardGrid(board)}
        {@const sectionBreak = boardSectionBreak(board)}
        {@const layoutLabel = boardLayoutLabel(board)}
        <article class="chip-board-card">
          <div class="chip-board-head">
            <span class="chip-board-label">{board.label}</span>
            <span class="chip-board-meta">
              {board.chips.length} {msg("data.chips")}
              {#if board.matrix_id}
                · {board.matrix_id}
              {:else if layoutLabel}
                · {layoutLabel}
              {:else if board.chips_per_domain > 0}
                · {board.chips_per_domain}/d
              {/if}
            </span>
          </div>
          <div class="chip-grid">
            {#each grid as row, rowIndex (rowIndex)}
              <div
                class="chip-row"
                class:chip-row-section-gap={sectionBreak >= 0 && rowIndex === sectionBreak}
              >
                {#each row as cell, cellIndex (`${board.slot}-${rowIndex}-${cellIndex}`)}
                  {#if cell.empty}
                    <div class="chip-cell chip-cell-empty" aria-hidden="true"></div>
                  {:else}
                    <div
                      class="chip-cell"
                      class:chip-cell-fault={hasCrcFault(cell)}
                      style={`background:${chipCellBackground(cell, displayMetric, metricRange)}`}
                      title={chipTitle(cell)}
                    >
                      <span class="chip-cell-id">C{cell.index}</span>
                      <span class="chip-cell-value">{chipCellDisplayValue(cell, displayMetric, voltageUnit)}</span>
                      {#if hasCrcFault(cell) && displayMetric !== "crc"}
                        <span class="chip-cell-badge">{cell.errors}</span>
                      {/if}
                    </div>
                  {/if}
                {/each}
              </div>
            {/each}
          </div>
        </article>
      {/each}
    </div>
  </div>
{/if}
