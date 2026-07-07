<script lang="ts">
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { t, type Locale, type MessageKey } from "$lib/i18n";

  const GITHUB_URL = "https://github.com/BobJustFry/MinerPulse";
  const TELEGRAM_URL = "https://t.me/miner_pulse";
  const PORTAL_URL = "https://mpulse.bob4.fun/";
  const DONATE_NETWORK = "USDT TRC20";
  const DONATE_WALLET = "TAQLsXQA7WzNfoCTHvXxj8yFBXTJRKz99w";

  let {
    open = $bindable(false),
    updateProgressOpen = $bindable(false),
    locale,
    version,
    build,
    product,
  }: {
    open?: boolean;
    updateProgressOpen?: boolean;
    locale: Locale;
    version: string;
    build: number;
    product: string;
  } = $props();
  let donateCopied = $state(false);
  let donateCopyTimer: ReturnType<typeof setTimeout> | undefined;

  function msg(key: MessageKey, args?: Record<string, string | number>) {
    return t(locale, key, args);
  }

  function close() {
    open = false;
  }

  function onBackdropClick(event: MouseEvent) {
    if (event.target === event.currentTarget) {
      close();
    }
  }

  function onKeydown(event: KeyboardEvent) {
    if (event.key === "Escape" && !updateProgressOpen) {
      close();
    }
  }

  async function openExternal(url: string) {
    try {
      await openUrl(url);
    } catch {
      window.open(url, "_blank", "noopener,noreferrer");
    }
  }

  async function copyDonateAddress() {
    try {
      await navigator.clipboard.writeText(DONATE_WALLET);
      donateCopied = true;
      clearTimeout(donateCopyTimer);
      donateCopyTimer = setTimeout(() => {
        donateCopied = false;
      }, 2000);
    } catch {
      /* ignore */
    }
  }

  function startUpdateCheck() {
    updateProgressOpen = true;
  }

  $effect(() => {
    if (!open) {
      donateCopied = false;
      clearTimeout(donateCopyTimer);
    }
  });
</script>

<svelte:window onkeydown={onKeydown} />

{#if open}
  <div class="modal-backdrop" onclick={onBackdropClick} role="presentation">
    <div class="modal-card about-modal" role="dialog" aria-modal="true" aria-labelledby="about-modal-title">
      <header class="modal-head">
        <div>
          <div class="modal-kicker">{product}</div>
          <h3 id="about-modal-title" class="modal-title">{msg("about.title")}</h3>
        </div>
        <button
          type="button"
          class="modal-close"
          disabled={updateProgressOpen}
          onclick={close}
          aria-label={msg("about.close")}
        >
          ×
        </button>
      </header>

      <div class="modal-body about-body">
        <img class="about-logo" src="/logo.png" width="96" height="96" alt={product} />

        <div class="about-meta">
          <div class="about-meta-row">
            <span class="about-meta-label">{msg("about.version")}</span>
            <span class="about-meta-value">{version} ({build})</span>
          </div>
          <div class="about-meta-row">
            <span class="about-meta-label">{msg("about.developer")}</span>
            <span class="about-meta-value">{msg("about.developerName")}</span>
          </div>
        </div>

        <div class="about-links">
          <button type="button" class="about-link" onclick={() => openExternal(PORTAL_URL)}>
            {msg("about.website")}
          </button>
          <button type="button" class="about-link" onclick={() => openExternal(GITHUB_URL)}>
            {msg("about.github")}
          </button>
          <button type="button" class="about-link" onclick={() => openExternal(TELEGRAM_URL)}>
            {msg("about.telegram")}
          </button>
        </div>

        <p class="about-portal-hint">{msg("about.portalHint")}</p>

        <section class="about-donate">
          <div class="about-donate-title">{msg("about.donate.title")}</div>
          <p class="about-donate-hint">{msg("about.donate.hint")}</p>
          <div class="about-donate-network">{DONATE_NETWORK}</div>
          <div class="about-donate-wallet-row">
            <code class="about-donate-wallet">{DONATE_WALLET}</code>
            <button
              type="button"
              class="about-donate-copy-btn"
              class:about-donate-copy-btn-done={donateCopied}
              onclick={copyDonateAddress}
              aria-label={donateCopied ? msg("about.donate.copied") : msg("about.donate.copy")}
              title={donateCopied ? msg("about.donate.copied") : msg("about.donate.copy")}
            >
              {#if donateCopied}
                <svg viewBox="0 0 16 16" aria-hidden="true">
                  <path
                    d="M3.5 8.5 6.5 11.5 12.5 4.5"
                    fill="none"
                    stroke="currentColor"
                    stroke-width="1.5"
                    stroke-linecap="round"
                    stroke-linejoin="round"
                  />
                </svg>
              {:else}
                <svg viewBox="0 0 16 16" aria-hidden="true">
                  <rect
                    x="5.5"
                    y="5.5"
                    width="7"
                    height="7"
                    rx="1"
                    fill="none"
                    stroke="currentColor"
                    stroke-width="1.2"
                  />
                  <path
                    d="M4.5 10.5V4.5a1 1 0 0 1 1-1h6"
                    fill="none"
                    stroke="currentColor"
                    stroke-width="1.2"
                    stroke-linecap="round"
                  />
                </svg>
              {/if}
            </button>
          </div>
        </section>

        <div class="about-actions">
          <button type="button" class="btn primary" disabled={updateProgressOpen} onclick={startUpdateCheck}>
            {msg("about.checkUpdates")}
          </button>
        </div>
      </div>
    </div>
  </div>
{/if}
