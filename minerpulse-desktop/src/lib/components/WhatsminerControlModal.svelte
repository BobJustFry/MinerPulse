<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { save } from "@tauri-apps/plugin-dialog";
  import { t, type Locale, type MessageKey } from "$lib/i18n";
  import CupertinoSwitch from "$lib/components/CupertinoSwitch.svelte";
  import CraneApplyAnimation from "$lib/components/CraneApplyAnimation.svelte";
  import type { WhatsminerControlAction, WhatsminerControlState } from "$lib/types";

  let {
    open = $bindable(false),
    locale,
    ip = "",
    port = 4028,
    password = "",
    busy = false,
    onApplied,
  }: {
    open?: boolean;
    locale: Locale;
    ip?: string;
    port?: number;
    password?: string;
    busy?: boolean;
    onApplied?: () => void | Promise<void>;
  } = $props();

  let controlState = $state<WhatsminerControlState | null>(null);
  let loading = $state(false);
  let applying = $state(false);
  let applyStatus = $state<"idle" | "loading" | "success" | "error">("idle");
  let errorText = $state("");

  const modalBusy = $derived(busy || loading || applying);

  function msg(key: MessageKey, args?: Record<string, string | number>) {
    return t(locale, key, args);
  }

  function close() {
    if (modalBusy) return;
    open = false;
    errorText = "";
    applyStatus = "idle";
  }

  function onKeydown(event: KeyboardEvent) {
    if (event.key === "Escape" && !modalBusy) close();
  }

  async function refresh() {
    if (!ip.trim() || !password) return;
    loading = true;
    errorText = "";
    try {
      controlState = await invoke<WhatsminerControlState>("get_whatsminer_control_state", {
        request: { ip: ip.trim(), port, password },
      });
    } catch (err) {
      errorText = String(err);
    } finally {
      loading = false;
    }
  }

  async function apply(action: WhatsminerControlAction) {
    if (!ip.trim() || !password || applying) return;
    applying = true;
    applyStatus = "loading";
    errorText = "";
    try {
      const result = await invoke<{ ok: boolean; message?: string | null; state?: WhatsminerControlState | null }>(
        "apply_whatsminer_control",
        {
          request: { ip: ip.trim(), port, password, action },
        },
      );
      if (result.state) controlState = result.state;
      if (result.ok) {
        applyStatus = "success";
        await onApplied?.();
      } else {
        applyStatus = "error";
        errorText = result.message ?? msg("control.applyFailed");
      }
    } catch (err) {
      applyStatus = "error";
      errorText = String(err);
    } finally {
      applying = false;
      setTimeout(() => {
        if (applyStatus !== "loading") applyStatus = "idle";
      }, 1200);
    }
  }

  async function toggleMining(enabled: boolean) {
    await apply({ set_mining: { enabled } });
  }

  async function toggleApi(enabled: boolean) {
    await apply({ set_api_switch: { enabled } });
  }

  async function toggleFastBoot(enabled: boolean) {
    await apply({ set_fast_boot: { enabled } });
  }

  async function toggleWebPools(enabled: boolean) {
    await apply({ set_web_pools: { enabled } });
  }

  async function setLed(mode: string) {
    await apply({ set_led: { mode } });
  }

  async function setPowerMode(mode: string) {
    await apply({ set_power_mode: { mode } });
  }

  async function adjustNumber(
    field: "power_limit" | "target_freq" | "upfreq_speed" | "power_percent",
    delta: number,
  ) {
    if (!controlState) return;
    const map = {
      power_limit: controlState.power_limit_w ?? 0,
      target_freq: controlState.target_freq_pct ?? 0,
      upfreq_speed: controlState.upfreq_speed ?? 0,
      power_percent: controlState.power_percent ?? 0,
    };
    const next = Math.max(0, map[field] + delta);
    const action: WhatsminerControlAction =
      field === "power_limit"
        ? { set_power_limit: { watts: next } }
        : field === "target_freq"
          ? { set_target_freq: { percent: next } }
          : field === "upfreq_speed"
            ? { set_upfreq_speed: { speed: next } }
            : { set_power_percent: { percent: next } };
    await apply(action);
  }

  async function confirmReboot() {
    if (!confirm(msg("control.rebootConfirm"))) return;
    await apply({ reboot: null });
  }

  async function confirmRestore() {
    if (!confirm(msg("control.restoreConfirm"))) return;
    await apply({ restore_settings: null });
  }

  async function exportLog() {
    const path = await save({
      defaultPath: `whatsminer-log-${ip.replace(/\./g, "-")}.txt`,
      filters: [{ name: "Log", extensions: ["txt", "log"] }],
    });
    if (!path) return;
    try {
      await invoke("export_whatsminer_log", {
        request: { ip: ip.trim(), password, path },
      });
    } catch (err) {
      errorText = String(err);
    }
  }

  $effect(() => {
    if (open && ip.trim() && password) {
      void refresh();
    }
  });
</script>

<svelte:window onkeydown={onKeydown} />

{#if open}
  <div class="modal-backdrop" role="presentation">
    <div
      class="modal-card control-modal"
      role="dialog"
      aria-modal="true"
      aria-labelledby="wm-control-title"
      onclick={(e) => e.stopPropagation()}
      onmousedown={(e) => e.stopPropagation()}
    >
      <header class="modal-head">
        <div>
          <div class="modal-kicker">{msg("control.kicker")}</div>
          <h3 id="wm-control-title" class="modal-title">{msg("control.title")}</h3>
        </div>
        <button type="button" class="modal-close" disabled={modalBusy} onclick={close}>×</button>
      </header>

      <div class="modal-body control-body">
        {#if controlState?.writes_blocked}
          <p class="control-hint warn">{msg("control.defaultPasswordBlocked")}</p>
        {/if}
        {#if controlState?.api_switch === false}
          <p class="control-hint warn">{msg("control.apiSwitchOff")}</p>
        {/if}
        {#if errorText}
          <p class="control-hint error">{errorText}</p>
        {/if}

        <div class="control-crane-wrap">
          <CraneApplyAnimation active={applying} status={applyStatus} />
        </div>

        {#if controlState}
          <section class="control-section">
            <h4>{msg("control.section.toggles")}</h4>
            <div class="control-rows">
              <CupertinoSwitch
                label={msg("control.mining")}
                checked={controlState.mining ?? false}
                disabled={modalBusy}
                onchange={toggleMining}
              />
              <CupertinoSwitch
                label={msg("control.apiSwitch")}
                checked={controlState.api_switch ?? false}
                disabled={modalBusy}
                onchange={toggleApi}
              />
              <CupertinoSwitch
                label={msg("control.fastBoot")}
                checked={controlState.fast_boot ?? false}
                disabled={modalBusy}
                onchange={toggleFastBoot}
              />
              <CupertinoSwitch
                label={msg("control.webPools")}
                checked={controlState.web_pools ?? false}
                disabled={modalBusy}
                onchange={toggleWebPools}
              />
            </div>
          </section>

          <section class="control-section">
            <h4>{msg("control.section.led")}</h4>
            <div class="control-segments">
              {#each ["auto", "on", "off"] as mode}
                <button
                  type="button"
                  class="control-segment"
                  class:active={controlState.led_mode === mode}
                  disabled={modalBusy}
                  onclick={() => setLed(mode)}
                >
                  {msg(`control.led.${mode}` as MessageKey)}
                </button>
              {/each}
            </div>
          </section>

          <section class="control-section">
            <h4>{msg("control.section.performance")}</h4>
            <div class="control-segments">
              {#each ["low", "normal", "high"] as mode}
                <button
                  type="button"
                  class="control-segment"
                  class:active={controlState.power_mode === mode}
                  disabled={modalBusy}
                  onclick={() => setPowerMode(mode)}
                >
                  {msg(`control.powerMode.${mode}` as MessageKey)}
                </button>
              {/each}
            </div>
          </section>

          <section class="control-section">
            <h4>{msg("control.section.numeric")}</h4>
            {#each [
              { key: "power_limit", label: "control.powerLimit", value: controlState.power_limit_w ?? 0, step: 50 },
              { key: "target_freq", label: "control.targetFreq", value: controlState.target_freq_pct ?? 0, step: 1 },
              { key: "upfreq_speed", label: "control.upfreqSpeed", value: controlState.upfreq_speed ?? 0, step: 1 },
              { key: "power_percent", label: "control.powerPercent", value: controlState.power_percent ?? 0, step: 1 },
            ] as row (row.key)}
              <div class="control-stepper">
                <span>{msg(row.label as MessageKey)}</span>
                <div class="control-stepper-actions">
                  <button
                    type="button"
                    disabled={modalBusy}
                    onclick={() => void adjustNumber(row.key as "power_limit" | "target_freq" | "upfreq_speed" | "power_percent", -row.step)}>−</button>
                  <strong>{row.value}</strong>
                  <button
                    type="button"
                    disabled={modalBusy}
                    onclick={() => void adjustNumber(row.key as "power_limit" | "target_freq" | "upfreq_speed" | "power_percent", row.step)}>+</button>
                </div>
              </div>
            {/each}
          </section>

          <section class="control-section">
            <h4>{msg("control.section.actions")}</h4>
            <div class="control-actions">
              <button type="button" class="btn-secondary" disabled={modalBusy} onclick={confirmReboot}>
                {msg("control.reboot")}
              </button>
              <button type="button" class="btn-secondary" disabled={modalBusy} onclick={confirmRestore}>
                {msg("control.restore")}
              </button>
              <button type="button" class="btn-secondary" disabled={modalBusy} onclick={() => void exportLog()}>
                {msg("control.exportLog")}
              </button>
            </div>
          </section>
        {:else if loading}
          <p class="control-hint">{msg("control.loading")}</p>
        {/if}
      </div>

      <footer class="modal-footer control-footer">
        <button type="button" class="btn-secondary" disabled={modalBusy} onclick={() => void refresh()}>
          {msg("control.refresh")}
        </button>
        <button type="button" class="btn-secondary" disabled={modalBusy} onclick={close}>
          {msg("control.close")}
        </button>
      </footer>
    </div>
  </div>
{/if}

<style>
  .control-modal {
    width: min(520px, 92vw);
    max-height: 88vh;
    display: flex;
    flex-direction: column;
  }

  .control-body {
    overflow: auto;
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }

  .control-hint {
    margin: 0;
    font-size: 0.88rem;
    color: var(--text-muted);
  }

  .control-hint.warn {
    color: #e6a700;
  }

  .control-hint.error {
    color: var(--danger);
  }

  .control-crane-wrap {
    display: flex;
    justify-content: center;
  }

  .control-section h4 {
    margin: 0 0 0.5rem;
    font-size: 0.82rem;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--text-muted);
  }

  .control-rows {
    display: flex;
    flex-direction: column;
    gap: 0.65rem;
  }

  .control-segments {
    display: flex;
    gap: 0.35rem;
    flex-wrap: wrap;
  }

  .control-segment {
    flex: 1;
    min-width: 4.5rem;
    border: 1px solid var(--border);
    background: var(--bg-elevated);
    color: var(--text-primary);
    border-radius: 8px;
    padding: 0.45rem 0.6rem;
    cursor: pointer;
  }

  .control-segment.active {
    border-color: var(--accent);
    background: color-mix(in srgb, var(--accent) 18%, transparent);
  }

  .control-stepper {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 0.75rem;
    padding: 0.35rem 0;
  }

  .control-stepper-actions {
    display: inline-flex;
    align-items: center;
    gap: 0.5rem;
  }

  .control-stepper-actions button {
    width: 28px;
    height: 28px;
    border-radius: 6px;
    border: 1px solid var(--border);
    background: var(--bg-elevated);
    cursor: pointer;
  }

  .control-actions {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
  }

  .control-footer {
    display: flex;
    justify-content: flex-end;
    gap: 0.5rem;
  }
</style>
