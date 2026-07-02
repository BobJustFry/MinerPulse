<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { t, type Locale, type MessageKey } from "$lib/i18n";
  import { formatAppError } from "$lib/formatAppError";
  import type { Entitlements, ErrorResponse, LicenseInfo } from "$lib/types";

  let {
    open = $bindable(false),
    locale,
    onUpdated,
  }: {
    open?: boolean;
    locale: Locale;
    onUpdated?: (entitlements: Entitlements) => void;
  } = $props();

  type AuthTab = "code" | "login";

  let tab = $state<AuthTab>("code");
  let code = $state("");
  let email = $state("");
  let password = $state("");
  let busy = $state(false);
  let error = $state("");
  let info = $state<LicenseInfo | null>(null);

  function msg(key: MessageKey, args?: Record<string, string | number>) {
    return t(locale, key, args);
  }

  function formatError(err: unknown): string {
    return formatAppError(locale, err);
  }

  function close() {
    if (busy) return;
    open = false;
    error = "";
  }

  function onBackdropClick(event: MouseEvent) {
    if (event.target === event.currentTarget) close();
  }

  function onKeydown(event: KeyboardEvent) {
    if (event.key === "Escape" && !busy) close();
  }

  async function refreshInfo() {
    info = await invoke<LicenseInfo>("get_license_info");
  }

  async function afterAuth() {
    await refreshInfo();
    const entitlements = await invoke<Entitlements>("get_entitlements");
    onUpdated?.(entitlements);
  }

  async function submitCode(event: Event) {
    event.preventDefault();
    busy = true;
    error = "";
    try {
      await invoke("activate_license", { code: code.trim() });
      await afterAuth();
      close();
    } catch (e) {
      error = formatError(e);
    } finally {
      busy = false;
    }
  }

  async function submitLogin(event: Event) {
    event.preventDefault();
    busy = true;
    error = "";
    try {
      await invoke("login_license", { email: email.trim(), password });
      await afterAuth();
      close();
    } catch (e) {
      error = formatError(e);
    } finally {
      busy = false;
    }
  }

  async function logout() {
    busy = true;
    error = "";
    try {
      await invoke("logout_license");
      code = "";
      password = "";
      await afterAuth();
    } catch (e) {
      error = formatError(e);
    } finally {
      busy = false;
    }
  }

  $effect(() => {
    if (open) {
      refreshInfo().catch(() => {});
    }
  });
</script>

<svelte:window onkeydown={onKeydown} />

{#if open}
  <div class="modal-backdrop" onclick={onBackdropClick} role="presentation">
    <div
      class="modal-card subscription-modal"
      role="dialog"
      aria-modal="true"
      aria-labelledby="subscription-modal-title"
    >
      <header class="modal-head">
        <div>
          <div class="modal-kicker">Miner Pulse</div>
          <h3 id="subscription-modal-title" class="modal-title">{msg("subscription.title")}</h3>
        </div>
        <button
          type="button"
          class="modal-close"
          disabled={busy}
          onclick={close}
          aria-label={msg("subscription.close")}
        >
          ×
        </button>
      </header>

      <div class="modal-body subscription-body">
        {#if info?.licensed}
          <div class="subscription-status">
            <p class="subscription-status-title">{msg("subscription.active")}</p>
            {#if info.plan_name}<p>{info.plan_name}</p>{/if}
            {#if info.user_email}<p>{info.user_email}</p>{/if}
            {#if info.user_nickname}<p>@{info.user_nickname}</p>{/if}
            {#if info.expires_at}
              <p class="subscription-status-meta">
                {msg("subscription.expires")}: {info.expires_at}
              </p>
            {/if}
          </div>
          <button type="button" class="btn" disabled={busy} onclick={logout}>
            {msg("subscription.logout")}
          </button>
        {:else}
          <div class="subscription-tabs" role="tablist" aria-label={msg("subscription.title")}>
            <button
              type="button"
              role="tab"
              class="subscription-tab"
              class:active={tab === "code"}
              aria-selected={tab === "code"}
              disabled={busy}
              onclick={() => (tab = "code")}
            >
              {msg("subscription.tabCode")}
            </button>
            <button
              type="button"
              role="tab"
              class="subscription-tab"
              class:active={tab === "login"}
              aria-selected={tab === "login"}
              disabled={busy}
              onclick={() => (tab = "login")}
            >
              {msg("subscription.tabLogin")}
            </button>
          </div>

          {#if tab === "code"}
            <form class="subscription-form" onsubmit={submitCode}>
              <label class="subscription-field" for="subscription-code">
                <span>{msg("subscription.codeLabel")}</span>
                <input
                  id="subscription-code"
                  bind:value={code}
                  placeholder="AB12CD"
                  autocomplete="off"
                  disabled={busy}
                  required
                />
              </label>
              <p class="subscription-hint">{msg("subscription.codeHint")}</p>
              <button type="submit" class="btn primary" disabled={busy || !code.trim()}>
                {msg("subscription.activate")}
              </button>
            </form>
          {:else}
            <form class="subscription-form" onsubmit={submitLogin}>
              <label class="subscription-field" for="subscription-email">
                <span>{msg("subscription.emailLabel")}</span>
                <input
                  id="subscription-email"
                  type="email"
                  bind:value={email}
                  autocomplete="email"
                  disabled={busy}
                  required
                />
              </label>
              <label class="subscription-field" for="subscription-password">
                <span>{msg("subscription.passwordLabel")}</span>
                <input
                  id="subscription-password"
                  type="password"
                  bind:value={password}
                  autocomplete="current-password"
                  disabled={busy}
                  required
                />
              </label>
              <p class="subscription-hint">{msg("subscription.loginHint")}</p>
              <button
                type="submit"
                class="btn primary"
                disabled={busy || !email.trim() || !password}
              >
                {msg("subscription.login")}
              </button>
            </form>
          {/if}
        {/if}

        {#if error}
          <p class="subscription-error">{error}</p>
        {/if}
      </div>
    </div>
  </div>
{/if}
