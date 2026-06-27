<script lang="ts">
  import { t, type Locale, type MessageKey } from "$lib/i18n";

  let {
    open = $bindable(false),
    locale,
    username = $bindable(""),
    password = $bindable(""),
    busy = false,
    errorText = "",
    onSubmit,
  }: {
    open?: boolean;
    locale: Locale;
    username?: string;
    password?: string;
    busy?: boolean;
    errorText?: string;
    onSubmit?: () => void | Promise<void>;
  } = $props();

  function msg(key: MessageKey, args?: Record<string, string | number>) {
    return t(locale, key, args);
  }

  function close() {
    if (busy) return;
    open = false;
  }

  function onBackdropClick(event: MouseEvent) {
    if (event.target === event.currentTarget) {
      close();
    }
  }

  function onKeydown(event: KeyboardEvent) {
    if (event.key === "Escape" && !busy) {
      close();
    }
  }

  function submit(event: SubmitEvent) {
    event.preventDefault();
    if (busy || !username.trim()) return;
    void onSubmit?.();
  }
</script>

<svelte:window onkeydown={onKeydown} />

{#if open}
  <div class="modal-backdrop" onclick={onBackdropClick} role="presentation">
    <div
      class="modal-card whatsminer-auth-modal"
      role="dialog"
      aria-modal="true"
      aria-labelledby="whatsminer-auth-title"
    >
      <header class="modal-head">
        <div>
          <div class="modal-kicker">{msg("auth.modalKicker")}</div>
          <h3 id="whatsminer-auth-title" class="modal-title">{msg("auth.modalTitle")}</h3>
        </div>
        <button
          type="button"
          class="modal-close"
          disabled={busy}
          onclick={close}
          aria-label={msg("auth.cancel")}
        >
          ×
        </button>
      </header>

      <form class="modal-body whatsminer-auth-body" onsubmit={submit}>
        <p class="whatsminer-auth-hint">{msg("auth.modalHint")}</p>

        <label class="whatsminer-auth-field" for="whatsminer-auth-user">
          <span>{msg("auth.username")}</span>
          <input
            id="whatsminer-auth-user"
            bind:value={username}
            disabled={busy}
            autocomplete="username"
          />
        </label>

        <label class="whatsminer-auth-field" for="whatsminer-auth-pass">
          <span>{msg("auth.password")}</span>
          <input
            id="whatsminer-auth-pass"
            type="password"
            bind:value={password}
            disabled={busy}
            autocomplete="current-password"
          />
        </label>

        {#if errorText}
          <p class="whatsminer-auth-error">{errorText}</p>
        {/if}

        <div class="whatsminer-auth-actions">
          <button type="button" class="btn" disabled={busy} onclick={close}>
            {msg("auth.cancel")}
          </button>
          <button type="submit" class="btn primary" disabled={busy || !username.trim()}>
            {busy ? msg("auth.submitting") : msg("auth.submit")}
          </button>
        </div>
      </form>
    </div>
  </div>
{/if}
