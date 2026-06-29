<script lang="ts">
  import { t, type Locale, type MessageKey } from "$lib/i18n";

  let {
    locale,
    versionLabel,
    onInstall,
    onDismiss,
  }: {
    locale: Locale;
    versionLabel: string;
    onInstall: () => void;
    onDismiss: () => void;
  } = $props();

  function msg(key: MessageKey, args?: Record<string, string | number>) {
    return t(locale, key, args);
  }
</script>

<div class="update-notice" role="status">
  <span class="update-notice-dot" aria-hidden="true"></span>
  <p class="update-notice-text">{msg("updateNotice.message", { version: versionLabel })}</p>
  <div class="update-notice-actions">
    <button type="button" class="btn primary update-notice-install" onclick={onInstall}>
      {msg("updateNotice.install")}
    </button>
    <button type="button" class="btn ghost update-notice-dismiss" onclick={onDismiss}>
      {msg("updateNotice.dismiss")}
    </button>
  </div>
</div>
