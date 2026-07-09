<script lang="ts">
  import { t, type Locale, type MessageKey } from "$lib/i18n";
  import {
    loadWhatsminerErrorCatalog,
    lookupWhatsminerError,
    pickLocalizedText,
    type WhatsminerErrorEntry,
  } from "$lib/whatsminerErrors";
  import ManagedModalCard from "$lib/components/ManagedModalCard.svelte";

  let {
    open = $bindable(false),
    code = $bindable(""),
    locale,
  }: {
    open?: boolean;
    code?: string;
    locale: Locale;
  } = $props();

  let entry = $state<WhatsminerErrorEntry | undefined>(undefined);
  let loading = $state(false);

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
    if (event.key === "Escape") {
      close();
    }
  }

  $effect(() => {
    if (!open || !code) {
      entry = undefined;
      return;
    }

    loading = true;
    loadWhatsminerErrorCatalog()
      .then((catalog) => {
        entry = lookupWhatsminerError(catalog, code);
      })
      .catch(() => {
        entry = undefined;
      })
      .finally(() => {
        loading = false;
      });
  });
</script>

<svelte:window onkeydown={onKeydown} />

{#if open}
  <div class="modal-backdrop" onclick={onBackdropClick} role="presentation">
    <ManagedModalCard layoutId="whatsminer-error" defaultWidth={560} role="dialog" aria-modal="true" aria-labelledby="error-modal-title">
      <header class="modal-head">
        <div>
          <div class="modal-kicker">{msg("errors.modalTitle")}</div>
          <h3 id="error-modal-title" class="modal-title">
            {msg("errors.code")} {code}
          </h3>
        </div>
        <button type="button" class="modal-close" onclick={close} aria-label={msg("errors.close")}>
          ×
        </button>
      </header>

      {#if loading}
        <p class="modal-body">{msg("errors.loading")}</p>
      {:else if entry}
        <div class="modal-body">
          <div class="modal-section">
            <div class="modal-section-label">{msg("errors.name")}</div>
            <div class="modal-section-value">{pickLocalizedText(locale, entry.name, code)}</div>
          </div>
          <div class="modal-section">
            <div class="modal-section-label">{msg("errors.description")}</div>
            <div class="modal-section-value">
              {pickLocalizedText(locale, entry.description)}
            </div>
          </div>
          <div class="modal-section">
            <div class="modal-section-label">{msg("errors.action")}</div>
            <div class="modal-section-value action">
              {pickLocalizedText(locale, entry.action)}
            </div>
          </div>
        </div>
      {:else}
        <div class="modal-body">
          <div class="modal-section">
            <div class="modal-section-label">{msg("errors.name")}</div>
            <div class="modal-section-value">{msg("errors.unknown", { code })}</div>
          </div>
          <div class="modal-section">
            <div class="modal-section-label">{msg("errors.action")}</div>
            <div class="modal-section-value action">{msg("errors.unknownAction")}</div>
          </div>
        </div>
      {/if}
    </ManagedModalCard>
  </div>
{/if}
