<script lang="ts">
  let {
    checked = $bindable(false),
    disabled = false,
    label = "",
    onchange,
  }: {
    checked?: boolean;
    disabled?: boolean;
    label?: string;
    onchange?: (value: boolean) => void | Promise<void>;
  } = $props();

  async function toggle() {
    if (disabled) return;
    const next = !checked;
    checked = next;
    await onchange?.(next);
  }
</script>

<button
  type="button"
  class="cupertino-switch"
  class:is-on={checked}
  class:is-disabled={disabled}
  aria-pressed={checked}
  aria-label={label}
  {disabled}
  onclick={toggle}
>
  <span class="cupertino-switch-track">
    <span class="cupertino-switch-thumb"></span>
  </span>
  {#if label}
    <span class="cupertino-switch-label">{label}</span>
  {/if}
</button>

<style>
  .cupertino-switch {
    display: inline-flex;
    align-items: center;
    gap: 0.65rem;
    border: none;
    background: transparent;
    padding: 0;
    cursor: pointer;
    color: var(--text-primary);
    font: inherit;
  }

  .cupertino-switch.is-disabled {
    opacity: 0.55;
    cursor: not-allowed;
  }

  .cupertino-switch-track {
    width: 46px;
    height: 28px;
    border-radius: 999px;
    background: var(--border-strong);
    position: relative;
    transition: background 0.2s ease;
  }

  .cupertino-switch.is-on .cupertino-switch-track {
    background: #34c759;
  }

  .cupertino-switch-thumb {
    position: absolute;
    top: 2px;
    left: 2px;
    width: 24px;
    height: 24px;
    border-radius: 50%;
    background: #fff;
    box-shadow: 0 1px 4px rgba(0, 0, 0, 0.25);
    transition: transform 0.2s ease;
  }

  .cupertino-switch.is-on .cupertino-switch-thumb {
    transform: translateX(18px);
  }

  .cupertino-switch-label {
    font-size: 0.92rem;
  }
</style>
