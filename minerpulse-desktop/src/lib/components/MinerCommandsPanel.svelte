<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { ask } from "@tauri-apps/plugin-dialog";
  import { t, type Locale, type MessageKey } from "$lib/i18n";
  import { formatFreqDomains, parseFreqDomains } from "$lib/avalonCommands";
  import type { MinerSnapshot } from "$lib/types";

  let {
    snapshot,
    ip,
    port,
    locale,
    onStatus,
  }: {
    snapshot: MinerSnapshot;
    ip: string;
    port: string;
    locale: Locale;
    onStatus: (text: string) => void;
  } = $props();

  let busy = $state(false);
  let targetTemp = $state("65");
  let voltageLevel = $state("");
  let workMode = $state("1");
  let freqDomains = $state("496:516:536:556");
  let freqBoard = $state("0");
  let freqFlags = $state("0");
  let customParameter = $state("");

  function msg(key: MessageKey, args?: Record<string, string | number>) {
    return t(locale, key, args);
  }

  function isAvalon() {
    return snapshot.identity.vendor === "avalon";
  }

  function initFromSnapshot(current: MinerSnapshot) {
    if (current.work_mode != null) {
      workMode = String(current.work_mode);
    }
    const board = current.boards[0];
    if (board?.voltage_level != null) {
      voltageLevel = String(board.voltage_level);
    }
    if (board?.freq_domains_mhz?.length === 4) {
      freqDomains = formatFreqDomains(board.freq_domains_mhz);
    }
  }

  $effect(() => {
    initFromSnapshot(snapshot);
  });

  async function sendParameter(parameter: string, successKey: MessageKey) {
    if (busy) return;
    busy = true;
    onStatus(msg("commands.sending"));
    try {
      const result = await invoke<{ response: string }>("send_miner_command", {
        request: {
          ip,
          port: Number.parseInt(port, 10) || 4028,
          driver_id: snapshot.identity.driver_id,
          parameter,
        },
      });
      const preview = result.response.trim().slice(0, 120);
      onStatus(msg(successKey, { preview: preview || "OK" }));
    } catch (err) {
      onStatus(String(err));
    } finally {
      busy = false;
    }
  }

  async function rebootMiner() {
    const confirmed = await ask(msg("commands.rebootConfirm"), {
      title: msg("commands.reboot"),
      kind: "warning",
    });
    if (!confirmed) return;
    await sendParameter("0,reboot,0", "commands.rebootSent");
  }

  async function applyWorkMode() {
    await sendParameter(`0,workmode,${workMode}`, "commands.workModeSent");
  }

  async function applyTargetTemp() {
    const temp = Number.parseInt(targetTemp, 10);
    if (Number.isNaN(temp) || temp < 40 || temp > 95) {
      onStatus(msg("commands.invalidTemp"));
      return;
    }
    await sendParameter(`0,target-temp,${temp}`, "commands.targetTempSent");
  }

  async function applyVoltageLevel() {
    const level = Number.parseInt(voltageLevel, 10);
    if (Number.isNaN(level) || level < 0 || level > 100) {
      onStatus(msg("commands.invalidVoltage"));
      return;
    }
    await sendParameter(`0,voltage-level,${level}`, "commands.voltageSent");
  }

  async function applyFrequency() {
    const freqs = parseFreqDomains(freqDomains);
    if (!freqs) {
      onStatus(msg("commands.invalidFreq"));
      return;
    }
    const board = Number.parseInt(freqBoard, 10);
    const flags = Number.parseInt(freqFlags, 10);
    if (Number.isNaN(board) || board < 0 || board > 3) {
      onStatus(msg("commands.invalidBoard"));
      return;
    }
    if (Number.isNaN(flags) || flags < 0 || flags > 16) {
      onStatus(msg("commands.invalidFlags"));
      return;
    }
    await sendParameter(
      `0,frequency,${freqs.join(":")}-0-${board}-${flags}`,
      "commands.frequencySent",
    );
  }

  async function applyCustom() {
    const parameter = customParameter.trim();
    if (!parameter) {
      onStatus(msg("commands.invalidCustom"));
      return;
    }
    await sendParameter(parameter, "commands.customSent");
  }
</script>

{#if !isAvalon()}
  <div class="locked-panel">{msg("commands.unsupportedVendor")}</div>
{:else}
  <div class="commands-panel">
    <section class="commands-section">
      <header class="commands-head">{msg("commands.quick")}</header>
      <div class="commands-actions">
        <button class="btn danger" disabled={busy} onclick={rebootMiner}>
          {msg("commands.reboot")}
        </button>
      </div>
    </section>

    <section class="commands-section">
      <header class="commands-head">{msg("commands.tuning")}</header>
      <div class="commands-grid">
        <label class="commands-field">
          <span>{msg("commands.workMode")}</span>
          <div class="commands-inline">
            <select class="btn" bind:value={workMode} disabled={busy}>
              <option value="0">{msg("commands.workModeNormal")}</option>
              <option value="1">{msg("commands.workModeHigh")}</option>
            </select>
            <button class="btn" disabled={busy} onclick={applyWorkMode}>
              {msg("commands.apply")}
            </button>
          </div>
        </label>

        <label class="commands-field">
          <span>{msg("commands.targetTemp")}</span>
          <div class="commands-inline">
            <input bind:value={targetTemp} disabled={busy} />
            <button class="btn" disabled={busy} onclick={applyTargetTemp}>
              {msg("commands.apply")}
            </button>
          </div>
        </label>

        <label class="commands-field">
          <span>{msg("commands.voltageLevel")}</span>
          <div class="commands-inline">
            <input bind:value={voltageLevel} disabled={busy} placeholder="48" />
            <button class="btn" disabled={busy} onclick={applyVoltageLevel}>
              {msg("commands.apply")}
            </button>
          </div>
        </label>
      </div>
    </section>

    <section class="commands-section">
      <header class="commands-head">{msg("commands.frequency")}</header>
      <div class="commands-grid">
        <label class="commands-field span-full">
          <span>{msg("commands.freqDomains")}</span>
          <input bind:value={freqDomains} disabled={busy} placeholder="496:516:536:556" />
        </label>
        <label class="commands-field">
          <span>{msg("commands.freqBoard")}</span>
          <input bind:value={freqBoard} disabled={busy} />
        </label>
        <label class="commands-field">
          <span>{msg("commands.freqFlags")}</span>
          <input bind:value={freqFlags} disabled={busy} />
        </label>
        <div class="commands-field">
          <span>&nbsp;</span>
          <button class="btn primary" disabled={busy} onclick={applyFrequency}>
            {msg("commands.applyFrequency")}
          </button>
        </div>
      </div>
    </section>

    <section class="commands-section">
      <header class="commands-head">{msg("commands.advanced")}</header>
      <label class="commands-field span-full">
        <span>{msg("commands.customParameter")}</span>
        <input
          bind:value={customParameter}
          disabled={busy}
          placeholder="0,aging-parameter,60-600:620:640:660"
        />
      </label>
      <button class="btn" disabled={busy} onclick={applyCustom}>
        {msg("commands.sendCustom")}
      </button>
    </section>
  </div>
{/if}
