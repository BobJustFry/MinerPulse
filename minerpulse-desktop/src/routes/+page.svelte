<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { save, open } from "@tauri-apps/plugin-dialog";
  import { onMount } from "svelte";
  import AboutModal from "$lib/components/AboutModal.svelte";
  import SubscriptionModal from "$lib/components/SubscriptionModal.svelte";
  import UpdateAvailableNotice from "$lib/components/UpdateAvailableNotice.svelte";
  import UpdateProgressModal from "$lib/components/UpdateProgressModal.svelte";
  import WhatsminerSetupModal from "$lib/components/WhatsminerSetupModal.svelte";
  import WindowCaption from "$lib/components/WindowCaption.svelte";
  import ToolbarBtn from "$lib/components/ToolbarBtn.svelte";
  import ToolbarIcon, { type ToolbarIconName } from "$lib/components/ToolbarIcon.svelte";
  import { locales, t, type Locale, type MessageKey } from "$lib/i18n";
  import { openScanWindow } from "$lib/openScanWindow";
  import MinerChipMatrix from "$lib/components/MinerChipMatrix.svelte";
  import MinerDataPanel from "$lib/components/MinerDataPanel.svelte";
  import MinerCommandsPanel from "$lib/components/MinerCommandsPanel.svelte";
  import MinerChartsPanel from "$lib/components/MinerChartsPanel.svelte";
  import {
    appendChartPoint,
    downsampleChartPoints,
    pointFromSnapshot,
    type ChartPoint,
  } from "$lib/chartHistory";
  import {
    type ParseImportResponse,
  } from "$lib/importFile";
  import {
    chartPointsFromStored,
    closeOpenedSession,
    defaultSessionSaveName,
    defaultSnapshotSaveName,
    getSessionFrame,
    MINER_FILE_EXTENSIONS,
    openMinerFile,
    type OpenMinerFileResponse,
  } from "$lib/openFile";
  import { setupFileDrop } from "$lib/setupFileDrop";
  import {
    DEFAULT_POLL_RATE_HZ,
    getPollStatus,
    isPollRateHz,
    POLL_RATES_HZ,
    startPoll,
    stopPoll,
    type PollFinishedEvent,
    type PollRateHz,
    type PollSnapshotEvent,
  } from "$lib/pollSession";
  import { SessionPlayer, type PlaybackSpeed } from "$lib/sessionPlayer";
  import SessionPlayerBar from "$lib/components/SessionPlayerBar.svelte";
  import { formatAppError } from "$lib/formatAppError";
  import { driverLabel, statusMessageKey } from "$lib/formatMiner";
  import { invokeWithTimeout, MINER_READ_TIMEOUT_MS, WHATSMINER_AUTH_TEST_TIMEOUT_MS } from "$lib/minerInvoke";
  import { isSnapshotEmpty } from "$lib/snapshotUtils";
  import { checkForAppUpdate, UPDATE_CHECK_INTERVAL_MS } from "$lib/updateCheck";
  import type { ChartsLayout } from "$lib/types";
  import type {
    Entitlements,
    LicenseInfo,
    MinerSnapshot,
    SubscriptionTier,
    TabId,
    Theme,
    Density,
    ErrorResponse,
    WhatsminerAccessInfo,
  } from "$lib/types";

  let theme = $state<Theme>("light");
  let density = $state<Density>("comfortable");
  let locale = $state<Locale>("ru");
  let activeTab = $state<TabId>("data");
  let ip = $state("192.168.0.100");
  let port = $state("4028");
  let whatsminerUser = $state("");
  let whatsminerPassword = $state("");
  let whatsminerCustomAuth = $state(false);
  let whatsminerAuthOpen = $state(false);
  let whatsminerAuthError = $state("");
  let whatsminerAuthBusy = $state(false);
  let whatsminerEnableBusy = $state(false);
  let whatsminerTestBusy = $state(false);
  let whatsminerTestOk = $state<boolean | null>(null);
  let whatsminerSetupAccess = $state<WhatsminerAccessInfo | null>(null);
  let whatsminerAuthSession = $state(0);
  let whatsminerAuthDismissedIp = $state("");
  let readGeneration = 0;
  let connectionIp = $state("");
  let licenseInfo = $state<LicenseInfo>({ tier: "free", licensed: false, signed_in: false, hwid: "" });
  let pendingAuthRetry = $state<(() => Promise<void>) | null>(null);
  let busy = $state(false);
  let reading = $state(false);
  let statusText = $state("");
  let snapshot = $state<MinerSnapshot | null>(null);
  let snapshotSourceIp = $state("");
  let appVersion = $state("Miner Pulse 1.0.1 (26)");
  let appVersionNumber = $state("1.0.1");
  let appBuild = $state(26);
  let appProduct = $state("Miner Pulse");
  let aboutOpen = $state(false);
  let subscriptionOpen = $state(false);
  let updateAvailable = $state(false);
  let updateVersionLabel = $state("");
  let updateNoticeDismissed = $state(false);
  let updateProgressOpen = $state(false);
  let updateCheckInFlight = false;
  let connectionLoaded = $state(false);
  let dropActive = $state(false);
  let polling = $state(false);
  let recording = $state(false);
  let connectionLocked = $state(false);
  let pollFrame = $state(0);
  let sessionLoaded = $state(false);
  let sessionFileName = $state("");
  let playbackActive = $state(false);
  let playerSpeed = $state<PlaybackSpeed>(1);
  let playerFrameIndex = $state(0);
  let playerFrameTotal = $state(0);
  let playerCurrentMs = $state(0);
  let playerDurationMs = $state(0);
  let sessionPlayer: SessionPlayer | null = null;
  let chartPoints = $state<ChartPoint[]>([]);
  let chartCursorMs = $state<number | null>(null);
  let chartsLive = $state(false);
  let chartsLayout = $state<ChartsLayout>("tile");
  let pollRateHz = $state<PollRateHz>(DEFAULT_POLL_RATE_HZ);
  type ReadActionMode = "read" | "poll" | "record";
  let readActionMode = $state<ReadActionMode>("read");
  let actionMenuOpen = $state(false);
  let entitlements = $state<Entitlements>({
    tier: "free",
    can_poll: false,
    can_record_session: false,
    can_play: false,
    can_show_charts: false,
    can_save_snapshot: true,
    min_read_interval_sec: 10,
  });

  const tabs: TabId[] = ["data", "chips", "console", "charts", "commands"];
  const CONNECTION_KEY = "minerpulse.connection";

  const chartsUnlocked = $derived(
    entitlements.can_show_charts || polling || recording || sessionLoaded,
  );
  // Sticky per-miner chip availability: hide the Chips tab when a miner has no
  // chip data, but never flicker during poll/record (empty ticks keep the latch).
  let chipsAvailable = $state(false);
  let chipsMinerKey = "";
  $effect(() => {
    const data = snapshot;
    if (!data || data.identity.vendor === "unknown") {
      chipsAvailable = false;
      chipsMinerKey = "";
      return;
    }
    const key = data.identity.mac || data.identity.model || data.identity.vendor;
    const hasChips = (data.board_chips?.length ?? 0) > 0;
    if (key !== chipsMinerKey) {
      chipsMinerKey = key;
      chipsAvailable = hasChips;
    } else if (hasChips) {
      chipsAvailable = true;
    }
  });

  const visibleTabs = $derived.by((): TabId[] =>
    tabs.filter((tab) => {
      if (tab === "charts") return chartsUnlocked;
      if (tab === "commands") return entitlements.can_poll;
      if (tab === "chips") return chipsAvailable;
      return true;
    }),
  );
  const chartsLiveNow = $derived(chartsLive || polling || recording);
  const readActionDisabled = $derived(reading || polling || connectionLocked);
  const readActionLabel = $derived(
    readActionMode === "read"
      ? msg("toolbar.read")
      : readActionMode === "poll"
        ? msg("toolbar.poll")
        : msg("toolbar.record"),
  );
  const readActionIcon = $derived.by((): ToolbarIconName => {
    if (readActionMode === "poll") return "poll";
    if (readActionMode === "record") return "record";
    return "read";
  });
  const compactToolbar = $derived(density === "compact");
  function readActionModeOptions(): Array<{ id: ReadActionMode; label: string }> {
    const options: Array<{ id: ReadActionMode; label: string }> = [
      { id: "read", label: msg("toolbar.read") },
      { id: "poll", label: msg("toolbar.poll") },
    ];
    if (entitlements.can_record_session) {
      options.push({ id: "record", label: msg("toolbar.record") });
    }
    return options;
  }

  function toggleActionMenu() {
    if (connectionLocked) return;
    actionMenuOpen = !actionMenuOpen;
  }

  function msg(key: MessageKey, args?: Record<string, string | number>) {
    return t(locale, key, args);
  }

  function applyUiPrefs() {
    document.documentElement.dataset.theme = theme;
    document.documentElement.dataset.density = density;
    document.documentElement.lang = locale === "zh-CN" ? "zh-CN" : locale;
    localStorage.setItem(
      "minerpulse.ui",
      JSON.stringify({
        theme,
        density,
        locale,
        chartsLayout,
        pollRateHz,
        readActionMode,
      }),
    );
  }

  function bumpReadGeneration(): number {
    readGeneration += 1;
    return readGeneration;
  }

  function invalidateRead() {
    bumpReadGeneration();
    void invoke("cancel_read_miner");
  }

  function isStaleRead(gen: number) {
    return gen !== readGeneration;
  }

  function isOperationCancelled(err: unknown) {
    return (err as { code?: string })?.code === "OPERATION_CANCELLED";
  }

  function whatsminerAuthPayload(): { username: string; password: string } | null {
    if (!whatsminerCustomAuth) return null;
    const username = whatsminerUser.trim();
    if (!username) return null;
    return { username, password: whatsminerPassword };
  }

  function isDefaultWhatsminerCredentials(user: string, pass: string) {
    return (user === "admin" && pass === "admin") || (user === "root" && pass === "root");
  }

  function whatsminerNeedsAuth(data: MinerSnapshot | null): boolean {
    if (!data || data.identity.vendor !== "whatsminer") return false;
    if ((data.board_chips?.length ?? 0) > 0) return false;
    if (isSnapshotEmpty(data)) {
      return data.whatsminer_access?.needs_setup === true;
    }
    // Telemetry without chips — always offer LuCI auth.
    return true;
  }

  function maybePromptWhatsminerAuth(
    nextSnapshot: MinerSnapshot,
    retry?: () => Promise<void>,
  ) {
    if (!whatsminerNeedsAuth(nextSnapshot)) return;
    if (whatsminerAuthBusy || whatsminerEnableBusy || whatsminerTestBusy) return;
    promptWhatsminerSetup(nextSnapshot, retry);
  }

  function dismissWhatsminerSetup() {
    whatsminerAuthDismissedIp = ip.trim();
    whatsminerAuthOpen = false;
    whatsminerTestOk = null;
    whatsminerAuthError = "";
    // Cancel of the auth window: drop the partial (chip-less) snapshot and reset status.
    if (snapshot && whatsminerNeedsAuth(snapshot)) {
      snapshot = null;
      snapshotSourceIp = "";
      clearCharts();
      statusText = msg("status.ready");
    }
  }

  function promptWhatsminerSetup(nextSnapshot?: MinerSnapshot, retry?: () => Promise<void>) {
    pendingAuthRetry = retry ?? null;
    whatsminerAuthError = "";
    whatsminerTestOk = null;
    whatsminerSetupAccess = nextSnapshot?.whatsminer_access ?? null;
    if (!whatsminerUser.trim()) {
      whatsminerUser = "admin";
      whatsminerPassword = "admin";
    }
    whatsminerAuthSession += 1;
    whatsminerAuthOpen = true;
  }

  function loadConnection() {
    const saved = localStorage.getItem(CONNECTION_KEY);
    if (!saved) return;
    try {
      const parsed = JSON.parse(saved) as {
        ip?: string;
        port?: string | number;
        whatsminerUser?: string;
        whatsminerPassword?: string;
      };
      ip = parsed.ip ?? ip;
      port = String(parsed.port ?? port);
      const savedUser = parsed.whatsminerUser?.trim() ?? "";
      const savedPass = parsed.whatsminerPassword ?? "";
      if (savedUser && !isDefaultWhatsminerCredentials(savedUser, savedPass)) {
        whatsminerUser = savedUser;
        whatsminerPassword = savedPass;
        whatsminerCustomAuth = true;
      }
    } catch {
      /* ignore */
    }
  }

  function saveConnection() {
    const payload: {
      ip: string;
      port: string;
      whatsminerUser?: string;
      whatsminerPassword?: string;
    } = { ip, port: String(port) };
    if (whatsminerCustomAuth) {
      payload.whatsminerUser = whatsminerUser.trim();
      payload.whatsminerPassword = whatsminerPassword;
    }
    localStorage.setItem(CONNECTION_KEY, JSON.stringify(payload));
  }

  function tabLabel(tab: TabId) {
    const map: Record<TabId, MessageKey> = {
      data: "tabs.data",
      chips: "tabs.chips",
      console: "tabs.console",
      charts: "tabs.charts",
      commands: "tabs.commands",
    };
    return msg(map[tab]);
  }

  function tierLabel(tier: SubscriptionTier) {
    const map: Record<SubscriptionTier, MessageKey> = {
      free: "tier.free",
      client: "tier.client",
      service: "tier.service",
    };
    return msg(map[tier]);
  }

  function formatError(err: unknown): string {
    return formatAppError(locale, err, { minReadIntervalSec: entitlements.min_read_interval_sec });
  }

  function minerStatusLabel(status: string): string {
    const key = statusMessageKey(status);
    return key ? msg(key as MessageKey) : status;
  }

  function snapshotStatusText(data: MinerSnapshot): string {
    const driver = driverLabel(data.identity.driver_id);
    return `${data.identity.model} · ${minerStatusLabel(data.status)} · ${msg("status.driver", { driver })}`;
  }

  async function refreshEntitlements() {
    entitlements = await invoke<Entitlements>("get_entitlements");
  }

  async function openScan() {
    if (connectionLocked) return;
    statusText = msg("scan.opening");
    try {
      await openScanWindow();
      statusText = msg("status.ready");
    } catch (err) {
      statusText = formatError(err);
    }
  }

  function clearCharts() {
    chartPoints = [];
    chartCursorMs = null;
    chartsLive = false;
  }

  function pushChartPoint(nextSnapshot: MinerSnapshot, t_ms: number) {
    chartPoints = appendChartPoint(
      chartPoints,
      pointFromSnapshot(nextSnapshot, t_ms),
    );
  }

  async function readMiner() {
    if (polling) return;

    const targetIp = ip.trim();
    const previousMac = snapshot?.identity.mac ?? null;
    const previousVendor = snapshot?.identity.vendor ?? null;

    const gen = bumpReadGeneration();
    const targetPort = Number(port) || 4028;
    reading = true;
    connectionLocked = true;
    dropActive = false;
    statusText = msg("status.reading");

    try {
      const response = await invoke<{ snapshot: MinerSnapshot }>("read_miner", {
        request: {
          ip: targetIp,
          port: targetPort,
          whatsminer_auth: whatsminerAuthPayload(),
        },
      });

      if (isStaleRead(gen)) return;

      if (isSnapshotEmpty(response.snapshot)) {
        if (whatsminerNeedsAuth(response.snapshot)) {
          snapshot = response.snapshot;
          snapshotSourceIp = targetIp;
          maybePromptWhatsminerAuth(response.snapshot);
        }
        statusText = msg("status.readEmpty");
        return;
      }

      // Same IP now hosts a different miner (MAC/vendor changed) — drop stale auth state.
      const nextMac = response.snapshot.identity.mac ?? null;
      const nextVendor = response.snapshot.identity.vendor ?? null;
      const minerChanged =
        (previousMac != null && nextMac != null && previousMac !== nextMac) ||
        (previousVendor != null && nextVendor != null && previousVendor !== nextVendor);
      if (minerChanged) {
        whatsminerAuthDismissedIp = "";
        whatsminerAuthOpen = false;
      }

      snapshot = response.snapshot;
      snapshotSourceIp = targetIp;
      clearCharts();
      pushChartPoint(response.snapshot, 0);
      statusText = snapshotStatusText(response.snapshot);

      if ((response.snapshot.board_chips?.length ?? 0) > 0) {
        whatsminerAuthDismissedIp = "";
      }

      maybePromptWhatsminerAuth(response.snapshot);
    } catch (err) {
      if (isStaleRead(gen) || isOperationCancelled(err)) return;
      statusText = formatError(err);
    } finally {
      if (!isStaleRead(gen)) {
        reading = false;
        connectionLocked = false;
      }
    }
  }

  function cancelReading() {
    if (!reading) return;
    invalidateRead();
    dropActive = false;
    reading = false;
    connectionLocked = false;
    statusText = msg("status.readCancelled");
  }

  async function enableWhatsminerApi() {
    const username = whatsminerUser.trim();
    if (!username) return;
    whatsminerEnableBusy = true;
    whatsminerAuthError = "";
    try {
      const response = await invokeWithTimeout<{ enabled: boolean; access: WhatsminerAccessInfo }>(
        "enable_whatsminer_api",
        {
          request: { ip, username, password: whatsminerPassword },
        },
        WHATSMINER_AUTH_TEST_TIMEOUT_MS,
      );
      whatsminerSetupAccess = response.access;
      if (!response.enabled) {
        whatsminerAuthError = msg("auth.statusApiOff");
      }
    } catch (err) {
      whatsminerAuthError = formatError(err);
    } finally {
      whatsminerEnableBusy = false;
    }
  }

  async function testWhatsminerLogin() {
    const username = whatsminerUser.trim();
    if (!username) return;
    whatsminerTestBusy = true;
    whatsminerAuthError = "";
    try {
      const response = await invokeWithTimeout<{ ok: boolean; mac: string | null }>(
        "test_whatsminer_credentials",
        {
          request: { ip, username, password: whatsminerPassword },
        },
        WHATSMINER_AUTH_TEST_TIMEOUT_MS,
      );
      whatsminerTestOk = response.ok;
      if (response.mac && whatsminerSetupAccess) {
        whatsminerSetupAccess = { ...whatsminerSetupAccess, mac: response.mac };
      } else if (response.mac) {
        whatsminerSetupAccess = {
          mac: response.mac,
          api_switch: null,
          luci_reachable: response.ok,
          luci_auth_ok: response.ok,
          api_reachable: false,
          api_auth_ok: false,
          needs_setup: !response.ok,
        };
      }
      if (!response.ok) {
        whatsminerAuthError = msg("auth.testFail");
      }
    } catch (err) {
      whatsminerTestOk = false;
      whatsminerAuthError = formatError(err);
    } finally {
      whatsminerTestBusy = false;
    }
  }

  async function saveAndReadWhatsminer() {
    const username = whatsminerUser.trim();
    if (!username) return;
    whatsminerAuthBusy = true;
    whatsminerAuthError = "";
    try {
      if (whatsminerTestOk !== true) {
        await testWhatsminerLogin();
        if (!whatsminerTestOk) {
          whatsminerAuthError = msg("auth.invalid");
          return;
        }
      }

      const mac =
        whatsminerSetupAccess?.mac ??
        (
          await invokeWithTimeout<{ ok: boolean; mac: string | null }>(
            "test_whatsminer_credentials",
            {
              request: { ip, username, password: whatsminerPassword },
            },
            WHATSMINER_AUTH_TEST_TIMEOUT_MS,
          )
        ).mac;

      if (mac && !isDefaultWhatsminerCredentials(username, whatsminerPassword)) {
        try {
          await invoke("save_miner_credential", {
            mac,
            username,
            password: whatsminerPassword,
            ip,
          });
        } catch {
          /* local save is best-effort */
        }
      }

      const response = await invokeWithTimeout<{ snapshot: MinerSnapshot }>(
        "read_miner",
        {
          request: {
            ip,
            port: Number(port) || 4028,
            whatsminer_auth: { username, password: whatsminerPassword },
          },
        },
        MINER_READ_TIMEOUT_MS,
      );

      if ((response.snapshot.board_chips?.length ?? 0) === 0 && !isSnapshotEmpty(response.snapshot)) {
        whatsminerAuthError = msg("auth.invalid");
        whatsminerSetupAccess = response.snapshot.whatsminer_access ?? whatsminerSetupAccess;
        return;
      }

      if (isSnapshotEmpty(response.snapshot) && whatsminerNeedsAuth(response.snapshot)) {
        whatsminerAuthError = msg("auth.invalid");
        whatsminerSetupAccess = response.snapshot.whatsminer_access ?? whatsminerSetupAccess;
        return;
      }

      whatsminerCustomAuth = !isDefaultWhatsminerCredentials(username, whatsminerPassword);
      saveConnection();
      snapshot = response.snapshot;
      clearCharts();
      pushChartPoint(response.snapshot, 0);
      statusText = snapshotStatusText(response.snapshot);
      whatsminerAuthOpen = false;
      whatsminerAuthError = "";
      const retry = pendingAuthRetry;
      pendingAuthRetry = null;
      if (retry) {
        await retry();
      }
    } catch (err) {
      whatsminerAuthError = formatError(err);
    } finally {
      whatsminerAuthBusy = false;
    }
  }

  function syncPlayerState(state: {
    index: number;
    total: number;
    current_ms: number;
    duration_ms: number;
    playing: boolean;
    speed: PlaybackSpeed;
  }) {
    playerFrameIndex = state.index;
    playerFrameTotal = state.total;
    playerCurrentMs = state.current_ms;
    playerDurationMs = state.duration_ms;
    playbackActive = state.playing;
    playerSpeed = state.speed;
  }

  function unloadSessionPlayer() {
    sessionPlayer?.unload();
    playbackActive = false;
    playerFrameIndex = 0;
    playerFrameTotal = 0;
    playerCurrentMs = 0;
    playerDurationMs = 0;
    playerSpeed = 1;
  }

  function stopPlayback() {
    unloadSessionPlayer();
    void closeOpenedSession();
    sessionLoaded = false;
    sessionFileName = "";
    chartPoints = [];
    chartCursorMs = null;
    chartsLive = false;
  }

  function initSessionPlayer() {
    sessionPlayer = new SessionPlayer(
      (frame, index, total) => {
        snapshot = frame.snapshot;
        chartCursorMs = frame.t_ms;
        if (playbackActive) {
          statusText = msg("status.playing", {
            current: index + 1,
            total,
          });
        }
      },
      () => {
        playbackActive = false;
        statusText = msg("status.playbackEnded");
        syncPlayerState(sessionPlayer!.getState());
      },
      (state) => {
        syncPlayerState(state);
      },
    );
  }

  function playerPlay() {
    sessionPlayer?.play(playerFrameIndex);
  }

  function playerPause() {
    sessionPlayer?.pause();
  }

  function playerStop() {
    sessionPlayer?.stop();
    statusText = msg("status.sessionLoaded", { count: playerFrameTotal });
  }

  function playerSeek(progress: number) {
    sessionPlayer?.seekToProgress(progress);
  }

  function playerSetSpeed(speed: PlaybackSpeed) {
    sessionPlayer?.setSpeed(speed);
  }

  async function startPolling(record: boolean) {
    if (polling || connectionLocked) return;
    connectionLocked = true;
    stopPlayback();
    clearCharts();

    let recordPath: string | null = null;
    if (record) {
      if (!entitlements.can_record_session) {
        connectionLocked = false;
        return;
      }
      const path = await save({
        defaultPath: defaultSessionSaveName(),
        filters: [{ name: "MinerPulse", extensions: ["mprs"] }],
      });
      if (!path || Array.isArray(path)) {
        connectionLocked = false;
        return;
      }
      recordPath = path;
    }

    busy = true;
    try {
      await startPoll({
        ip,
        port: Number(port) || 4028,
        recordPath,
        pollRateHz,
        whatsminerAuth: whatsminerAuthPayload() ?? undefined,
      });
      polling = true;
      recording = record;
      pollFrame = 0;
      chartsLive = true;
      statusText = record
        ? msg("status.recording", { frame: 0 })
        : msg("status.polling", { frame: 0 });
    } catch (err) {
      connectionLocked = false;
      statusText = formatError(err);
    } finally {
      busy = false;
    }
  }

  async function stopPolling() {
    if (!polling) return;
    await stopPoll();
  }

  function runReadAction() {
    if (polling || connectionLocked) return;
    if (readActionMode === "read") {
      if (reading) return;
      void readMiner();
      return;
    }
    if (busy) return;
    if (readActionMode === "poll") {
      void startPolling(false);
      return;
    }
    void startPolling(true);
  }

  function setReadActionMode(next: ReadActionMode) {
    if (next === "record" && !entitlements.can_record_session) return;
    readActionMode = next;
    actionMenuOpen = false;
  }

  async function applyOpenedSnapshot(result: {
    snapshot: MinerSnapshot;
    miner_ip?: string | null;
  }) {
    snapshot = result.snapshot;
    await invoke("remember_snapshot", { snapshot: result.snapshot });
    if (result.miner_ip) {
      ip = result.miner_ip;
      saveConnection();
    }
  }

  async function openLoadedSession(payload: Extract<OpenMinerFileResponse, { kind: "session" }>) {
    if (!sessionPlayer) initSessionPlayer();
    sessionPlayer!.loadRemote(
      {
        frame_count: payload.frame_count,
        timeline_ms: payload.timeline_ms,
      },
      getSessionFrame,
    );
    sessionLoaded = true;
    sessionFileName = payload.file_label;
    const firstFrame = await getSessionFrame(0);
    snapshot = firstFrame.snapshot;
    chartPoints = downsampleChartPoints(chartPointsFromStored(payload.chart_points));
    chartCursorMs = firstFrame.t_ms;
    chartsLive = false;
    activeTab = "charts";
    syncPlayerState(sessionPlayer!.getState());
    if (payload.miner_ip) {
      ip = payload.miner_ip;
      saveConnection();
    }
    statusText = msg("status.sessionLoaded", { count: payload.frame_count });
  }

  async function openMinerFileAction() {
    if (busy || polling) return;

    const path = await open({
      title: msg("toolbar.open"),
      multiple: false,
      filters: [
        {
          name: "MinerPulse",
          extensions: [...MINER_FILE_EXTENSIONS],
        },
      ],
    });
    if (!path || Array.isArray(path)) return;

    busy = true;
    statusText = msg("status.loadingSession");
    try {
      stopPlayback();
      const result = await openMinerFile(path);
      if (result.kind === "session") {
        if (!entitlements.can_play) {
          statusText = msg("error.NOT_SUPPORTED");
          await closeOpenedSession();
          return;
        }
        await openLoadedSession(result);
        return;
      }

      clearCharts();
      await applyOpenedSnapshot(result);
      pushChartPoint(result.snapshot, 0);
      statusText =
        result.kind === "log"
          ? msg("import.opened", { name: result.source_label })
          : msg("status.ready");
    } catch (err) {
      statusText = formatError(err);
    } finally {
      busy = false;
    }
  }

  async function saveSnapshot() {
    if (!snapshot) {
      statusText = msg("error.NO_SNAPSHOT");
      return;
    }

    const path = await save({
      defaultPath: defaultSnapshotSaveName(),
      filters: [{ name: "MinerPulse", extensions: ["mpsn"] }],
    });

    if (!path) return;

    try {
      const saved = await invoke<string>("save_snapshot_file", {
        request: { ip, path },
      });
      statusText = msg("status.saved", { path: saved });
    } catch (err) {
      statusText = formatError(err);
    }
  }

  function toggleTheme() {
    theme = theme === "light" ? "dark" : "light";
    applyUiPrefs();
  }

  function toggleDensity() {
    density = density === "comfortable" ? "compact" : "comfortable";
    applyUiPrefs();
  }

  function setLocale(next: Locale) {
    locale = next;
    applyUiPrefs();
    statusText = msg("status.ready");
  }

  function openAbout() {
    aboutOpen = true;
  }

  async function runBackgroundUpdateCheck() {
    if (updateCheckInFlight || updateProgressOpen) return;
    updateCheckInFlight = true;
    try {
      const result = await checkForAppUpdate(appVersionNumber);
      if (result.status === "available") {
        updateAvailable = true;
        updateVersionLabel = result.versionLabel;
        updateNoticeDismissed = false;
      } else if (result.status === "up_to_date") {
        updateAvailable = false;
        updateVersionLabel = "";
      }
    } finally {
      updateCheckInFlight = false;
    }
  }

  function startUpdateInstall() {
    updateProgressOpen = true;
  }

  function dismissUpdateNotice() {
    updateNoticeDismissed = true;
  }

  function openSubscription() {
    subscriptionOpen = true;
  }

  async function cycleTier() {
    const order: SubscriptionTier[] = ["free", "client", "service"];
    const idx = order.indexOf(entitlements.tier);
    const next = order[(idx + 1) % order.length];
    await invoke("set_tier", { tier: next });
    await refreshEntitlements();
  }

  async function applyImportedSnapshot(result: ParseImportResponse) {
    stopPlayback();
    clearCharts();
    await applyOpenedSnapshot(result);
    pushChartPoint(result.snapshot, 0);
    statusText = msg("import.opened", { name: result.source_label });
  }

  onMount(() => {
    let unlistenMiner: (() => void) | undefined;
    let unlistenDrop: (() => void) | undefined;
    let unlistenPollSnapshot: (() => void) | undefined;
    let unlistenPollFinished: (() => void) | undefined;
    let unlistenLicense: (() => void) | undefined;
    let unlistenCredsSync: (() => void) | undefined;
    let updateCheckTimer: ReturnType<typeof setInterval> | undefined;

    initSessionPlayer();

    void (async () => {
      const saved = localStorage.getItem("minerpulse.ui");
      if (saved) {
        try {
          const parsed = JSON.parse(saved);
          theme = parsed.theme ?? theme;
          density = parsed.density ?? density;
          locale = parsed.locale ?? locale;
          chartsLayout = parsed.chartsLayout === "list" ? "list" : "tile";
          if (isPollRateHz(parsed.pollRateHz)) {
            pollRateHz = parsed.pollRateHz;
          }
          if (
            parsed.readActionMode === "read" ||
            parsed.readActionMode === "poll" ||
            parsed.readActionMode === "record"
          ) {
            readActionMode = parsed.readActionMode;
          }
        } catch {
          /* ignore */
        }
      }
      applyUiPrefs();
      loadConnection();
      connectionLoaded = true;

      unlistenLicense = await listen("license://updated", () => {
        void refreshEntitlements();
        void refreshLicenseInfo();
      });

      unlistenCredsSync = await listen<{ ok: boolean; code?: string }>(
        "miner-credentials://sync",
        (event) => {
          if (event.payload?.ok === false && !reading && !polling) {
            statusText = msg("status.syncFailed");
          }
        },
      );

      async function refreshLicenseInfo() {
        try {
          licenseInfo = await invoke<LicenseInfo>("get_license_info");
        } catch {
          licenseInfo = { tier: "free", licensed: false, signed_in: false, hwid: "" };
        }
      }

      await refreshEntitlements();
      await refreshLicenseInfo();
      try {
        const v = await invoke<{ display: string; version: string; build: number; product: string }>(
          "get_app_version",
        );
        appVersion = v.display;
        appVersionNumber = v.version;
        appBuild = v.build;
        appProduct = v.product;
      } catch {
        /* ignore in web preview */
      }
      void runBackgroundUpdateCheck();
      updateCheckTimer = setInterval(() => {
        void runBackgroundUpdateCheck();
      }, UPDATE_CHECK_INTERVAL_MS);
      statusText = msg("status.ready");

      unlistenMiner = await listen<{
        ip: string;
        port: number;
        model: string;
      }>("miner-selected", (event) => {
        if (connectionLocked) return;
        ip = event.payload.ip;
        port = String(event.payload.port);
        statusText = `${event.payload.model} · ${event.payload.ip}`;
        saveConnection();
      });

      unlistenDrop = setupFileDrop({
        onHover: () => {
          dropActive = true;
        },
        onLeave: () => {
          dropActive = false;
        },
        onDrop: async (result) => {
          dropActive = false;
          try {
            await applyImportedSnapshot(result);
          } catch (err) {
            statusText = formatError(err);
          }
        },
        onError: (err) => {
          dropActive = false;
          statusText = formatError(err);
        },
        onTooLarge: () => {
          dropActive = false;
          statusText = msg("import.tooLarge");
        },
      });

      unlistenPollSnapshot = await listen<PollSnapshotEvent>("poll://snapshot", (event) => {
        snapshot = event.payload.snapshot;
        pollFrame = event.payload.frame_index + 1;
        pushChartPoint(event.payload.snapshot, event.payload.t_ms);
        chartCursorMs = event.payload.t_ms;
        statusText = event.payload.recording
          ? msg("status.recording", { frame: pollFrame })
          : msg("status.polling", { frame: pollFrame });
        if (
          event.payload.frame_index === 0 &&
          whatsminerNeedsAuth(event.payload.snapshot)
        ) {
          void (async () => {
            const wasRecording = event.payload.recording;
            await stopPoll();
            maybePromptWhatsminerAuth(event.payload.snapshot, () =>
              startPolling(wasRecording),
            );
          })();
        }
      });

      unlistenPollFinished = await listen<PollFinishedEvent>("poll://finished", (event) => {
        polling = false;
        recording = false;
        connectionLocked = false;
        busy = false;
        chartsLive = false;
        if (event.payload.saved_path) {
          statusText = msg("status.sessionSaved", { path: event.payload.saved_path });
        } else if (event.payload.error) {
          statusText = event.payload.error;
        } else {
          statusText = msg("status.pollStopped", { count: event.payload.frame_count });
        }
      });

      try {
        const pollStatus = await getPollStatus();
        polling = pollStatus.running;
        recording = pollStatus.recording;
        if (pollStatus.running) {
          connectionLocked = true;
          chartsLive = true;
        }
      } catch {
        /* ignore in web preview */
      }
    })();

    return () => {
      unlistenMiner?.();
      unlistenDrop?.();
      unlistenPollSnapshot?.();
      unlistenPollFinished?.();
      unlistenLicense?.();
      unlistenCredsSync?.();
      if (updateCheckTimer) clearInterval(updateCheckTimer);
      sessionPlayer?.pause();
    };
  });

  $effect(() => {
    if (!connectionLoaded) return;
    const nextIp = ip.trim();
    if (connectionIp && connectionIp !== nextIp) {
      invalidateRead();
      reading = false;
      connectionLocked = false;
      whatsminerAuthDismissedIp = "";
      whatsminerAuthOpen = false;
      whatsminerTestOk = null;
      whatsminerAuthError = "";
      snapshot = null;
      snapshotSourceIp = "";
      clearCharts();
      statusText = msg("status.ready");
    }
    connectionIp = nextIp;
  });

  $effect(() => {
    if (!connectionLoaded) return;
    ip;
    port;
    whatsminerUser;
    whatsminerPassword;
    whatsminerCustomAuth;
    saveConnection();
  });

  $effect(() => {
    if (!connectionLoaded) return;
    chartsLayout;
    pollRateHz;
    readActionMode;
    applyUiPrefs();
  });

  $effect(() => {
    if (readActionMode === "record" && !entitlements.can_record_session) {
      readActionMode = "read";
    }
  });

  $effect(() => {
    if (!visibleTabs.includes(activeTab)) {
      activeTab = visibleTabs[0] ?? "data";
    }
  });

  $effect(() => {
    if (!actionMenuOpen) return;
    const onDocumentClick = (event: MouseEvent) => {
      const target = event.target as Element;
      if (actionMenuOpen && !target.closest(".action-control")) {
        actionMenuOpen = false;
      }
    };
    document.addEventListener("click", onDocumentClick);
    return () => document.removeEventListener("click", onDocumentClick);
  });
</script>

<div class="app-shell" class:drop-active={dropActive}>
  <WindowCaption {locale} {theme} product={appProduct} version={appVersionNumber} build={appBuild} />

  {#if dropActive}
    <div class="drop-overlay" aria-hidden="true">
      <div class="drop-overlay-card">
        <div class="drop-overlay-title">{msg("import.dropTitle")}</div>
        <div class="drop-overlay-hint">{msg("import.dropHint")}</div>
      </div>
    </div>
  {/if}
  <header class="toolbar" class:toolbar-compact={compactToolbar}>
    <div class="field host-field">
      <label for="ip">{msg("toolbar.ip")}</label>
      <div class="host-inputs">
        <input id="ip" bind:value={ip} disabled={connectionLocked} />
        <span class="host-sep" aria-hidden="true">:</span>
        <input
          id="port"
          class="port"
          bind:value={port}
          disabled={connectionLocked}
          aria-label={msg("toolbar.port")}
        />
      </div>
      <button
        class="btn"
        class:btn-icon-only={compactToolbar}
        disabled={connectionLocked}
        onclick={openScan}
        title={msg("toolbar.scan")}
        aria-label={compactToolbar ? msg("toolbar.scan") : undefined}
      >
        {#if compactToolbar}
          <ToolbarIcon name="scan" />
        {:else}
          {msg("toolbar.scan")}
        {/if}
      </button>
    </div>

    {#if entitlements.can_poll}
      {#if polling}
        <ToolbarBtn
          class="danger"
          label={msg("toolbar.stop")}
          icon="stop"
          {density}
          disabled={busy}
          onclick={stopPolling}
        />
      {:else if reading}
        <ToolbarBtn
          class="danger"
          label={msg("toolbar.cancel")}
          icon="cancel"
          {density}
          onclick={cancelReading}
        />
      {:else}
        {#if readActionMode !== "read"}
          <div class="field poll-rate-field">
            <label for="poll-rate">{msg("toolbar.pollRate")}</label>
            <select
              id="poll-rate"
              class="btn poll-rate-select"
              bind:value={pollRateHz}
              disabled={connectionLocked}
              title={msg("toolbar.pollRate")}
            >
              {#each POLL_RATES_HZ as rate (rate)}
                <option value={rate}>{msg("toolbar.pollRateOption", { rate })}</option>
              {/each}
            </select>
          </div>
        {/if}
        <div class="action-control" class:open={actionMenuOpen}>
          <div class="split-action">
            <button
              type="button"
              class="split-action-main btn-with-spinner"
              class:btn-icon-only={compactToolbar}
              disabled={(readActionMode === "read" ? readActionDisabled : busy || connectionLocked)}
              title={readActionLabel}
              aria-label={compactToolbar ? readActionLabel : undefined}
              onclick={runReadAction}
            >
              {#if reading && readActionMode === "read"}
                <span class="btn-spinner" aria-hidden="true"></span>
              {/if}
              {#if compactToolbar}
                <ToolbarIcon name={readActionIcon} />
              {:else}
                {readActionLabel}
              {/if}
            </button>
            <button
              type="button"
              class="split-action-toggle"
              disabled={connectionLocked}
              aria-expanded={actionMenuOpen}
              aria-label={msg("toolbar.actionMode")}
              onclick={(event) => {
                event.stopPropagation();
                toggleActionMenu();
              }}
            >
              <span class="split-action-chevron" aria-hidden="true"></span>
            </button>
          </div>
          {#if actionMenuOpen}
            <div class="action-menu" role="menu">
              {#each readActionModeOptions() as mode (mode.id)}
                <button
                  type="button"
                  role="menuitemradio"
                  class="action-menu-item"
                  class:active={readActionMode === mode.id}
                  aria-checked={readActionMode === mode.id}
                  onclick={(event) => {
                    event.stopPropagation();
                    setReadActionMode(mode.id);
                  }}
                >
                  {mode.label}
                </button>
              {/each}
            </div>
          {/if}
        </div>
      {/if}
    {:else}
      {#if reading}
        <ToolbarBtn
          class="danger"
          label={msg("toolbar.cancel")}
          icon="cancel"
          {density}
          onclick={cancelReading}
        />
      {:else}
      <ToolbarBtn
        class="primary btn-with-spinner"
        label={msg("toolbar.read")}
        icon="read"
        {density}
        disabled={readActionDisabled}
        onclick={readMiner}
      >
        {#if reading}
          <span class="btn-spinner" aria-hidden="true"></span>
        {/if}
      </ToolbarBtn>
      {/if}
    {/if}

    <ToolbarBtn
      label={msg("toolbar.open")}
      icon="open"
      {density}
      disabled={busy || polling}
      onclick={openMinerFileAction}
    />

    <ToolbarBtn
      label={msg("toolbar.save")}
      icon="save"
      {density}
      disabled={!snapshot || busy}
      onclick={saveSnapshot}
    />

    <div class="spacer"></div>

    <ToolbarBtn
      class="ghost"
      label={theme === "light" ? msg("toolbar.themeDark") : msg("toolbar.themeLight")}
      icon={theme === "light" ? "theme-dark" : "theme-light"}
      {density}
      onclick={toggleTheme}
    />
    <ToolbarBtn
      class="ghost"
      label={density === "comfortable"
        ? msg("toolbar.densityCompact")
        : msg("toolbar.densityComfortable")}
      icon={density === "comfortable" ? "density-compact" : "density-comfortable"}
      {density}
      onclick={toggleDensity}
    />

    <select
      class="btn"
      value={locale}
      onchange={(e) => setLocale(e.currentTarget.value as Locale)}
    >
      {#each locales as item}
        <option value={item.id}>{item.label}</option>
      {/each}
    </select>

    <button
      class="tier-badge"
      class:service={entitlements.tier === "service"}
      onclick={import.meta.env.DEV ? cycleTier : openSubscription}
      title={import.meta.env.DEV ? "Dev: cycle tier" : msg("subscription.open")}
      type="button"
    >
      {tierLabel(entitlements.tier)}
    </button>

    <ToolbarBtn
      class="ghost"
      label={msg("toolbar.about")}
      icon="about"
      {density}
      onclick={openAbout}
    />
  </header>

  <SessionPlayerBar
    {locale}
    {density}
    visible={sessionLoaded && playerFrameTotal > 0}
    playing={playbackActive}
    speed={playerSpeed}
    frameIndex={playerFrameIndex}
    frameTotal={playerFrameTotal}
    currentMs={playerCurrentMs}
    durationMs={playerDurationMs}
    fileLabel={sessionFileName}
    onPlay={playerPlay}
    onPause={playerPause}
    onStop={playerStop}
    onClose={stopPlayback}
    onSeek={playerSeek}
    onSpeed={playerSetSpeed}
  />

  <nav class="tabs">
    {#each visibleTabs as tab}
      <button
        class="tab"
        class:active={activeTab === tab}
        onclick={() => (activeTab = tab)}
      >
        {tabLabel(tab)}
      </button>
    {/each}
  </nav>

  <main
    class="content"
    class:content-charts-list={activeTab === "charts" &&
      chartsLayout === "list" &&
      chartsUnlocked &&
      chartPoints.length > 0}
  >
    {#if activeTab === "data"}
      {#if snapshot}
        <MinerDataPanel {snapshot} {locale} {density} />
      {:else}
        <div class="locked-panel">{msg("status.noData")}</div>
      {/if}
    {:else if activeTab === "chips"}
      {#if snapshot && (snapshot.board_chips?.length ?? 0) > 0}
        <MinerChipMatrix
          boards={snapshot.board_chips ?? []}
          vendor={snapshot.identity.vendor}
          {locale}
        />
      {:else if snapshot}
        <div class="locked-panel">{msg("chips.empty")}</div>
      {:else}
        <div class="locked-panel">{msg("status.noData")}</div>
      {/if}
    {:else if activeTab === "console"}
      <div class="console-box">{snapshot?.raw_log || msg("status.noData")}</div>
    {:else if activeTab === "charts"}
      <MinerChartsPanel
        points={chartPoints}
        cursorMs={chartCursorMs}
        live={chartsLiveNow || playbackActive}
        bind:layout={chartsLayout}
        {locale}
      />
    {:else if activeTab === "commands"}
      {#if snapshot}
        <MinerCommandsPanel
          {snapshot}
          {ip}
          {port}
          {locale}
          onStatus={(text) => {
            statusText = text;
          }}
        />
      {:else}
        <div class="locked-panel">{msg("status.noData")}</div>
      {/if}
    {/if}
  </main>

  {#if updateAvailable && !updateNoticeDismissed}
    <UpdateAvailableNotice
      {locale}
      versionLabel={updateVersionLabel}
      onInstall={startUpdateInstall}
      onDismiss={dismissUpdateNotice}
    />
  {/if}

  <footer class="statusbar">
    <span class="status-dot" class:busy={busy || polling || playbackActive}></span>
    <span>{statusText || msg("status.ready")}</span>
    <span class="spacer"></span>
    <button
      type="button"
      class="status-version"
      class:status-version-update={updateAvailable}
      onclick={openAbout}
      title={updateAvailable
        ? msg("updateNotice.message", { version: updateVersionLabel })
        : msg("toolbar.about")}
    >
      {appVersion}
    </button>
  </footer>

  <UpdateProgressModal
    bind:open={updateProgressOpen}
    {locale}
    productVersion={appVersionNumber}
  />

  <AboutModal
    bind:open={aboutOpen}
    bind:updateProgressOpen
    {locale}
    version={appVersionNumber}
    build={appBuild}
    product={appProduct}
    signedIn={licenseInfo.signed_in}
    hwid={licenseInfo.hwid}
  />

  <SubscriptionModal
    bind:open={subscriptionOpen}
    {locale}
    onUpdated={(e) => {
      entitlements = e;
    }}
  />

  {#key whatsminerAuthSession}
    <WhatsminerSetupModal
      bind:open={whatsminerAuthOpen}
      {locale}
      {ip}
      access={whatsminerSetupAccess}
      bind:username={whatsminerUser}
      bind:password={whatsminerPassword}
      busy={whatsminerAuthBusy}
      enableBusy={whatsminerEnableBusy}
      testBusy={whatsminerTestBusy}
      testOk={whatsminerTestOk}
      errorText={whatsminerAuthError}
      cloudAvailable={!!licenseInfo.user_email}
      onEnableApi={enableWhatsminerApi}
      onTestLogin={testWhatsminerLogin}
      onSaveAndRead={saveAndReadWhatsminer}
      onDismiss={dismissWhatsminerSetup}
    />
  {/key}
</div>
