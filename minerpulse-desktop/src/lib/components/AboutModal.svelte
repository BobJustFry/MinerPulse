<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { t, type Locale, type MessageKey } from "$lib/i18n";
  import { formatAppError } from "$lib/formatAppError";

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
    signedIn = false,
    hwid = "",
  }: {
    open?: boolean;
    updateProgressOpen?: boolean;
    locale: Locale;
    version: string;
    build: number;
    product: string;
    signedIn?: boolean;
    hwid?: string;
  } = $props();
  let donateCopied = $state(false);
  let donateCopyTimer: ReturnType<typeof setTimeout> | undefined;
  let logUploadBusy = $state(false);
  let logUploadMessage = $state("");
  let logUploadError = $state(false);

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

  function diagnosticLogFilename(): string {
    const now = new Date();
    const pad = (value: number) => String(value).padStart(2, "0");
    const stamp = `${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(now.getDate())}_${pad(now.getHours())}-${pad(now.getMinutes())}-${pad(now.getSeconds())}`;
    const hw = hwid.replace(/[^a-zA-Z0-9]/g, "").slice(0, 16) || "unknown";
    return `MinerPulse-log-${stamp}_${hw}.zip`;
  }

  async function uploadDiagnosticLog() {
    if (!signedIn || logUploadBusy) return;
    logUploadBusy = true;
    logUploadMessage = "";
    logUploadError = false;
    try {
      const timezone = Intl.DateTimeFormat().resolvedOptions().timeZone || "local";
      await invoke<{ id: string; filename: string }>("upload_diagnostic_log", {
        localFilename: diagnosticLogFilename(),
        timezone,
      });
      logUploadMessage = msg("about.uploadLogOk");
    } catch (err) {
      logUploadError = true;
      logUploadMessage = formatAppError(locale, err);
    } finally {
      logUploadBusy = false;
    }
  }

  $effect(() => {
    if (!open) {
      donateCopied = false;
      clearTimeout(donateCopyTimer);
      logUploadMessage = "";
      logUploadError = false;
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
          {#if signedIn}
            <button
              type="button"
              class="btn"
              disabled={updateProgressOpen || logUploadBusy}
              onclick={uploadDiagnosticLog}
            >
              {logUploadBusy ? msg("about.uploadLogBusy") : msg("about.uploadLog")}
            </button>
          {/if}
        </div>
        {#if logUploadMessage}
          <p class="about-upload-status" class:about-upload-error={logUploadError}>{logUploadMessage}</p>
        {/if}
      </div>
    </div>
  </div>
{/if}
