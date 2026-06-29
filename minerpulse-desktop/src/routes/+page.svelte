<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { save, open } from "@tauri-apps/plugin-dialog";
  import { onMount } from "svelte";
  import AboutModal from "$lib/components/AboutModal.svelte";
  import SubscriptionModal from "$lib/components/SubscriptionModal.svelte";
  import WhatsminerAuthModal from "$lib/components/WhatsminerAuthModal.svelte";
  import WindowCaption from "$lib/components/WindowCaption.svelte";
  import { locales, t, type Locale, type MessageKey } from "$lib/i18n";
  import { openScanWindow } from "$lib/openScanWindow";
  import MinerChipMatrix from "$lib/components/MinerChipMatrix.svelte";
  import MinerDataPanel from "$lib/components/MinerDataPanel.svelte";
  import MinerCommandsPanel from "$lib/components/MinerCommandsPanel.svelte";
  import MinerPoolsPanel from "$lib/components/MinerPoolsPanel.svelte";
  import MinerChartsPanel from "$lib/components/MinerChartsPanel.svelte";
  import {
    appendChartPoint,
    chartPointsFromFrames,
    pointFromSnapshot,
    type ChartPoint,
  } from "$lib/chartHistory";
  import {
    type ParseImportResponse,
  } from "$lib/importFile";
  import { setupFileDrop } from "$lib/setupFileDrop";
  import {
    DEFAULT_POLL_RATE_HZ,
    getPollStatus,
    loadSessionFile,
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
  import { isSnapshotEmpty } from "$lib/snapshotUtils";
  import type { ChartsLayout } from "$lib/components/MinerChartsPanel";
  import type {
    Entitlements,
    MinerSnapshot,
    SubscriptionTier,
    TabId,
    Theme,
    Density,
    ErrorResponse,
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
  let whatsminerAuthPrompted = $state(false);
  let pendingAuthRetry = $state<(() => Promise<void>) | null>(null);
  let busy = $state(false);
  let reading = $state(false);
  let readCooldownSec = $state(0);
  let readCooldownTimer: ReturnType<typeof setInterval> | null = null;
  let statusText = $state("");
  let snapshot = $state<MinerSnapshot | null>(null);
  let appVersion = $state("Miner Pulse 1.0.1 (26)");
  let appVersionNumber = $state("1.0.1");
  let appBuild = $state(26);
  let appProduct = $state("Miner Pulse");
  let aboutOpen = $state(false);
  let subscriptionOpen = $state(false);
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
  type OpenFileMode = "session" | "log";
  let openFileMode = $state<OpenFileMode>("log");
  let openFileMenuOpen = $state(false);
  let entitlements = $state<Entitlements>({
    tier: "free",
    can_poll: false,
    can_record_session: false,
    can_play: false,
    can_show_charts: false,
    can_save_snapshot: true,
    min_read_interval_sec: 10,
  });

  const tabs: TabId[] = ["data", "chips", "console", "pools", "charts", "commands"];
  const CONNECTION_KEY = "minerpulse.connection";

  const chartsUnlocked = $derived(
    entitlements.can_show_charts || polling || recording || sessionLoaded,
  );
  const visibleTabs = $derived.by((): TabId[] =>
    tabs.filter((tab) => {
      if (tab === "charts") return chartsUnlocked;
      if (tab === "commands") return entitlements.can_poll;
      return true;
    }),
  );
  const chartsLiveNow = $derived(chartsLive || polling || recording);
  const readCooldownActive = $derived(
    !entitlements.can_poll && readCooldownSec > 0,
  );
  const readActionDisabled = $derived(
    reading || polling || connectionLocked || readCooldownActive,
  );
  const readActionLabel = $derived(
    readActionMode === "read"
      ? msg("toolbar.read")
      : readActionMode === "poll"
        ? msg("toolbar.poll")
        : msg("toolbar.record"),
  );
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

  const openFileLabel = $derived(
    openFileMode === "session" ? msg("toolbar.openSession") : msg("toolbar.openLog"),
  );

  function openFileModeOptions(): Array<{ id: OpenFileMode; label: string }> {
    const options: Array<{ id: OpenFileMode; label: string }> = [
      { id: "log", label: msg("toolbar.openLog") },
    ];
    if (entitlements.can_play) {
      options.unshift({ id: "session", label: msg("toolbar.openSession") });
    }
    return options;
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
        openFileMode,
      }),
    );
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

  function shouldPromptWhatsminerAuth(nextSnapshot: MinerSnapshot) {
    return (
      nextSnapshot.identity.vendor === "whatsminer" &&
      (nextSnapshot.board_chips?.length ?? 0) === 0 &&
      !whatsminerAuthPrompted
    );
  }

  function promptWhatsminerAuth(retry?: () => Promise<void>) {
    whatsminerAuthPrompted = true;
    pendingAuthRetry = retry ?? null;
    whatsminerAuthError = "";
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
      pools: "tabs.pools",
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
    const e = err as ErrorResponse;
    if (e?.code) {
      const key = `error.${e.code}` as MessageKey;
      return msg(key, { sec: e.args?.sec ?? entitlements.min_read_interval_sec });
    }
    return String(err);
  }

  function snapshotStatusText(data: MinerSnapshot): string {
    return `${data.identity.model} · ${data.status}`;
  }

  function startReadCooldown(sec?: number) {
    const duration = Math.max(1, sec ?? entitlements.min_read_interval_sec);
    readCooldownSec = duration;
    if (readCooldownTimer) clearInterval(readCooldownTimer);
    readCooldownTimer = setInterval(() => {
      if (readCooldownSec <= 1) {
        readCooldownSec = 0;
        if (readCooldownTimer) {
          clearInterval(readCooldownTimer);
          readCooldownTimer = null;
        }
      } else {
        readCooldownSec -= 1;
      }
    }, 1000);
  }

  function handleReadRateLimit(err: unknown) {
    const e = err as ErrorResponse;
    if (e?.code === "RATE_LIMIT") {
      startReadCooldown(Number(e.args?.sec ?? entitlements.min_read_interval_sec));
    }
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
    if (polling || connectionLocked || reading) return;
    if (readCooldownActive) return;

    const hadSnapshot = snapshot !== null;
    reading = true;
    connectionLocked = true;
    if (!hadSnapshot) {
      statusText = msg("status.reading");
    }

    try {
      const response = await invoke<{ snapshot: MinerSnapshot }>("read_miner", {
        request: {
          ip,
          port: Number(port) || 4028,
          whatsminer_auth: whatsminerAuthPayload(),
        },
      });

      if (isSnapshotEmpty(response.snapshot)) {
        statusText = msg("status.readEmpty");
        return;
      }

      snapshot = response.snapshot;
      clearCharts();
      pushChartPoint(response.snapshot, 0);
      statusText = snapshotStatusText(response.snapshot);

      if (!entitlements.can_poll) {
        startReadCooldown();
      }

      if (shouldPromptWhatsminerAuth(response.snapshot)) {
        promptWhatsminerAuth();
      }
    } catch (err) {
      handleReadRateLimit(err);
      statusText = formatError(err);
    } finally {
      reading = false;
      connectionLocked = false;
    }
  }

  async function submitWhatsminerAuth() {
    const username = whatsminerUser.trim();
    if (!username) return;
    whatsminerAuthBusy = true;
    whatsminerAuthError = "";
    try {
      const response = await invoke<{ snapshot: MinerSnapshot }>("read_miner", {
        request: {
          ip,
          port: Number(port) || 4028,
          whatsminer_auth: { username, password: whatsminerPassword },
        },
      });
      if ((response.snapshot.board_chips?.length ?? 0) === 0) {
        whatsminerAuthError = msg("auth.invalid");
        return;
      }
      whatsminerCustomAuth = true;
      saveConnection();
      snapshot = response.snapshot;
      clearCharts();
      pushChartPoint(response.snapshot, 0);
      statusText = `${snapshot.identity.model} · ${snapshot.status}`;
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

  function stopPlayback() {
    sessionPlayer?.unload();
    sessionLoaded = false;
    sessionFileName = "";
    playbackActive = false;
    playerFrameIndex = 0;
    playerFrameTotal = 0;
    playerCurrentMs = 0;
    playerDurationMs = 0;
    playerSpeed = 1;
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
        defaultPath: `minerpulse-${Date.now()}.mpulse`,
        filters: [{ name: "MinerPulse", extensions: ["mpulse"] }],
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
        whatsminerAuth: whatsminerAuthPayload(),
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
      if (reading || readCooldownActive) return;
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

  function toggleActionMenu() {
    if (connectionLocked) return;
    openFileMenuOpen = false;
    actionMenuOpen = !actionMenuOpen;
  }

  function toggleOpenFileMenu() {
    if (busy || polling) return;
    actionMenuOpen = false;
    openFileMenuOpen = !openFileMenuOpen;
  }

  function setOpenFileMode(next: OpenFileMode) {
    if (next === "session" && !entitlements.can_play) return;
    openFileMode = next;
    openFileMenuOpen = false;
  }

  function runOpenFileAction() {
    if (busy || polling) return;
    if (openFileMode === "session") {
      if (!entitlements.can_play) return;
      void openSessionFile();
      return;
    }
    void importFromFile();
  }

  async function openSessionFile() {
    const path = await open({
      title: msg("toolbar.openSession"),
      multiple: false,
      filters: [{ name: "MinerPulse", extensions: ["mpulse"] }],
    });
    if (!path || Array.isArray(path)) return;

    busy = true;
    try {
      const file = await loadSessionFile(path);
      if (file.kind !== "session" || file.frames.length === 0) {
        const frame = file.frames[0];
        if (frame) {
          snapshot = frame.snapshot;
          clearCharts();
          pushChartPoint(frame.snapshot, 0);
          await invoke("remember_snapshot", { snapshot: frame.snapshot });
          if (file.miner_ip) {
            ip = file.miner_ip;
            saveConnection();
          }
        }
        stopPlayback();
        return;
      }

      if (!sessionPlayer) initSessionPlayer();
      sessionPlayer!.load(file.frames);
      sessionLoaded = true;
      sessionFileName = path.split(/[/\\]/).pop() ?? path;
      snapshot = file.frames[0].snapshot;
      chartPoints = chartPointsFromFrames(file.frames);
      chartCursorMs = file.frames[0].t_ms;
      chartsLive = false;
      syncPlayerState(sessionPlayer!.getState());
      if (file.miner_ip) {
        ip = file.miner_ip;
        saveConnection();
      }
      statusText = msg("status.sessionLoaded", { count: file.frames.length });
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
      defaultPath: `minerpulse-${Date.now()}.mpulse`,
      filters: [{ name: "MinerPulse", extensions: ["mpulse"] }],
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

  async function importFromFile() {
    const path = await open({
      title: msg("import.title"),
      multiple: false,
      filters: [
        {
          name: msg("import.fileFilter"),
          extensions: ["txt", "log", "json", "mpulse"],
        },
      ],
    });

    if (!path || Array.isArray(path)) return;

    busy = true;
    try {
      const result = await invoke<ParseImportResponse>("import_file_path", { path });
      await applyImportedSnapshot(result);
    } catch (err) {
      statusText = formatError(err);
    } finally {
      busy = false;
    }
  }

  async function applyImportedSnapshot(result: ParseImportResponse) {
    snapshot = result.snapshot;
    await invoke("remember_snapshot", { snapshot: result.snapshot });

    if (result.miner_ip) {
      ip = result.miner_ip;
      saveConnection();
    }
  }

  onMount(() => {
    let unlistenMiner: (() => void) | undefined;
    let unlistenDrop: (() => void) | undefined;
    let unlistenPollSnapshot: (() => void) | undefined;
    let unlistenPollFinished: (() => void) | undefined;

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
          if (parsed.openFileMode === "session" || parsed.openFileMode === "log") {
            openFileMode = parsed.openFileMode;
          }
        } catch {
          /* ignore */
        }
      }
      applyUiPrefs();
      loadConnection();
      connectionLoaded = true;
      await refreshEntitlements();
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
          shouldPromptWhatsminerAuth(event.payload.snapshot)
        ) {
          void (async () => {
            const wasRecording = event.payload.recording;
            await stopPoll();
            promptWhatsminerAuth(() => startPolling(wasRecording));
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
      sessionPlayer?.pause();
      if (readCooldownTimer) clearInterval(readCooldownTimer);
    };
  });

  $effect(() => {
    if (!connectionLoaded) return;
    ip;
    whatsminerAuthPrompted = false;
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
    openFileMode;
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
    if (openFileMode === "session" && !entitlements.can_play) {
      openFileMode = "log";
    }
  });

  $effect(() => {
    if (!actionMenuOpen && !openFileMenuOpen) return;
    const onDocumentClick = (event: MouseEvent) => {
      const target = event.target as Element;
      if (actionMenuOpen && !target.closest(".action-control")) {
        actionMenuOpen = false;
      }
      if (openFileMenuOpen && !target.closest(".open-file-control")) {
        openFileMenuOpen = false;
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
  <header class="toolbar">
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
      <button class="btn" disabled={connectionLocked} onclick={openScan}>
        {msg("toolbar.scan")}
      </button>
    </div>

    {#if entitlements.can_poll}
      {#if polling}
        <button class="btn danger" disabled={busy} onclick={stopPolling}>
          {msg("toolbar.stop")}
        </button>
      {:else}
        {#if readActionMode !== "read"}
          <div class="field poll-rate-field">
            <label for="poll-rate">{msg("toolbar.pollRate")}</label>
            <select
              id="poll-rate"
              class="btn poll-rate-select"
              bind:value={pollRateHz}
              disabled={connectionLocked}
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
              disabled={(readActionMode === "read" ? readActionDisabled : busy || connectionLocked)}
              onclick={runReadAction}
            >
              {#if reading && readActionMode === "read"}
                <span class="btn-spinner" aria-hidden="true"></span>
              {/if}
              {readActionLabel}
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
      <button
        class="btn primary btn-with-spinner"
        disabled={readActionDisabled}
        title={readCooldownActive ? msg("status.readCooldown", { sec: readCooldownSec }) : undefined}
        onclick={readMiner}
      >
        {#if reading}
          <span class="btn-spinner" aria-hidden="true"></span>
        {/if}
        {msg("toolbar.read")}
      </button>
    {/if}

    <div class="open-file-control action-control" class:open={openFileMenuOpen}>
      <div class="split-action">
        <button
          type="button"
          class="split-action-main"
          disabled={busy || polling}
          title={msg("import.dropHint")}
          onclick={runOpenFileAction}
        >
          {openFileLabel}
        </button>
        <button
          type="button"
          class="split-action-toggle"
          disabled={busy || polling}
          aria-expanded={openFileMenuOpen}
          aria-label={msg("toolbar.openFileMode")}
          onclick={(event) => {
            event.stopPropagation();
            toggleOpenFileMenu();
          }}
        >
          <span class="split-action-chevron" aria-hidden="true"></span>
        </button>
      </div>
      {#if openFileMenuOpen}
        <div class="action-menu" role="menu">
          {#each openFileModeOptions() as mode (mode.id)}
            <button
              type="button"
              role="menuitemradio"
              class="action-menu-item"
              class:active={openFileMode === mode.id}
              aria-checked={openFileMode === mode.id}
              onclick={(event) => {
                event.stopPropagation();
                setOpenFileMode(mode.id);
              }}
            >
              {mode.label}
            </button>
          {/each}
        </div>
      {/if}
    </div>

    <button class="btn" disabled={!snapshot || busy} onclick={saveSnapshot}>
      {msg("toolbar.save")}
    </button>

    <div class="spacer"></div>

    <button class="btn ghost" onclick={toggleTheme}>
      {theme === "light" ? msg("toolbar.themeDark") : msg("toolbar.themeLight")}
    </button>
    <button class="btn ghost" onclick={toggleDensity}>
      {density === "comfortable"
        ? msg("toolbar.densityCompact")
        : msg("toolbar.densityComfortable")}
    </button>

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

    <button class="btn ghost" onclick={openAbout}>
      {msg("toolbar.about")}
    </button>
  </header>

  <SessionPlayerBar
    {locale}
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
        <MinerDataPanel {snapshot} {locale} />
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
    {:else if activeTab === "pools"}
      {#if snapshot}
        <MinerPoolsPanel {snapshot} {locale} />
      {:else}
        <div class="locked-panel">{msg("pools.empty")}</div>
      {/if}
    {:else if activeTab === "charts"}
      <MinerChartsPanel
        points={chartPoints}
        cursorMs={chartCursorMs}
        live={chartsLiveNow}
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

  <footer class="statusbar">
    <span class="status-dot" class:busy={busy || polling || playbackActive}></span>
    <span>{statusText || msg("status.ready")}</span>
    <span class="spacer"></span>
    <button type="button" class="status-version" onclick={openAbout} title={msg("toolbar.about")}>
      {appVersion}
    </button>
  </footer>

  <AboutModal
    bind:open={aboutOpen}
    {locale}
    version={appVersionNumber}
    build={appBuild}
    product={appProduct}
  />

  <SubscriptionModal
    bind:open={subscriptionOpen}
    {locale}
    onUpdated={(e) => {
      entitlements = e;
    }}
  />

  <WhatsminerAuthModal
    bind:open={whatsminerAuthOpen}
    {locale}
    bind:username={whatsminerUser}
    bind:password={whatsminerPassword}
    busy={whatsminerAuthBusy}
    errorText={whatsminerAuthError}
    onSubmit={submitWhatsminerAuth}
  />
</div>
