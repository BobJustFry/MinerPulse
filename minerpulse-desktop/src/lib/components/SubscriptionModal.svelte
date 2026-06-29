<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { t, type Locale, type MessageKey } from "$lib/i18n";
  import type { Entitlements, LicenseInfo } from "$lib/types";

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

  function close() {
    open = false;
    error = "";
  }

  function onBackdropClick(event: MouseEvent) {
    if (event.target === event.currentTarget) close();
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
      error = String(e);
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
      error = String(e);
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
      error = String(e);
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

{#if open}
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <div class="modal-backdrop" role="presentation" onclick={onBackdropClick} onkeydown={(e) => e.key === "Escape" && close()}>
    <div class="modal card" role="dialog" aria-labelledby="sub-title">
      <header class="modal-header">
        <h2 id="sub-title">{msg("subscription.title")}</h2>
        <button type="button" class="icon-btn" onclick={close} aria-label={msg("subscription.close")}>×</button>
      </header>

      {#if info?.licensed}
        <div class="status">
          <p><strong>{msg("subscription.active")}</strong></p>
          {#if info.plan_name}<p>{info.plan_name}</p>{/if}
          {#if info.user_email}<p>{info.user_email}</p>{/if}
          {#if info.user_nickname}<p>@{info.user_nickname}</p>{/if}
          {#if info.expires_at}<p>{msg("subscription.expires")}: {info.expires_at}</p>{/if}
        </div>
        <button type="button" class="secondary" disabled={busy} onclick={logout}>{msg("subscription.logout")}</button>
      {:else}
        <div class="tabs">
          <button type="button" class:active={tab === "code"} onclick={() => (tab = "code")}>{msg("subscription.tabCode")}</button>
          <button type="button" class:active={tab === "login"} onclick={() => (tab = "login")}>{msg("subscription.tabLogin")}</button>
        </div>

        {#if tab === "code"}
          <form onsubmit={submitCode}>
            <label>
              {msg("subscription.codeLabel")}
              <input bind:value={code} placeholder="AB12CD" autocomplete="off" required />
            </label>
            <p class="hint">{msg("subscription.codeHint")}</p>
            <button type="submit" disabled={busy}>{msg("subscription.activate")}</button>
          </form>
        {:else}
          <form onsubmit={submitLogin}>
            <label>
              {msg("subscription.emailLabel")}
              <input type="email" bind:value={email} autocomplete="email" required />
            </label>
            <label>
              {msg("subscription.passwordLabel")}
              <input type="password" bind:value={password} autocomplete="current-password" required />
            </label>
            <p class="hint">{msg("subscription.loginHint")}</p>
            <button type="submit" disabled={busy}>{msg("subscription.login")}</button>
          </form>
        {/if}
      {/if}

      {#if error}
        <p class="error">{error}</p>
      {/if}
    </div>
  </div>
{/if}

<style>
  .modal-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.55);
    display: grid;
    place-items: center;
    z-index: 1000;
    padding: 16px;
  }
  .modal {
    width: min(440px, 100%);
    background: var(--surface-1, #1a1d26);
    border: 1px solid var(--border, #2d3348);
    border-radius: 12px;
    padding: 16px;
  }
  .modal-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 12px;
  }
  .modal-header h2 {
    margin: 0;
    font-size: 1.1rem;
  }
  .icon-btn {
    border: none;
    background: transparent;
    color: inherit;
    font-size: 1.4rem;
    cursor: pointer;
  }
  .tabs {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px;
    margin-bottom: 12px;
  }
  .tabs button {
    padding: 8px;
    border-radius: 8px;
    border: 1px solid var(--border, #2d3348);
    background: var(--surface-2, #242836);
    color: inherit;
    cursor: pointer;
  }
  .tabs button.active {
    background: var(--accent, #0066cc);
    border-color: var(--accent, #0066cc);
  }
  label {
    display: block;
    margin-bottom: 10px;
    font-size: 0.9rem;
  }
  input {
    display: block;
    width: 100%;
    margin-top: 4px;
    padding: 8px 10px;
    border-radius: 8px;
    border: 1px solid var(--border, #2d3348);
    background: var(--surface-2, #242836);
    color: inherit;
    box-sizing: border-box;
  }
  button[type="submit"], .secondary {
    width: 100%;
    margin-top: 8px;
    padding: 10px;
    border-radius: 8px;
    border: 1px solid var(--accent, #0066cc);
    background: var(--accent, #0066cc);
    color: #fff;
    cursor: pointer;
  }
  .secondary {
    background: var(--surface-2, #242836);
    color: inherit;
    border-color: var(--border, #2d3348);
  }
  .hint {
    font-size: 0.85rem;
    color: var(--text-muted, #9aa0b4);
    margin: 0 0 8px;
  }
  .error {
    color: #f0b7b7;
    font-size: 0.9rem;
    margin-top: 10px;
  }
  .status p {
    margin: 4px 0;
  }
</style>
