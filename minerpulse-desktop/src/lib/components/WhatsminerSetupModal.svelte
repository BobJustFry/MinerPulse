<script lang="ts">
  import { t, type Locale, type MessageKey } from "$lib/i18n";
  import ManagedModalCard from "$lib/components/ManagedModalCard.svelte";
  import type { WhatsminerAccessInfo } from "$lib/types";

  let {
    open = $bindable(false),
    locale,
    ip = "",
    access = null,
    username = $bindable(""),
    password = $bindable(""),
    busy = false,
    enableBusy = false,
    testBusy = false,
    testOk = null,
    errorText = "",
    cloudAvailable = false,
    onEnableApi,
    onTestLogin,
    onSaveAndRead,
    onDismiss,
  }: {
    open?: boolean;
    locale: Locale;
    ip?: string;
    access?: WhatsminerAccessInfo | null;
    username?: string;
    password?: string;
    busy?: boolean;
    enableBusy?: boolean;
    testBusy?: boolean;
    testOk?: boolean | null;
    errorText?: string;
    cloudAvailable?: boolean;
    onEnableApi?: () => void | Promise<void>;
    onTestLogin?: () => void | Promise<void>;
    onSaveAndRead?: () => void | Promise<void>;
    onDismiss?: () => void;
  } = $props();

  const anyBusy = $derived(busy || enableBusy || testBusy);

  function msg(key: MessageKey, args?: Record<string, string | number>) {
    return t(locale, key, args);
  }

  function close() {
    if (anyBusy) return;
    open = false;
    onDismiss?.();
  }

  function stopModalPointer(event: Event) {
    event.stopPropagation();
  }

  function onKeydown(event: KeyboardEvent) {
    if (event.key === "Escape" && !anyBusy) {
      close();
    }
  }

  function statusLines(): string[] {
    const lines: string[] = [];
    if (!access) {
      lines.push(msg("auth.statusUnknown"));
      return lines;
    }
    if (access.mac) {
      lines.push(msg("auth.statusMac", { mac: access.mac }));
    } else {
      lines.push(msg("auth.statusMacUnknown"));
    }
    if (access.api_switch === false) {
      lines.push(msg("auth.statusApiOff"));
    } else if (access.api_switch === true) {
      lines.push(msg("auth.statusApiOn"));
    }
    if (!access.luci_reachable) {
      lines.push(msg("auth.statusLuciUnreachable"));
    } else if (!access.luci_auth_ok) {
      lines.push(msg("auth.statusLuciAuthFail"));
    } else {
      lines.push(msg("auth.statusLuciOk"));
    }
    return lines;
  }

  function submit(event: SubmitEvent) {
    event.preventDefault();
    if (anyBusy || !username.trim()) return;
    void onSaveAndRead?.();
  }
</script>

<svelte:window onkeydown={onKeydown} />

{#if open}
  <div class="modal-backdrop whatsminer-setup-backdrop" role="presentation">
    <ManagedModalCard
      layoutId="whatsminer-setup"
      class="whatsminer-auth-modal"
      defaultWidth={420}
      dragDisabled={anyBusy}
      role="dialog"
      aria-modal="true"
      aria-labelledby="whatsminer-setup-title"
      onclick={stopModalPointer}
      onmousedown={stopModalPointer}
    >
      <header class="modal-head">
        <div>
          <div class="modal-kicker">{msg("auth.modalKicker")}</div>
          <h3 id="whatsminer-setup-title" class="modal-title">{msg("auth.setupTitle")}</h3>
        </div>
        <button
          type="button"
          class="modal-close"
          disabled={anyBusy}
          onclick={close}
          aria-label={msg("auth.cancel")}
        >
          ×
        </button>
      </header>

      <form class="modal-body whatsminer-auth-body" onsubmit={submit}>
        <p class="whatsminer-auth-hint">{msg("auth.setupHint", { ip })}</p>

        <ul class="whatsminer-setup-status">
          {#each statusLines() as line}
            <li>{line}</li>
          {/each}
        </ul>

        <label class="whatsminer-auth-field" for="whatsminer-setup-user">
          <span>{msg("auth.username")}</span>
          <input
            id="whatsminer-setup-user"
            bind:value={username}
            disabled={anyBusy}
            autocomplete="username"
          />
        </label>

        <label class="whatsminer-auth-field" for="whatsminer-setup-pass">
          <span>{msg("auth.password")}</span>
          <input
            id="whatsminer-setup-pass"
            type="password"
            bind:value={password}
            disabled={anyBusy}
            autocomplete="current-password"
          />
        </label>

        <div class="whatsminer-setup-actions-row">
          <button
            type="button"
            class="btn btn-with-spinner"
            disabled={anyBusy || !username.trim()}
            onclick={() => void onTestLogin?.()}
          >
            {#if testBusy}
              <span class="btn-spinner" aria-hidden="true"></span>
            {/if}
            {testBusy ? msg("auth.testingLogin") : msg("auth.testLogin")}
          </button>
          {#if testOk === true}
            <span class="whatsminer-setup-ok">{msg("auth.testOk")}</span>
          {:else if testOk === false}
            <span class="whatsminer-auth-error">{msg("auth.testFail")}</span>
          {/if}
        </div>

        {#if !cloudAvailable}
          <p class="whatsminer-setup-cloud-hint">{msg("auth.cloudHint")}</p>
        {/if}

        {#if errorText}
          <p class="whatsminer-auth-error">{errorText}</p>
        {/if}

        <div class="whatsminer-auth-actions">
          <button type="button" class="btn" disabled={anyBusy} onclick={close}>
            {msg("auth.cancel")}
          </button>
          <button type="submit" class="btn primary btn-with-spinner" disabled={anyBusy || !username.trim()}>
            {#if busy}
              <span class="btn-spinner" aria-hidden="true"></span>
            {/if}
            {busy ? msg("auth.savingAndReading") : msg("auth.saveAndRead")}
          </button>
        </div>
      </form>
    </ManagedModalCard>
  </div>
{/if}

<style>
  .whatsminer-setup-backdrop {
    /* Auth must close only via Cancel / × — not accidental backdrop clicks */
    cursor: default;
  }

  .whatsminer-setup-status {
    margin: 0 0 0.75rem;
    padding-left: 1.1rem;
    font-size: 0.9rem;
    color: var(--text-muted, #6b7280);
  }

  .whatsminer-setup-actions-row {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    margin-bottom: 0.75rem;
    flex-wrap: wrap;
  }

  .whatsminer-setup-ok {
    color: var(--status-ok, #16a34a);
    font-size: 0.9rem;
  }

  .whatsminer-setup-cloud-hint {
    margin: 0 0 0.75rem;
    font-size: 0.85rem;
    color: var(--text-muted, #6b7280);
  }
</style>
