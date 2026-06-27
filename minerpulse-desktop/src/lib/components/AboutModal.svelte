<script lang="ts">
  import { check } from "@tauri-apps/plugin-updater";
  import { relaunch } from "@tauri-apps/plugin-process";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { t, type Locale, type MessageKey } from "$lib/i18n";

  const GITHUB_URL = "https://github.com/BobJustFry/MinerPulse";
  const TELEGRAM_URL = "https://t.me/miner_pulse";
  const DONATE_NETWORK = "USDT TRC20";
  const DONATE_WALLET = "TAQLsXQA7WzNfoCTHvXxj8yFBXTJRKz99w";

  let {
    open = $bindable(false),
    locale,
    version,
    build,
    product,
  }: {
    open?: boolean;
    locale: Locale;
    version: string;
    build: number;
    product: string;
  } = $props();

  let checking = $state(false);
  let updateStatus = $state("");
  let updateError = $state(false);
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
    if (event.key === "Escape" && !checking) {
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

  async function checkForUpdates() {
    checking = true;
    updateError = false;
    updateStatus = msg("about.checking");
    try {
      const update = await check();
      if (!update) {
        updateStatus = msg("status.upToDate");
        return;
      }
      updateStatus = msg("status.updateAvailable", { version: update.version });
      await update.downloadAndInstall();
      await relaunch();
    } catch (err) {
      updateError = true;
      updateStatus = msg("about.updateError", { detail: String(err) });
    } finally {
      checking = false;
    }
  }

  $effect(() => {
    if (!open) {
      updateStatus = "";
      updateError = false;
      checking = false;
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
          disabled={checking}
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
          <button type="button" class="about-link" onclick={() => openExternal(GITHUB_URL)}>
            {msg("about.github")}
          </button>
          <button type="button" class="about-link" onclick={() => openExternal(TELEGRAM_URL)}>
            {msg("about.telegram")}
          </button>
        </div>

        <section class="about-donate">
          <div class="about-donate-title">{msg("about.donate.title")}</div>
          <p class="about-donate-hint">{msg("about.donate.hint")}</p>
          <div class="about-donate-network">{DONATE_NETWORK}</div>
          <code class="about-donate-wallet">{DONATE_WALLET}</code>
          <button type="button" class="about-donate-copy" onclick={copyDonateAddress}>
            {donateCopied ? msg("about.donate.copied") : msg("about.donate.copy")}
          </button>
        </section>

        <div class="about-actions">
          <button type="button" class="btn primary" disabled={checking} onclick={checkForUpdates}>
            {checking ? msg("about.checking") : msg("about.checkUpdates")}
          </button>
        </div>

        {#if updateStatus}
          <p class="about-update-status" class:about-update-status-error={updateError}>{updateStatus}</p>
        {/if}
      </div>
    </div>
  </div>
{/if}
