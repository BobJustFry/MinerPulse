<script lang="ts">
  import { t, type Locale, type MessageKey } from "$lib/i18n";
  import { shortPoolUrl, statusTone } from "$lib/formatMiner";
  import type { MinerSnapshot } from "$lib/types";

  let { snapshot, locale }: { snapshot: MinerSnapshot; locale: Locale } = $props();

  function msg(key: MessageKey) {
    return t(locale, key);
  }
</script>

{#if snapshot.pools.length > 0}
  <div class="data-pools-page">
    {#each snapshot.pools as pool, index (index)}
      <article class="data-pool-card large">
        <div class="data-pool-head">
          <span class="data-pool-status tone-{statusTone(pool.status)}">{pool.status}</span>
          <span class="data-pool-index">#{index + 1}</span>
        </div>
        <div class="data-pool-url">{pool.url || "—"}</div>
        <div class="data-pool-worker">{pool.worker || "—"}</div>
        <div class="data-pool-metrics">
          <div>
            <span>{msg("data.accepted")}</span>
            <strong>{pool.accepted.toLocaleString()}</strong>
          </div>
          <div>
            <span>{msg("data.rejected")}</span>
            <strong>{pool.rejected.toLocaleString()}</strong>
          </div>
          <div>
            <span>{msg("data.poolHost")}</span>
            <strong>{shortPoolUrl(pool.url)}</strong>
          </div>
        </div>
      </article>
    {/each}
  </div>
{:else}
  <div class="locked-panel">{msg("pools.empty")}</div>
{/if}
