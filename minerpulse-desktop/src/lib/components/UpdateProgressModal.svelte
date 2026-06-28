<script lang="ts">
  import { check, type Update } from "@tauri-apps/plugin-updater";
  import { relaunch } from "@tauri-apps/plugin-process";
  import { t, type Locale, type MessageKey } from "$lib/i18n";

  type UpdatePhase =
    | "checking"
    | "downloading"
    | "installing"
    | "relaunching"
    | "done"
    | "cancelled"
    | "error";

  let {
    open = $bindable(false),
    locale,
    productVersion,
  }: {
    open?: boolean;
    locale: Locale;
    productVersion: string;
  } = $props();

  let phase = $state<UpdatePhase>("checking");
  let statusKey = $state<MessageKey>("updateProgress.checking");
  let statusArgs = $state<Record<string, string | number> | undefined>(undefined);
  let detailText = $state("");
  let progressPercent = $state<number | null>(null);
  let canCancel = $state(true);
  let finished = $state(false);

  let cancelled = false;
  let activeUpdate: Update | null = null;
  let downloadedBytes = 0;
  let totalBytes = 0;

  function msg(key: MessageKey, args?: Record<string, string | number>) {
    return t(locale, key, args);
  }

  function resetState() {
    phase = "checking";
    statusKey = "updateProgress.checking";
    statusArgs = undefined;
    detailText = "";
    progressPercent = null;
    canCancel = true;
    finished = false;
    cancelled = false;
    activeUpdate = null;
    downloadedBytes = 0;
    totalBytes = 0;
  }

  function formatBytes(bytes: number): string {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  }

  function formatUpdateVersion(update: Update): string {
    const build = update.rawJson?.["build"];
    if (typeof build === "number") {
      return `${productVersion} (${build})`;
    }
    return update.version;
  }

  function setStatus(key: MessageKey, args?: Record<string, string | number>) {
    statusKey = key;
    statusArgs = args;
  }

  function updateDownloadDetail() {
    if (totalBytes > 0) {
      const percent = Math.min(100, Math.round((downloadedBytes / totalBytes) * 100));
      progressPercent = percent;
      detailText = msg("updateProgress.downloadDetail", {
        percent,
        downloaded: formatBytes(downloadedBytes),
        total: formatBytes(totalBytes),
      });
      return;
    }
    progressPercent = null;
    detailText =
      downloadedBytes > 0
        ? msg("updateProgress.downloadBytes", { downloaded: formatBytes(downloadedBytes) })
        : "";
  }

  function closeModal() {
    open = false;
  }

  function onBackdropClick(event: MouseEvent) {
    if (event.target === event.currentTarget && finished) {
      closeModal();
    }
  }

  function onKeydown(event: KeyboardEvent) {
    if (event.key === "Escape") {
      if (finished) {
        closeModal();
      } else if (canCancel) {
        void cancelUpdate();
      }
    }
  }

  async function cancelUpdate() {
    if (!canCancel || finished) return;
    cancelled = true;
    canCancel = false;
    phase = "cancelled";
    setStatus("updateProgress.cancelled");
    detailText = "";
    progressPercent = null;
    finished = true;
    try {
      await activeUpdate?.close();
    } catch {
      /* ignore */
    } finally {
      activeUpdate = null;
    }
  }

  async function runUpdateFlow() {
    resetState();
    cancelled = false;

    try {
      setStatus("updateProgress.checking");
      progressPercent = null;

      const update = await check();
      if (cancelled) {
        await update?.close();
        return;
      }

      if (!update) {
        phase = "done";
        progressPercent = 100;
        setStatus("updateProgress.upToDate");
        canCancel = false;
        finished = true;
        return;
      }

      activeUpdate = update;
      phase = "downloading";
      setStatus("updateProgress.downloading", { version: formatUpdateVersion(update) });
      downloadedBytes = 0;
      totalBytes = 0;
      updateDownloadDetail();

      await update.download((event) => {
        if (cancelled) return;
        if (event.event === "Started") {
          totalBytes = event.data.contentLength ?? 0;
          updateDownloadDetail();
        } else if (event.event === "Progress") {
          downloadedBytes += event.data.chunkLength;
          updateDownloadDetail();
        } else if (event.event === "Finished") {
          progressPercent = 100;
        }
      });

      if (cancelled) return;

      phase = "installing";
      canCancel = false;
      setStatus("updateProgress.installing");
      detailText = "";
      progressPercent = 100;

      await update.install();

      if (cancelled) return;

      phase = "relaunching";
      setStatus("updateProgress.relaunching");
      await relaunch();
    } catch (err) {
      if (cancelled) return;
      phase = "error";
      setStatus("updateProgress.error", { detail: String(err) });
      detailText = "";
      progressPercent = null;
      canCancel = false;
      finished = true;
    } finally {
      if (!finished && !cancelled) {
        canCancel = false;
        finished = true;
      }
      if (!cancelled) {
        activeUpdate = null;
      }
    }
  }

  $effect(() => {
    if (!open) {
      resetState();
      return;
    }
    void runUpdateFlow();
  });
</script>

<svelte:window onkeydown={onKeydown} />

{#if open}
  <div class="modal-backdrop update-progress-backdrop" onclick={onBackdropClick} role="presentation">
    <div
      class="modal-card update-progress-modal"
      role="dialog"
      aria-modal="true"
      aria-labelledby="update-progress-title"
      aria-busy={!finished}
    >
      <header class="modal-head">
        <div>
          <div class="modal-kicker">{msg("updateProgress.kicker")}</div>
          <h3 id="update-progress-title" class="modal-title">{msg("updateProgress.title")}</h3>
        </div>
        {#if finished}
          <button
            type="button"
            class="modal-close"
            onclick={closeModal}
            aria-label={msg("updateProgress.close")}
          >
            ×
          </button>
        {/if}
      </header>

      <div class="modal-body update-progress-body">
        <div class="update-progress-panel">
          <div
            class="update-progress-track"
            class:update-progress-track-indeterminate={progressPercent === null && !finished}
            role="progressbar"
            aria-valuemin="0"
            aria-valuemax="100"
            aria-valuenow={progressPercent ?? undefined}
            aria-label={msg(statusKey, statusArgs)}
          >
            <div
              class="update-progress-fill"
              style:width={progressPercent === null ? "35%" : `${progressPercent}%`}
            ></div>
          </div>

          <p class="update-progress-status" class:update-progress-status-error={phase === "error"}>
            {msg(statusKey, statusArgs)}
          </p>

          {#if detailText}
            <p class="update-progress-detail">{detailText}</p>
          {/if}
        </div>

        <div class="update-progress-actions">
          {#if canCancel && !finished}
            <button type="button" class="btn" onclick={cancelUpdate}>
              {msg("updateProgress.cancel")}
            </button>
          {:else if finished}
            <button type="button" class="btn primary" onclick={closeModal}>
              {msg("updateProgress.close")}
            </button>
          {/if}
        </div>
      </div>
    </div>
  </div>
{/if}
