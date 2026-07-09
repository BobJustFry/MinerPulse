<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { save } from "@tauri-apps/plugin-dialog";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { formatAppError } from "$lib/formatAppError";
  import { t, type Locale, type MessageKey } from "$lib/i18n";
  import CupertinoSwitch from "$lib/components/CupertinoSwitch.svelte";
  import CraneApplyAnimation from "$lib/components/CraneApplyAnimation.svelte";
  import ManagedModalCard from "$lib/components/ManagedModalCard.svelte";
  import type { ErrorResponse, WhatsminerControlAction, WhatsminerControlState } from "$lib/types";

  type ControlDraft = {
    mining: boolean;
    api_switch: boolean;
    fast_boot: boolean;
    web_pools: boolean;
    led_mode: string;
    power_mode: string;
    power_limit_w: number;
    target_freq_pct: number;
    upfreq_speed: number;
    power_percent: number;
  };

  type ApplyPhase =
    | "idle"
    | "sending"
    | "verifying"
    | "waiting_online"
    | "monitoring"
    | "exporting"
    | "success"
    | "error"
    | "reboot_offer";

  function applyPhaseKeepsBusy(phase: ApplyPhase): boolean {
    return phase === "reboot_offer" || phase === "monitoring" || phase === "waiting_online";
  }

  const VERIFY_POLL_MS = 2500;
  const VERIFY_TIMEOUT_MS = 90_000;
  const MONITOR_TIMEOUT_MS = 300_000;
  const ONLINE_WAIT_MS = 180_000;

  let {
    open = $bindable(false),
    locale,
    ip = "",
    port = 4028,
    username = "admin",
    password = "",
    busy = false,
    onApplied,
    onPasswordChanged,
  }: {
    open?: boolean;
    locale: Locale;
    ip?: string;
    port?: number;
    username?: string;
    password?: string;
    busy?: boolean;
    onApplied?: () => void | Promise<void>;
    onPasswordChanged?: (newPassword: string) => void | Promise<void>;
  } = $props();

  let controlState = $state<WhatsminerControlState | null>(null);
  let baseline = $state<ControlDraft | null>(null);
  let draft = $state<ControlDraft | null>(null);
  let loading = $state(false);
  let applying = $state(false);
  let verifyingAuth = $state(false);
  let applyPhase = $state<ApplyPhase>("idle");
  let applyStatus = $state<"idle" | "loading" | "success" | "error">("idle");
  let errorText = $state("");
  let authPromptOpen = $state(false);
  let authPasswordInput = $state("");
  let authError = $state("");
  let authDismissed = $state(false);
  let sessionPassword = $state("");
  let pendingBatch = $state<WhatsminerControlAction[] | null>(null);
  let monitorCancelled = $state(false);
  let pendingExpectedDraft = $state<ControlDraft | null>(null);
  let pendingChangedKeys = $state<(keyof ControlDraft)[]>([]);
  let newPasswordInput = $state("");
  let confirmPasswordInput = $state("");
  let passwordChanging = $state(false);
  let passwordError = $state("");
  let passwordSuccess = $state("");
  let exportFlow = $state(false);
  let exportRunId = 0;

  const modalBusy = $derived(
    busy ||
      loading ||
      applying ||
      verifyingAuth ||
      passwordChanging ||
      applyPhase === "monitoring" ||
      applyPhase === "waiting_online",
  );
  const applyOverlayOpen = $derived(applyPhase !== "idle");
  const authOverlayOpen = $derived(authPromptOpen && !applyOverlayOpen);
  const applyStatusText = $derived.by(() => {
    switch (applyPhase) {
      case "sending":
        return msg("control.phase.sending");
      case "verifying":
        return msg("control.phase.verifying");
      case "waiting_online":
        return msg("control.phase.waitingOnline");
      case "monitoring":
        return msg("control.phase.monitoring");
      case "exporting":
        return msg("control.phase.exporting");
      case "success":
        return exportFlow ? msg("control.phase.exportSuccess") : msg("control.phase.success");
      case "error":
        return exportFlow ? msg("control.phase.exportError") : msg("control.phase.error");
      case "reboot_offer":
        return msg("control.reboot.hint");
      default:
        return "";
    }
  });
  const apiSwitchOff = $derived(controlState?.apiSwitch === false);
  const hasPendingChanges = $derived(
    baseline != null && draft != null && draftDiffers(baseline, draft),
  );

  function formatErr(err: unknown): string {
    return formatAppError(locale, err);
  }

  function msg(key: MessageKey, args?: Record<string, string | number>) {
    return t(locale, key, args);
  }

  const NUMERIC_FIELDS = [
    { key: "power_limit_w", label: "control.powerLimit", tip: "control.tip.powerLimit", min: 0, max: 99999, step: 50 },
    { key: "target_freq_pct", label: "control.targetFreq", tip: "control.tip.targetFreq", min: -100, max: 100, step: 1 },
    { key: "upfreq_speed", label: "control.upfreqSpeed", tip: "control.tip.upfreqSpeed", min: 0, max: 9, step: 1 },
    { key: "power_percent", label: "control.powerPercent", tip: "control.tip.powerPercent", min: 0, max: 100, step: 1 },
  ] as const;

  type NumericFieldKey = (typeof NUMERIC_FIELDS)[number]["key"];

  function clampNumeric(value: number, min: number, max: number): number {
    return Math.min(max, Math.max(min, value));
  }

  function markApiWrite(label: string): string {
    return `${label} *`;
  }

  function controlHint(tipKey: MessageKey, requiresApiWrite = true): string {
    if (requiresApiWrite && apiSwitchOff) return msg("control.apiSwitchDisabledHint");
    return msg(tipKey);
  }

  function controlDisabled(requiresApiWrite = false): boolean {
    if (modalBusy) return true;
    if (requiresApiWrite && apiSwitchOff) return true;
    return false;
  }

  function draftFromState(state: WhatsminerControlState): ControlDraft {
    return {
      mining: state.mining ?? false,
      api_switch: state.apiSwitch ?? false,
      fast_boot: state.fastBoot ?? false,
      web_pools: state.webPools ?? false,
      led_mode: state.ledMode ?? "auto",
      power_mode: state.powerMode ?? "normal",
      power_limit_w: state.powerLimitW ?? 0,
      target_freq_pct: state.targetFreqPct ?? 0,
      upfreq_speed: state.upfreqSpeed ?? 0,
      power_percent: state.powerPercent ?? 0,
    };
  }

  function draftDiffers(a: ControlDraft, b: ControlDraft): boolean {
    return (
      a.mining !== b.mining ||
      a.api_switch !== b.api_switch ||
      a.fast_boot !== b.fast_boot ||
      a.web_pools !== b.web_pools ||
      a.led_mode !== b.led_mode ||
      a.power_mode !== b.power_mode ||
      a.power_limit_w !== b.power_limit_w ||
      a.target_freq_pct !== b.target_freq_pct ||
      a.upfreq_speed !== b.upfreq_speed ||
      a.power_percent !== b.power_percent
    );
  }

  function syncDraftFromState(state: WhatsminerControlState) {
    const next = draftFromState(state);
    baseline = next;
    draft = { ...next };
  }

  function setDraftField<K extends keyof ControlDraft>(key: K, value: ControlDraft[K]) {
    if (!draft) return;
    draft = { ...draft, [key]: value };
  }

  function buildPendingActions(base: ControlDraft, next: ControlDraft): WhatsminerControlAction[] {
    const actions: WhatsminerControlAction[] = [];
    if (base.mining !== next.mining) actions.push({ set_mining: { enabled: next.mining } });
    if (base.api_switch !== next.api_switch) {
      actions.push({ set_api_switch: { enabled: next.api_switch } });
    }
    if (base.fast_boot !== next.fast_boot) actions.push({ set_fast_boot: { enabled: next.fast_boot } });
    if (base.web_pools !== next.web_pools) actions.push({ set_web_pools: { enabled: next.web_pools } });
    if (base.led_mode !== next.led_mode) actions.push({ set_led: { mode: next.led_mode } });
    if (base.power_mode !== next.power_mode) actions.push({ set_power_mode: { mode: next.power_mode } });
    if (base.power_limit_w !== next.power_limit_w) {
      actions.push({ set_power_limit: { watts: next.power_limit_w } });
    }
    if (base.target_freq_pct !== next.target_freq_pct) {
      actions.push({ set_target_freq: { percent: next.target_freq_pct } });
    }
    if (base.upfreq_speed !== next.upfreq_speed) {
      actions.push({ set_upfreq_speed: { speed: next.upfreq_speed } });
    }
    if (base.power_percent !== next.power_percent) {
      actions.push({ set_power_percent: { percent: next.power_percent } });
    }
    return actions;
  }

  function mapResultMessage(message?: string | null): string {
    if (!message) return msg("control.applyFailed");
    if (message === "default_password_blocked") return msg("control.auth.required");
    if (message === "verify_failed") return msg("control.verifyFailed");
    if (message === "reboot_required") return msg("control.reboot.hint");
    if (message === "api_switch_web_auth_failed") return msg("control.apiSwitch.webAuthFailed");
    if (message === "api_switch_manual_required") return msg("control.apiSwitch.manualRequired");
    if (message === "api_switch_enable_failed") return msg("control.apiSwitch.enableFailed");
    return message;
  }

  function mapPasswordError(err: unknown): string {
    const e = err as { args?: { message?: string } };
    const code = e?.args?.message;
    if (code === "default_password_change_blocked") return msg("control.password.defaultBlocked");
    if (code === "change_password_failed") return msg("control.password.failed");
    if (code) return mapResultMessage(code);
    return formatAppError(locale, err);
  }

  function resetPasswordForm() {
    newPasswordInput = "";
    confirmPasswordInput = "";
    passwordError = "";
    passwordSuccess = "";
  }

  async function submitPasswordChange() {
    const next = newPasswordInput.trim();
    const confirm = confirmPasswordInput.trim();
    const current = sessionPassword.trim() || "admin";
    if (!ip.trim() || passwordChanging) return;

    passwordError = "";
    passwordSuccess = "";

    if (next.length < 4) {
      passwordError = msg("control.password.tooShort");
      return;
    }
    if (next !== confirm) {
      passwordError = msg("control.password.mismatch");
      return;
    }
    if (next === current) {
      passwordError = msg("control.password.sameAsCurrent");
      return;
    }

    passwordChanging = true;
    try {
      await invoke("change_whatsminer_super_password", {
        request: {
          ip: ip.trim(),
          username: username.trim() || "admin",
          old_password: current,
          new_password: next,
        },
      });
      sessionPassword = next;
      resetPasswordForm();
      passwordSuccess = msg("control.password.success");
      await onPasswordChanged?.(next);
      authDismissed = false;
      authPromptOpen = false;
      await refresh(next, false, { promptOnBlocked: false });
      setTimeout(() => {
        if (passwordSuccess) passwordSuccess = "";
      }, 4000);
    } catch (err) {
      passwordError = mapPasswordError(err);
    } finally {
      passwordChanging = false;
    }
  }

  function sleep(ms: number) {
    return new Promise<void>((resolve) => setTimeout(resolve, ms));
  }

  function changedKeys(base: ControlDraft, next: ControlDraft): (keyof ControlDraft)[] {
    return (Object.keys(base) as (keyof ControlDraft)[]).filter((key) => base[key] !== next[key]);
  }

  function draftMatchesExpected(current: ControlDraft, expected: ControlDraft, keys: (keyof ControlDraft)[]) {
    return keys.every((key) => current[key] === expected[key]);
  }

  function actionsMayNeedReboot(actions: WhatsminerControlAction[]): boolean {
    return actions.some(
      (a) =>
        "set_api_switch" in a ||
        "set_fast_boot" in a ||
        "set_web_pools" in a ||
        "set_power_limit" in a ||
        "set_target_freq" in a ||
        "set_upfreq_speed" in a ||
        "set_power_percent" in a ||
        "restore_settings" in a,
    );
  }

  function resetApplyFlow() {
    monitorCancelled = false;
    pendingExpectedDraft = null;
    pendingChangedKeys = [];
    applying = false;
    applyPhase = "idle";
    applyStatus = "idle";
    exportFlow = false;
  }

  function scheduleApplyIdle(delayMs = 1400) {
    setTimeout(() => {
      if (applyPhase === "success" || applyPhase === "error") {
        resetApplyFlow();
      }
    }, delayMs);
  }

  function dismissApplyOverlay() {
    applying = false;
    applyPhase = "idle";
    applyStatus = "idle";
    exportFlow = false;
  }

  function cancelApplyOperation() {
    const wasExport = applyPhase === "exporting";
    if (wasExport) {
      exportRunId += 1;
      void invoke("cancel_whatsminer_log_export");
    } else {
      monitorCancelled = true;
    }
    dismissApplyOverlay();
    errorText = msg(wasExport ? "control.export.cancelled" : "control.monitor.cancelled");
  }

  function cancelMonitor() {
    cancelApplyOperation();
  }

  function openAuthPrompt() {
    if (controlState?.apiSwitch === false) return;
    authPasswordInput = sessionPassword;
    authError = "";
    authPromptOpen = true;
    authDismissed = false;
  }

  function dismissAuthPrompt() {
    authPromptOpen = false;
    authDismissed = true;
    authError = "";
  }

  async function submitAuthVerify() {
    const candidate = authPasswordInput.trim();
    if (!ip.trim() || !candidate || verifyingAuth) return;
    verifyingAuth = true;
    authError = "";
    try {
      const ok = await refresh(candidate, false, { promptOnBlocked: false });
      if (ok && controlState) {
        sessionPassword = candidate;
        authPromptOpen = false;
        authDismissed = false;
        if (candidate !== password) {
          await onPasswordChanged?.(candidate);
        }
        if (pendingBatch?.length) {
          const batch = pendingBatch;
          pendingBatch = null;
          await runActions(batch);
        }
        return;
      }
      authError = msg("control.auth.failed");
    } finally {
      verifyingAuth = false;
    }
  }

  async function openWebUi() {
    const url = `https://${ip.trim()}/`;
    try {
      await openUrl(url);
    } catch {
      window.open(url, "_blank", "noopener,noreferrer");
    }
  }

  function dismissRebootOffer() {
    resetApplyFlow();
    errorText = msg("control.reboot.dismissed");
  }

  function close() {
    if (applyPhase === "monitoring" || applyPhase === "waiting_online" || applyPhase === "exporting") {
      cancelApplyOperation();
      return;
    }
    if (modalBusy) return;
    open = false;
    errorText = "";
    resetApplyFlow();
    authPromptOpen = false;
    authDismissed = false;
    authPasswordInput = "";
    authError = "";
    pendingBatch = null;
  }

  function onKeydown(event: KeyboardEvent) {
    if (event.key !== "Escape" || modalBusy) return;
    if (authPromptOpen) {
      dismissAuthPrompt();
      return;
    }
    close();
  }

  function discardDraft() {
    if (!baseline) return;
    draft = { ...baseline };
    errorText = "";
  }

  async function refresh(
    overridePassword?: string,
    silent = false,
    options: { promptOnBlocked?: boolean } = {},
  ) {
    const authPassword = overridePassword ?? sessionPassword;
    if (!ip.trim() || !authPassword) return false;
    const promptOnBlocked = options.promptOnBlocked ?? false;
    if (!silent) loading = true;
    if (!silent) errorText = "";
    try {
      controlState = await invoke<WhatsminerControlState>("get_whatsminer_control_state", {
        request: { ip: ip.trim(), port, password: authPassword },
      });
      if (!silent) {
        syncDraftFromState(controlState);
        sessionPassword = authPassword;
        authPromptOpen = false;
        authError = "";
      }
      return true;
    } catch (err) {
      if (!silent) errorText = formatErr(err);
      return false;
    } finally {
      if (!silent) loading = false;
    }
  }

  async function waitUntilApplied(
    expected: ControlDraft,
    keys: (keyof ControlDraft)[],
    timeoutMs = VERIFY_TIMEOUT_MS,
  ): Promise<boolean> {
    const started = Date.now();
    while (Date.now() - started < timeoutMs) {
      if (monitorCancelled) return false;
      const ok = await refresh(undefined, true);
      if (ok && controlState) {
        const current = draftFromState(controlState);
        if (draftMatchesExpected(current, expected, keys)) {
          syncDraftFromState(controlState);
          return true;
        }
      }
      await sleep(VERIFY_POLL_MS);
    }
    return false;
  }

  async function waitForMinerOnline(timeoutMs = ONLINE_WAIT_MS): Promise<boolean> {
    const started = Date.now();
    while (Date.now() - started < timeoutMs) {
      if (monitorCancelled) return false;
      const ok = await refresh(undefined, true);
      if (ok) return true;
      await sleep(3000);
    }
    return false;
  }

  async function runMonitor(timeoutMs = MONITOR_TIMEOUT_MS) {
    if (!pendingExpectedDraft || pendingChangedKeys.length === 0) return false;
    applyPhase = "monitoring";
    applyStatus = "loading";
    const ok = await waitUntilApplied(pendingExpectedDraft, pendingChangedKeys, timeoutMs);
    if (monitorCancelled) return false;
    if (ok) {
      applyPhase = "success";
      applyStatus = "success";
      pendingBatch = null;
      await onApplied?.();
      await refresh(undefined, false, { promptOnBlocked: false });
      scheduleApplyIdle();
      return true;
    }
    applyPhase = "error";
    applyStatus = "error";
    errorText = msg("control.monitor.timeout");
    scheduleApplyIdle();
    return false;
  }

  function offerRebootAfterApply(expected: ControlDraft, keys: (keyof ControlDraft)[]) {
    pendingExpectedDraft = expected;
    pendingChangedKeys = keys;
    applyPhase = "reboot_offer";
    applyStatus = "loading";
    applying = true;
  }

  async function rebootAndMonitor() {
    if (!pendingExpectedDraft) return;
    monitorCancelled = false;
    applyPhase = "sending";
    applyStatus = "loading";
    const rebootOk = (await applySingle({ reboot: null }, false)).ok;
    if (!rebootOk || monitorCancelled) {
      applyPhase = "error";
      applyStatus = "error";
      scheduleApplyIdle();
      return;
    }
    applyPhase = "waiting_online";
    const online = await waitForMinerOnline();
    if (!online || monitorCancelled) {
      applyPhase = "error";
      applyStatus = "error";
      errorText = msg("control.monitor.offlineTimeout");
      scheduleApplyIdle();
      return;
    }
    applying = true;
    await runMonitor();
    applying = false;
  }

  async function startMonitorOnly() {
    monitorCancelled = false;
    applying = true;
    await runMonitor();
    applying = false;
  }

  async function applySingle(
    action: WhatsminerControlAction,
    syncDraft = true,
  ): Promise<{ ok: boolean; rebootRequired?: boolean }> {
    const result = await invoke<{ ok: boolean; message?: string | null; state?: WhatsminerControlState | null }>(
      "apply_whatsminer_control",
      {
        request: { ip: ip.trim(), port, username: username.trim() || "admin", password: sessionPassword, action },
      },
    );
    if (result.state) {
      controlState = result.state;
      if (syncDraft) syncDraftFromState(result.state);
    }
    if (result.ok) return { ok: true };

    const message = mapResultMessage(result.message);
    errorText = message;
    if (result.message === "default_password_blocked") {
      openAuthPrompt();
    }
    if (result.message === "reboot_required") {
      return { ok: false, rebootRequired: true };
    }
    return { ok: false };
  }

  async function runActions(actions: WhatsminerControlAction[], options: { restoreFlow?: boolean } = {}) {
    if (!ip.trim() || !sessionPassword || applying || actions.length === 0 || !baseline || !draft) return;

    const apiEnabling = baseline.api_switch === false && draft.api_switch === true;
    const expected = { ...draft };
    const keys = changedKeys(baseline, expected);

    applying = true;
    exportFlow = false;
    monitorCancelled = false;
    applyPhase = "sending";
    applyStatus = "loading";
    errorText = "";
    try {
      for (const action of actions) {
        const outcome = await applySingle(action, false);
        if (!outcome.ok) {
          if (outcome.rebootRequired || actionsMayNeedReboot(actions)) {
            offerRebootAfterApply(expected, keys);
            return;
          }
          applyPhase = "error";
          applyStatus = "error";
          pendingBatch = actions;
          scheduleApplyIdle();
          return;
        }
      }

      applyPhase = "verifying";
      const verified = await waitUntilApplied(expected, keys);
      if (monitorCancelled) return;

      if (verified) {
        applyPhase = "success";
        applyStatus = "success";
        baseline = { ...expected };
        draft = { ...expected };
        pendingBatch = null;
        await onApplied?.();
        if (apiEnabling) {
          await refresh(undefined, false, { promptOnBlocked: false });
        }
        scheduleApplyIdle();
        return;
      }

      if (options.restoreFlow || actionsMayNeedReboot(actions)) {
        offerRebootAfterApply(expected, keys);
        return;
      }

      applyPhase = "error";
      applyStatus = "error";
      errorText = msg("control.verifyFailed");
      scheduleApplyIdle();
    } catch (err) {
      applyPhase = "error";
      applyStatus = "error";
      errorText = formatErr(err);
      scheduleApplyIdle();
    } finally {
      if (!applyPhaseKeepsBusy(applyPhase as ApplyPhase)) {
        applying = false;
      }
    }
  }

  async function commitDraft() {
    if (!baseline || !draft) return;
    const actions = buildPendingActions(baseline, draft);
    await runActions(actions);
  }

  async function confirmReboot() {
    if (!confirm(msg("control.rebootConfirm"))) return;
    applying = true;
    applyPhase = "sending";
    applyStatus = "loading";
    errorText = "";
    try {
      const outcome = await applySingle({ reboot: null }, false);
      if (outcome.ok) {
        applyPhase = "success";
        applyStatus = "success";
        scheduleApplyIdle();
      } else {
        applyPhase = "error";
        applyStatus = "error";
        scheduleApplyIdle();
      }
    } catch (err) {
      applyPhase = "error";
      applyStatus = "error";
      errorText = formatErr(err);
      scheduleApplyIdle();
    } finally {
      if (!applyPhaseKeepsBusy(applyPhase as ApplyPhase)) applying = false;
    }
  }

  async function confirmRestore() {
    if (!confirm(msg("control.restoreConfirm"))) return;
    await runActions([{ restore_settings: null }], { restoreFlow: true });
  }

  async function exportLog() {
    if (!confirm(msg("control.exportConfirm"))) return;
    const path = await save({
      defaultPath: `whatsminer-log-${ip.replace(/\./g, "-")}.txt`,
      filters: [{ name: "Log", extensions: ["txt", "log"] }],
    });
    if (!path) return;
    const runId = exportRunId + 1;
    exportRunId = runId;
    exportFlow = true;
    monitorCancelled = false;
    applying = true;
    applyPhase = "exporting";
    applyStatus = "loading";
    errorText = "";
    try {
      await invoke("export_whatsminer_log", {
        request: { ip: ip.trim(), password: sessionPassword, path },
      });
      if (runId !== exportRunId) return;
      applyPhase = "success";
      applyStatus = "success";
      scheduleApplyIdle();
    } catch (err) {
      if (runId !== exportRunId) return;
      const code = (err as ErrorResponse)?.code;
      if (code === "OPERATION_CANCELLED") return;
      applyPhase = "error";
      applyStatus = "error";
      errorText = formatErr(err);
      if (formatErr(err).toLowerCase().includes("password")) {
        openAuthPrompt();
      }
      scheduleApplyIdle();
    } finally {
      if (runId === exportRunId && !applyPhaseKeepsBusy(applyPhase as ApplyPhase)) {
        applying = false;
      }
    }
  }

  function adjustDraftNumber(field: NumericFieldKey, delta: number, step: number, min: number, max: number) {
    if (!draft) return;
    setDraftField(field, clampNumeric(draft[field] + delta * step, min, max));
  }

  function fieldChanged(key: keyof ControlDraft): boolean {
    return baseline != null && draft != null && baseline[key] !== draft[key];
  }

  $effect(() => {
    if (!open || !ip.trim()) return;
    sessionPassword = password || "admin";
    authDismissed = false;
    authPromptOpen = false;
    resetPasswordForm();
    void refresh(undefined, false, { promptOnBlocked: false });
  });
</script>

<svelte:window onkeydown={onKeydown} />

{#if open}
  <div class="modal-backdrop control-modal-backdrop" role="presentation">
    <ManagedModalCard
      layoutId="whatsminer-control"
      class="control-modal"
      defaultWidth={520}
      defaultHeight={680}
      minWidth={360}
      minHeight={400}
      dragDisabled={modalBusy || applyOverlayOpen}
      style="max-width: min(96vw, 920px)"
      role="dialog"
      aria-modal="true"
      aria-labelledby="wm-control-title"
      onclick={(e: MouseEvent) => e.stopPropagation()}
      onmousedown={(e: MouseEvent) => e.stopPropagation()}
    >
      {#if authOverlayOpen}
        <div class="control-apply-overlay control-auth-overlay" role="dialog" aria-labelledby="wm-control-auth-title">
          <h4 id="wm-control-auth-title" class="control-auth-title">{msg("control.auth.title")}</h4>
          <p class="control-apply-status">{msg("control.auth.hint")}</p>
          <label class="password-field control-auth-field">
            <span>{msg("control.auth.password")}</span>
            <input
              type="password"
              bind:value={authPasswordInput}
              disabled={modalBusy}
              autocomplete="current-password"
              onkeydown={(e) => {
                if (e.key === "Enter") void submitAuthVerify();
              }}
            />
          </label>
          {#if authError}
            <p class="control-hint error">{authError}</p>
          {/if}
          <div class="control-reboot-actions">
            <button
              type="button"
              class="btn primary btn-with-spinner"
              disabled={modalBusy || !authPasswordInput.trim()}
              onclick={() => void submitAuthVerify()}
            >
              {#if verifyingAuth}
                <span class="btn-spinner" aria-hidden="true"></span>
              {/if}
              {verifyingAuth ? msg("control.auth.verifying") : msg("control.auth.verify")}
            </button>
            <button type="button" class="btn" disabled={modalBusy} onclick={() => void openWebUi()}>
              {msg("control.auth.openWeb")}
            </button>
            <button type="button" class="btn ghost" disabled={modalBusy} onclick={dismissAuthPrompt}>
              {msg("control.auth.cancel")}
            </button>
          </div>
        </div>
      {/if}

      {#if applyOverlayOpen}
        <div class="control-apply-overlay" role="status" aria-live="polite">
          {#if applyPhase !== "reboot_offer"}
            <CraneApplyAnimation
              active={applyPhase === "sending" || applyPhase === "verifying" || applyPhase === "monitoring" || applyPhase === "waiting_online" || applyPhase === "exporting"}
              status={applyStatus}
            />
            <p class="control-apply-status">{applyStatusText}</p>
            {#if applyPhase === "monitoring" || applyPhase === "waiting_online" || applyPhase === "exporting"}
              <button type="button" class="btn" onclick={cancelApplyOperation}>{msg("control.apply.cancel")}</button>
            {/if}
          {:else}
            <p class="control-apply-status">{msg("control.reboot.hint")}</p>
            <div class="control-reboot-actions">
              <button type="button" class="btn primary" onclick={() => void rebootAndMonitor()}>
                {msg("control.reboot.now")}
              </button>
              <button type="button" class="btn" onclick={() => void startMonitorOnly()}>
                {msg("control.reboot.monitor")}
              </button>
              <button type="button" class="btn ghost" onclick={dismissRebootOffer}>
                {msg("control.reboot.dismiss")}
              </button>
            </div>
          {/if}
        </div>
      {/if}

      <header class="modal-head control-modal-head">
        <div class="control-modal-head-text">
          <div class="modal-kicker">{msg("control.kicker")}</div>
          <h3 id="wm-control-title" class="modal-title">{msg("control.title")}</h3>
        </div>
        <button type="button" class="modal-close" disabled={modalBusy} onclick={close}>×</button>
      </header>

      <div class="modal-body control-body">
        {#if apiSwitchOff}
          <p class="control-hint warn">{msg("control.apiSwitchOff")}</p>
        {/if}
        {#if errorText}
          <p class="control-hint error">{errorText}</p>
        {/if}

        {#if draft}
          <section class="control-section">
            <h4>{msg("control.section.toggles")}</h4>
            <div class="control-rows">
              <CupertinoSwitch
                label={msg("control.mining")}
                hint={msg("control.tip.mining")}
                checked={draft.mining}
                disabled={modalBusy}
                onchange={(enabled) => setDraftField("mining", enabled)}
              />
              <CupertinoSwitch
                label={msg("control.apiSwitch")}
                hint={controlHint("control.tip.apiSwitch", false)}
                apiWrite
                checked={draft.api_switch}
                disabled={modalBusy}
                onchange={(enabled) => setDraftField("api_switch", enabled)}
              />
              <CupertinoSwitch
                label={msg("control.fastBoot")}
                hint={controlHint("control.tip.fastBoot")}
                apiWrite
                checked={draft.fast_boot}
                disabled={controlDisabled(true)}
                onchange={(enabled) => setDraftField("fast_boot", enabled)}
              />
              <CupertinoSwitch
                label={msg("control.webPools")}
                hint={controlHint("control.tip.webPools")}
                apiWrite
                checked={draft.web_pools}
                disabled={controlDisabled(true)}
                onchange={(enabled) => setDraftField("web_pools", enabled)}
              />
            </div>
          </section>

          <section class="control-section" class:api-blocked={apiSwitchOff}>
            <h4
              class="control-hint-label"
              class:field-changed={fieldChanged("led_mode")}
              title={controlHint("control.tip.led")}
            >
              {markApiWrite(msg("control.section.led"))}
            </h4>
            <div class="control-segment-group">
              {#each ["auto", "on", "off"] as mode}
                <button
                  type="button"
                  class="control-segment"
                  class:active={draft.led_mode === mode}
                  class:changed={baseline?.led_mode !== draft.led_mode && draft.led_mode === mode}
                  disabled={controlDisabled(true)}
                  title={controlHint(`control.tip.led.${mode}` as MessageKey)}
                  onclick={() => setDraftField("led_mode", mode)}
                >
                  {msg(`control.led.${mode}` as MessageKey)}
                </button>
              {/each}
            </div>
          </section>

          <section class="control-section">
            <h4 class="control-hint-label" class:field-changed={fieldChanged("power_mode")} title={msg("control.tip.powerMode")}>
              {msg("control.section.performance")}
            </h4>
            <div class="control-segment-group">
              {#each ["low", "normal", "high"] as mode}
                <button
                  type="button"
                  class="control-segment"
                  class:active={draft.power_mode === mode}
                  class:changed={baseline?.power_mode !== draft.power_mode && draft.power_mode === mode}
                  disabled={modalBusy}
                  title={msg(`control.tip.powerMode.${mode}` as MessageKey)}
                  onclick={() => setDraftField("power_mode", mode)}
                >
                  {msg(`control.powerMode.${mode}` as MessageKey)}
                </button>
              {/each}
            </div>
          </section>

          <section class="control-section" class:api-blocked={apiSwitchOff}>
            <h4 class="control-hint-label" title={apiSwitchOff ? msg("control.apiSwitchDisabledHint") : undefined}>
              {markApiWrite(msg("control.section.numeric"))}
            </h4>
            {#each NUMERIC_FIELDS as row (row.key)}
              <div class="control-stepper" class:field-changed={fieldChanged(row.key)}>
                <span class="control-hint-label" title={controlHint(row.tip)}>{markApiWrite(msg(row.label))}</span>
                <div class="control-stepper-actions">
                  <button
                    type="button"
                    class="btn btn-icon-only control-step-btn"
                    disabled={controlDisabled(true) || draft[row.key] <= row.min}
                    aria-label="-"
                    title={controlHint(row.tip)}
                    onclick={() => adjustDraftNumber(row.key, -1, row.step, row.min, row.max)}>−</button>
                  <strong class="control-step-value">{draft[row.key]}</strong>
                  <button
                    type="button"
                    class="btn btn-icon-only control-step-btn"
                    disabled={controlDisabled(true) || draft[row.key] >= row.max}
                    aria-label="+"
                    title={controlHint(row.tip)}
                    onclick={() => adjustDraftNumber(row.key, 1, row.step, row.min, row.max)}>+</button>
                </div>
              </div>
            {/each}
          </section>

          <section class="control-section control-password-section">
            <h4>{msg("control.password.title")}</h4>
            <p class="control-hint">{msg("control.password.hint")}</p>
            {#if passwordSuccess}
              <p class="control-hint success">{passwordSuccess}</p>
            {/if}
            {#if passwordError}
              <p class="control-hint error">{passwordError}</p>
            {/if}
            <div class="control-password-fields">
              <label class="password-field">
                <span>{msg("control.password.new")}</span>
                <input
                  type="password"
                  bind:value={newPasswordInput}
                  disabled={modalBusy}
                  autocomplete="new-password"
                  onkeydown={(e) => {
                    if (e.key === "Enter") void submitPasswordChange();
                  }}
                />
              </label>
              <label class="password-field">
                <span>{msg("control.password.confirm")}</span>
                <input
                  type="password"
                  bind:value={confirmPasswordInput}
                  disabled={modalBusy}
                  autocomplete="new-password"
                  onkeydown={(e) => {
                    if (e.key === "Enter") void submitPasswordChange();
                  }}
                />
              </label>
            </div>
            <div class="control-actions">
              <button
                type="button"
                class="btn primary btn-with-spinner"
                disabled={modalBusy || !newPasswordInput.trim() || !confirmPasswordInput.trim()}
                onclick={() => void submitPasswordChange()}
              >
                {#if passwordChanging}
                  <span class="btn-spinner" aria-hidden="true"></span>
                {/if}
                {passwordChanging ? msg("control.password.changing") : msg("control.password.submit")}
              </button>
              <button type="button" class="btn" disabled={modalBusy} onclick={() => void openWebUi()}>
                {msg("control.password.openWeb")}
              </button>
            </div>
          </section>

          <section class="control-section">
            <h4>{msg("control.section.actions")}</h4>
            <div class="control-actions">
              <button type="button" class="btn control-hint-label" disabled={modalBusy} title={msg("control.tip.reboot")} onclick={confirmReboot}>
                {msg("control.reboot")}
              </button>
              <button
                type="button"
                class="btn danger control-hint-label"
                class:api-blocked={apiSwitchOff}
                disabled={controlDisabled(true)}
                title={controlHint("control.tip.restore")}
                onclick={confirmRestore}
              >
                {markApiWrite(msg("control.restore"))}
              </button>
              <button
                type="button"
                class="btn control-hint-label"
                class:api-blocked={apiSwitchOff}
                disabled={controlDisabled(true)}
                title={controlHint("control.tip.exportLog")}
                onclick={() => void exportLog()}
              >
                {markApiWrite(msg("control.exportLog"))}
              </button>
            </div>
          </section>

          <p class="control-legend">{msg("control.legend.apiWrite")}</p>
        {:else if loading}
          <p class="control-hint">{msg("control.loading")}</p>
        {/if}
      </div>

      <div class="control-footer-wrap">
        <footer class="modal-footer control-footer">
          {#if hasPendingChanges}
            <button type="button" class="btn" disabled={modalBusy} onclick={discardDraft}>
              {msg("control.discard")}
            </button>
            <button
              type="button"
              class="btn primary btn-with-spinner control-apply-btn"
              disabled={modalBusy}
              onclick={() => void commitDraft()}
            >
              {#if applying}
                <span class="btn-spinner" aria-hidden="true"></span>
              {/if}
              {msg("control.apply")}
            </button>
          {/if}
          <button type="button" class="btn" disabled={modalBusy} onclick={() => void refresh()}>
            {msg("control.refresh")}
          </button>
          <button type="button" class="btn" disabled={modalBusy} onclick={close}>
            {msg("control.close")}
          </button>
        </footer>
        {#if hasPendingChanges}
          <p class="control-pending-hint" role="status">{msg("control.pendingChanges")}</p>
        {/if}
      </div>
    </ManagedModalCard>
  </div>
{/if}

<style>
  .control-modal-backdrop {
    touch-action: none;
  }

  .control-modal-head-text {
    min-width: 0;
    flex: 1;
  }

  .control-modal-head .modal-close {
    cursor: pointer;
  }

  .control-body {
    flex: 1 1 auto;
    min-height: 0;
    overflow-y: auto;
    overflow-x: hidden;
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

  .control-footer-wrap {
    flex-shrink: 0;
    border-top: 1px solid var(--border);
    background: var(--bg-elevated);
  }

  .control-pending-hint {
    margin: 0;
    padding: 0 14px 12px;
    font-size: 0.82rem;
    line-height: 1.4;
    color: var(--accent);
    text-align: center;
  }

  .control-hint.error {
    color: var(--danger);
  }

  .control-hint.success {
    color: var(--accent);
  }

  .control-password-fields {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(12rem, 1fr));
    gap: 0.75rem;
    margin-bottom: 0.5rem;
  }

  .control-password-section .password-field {
    margin-bottom: 0;
  }

  .control-hint-label[title] {
    cursor: help;
  }

  .control-legend {
    margin: 0;
    font-size: 0.8rem;
    line-height: 1.45;
    color: var(--text-muted);
  }

  .control-auth-overlay .control-auth-field {
    width: min(100%, 22rem);
    text-align: left;
  }

  .control-auth-title {
    margin: 0;
    font-size: 1rem;
    font-weight: 650;
    color: var(--text-primary);
  }

  .control-auth-overlay {
    z-index: 7;
  }

  .password-field {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    margin-bottom: 0.6rem;
    font-size: 0.88rem;
  }

  .password-field input {
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 0.45rem 0.6rem;
    background: var(--bg-elevated);
    color: var(--text-primary);
  }

  .password-field input:focus {
    outline: none;
    border-color: var(--accent);
  }

  .control-apply-overlay {
    position: absolute;
    inset: 0;
    z-index: 6;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 0.85rem;
    padding: 1.25rem;
    background: color-mix(in srgb, var(--bg-elevated) 90%, transparent);
    backdrop-filter: blur(3px);
    border-radius: inherit;
    text-align: center;
  }

  .control-apply-status {
    margin: 0;
    max-width: 28rem;
    font-size: 0.92rem;
    line-height: 1.45;
    color: var(--text-primary);
  }

  .control-reboot-actions {
    display: flex;
    flex-wrap: wrap;
    justify-content: center;
    gap: 0.5rem;
  }

  .control-section h4 {
    margin: 0 0 0.5rem;
    font-size: 0.82rem;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--text-muted);
  }

  .control-section h4.field-changed {
    color: var(--accent);
  }

  .control-rows {
    display: flex;
    flex-direction: column;
    gap: 0.65rem;
  }

  .control-section.api-blocked .control-segment-group,
  .control-section.api-blocked .control-stepper {
    opacity: 0.78;
  }

  .control-segment:disabled,
  .control-step-btn:disabled,
  .btn.api-blocked:disabled {
    cursor: not-allowed;
  }

  .control-segment-group {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 4px;
    padding: 4px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--bg-base);
  }

  .control-segment {
    min-height: 34px;
    padding: 8px 10px;
    border: none;
    border-radius: calc(var(--radius) - 2px);
    background: transparent;
    color: var(--text-muted);
    font: inherit;
    cursor: pointer;
    transition: background 120ms, color 120ms;
  }

  .control-segment:hover:not(:disabled):not(.active) {
    color: var(--text-primary);
    background: var(--bg-surface);
  }

  .control-segment.active {
    background: var(--accent);
    color: #fff;
    font-weight: 600;
  }

  .control-segment.changed:not(.active) {
    box-shadow: inset 0 0 0 1px var(--accent);
    color: var(--accent);
  }

  .control-segment:disabled {
    opacity: 0.55;
    cursor: not-allowed;
  }

  .control-stepper {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 0.75rem;
    padding: 0.35rem 0;
  }

  .control-stepper.field-changed .control-step-value {
    color: var(--accent);
  }

  .control-stepper-actions {
    display: inline-flex;
    align-items: center;
    gap: 0.5rem;
  }

  .control-step-btn {
    width: 30px;
    min-width: 30px;
    height: 30px;
    padding: 0;
  }

  .control-step-value {
    min-width: 3rem;
    text-align: center;
    font-variant-numeric: tabular-nums;
  }

  .control-actions {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
  }

  .control-footer {
    display: flex;
    justify-content: flex-end;
    flex-wrap: wrap;
    gap: 0.5rem;
    padding: 10px 14px;
  }

  .control-apply-btn {
    margin-right: auto;
  }
</style>
