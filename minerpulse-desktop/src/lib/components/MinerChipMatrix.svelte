<script lang="ts">
  import { t, type Locale, type MessageKey } from "$lib/i18n";
  import { buildChipGrid, chipTempColor } from "$lib/whatsminerErrors";
  import type { BoardChipMap } from "$lib/types";

  let {
    boards,
    locale,
  }: {
    boards: BoardChipMap[];
    locale: Locale;
  } = $props();

  function msg(key: MessageKey, args?: Record<string, string | number>) {
    return t(locale, key, args);
  }
</script>

{#if boards.length > 0}
  <section class="data-section span-full">
    <header class="data-section-head">{msg("data.group.chips")}</header>
    <div class="chip-boards">
      {#each boards as board (board.slot)}
        {@const grid = buildChipGrid(board.chips, board.chips_per_domain)}
        <article class="chip-board-card">
          <div class="chip-board-head">
            <span class="chip-board-label">{board.label}</span>
            <span class="chip-board-meta">
              {board.chips.length} {msg("data.chips")} · {board.chips_per_domain}/d
            </span>
          </div>
          <div class="chip-grid">
            {#each grid as row, rowIndex (rowIndex)}
              <div class="chip-row">
                {#each row as cell (`${board.slot}-${cell.index}`)}
                  <div
                    class="chip-cell"
                    style={`background:${chipTempColor(cell.temp)}`}
                    title={`C${cell.index}: ${cell.temp}°C`}
                  >
                    <span class="chip-cell-id">C{cell.index}</span>
                    <span class="chip-cell-temp">{cell.temp}°</span>
                  </div>
                {/each}
              </div>
            {/each}
          </div>
        </article>
      {/each}
    </div>
  </section>
{/if}
