<script lang="ts">
  import { invoke } from "$lib/ipc";
  import { listen } from "$lib/ipc";
  import { onMount, onDestroy } from "svelte";
  import { ui } from "$lib/stores/ui.svelte";

  interface Props {
    models?: string[]; // e.g. ['clap', 'qwen', 'essentia', 'sentence']
    onComplete?: () => void;
    onCancel?: () => void;
  }

  let { models = [], onComplete, onCancel }: Props = $props();

  let isDownloading = $state(false);
  let currentModelGroup = $state("");
  let currentFile = $state("");
  let bytesDone = $state(0);
  let bytesTotal = $state(0);
  let downloadSpeed = $state(0); // bytes/sec
  let etaSeconds = $state<number | null>(null);
  let errorMessage = $state("");
  let isComplete = $state(false);

  let resumableFiles = $state<Array<{ filename: string; offset: number }>>([]);
  let unlisteners: Array<() => void> = [];

  let lastBytes = 0;
  let lastTime = Date.now();

  async function checkResumable() {
    try {
      resumableFiles = await invoke<any[]>("check_pending_resume");
    } catch (e) {
      console.error("Failed to check resumable files:", e);
    }
  }

  async function startDownload() {
    errorMessage = "";
    isComplete = false;
    isDownloading = true;
    lastBytes = 0;
    lastTime = Date.now();
    downloadSpeed = 0;
    etaSeconds = null;

    let targets = [...models];
    if (targets.length === 0) {
      try {
        const exist = await invoke<any>("check_models_exist");
        if (!exist.essentia_exists) targets.push("essentia");
        if (!exist.clap_exists) targets.push("clap");
        if (!exist.qwen_exists) targets.push("qwen");
        if (!exist.sentence_exists) targets.push("sentence");
      } catch (e) {
        errorMessage = "Failed to auto-detect missing neural models.";
        isDownloading = false;
        return;
      }
    }

    if (targets.length === 0) {
      isComplete = true;
      isDownloading = false;
      onComplete?.();
      return;
    }

    try {
      await invoke("download_models", { models: targets });
    } catch (e: any) {
      errorMessage = e.toString();
      isDownloading = false;
    }
  }

  async function cancelDownload() {
    try {
      await invoke("cancel_model_download");
      isDownloading = false;
      onCancel?.();
      await checkResumable();
    } catch (e: any) {
      ui.showToast("Failed to cancel download: " + e.toString(), "error");
    }
  }

  async function checkActiveDownload() {
    try {
      const active = await invoke<any>("get_download_status");
      if (active) {
        isDownloading = true;
        currentModelGroup = active.model;
        currentFile = active.file;
        bytesDone = active.bytes_done;
        bytesTotal = active.bytes_total;
        lastBytes = bytesDone;
        lastTime = Date.now();
      }
    } catch (e) {
      console.error("Failed to check active download status:", e);
    }
  }

  onMount(async () => {
    await checkResumable();
    await checkActiveDownload();

    const progressUnlisten = await listen<any>("model-download-progress", (event) => {
      const payload = event.payload;
      currentModelGroup = payload.model;
      currentFile = payload.file;
      bytesDone = payload.bytes_done;
      bytesTotal = payload.bytes_total;
      isDownloading = true;

      const now = Date.now();
      const timeDiff = (now - lastTime) / 1000;
      if (timeDiff >= 0.5) {
        const bytesDiff = bytesDone - lastBytes;
        downloadSpeed = bytesDiff / timeDiff;
        lastBytes = bytesDone;
        lastTime = now;

        if (downloadSpeed > 0) {
          etaSeconds = Math.max(0, (bytesTotal - bytesDone) / downloadSpeed);
        } else {
          etaSeconds = null;
        }
      }
    });
    unlisteners.push(progressUnlisten);

    const completeUnlisten = await listen("model-download-all-complete", () => {
      isDownloading = false;
      isComplete = true;
      resumableFiles = [];
      ui.showToast("All neural models downloaded and verified successfully!", "success");
      onComplete?.();
    });
    unlisteners.push(completeUnlisten);

    const errorUnlisten = await listen<string>("model-download-all-error", (event) => {
      isDownloading = false;
      errorMessage = event.payload;
      checkResumable();
    });
    unlisteners.push(errorUnlisten);
  });

  onDestroy(() => {
    unlisteners.forEach((u) => u());
  });

  function formatBytes(bytes: number): string {
    if (bytes === 0) return "0 B";
    const k = 1024;
    const sizes = ["B", "KB", "MB", "GB"];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + " " + sizes[i];
  }

  function formatSpeed(bytesPerSec: number): string {
    if (bytesPerSec <= 0) return "Connecting...";
    return formatBytes(bytesPerSec) + "/s";
  }

  function formatEta(seconds: number | null): string {
    if (seconds === null) return "estimating...";
    if (seconds < 60) return `${Math.round(seconds)}s remaining`;
    const mins = Math.floor(seconds / 60);
    const secs = Math.round(seconds % 60);
    return `${mins}m ${secs}s remaining`;
  }

  const pct = $derived(bytesTotal > 0 ? (bytesDone / bytesTotal) * 100 : 0);
</script>

<div class="model-downloader-container">
  {#if errorMessage}
    <div class="error-banner">
      <div class="error-header">
        <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
          <circle cx="12" cy="12" r="10"/><line x1="12" y1="8" x2="12" y2="12"/><line x1="12" y1="16" x2="12.01" y2="16"/>
        </svg>
        <span>Download Failed</span>
      </div>
      <p class="error-text">{errorMessage}</p>
      <button class="dl-btn dl-btn-retry" onclick={startDownload}>
        Retry Download
      </button>
    </div>
  {:else if isComplete}
    <div class="success-banner">
      <div class="success-header">
        <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
          <polyline points="20 6 9 17 4 12"/>
        </svg>
        <span>All Models Ready</span>
      </div>
      <p class="success-text">Neural models verified successfully. High-fidelity audio analysis is fully unlocked.</p>
    </div>
  {:else if isDownloading}
    <div class="downloading-card">
      <div class="downloading-header">
        <div class="dl-status-spin">
          <svg class="spin-svg" xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><path d="M21 12a9 9 0 1 1-6.219-8.56"/></svg>
        </div>
        <span class="dl-status-title">Downloading {currentModelGroup.toUpperCase()}</span>
      </div>

      <div class="file-info-row">
        <span class="dl-filename" title={currentFile}>{currentFile}</span>
        <span class="dl-speed">{formatSpeed(downloadSpeed)}</span>
      </div>

      <div class="progress-bar-track">
        <div class="progress-bar-fill" style="width: {pct}%"></div>
      </div>

      <div class="dl-footer-metrics">
        <span class="dl-bytes">{formatBytes(bytesDone)} / {formatBytes(bytesTotal)}</span>
        <span class="dl-eta">{formatEta(etaSeconds)}</span>
      </div>

      <button class="dl-btn dl-btn-cancel" onclick={cancelDownload}>
        <svg xmlns="http://www.w3.org/2000/svg" width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
          <rect x="3" y="3" width="18" height="18" rx="2" ry="2"/>
        </svg>
        Cancel
      </button>
    </div>
  {:else}
    <div class="idle-card">
      {#if resumableFiles.length > 0}
        <div class="resume-badge">
          <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
            <polyline points="23 4 23 10 17 10"/><path d="M20.49 15a9 9 0 1 1-2.12-9.36L23 10"/>
          </svg>
          <span>{resumableFiles.length} download{resumableFiles.length !== 1 ? 's' : ''} can be resumed</span>
        </div>
      {/if}
      <button class="dl-btn dl-btn-primary" onclick={startDownload}>
        <svg xmlns="http://www.w3.org/2000/svg" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
          <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/>
        </svg>
        {#if resumableFiles.length > 0}
          Resume Model Download
        {:else}
          Download Neural Models
        {/if}
      </button>
    </div>
  {/if}
</div>

<style>
  .model-downloader-container {
    display: flex;
    flex-direction: column;
    width: 100%;
    box-sizing: border-box;
  }

  /* ── Common button styling ── */
  .dl-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-xs);
    font-weight: 700;
    padding: 6px 14px;
    border: 1px solid rgba(255, 255, 255, 0.12);
    border-radius: 4px;
    background: rgba(255, 255, 255, 0.04);
    color: var(--sg-on-surface, #e3e1e9);
    cursor: pointer;
    transition: all 0.12s ease;
    box-sizing: border-box;
  }

  .dl-btn:hover {
    background: rgba(255, 255, 255, 0.08);
    border-color: rgba(255, 255, 255, 0.25);
  }

  .dl-btn-primary {
    border-color: rgba(0, 240, 255, 0.35);
    color: var(--sg-primary, #00f0ff);
    background: rgba(0, 240, 255, 0.08);
  }

  .dl-btn-primary:hover {
    background: color-mix(in srgb, var(--sg-primary) 14%, transparent);
    border-color: var(--sg-primary);
    color: var(--sg-primary);
    box-shadow: 0 0 10px color-mix(in srgb, var(--sg-primary) 20%, transparent);
  }

  .dl-btn-cancel {
    border-color: color-mix(in srgb, var(--sg-error) 30%, transparent);
    color: var(--sg-error);
    background: color-mix(in srgb, var(--sg-error) 7%, transparent);
    margin-top: 8px;
    align-self: flex-start;
  }

  .dl-btn-cancel:hover {
    background: color-mix(in srgb, var(--sg-error) 15%, transparent);
    border-color: var(--sg-error);
  }

  .dl-btn-retry {
    border-color: color-mix(in srgb, var(--sg-warning) 30%, transparent);
    color: var(--sg-warning);
    background: color-mix(in srgb, var(--sg-warning) 7%, transparent);
  }

  .dl-btn-retry:hover {
    background: color-mix(in srgb, var(--sg-warning) 15%, transparent);
    border-color: var(--sg-warning);
  }

  /* ── Error Banner ── */
  .error-banner {
    padding: 0.85rem;
    border: 1px solid color-mix(in srgb, var(--sg-error) 25%, transparent);
    border-radius: 5px;
    background: color-mix(in srgb, var(--sg-error) 5%, transparent);
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .error-header {
    display: flex;
    align-items: center;
    gap: 6px;
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-sm);
    font-weight: 700;
    color: var(--sg-error);
  }

  .error-text {
    margin: 0;
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-2xs);
    color: var(--sg-outline, #849495);
    line-height: 1.4;
  }

  /* ── Success Banner ── */
  .success-banner {
    padding: 0.85rem;
    border: 1px solid color-mix(in srgb, var(--sg-primary) 25%, transparent);
    border-radius: 5px;
    background: color-mix(in srgb, var(--sg-primary) 5%, transparent);
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .success-header {
    display: flex;
    align-items: center;
    gap: 6px;
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-sm);
    font-weight: 700;
    color: var(--sg-primary, #00f0ff);
  }

  .success-text {
    margin: 0;
    font-size: var(--sg-text-xs);
    color: var(--sg-outline, #849495);
    line-height: 1.4;
  }

  /* ── Downloading Card ── */
  .downloading-card {
    padding: 0.85rem;
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 5px;
    background: rgba(255, 255, 255, 0.02);
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .downloading-header {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .dl-status-spin {
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--sg-primary, #00f0ff);
  }

  .spin-svg {
    animation: spin 1.2s linear infinite;
  }

  @keyframes spin {
    from { transform: rotate(0deg); }
    to { transform: rotate(360deg); }
  }

  .dl-status-title {
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-xs);
    font-weight: 700;
    color: var(--sg-on-surface, #e3e1e9);
  }

  .file-info-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 1rem;
  }

  .dl-filename {
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-2xs);
    color: var(--sg-outline, #849495);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    flex: 1;
  }

  .dl-speed {
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-2xs);
    font-weight: 700;
    color: var(--sg-primary, #00f0ff);
  }

  .progress-bar-track {
    width: 100%;
    height: 4px;
    background: rgba(255, 255, 255, 0.06);
    border-radius: 2px;
    overflow: hidden;
    position: relative;
  }

  .progress-bar-fill {
    height: 100%;
    background: linear-gradient(90deg, color-mix(in srgb, var(--sg-primary) 80%, #0000ff), var(--sg-primary));
    border-radius: 2px;
    box-shadow: 0 0 6px color-mix(in srgb, var(--sg-primary) 50%, transparent);
    transition: width 0.15s ease-out;
  }

  .dl-footer-metrics {
    display: flex;
    justify-content: space-between;
    align-items: center;
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-3xs);
    color: var(--sg-outline, #849495);
  }

  /* ── Idle Card ── */
  .idle-card {
    display: flex;
    flex-direction: column;
    gap: 8px;
    align-items: flex-start;
  }

  .resume-badge {
    display: flex;
    align-items: center;
    gap: 6px;
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-2xs);
    font-weight: 700;
    color: var(--sg-warning);
    padding: 3px 8px;
    background: color-mix(in srgb, var(--sg-warning) 7%, transparent);
    border: 1px solid color-mix(in srgb, var(--sg-warning) 25%, transparent);
    border-radius: 4px;
  }
</style>
