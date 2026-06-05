<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { onMount, onDestroy } from "svelte";
  import { theme } from "$lib/stores/theme.svelte";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { ui } from "$lib/stores/ui.svelte";
  import { library } from "$lib/stores/library.svelte";
  import ModelDownloader from "./ModelDownloader.svelte";
  import MetricsInspector from "./MetricsInspector.svelte";

  interface PassError {
    path: string;
    log: string | null;
    duration_ms: number | null;
    last_run_at: string | null;
  }

  interface PassStats {
    pass_name: string;
    pending: number;
    in_progress: number;
    done: number;
    failed: number;
    total: number;
    avg_duration_ms: number | null;
    concurrency: number;
    errors: PassError[];
  }

  interface ModelExistence {
    qwen_exists: boolean;
    sentence_exists: boolean;
    clap_exists: boolean;
    essentia_exists: boolean;
    all_exist: boolean;
    missing_files: string[];
    [key: string]: boolean | string[];
  }

  // Ordered by pipeline execution priority (sequential — no two passes run concurrently)
  const PASS_ORDER = [
    'audio_analysis',
    'bpm_correction',
    'clap',
    'essentia',
    'bpm_refinement',
    'qwen',
    'description_embed',
  ];

  // Dark / light color pairs per pass role
  const PASS_COLORS: Record<string, { dark: string; light: string }> = {
    audio:       { dark: '#00f0ff', light: '#0284c7' },  // cyan → sky blue
    neural_pink: { dark: '#fe00fe', light: '#9333ea' },  // magenta → purple
    amber:       { dark: '#c87800', light: '#b45309' },  // amber stays, slightly darker
    green:       { dark: '#76ff03', light: '#16a34a' },  // lime → forest green
    muted:       { dark: '#849495', light: '#64748b' },
  };

  const PASS_ROLE: Record<string, keyof typeof PASS_COLORS> = {
    audio_analysis:    'audio',
    bpm_correction:    'audio',
    bpm_refinement:    'audio',
    clap:              'neural_pink',
    qwen:              'neural_pink',
    description_embed: 'amber',
    essentia:          'green',
  };

  const PASS_META: Record<string, { label: string; description: string }> = {
    audio_analysis:   { label: 'Audio Analysis',       description: 'BPM, key, loudness, waveform, sample rate'          },
    bpm_correction:   { label: 'BPM Correction',       description: 'Halve/double BPM outliers to musical range'         },
    clap:             { label: 'CLAP Embeddings',       description: 'Audio fingerprint vectors for similarity search'    },
    qwen:             { label: 'Qwen Audio LLM',        description: 'AI description, genre, mood, instruments'          },
    description_embed:{ label: 'Description Embedder',  description: 'Text embedding vectors from AI descriptions'       },
    essentia:         { label: 'Essentia Classifier',   description: 'Genre, mood, vocal detection via neural classifier' },
    bpm_refinement:   { label: 'BPM Refinement',        description: 'Precision beat-tracking on corrected estimates'    },
  };

  const isLight = $derived(theme.resolvedTheme === 'light');

  function passColor(name: string): string {
    const role = PASS_ROLE[name] ?? 'muted';
    return isLight ? PASS_COLORS[role].light : PASS_COLORS[role].dark;
  }

  function passMeta(name: string) {
    return PASS_META[name] ?? { label: name, description: '' };
  }

  let stats           = $state<PassStats[]>([]);
  const sortedStats   = $derived(
    [...stats].sort((a, b) => {
      const ai = PASS_ORDER.indexOf(a.pass_name);
      const bi = PASS_ORDER.indexOf(b.pass_name);
      return (ai === -1 ? 999 : ai) - (bi === -1 ? 999 : bi);
    })
  );
  let isRunning       = $state(false);
  let showMetricsDrawer = $state(false);
  let errorMessage    = $state("");
  let unlisteners: Array<() => void> = [];

  let modelStatus       = $state<ModelExistence | null>(null);
  const missingModelGroups = $derived.by(() => {
    if (!modelStatus) return [];
    const missing: string[] = [];
    if (!modelStatus.essentia_exists) missing.push("essentia");
    if (!modelStatus.clap_exists) missing.push("clap");
    if (!modelStatus.qwen_exists) missing.push("qwen");
    if (!modelStatus.sentence_exists) missing.push("sentence");
    return missing;
  });
  let isCheckingModels  = $state(false);
  let showModelWarning  = $state(false);
  let hasCopiedCommand  = $state(false);
  let warningDismissed  = $state(false);
  let showUpdateBanner  = $state(false);
  let latestAppVersion  = $state("");

  async function checkAppUpdates() {
    try {
      const response = await invoke<{ manifest: any, update_available: boolean }>("fetch_app_manifest");
      if (response && response.update_available) {
        latestAppVersion = response.manifest.min_app_version;
        showUpdateBanner = true;
      }
    } catch (e) {
      console.error("Failed to check app updates:", e);
    }
  }

  async function downloadUpdate() {
    try {
      await openUrl("https://github.com/robertolupi/deep-cuts/releases");
    } catch (e) {
      console.error("Failed to open update URL:", e);
    }
  }

  function dismissUpdateMaybeLater() {
    showUpdateBanner = false;
  }

  async function disableUpdateChecking() {
    try {
      await invoke("set_update_settings", { enabled: false });
      showUpdateBanner = false;
      ui.showToast("Update checking disabled. Re-enable in Library Settings.", "success");
    } catch (e: any) {
      console.error("Failed to disable update settings:", e);
    }
  }

  async function checkModels() {
    isCheckingModels = true;
    try {
      const status = await invoke<ModelExistence>("check_models_exist");
      modelStatus = status;
      if (!status.all_exist && !warningDismissed) showModelWarning = true;
      else if (status.all_exist) showModelWarning = false;
    } catch (e) {
      console.error("Failed to check model existence:", e);
    } finally {
      isCheckingModels = false;
    }
  }

  function copyCommand() {
    navigator.clipboard.writeText("python3 tools/download_models.py");
    hasCopiedCommand = true;
    setTimeout(() => { hasCopiedCommand = false; }, 2000);
  }

  function dismissWarning() { showModelWarning = false; warningDismissed = true; }

  interface ThroughputSample { time: number; done: number; }
  let throughputBaseline = new Map<string, ThroughputSample>();

  function updateThroughput(newStats: PassStats[]) {
    const now = Date.now();
    for (const pass of newStats) {
      const existing = throughputBaseline.get(pass.pass_name);
      if (pass.done > 0 && pass.in_progress > 0) {
        if (!existing) throughputBaseline.set(pass.pass_name, { time: now, done: pass.done });
      } else if (pass.in_progress === 0) {
        throughputBaseline.delete(pass.pass_name);
      }
    }
  }

  function etaForPass(pass: PassStats): number {
    const remaining = pass.pending + pass.in_progress;
    if (remaining <= 0) return 0;
    const baseline = throughputBaseline.get(pass.pass_name);
    if (baseline) {
      const elapsedMs = Date.now() - baseline.time;
      const completed = pass.done - baseline.done;
      if (completed > 0 && elapsedMs > 0) return remaining / (completed / elapsedMs);
    }
    if (pass.avg_duration_ms) {
      const concurrency = pass.concurrency || 1;
      return (remaining * pass.avg_duration_ms) / concurrency;
    }
    return 0;
  }

  const estimatedTimeRemaining = $derived.by(() => {
    let totalMs = 0;
    for (const pass of stats) totalMs += etaForPass(pass);
    return totalMs;
  });
  const stuckPassCount = $derived.by(() => {
    if (isRunning) return 0;
    return stats.reduce((sum, pass) => sum + pass.in_progress, 0);
  });

  function formatEta(ms: number): string {
    if (ms <= 0) return "";
    const seconds = Math.ceil(ms / 1000);
    if (seconds < 60) return `${seconds}s`;
    const mins = Math.floor(seconds / 60);
    const secs = seconds % 60;
    return secs === 0 ? `${mins}m` : `${mins}m ${secs}s`;
  }

  function formatMs(ms: number | null): string {
    if (ms === null) return "—";
    if (ms < 1000) return `${Math.round(ms)}ms`;
    return `${(ms / 1000).toFixed(1)}s`;
  }

  async function loadStats() {
    try {
      const newStats = await invoke<PassStats[]>("get_pass_stats");
      updateThroughput(newStats);
      stats = newStats;
    } catch (e) { console.error("Failed to load pass stats:", e); }
  }

  async function startAnalysis() {
    errorMessage = "";
    throughputBaseline.clear();
    if (modelStatus && !modelStatus.all_exist) {
      ui.showToast("Cannot start pipeline: missing neural network model files.", "error");
      showModelWarning = true;
      return;
    }
    try {
      await invoke("run_analysis_pipeline");
      isRunning = true;
    } catch (e: any) { errorMessage = e?.toString() ?? "Unknown error"; }
  }

  async function resetAll() {
    try { await invoke("reset_all_passes"); await loadStats(); }
    catch (e: any) { errorMessage = e?.toString() ?? "Unknown error"; }
  }

  async function recoverStuckPasses() {
    errorMessage = "";
    try {
      await invoke<number>("recover_stuck_passes");
      await loadStats();
    } catch (e: any) { errorMessage = e?.toString() ?? "Unknown error"; }
  }

  async function resetPass(passName: string) {
    try { await invoke("reset_pass", { passName }); await loadStats(); }
    catch (e: any) { errorMessage = e?.toString() ?? "Unknown error"; }
  }

  async function toggleManualPause() {
    const nextState = !library.analysisManuallyPaused;
    try {
      await invoke("set_analysis_manually_paused", { paused: nextState });
      if (nextState) {
        ui.showToast("Analysis pipeline paused by user", "success");
      } else {
        ui.showToast("Analysis pipeline resumed", "success");
      }
    } catch (e: any) {
      errorMessage = e?.toString() ?? "Failed to toggle pause state";
    }
  }

  let checkInterval: ReturnType<typeof setInterval>;

  onMount(() => {
    checkAppUpdates();
    invoke<boolean>("is_analysis_running").then(v => { isRunning = v; });
    loadStats();
    checkModels();
    checkInterval = setInterval(() => {
      if (showModelWarning && !isCheckingModels && !isRunning) checkModels();
    }, 5000);
    listen("analysis-progress", () => loadStats()).then(u => unlisteners.push(u));
    listen("analysis-complete", () => { isRunning = false; loadStats(); }).then(u => unlisteners.push(u));
    listen<{ phase: string; message: string }>("analysis-error", (event) => {
      const payload = event.payload;
      errorMessage = `${payload.phase}: ${payload.message}`;
      isRunning = false;
      loadStats();
    }).then(u => unlisteners.push(u));
  });

  onDestroy(() => {
    unlisteners.forEach(u => u());
    clearInterval(checkInterval);
  });
</script>

<div class="analysis-panel">

  <!-- Header -->
  <div class="panel-header">
    <div class="header-left">
      <h2 class="panel-title">Analysis Pipeline</h2>
      <p class="panel-subtitle">BPM · Key · Loudness · Waveforms · Genre · Mood · CLAP · AI Description</p>
    </div>
    <div class="header-actions">
      <button class="action-btn" onclick={() => { showMetricsDrawer = true; }}>
        <svg xmlns="http://www.w3.org/2000/svg" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
          <line x1="18" y1="20" x2="18" y2="10"/><line x1="12" y1="20" x2="12" y2="4"/><line x1="6" y1="20" x2="6" y2="14"/>
        </svg>
        Inspect Metrics
      </button>
      {#if isRunning}
        {#if library.analysisPaused}
          <span class="running-badge paused-badge" style="color: var(--sg-amber, #f0a030); border-color: rgba(240,160,48,0.3); background: rgba(240,160,48,0.07)">
            <span class="pulse-dot" style="background: var(--sg-amber, #f0a030); animation: none;"></span> Paused
          </span>
          <button class="action-btn action-btn-primary" onclick={toggleManualPause}>
            <svg xmlns="http://www.w3.org/2000/svg" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
              <polygon points="5 3 19 12 5 21 5 3"/>
            </svg>
            Resume
          </button>
        {:else}
          {#if estimatedTimeRemaining > 0}
            <span class="eta-label">~{formatEta(estimatedTimeRemaining)} remaining</span>
          {/if}
          <span class="running-badge">
            <span class="pulse-dot"></span> Running
          </span>
          <button class="action-btn" onclick={toggleManualPause}>
            <svg xmlns="http://www.w3.org/2000/svg" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
              <rect x="6" y="4" width="4" height="16"/><rect x="14" y="4" width="4" height="16"/>
            </svg>
            Pause
          </button>
        {/if}
      {:else}
        <button class="action-btn action-btn-primary" onclick={startAnalysis}>
          <svg xmlns="http://www.w3.org/2000/svg" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
            <polygon points="5 3 19 12 5 21 5 3"/>
          </svg>
          Run Analysis
        </button>
        {#if stuckPassCount > 0}
          <button class="action-btn" onclick={recoverStuckPasses}>
            Recover {stuckPassCount} Stuck
          </button>
        {/if}
        {#if stats.length > 0}
          <button class="action-btn" onclick={resetAll}>Reset All</button>
        {/if}
      {/if}
    </div>
  </div>

  <!-- Update banner -->
  {#if showUpdateBanner}
    <div class="update-banner-card">
      <div class="warning-title-row">
        <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="var(--sg-primary, #00f0ff)" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
          <circle cx="12" cy="12" r="10"/><polyline points="12 8 12 12 16 14"/>
        </svg>
        <span class="warning-title-text" style="color: var(--sg-primary, #00f0ff)">New update available (v{latestAppVersion})</span>
        <button class="update-close" onclick={dismissUpdateMaybeLater}>×</button>
      </div>
      <p class="update-desc-text">
        An update with performance improvements, new features, and upgraded model support is ready. Update now to ensure compatibility.
      </p>
      <div class="update-actions">
        <button class="action-btn action-btn-primary" onclick={downloadUpdate}>
          <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
            <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/>
            <polyline points="7 10 12 15 17 10"/>
            <line x1="12" y1="15" x2="12" y2="3"/>
          </svg>
          Download
        </button>
        <button class="action-btn" onclick={dismissUpdateMaybeLater}>Maybe Later</button>
        <button class="action-btn action-btn-danger" onclick={disableUpdateChecking}>Do Not Ask Again</button>
      </div>
    </div>
  {/if}

  {#if errorMessage}
    <div class="error-banner">{errorMessage}</div>
  {/if}

  <!-- Model warning -->
  {#if showModelWarning && modelStatus}
    <div class="model-warning">
      <div class="warning-title-row">
        <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="#c87800" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
          <path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"/>
          <line x1="12" y1="9" x2="12" y2="13"/><line x1="12" y1="17" x2="12.01" y2="17"/>
        </svg>
        <span class="warning-title-text">Missing neural network model files</span>
        <button class="warning-close" onclick={dismissWarning}>×</button>
      </div>

      <div class="model-groups">
        {#each [
          { key: 'essentia_exists', groupKey: 'essentia', label: 'Essentia Classifier' },
          { key: 'clap_exists', groupKey: 'clap', label: 'CLAP Embedder' },
          { key: 'qwen_exists', groupKey: 'qwen', label: 'Qwen Audio LLM' },
          { key: 'sentence_exists', groupKey: 'sentence', label: 'MiniLM Text Embedder' },
        ] as group}
          {@const groupOk = modelStatus[group.key] as boolean}
          {@const groupMissing = (modelStatus.missing_files as string[]).filter(f => f.startsWith(group.groupKey + '/'))}
          <div class="model-group" class:group-ok={groupOk} class:group-missing={!groupOk}>
            <div class="group-header">
              <span class="group-label">{group.label}</span>
              <span class="group-status">{groupOk ? '● OK' : '▲ MISSING'}</span>
            </div>
            {#each groupMissing as f}
              <div class="model-file">
                <span class="file-dot dot-missing"></span>
                <span class="file-label">{f.split('/')[1]}</span>
                <span class="file-status file-missing">
                  missing
                </span>
              </div>
            {/each}
          </div>
        {/each}
      </div>

      <div class="warning-footer" style="flex-direction: column; align-items: stretch; gap: 0.5rem;">
        <ModelDownloader models={missingModelGroups} onComplete={checkModels} />
        <div class="warning-actions" style="margin-top: 0.5rem; justify-content: flex-end; width: 100%;">
          <button class="action-btn" onclick={checkModels} disabled={isCheckingModels}>
            {isCheckingModels ? 'Checking…' : 'Re-check'}
          </button>
          <button class="action-btn-ghost" onclick={dismissWarning}>Proceed anyway</button>
        </div>
      </div>
    </div>
  {/if}

  <!-- Pass cards -->
  {#if stats.length === 0}
    <div class="empty-state">
      <p>No analysis data yet — run the pipeline to get started.</p>
    </div>
  {:else}
    <div class="passes">
      {#each sortedStats as pass (pass.pass_name)}
        {@const meta   = passMeta(pass.pass_name)}
        {@const color  = passColor(pass.pass_name)}
        {@const pct    = pass.total > 0 ? (pass.done / pass.total) * 100 : 0}
        {@const active = pass.in_progress > 0}
        <div class="pass-card" class:pass-active={active}>
          <div class="pass-top">
            <div class="pass-info">
              <div class="pass-name-row">
                <span class="pass-accent" style="background:{color};"></span>
                <span class="pass-label">{meta.label}</span>
                {#if active}
                  <span class="pass-running-tag">processing</span>
                {/if}
              </div>
              {#if meta.description}
                <span class="pass-desc">{meta.description}</span>
              {/if}
            </div>
            <div class="pass-right">
              {#if isRunning && (pass.pending > 0 || pass.in_progress > 0)}
                {@const eta = etaForPass(pass)}
                {#if eta > 0}
                  <span class="pass-eta">~{formatEta(eta)}</span>
                {/if}
              {/if}
              {#if pass.avg_duration_ms !== null}
                <span class="pass-avg">avg {formatMs(pass.avg_duration_ms)}</span>
              {/if}
              {#if !isRunning}
                <button class="reset-btn" onclick={() => resetPass(pass.pass_name)}>Reset</button>
              {/if}
            </div>
          </div>

          <!-- Progress bar -->
          <div class="progress-track">
            <div class="progress-done" style="width:{pct}%; background:{color};"></div>
            <div class="progress-running" style="width:{pass.total > 0 ? (pass.in_progress/pass.total)*100 : 0}%; background:{color}44;"></div>
            <div class="progress-failed"  style="width:{pass.total > 0 ? (pass.failed/pass.total)*100  : 0}%;"></div>
          </div>

          <!-- Counts -->
          <div class="pass-counts">
            <span class="cnt cnt-done" style="color:{color}">{pass.done} done</span>
            {#if pass.in_progress > 0}<span class="cnt cnt-progress">{pass.in_progress} running</span>{/if}
            {#if pass.failed > 0}<span class="cnt cnt-failed">{pass.failed} failed</span>{/if}
            {#if pass.pending > 0}<span class="cnt cnt-pending">{pass.pending} pending</span>{/if}
            <span class="cnt cnt-total">/ {pass.total}</span>
          </div>

          <!-- Error list -->
          {#if pass.errors.length > 0}
            <details class="error-details">
              <summary>{pass.errors.length} failed track{pass.errors.length !== 1 ? 's' : ''}</summary>
              <div class="error-list">
                {#each pass.errors as err}
                  <div class="error-row">
                    <code class="error-path">{err.path.split('/').pop()}</code>
                    {#if err.log}<span class="error-log">{err.log}</span>{/if}
                    {#if err.duration_ms !== null}<span class="error-dur">{formatMs(err.duration_ms)}</span>{/if}
                  </div>
                {/each}
              </div>
            </details>
          {/if}
        </div>
      {/each}
    </div>
  {/if}
</div>

{#if showMetricsDrawer}
  <div class="drawer-overlay" onclick={() => { showMetricsDrawer = false; }}>
    <div class="drawer-content" onclick={(e) => e.stopPropagation()}>
      <div class="drawer-header">
        <div class="drawer-header-left">
          <h3 class="drawer-title">Pipeline Metrics</h3>
          <p class="drawer-subtitle">Inspect performance traces, latency statistics, and diagnostic logs</p>
        </div>
        <button class="drawer-close-btn" onclick={() => { showMetricsDrawer = false; }}>×</button>
      </div>
      <div class="drawer-body" style="overflow-y: auto;">
        <MetricsInspector />
      </div>
    </div>
  </div>
{/if}

<style>
  .analysis-panel {
    display: flex;
    flex-direction: column;
    gap: 0.85rem;
    padding: 1rem 1.25rem;
    height: 100%;
    overflow-y: auto;
    background: var(--sg-surface, #0d1117);
    scrollbar-width: thin;
    scrollbar-color: rgba(255,255,255,0.1) transparent;
  }

  /* ── Header ── */
  .panel-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
    padding: 0.85rem 1rem;
    background: var(--sg-surface-slate, #161b22);
    border: 1px solid rgba(255,255,255,0.07);
    border-radius: 6px;
    flex-shrink: 0;
  }

  .panel-title {
    font-family: "JetBrains Mono", monospace;
    font-size: 12px;
    font-weight: 700;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--sg-on-surface, #e3e1e9);
    margin: 0 0 3px;
  }

  .panel-subtitle {
    font-family: "JetBrains Mono", monospace;
    font-size: 9px;
    color: var(--sg-outline, #849495);
    margin: 0;
    letter-spacing: 0.05em;
  }

  .header-actions {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    flex-shrink: 0;
  }

  .eta-label {
    font-family: "JetBrains Mono", monospace;
    font-size: 10px;
    color: var(--sg-outline, #849495);
  }

  .running-badge {
    display: flex;
    align-items: center;
    gap: 6px;
    font-family: "JetBrains Mono", monospace;
    font-size: 10px;
    font-weight: 700;
    color: var(--sg-primary, #00f0ff);
    padding: 4px 10px;
    border: 1px solid rgba(0,240,255,0.3);
    border-radius: 999px;
    background: rgba(0,240,255,0.07);
  }

  .pulse-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--sg-primary, #00f0ff);
    animation: pulse 1.4s ease-in-out infinite;
  }

  @keyframes pulse {
    0%, 100% { opacity: 1; transform: scale(1); }
    50%       { opacity: 0.4; transform: scale(0.7); }
  }

  .action-btn {
    display: flex;
    align-items: center;
    gap: 5px;
    font-family: "JetBrains Mono", monospace;
    font-size: 10px;
    font-weight: 700;
    padding: 5px 12px;
    border: 1px solid rgba(255,255,255,0.12);
    border-radius: 4px;
    background: rgba(255,255,255,0.04);
    color: var(--sg-outline, #849495);
    cursor: pointer;
    transition: all 0.12s;
  }

  .action-btn:hover:not(:disabled) {
    border-color: rgba(255,255,255,0.25);
    color: var(--sg-on-surface, #e3e1e9);
    background: rgba(255,255,255,0.08);
  }

  .action-btn:disabled { opacity: 0.4; cursor: not-allowed; }

  .action-btn-primary {
    border-color: rgba(0,240,255,0.35);
    color: var(--sg-primary, #00f0ff);
    background: rgba(0,240,255,0.08);
  }

  .action-btn-primary:hover {
    background: rgba(0,240,255,0.14) !important;
    border-color: var(--sg-primary, #00f0ff) !important;
    color: var(--sg-primary, #00f0ff) !important;
  }

  .action-btn-ghost {
    font-family: "JetBrains Mono", monospace;
    font-size: 10px;
    background: none;
    border: none;
    color: var(--sg-outline, #849495);
    cursor: pointer;
    padding: 5px 8px;
  }

  .action-btn-ghost:hover { color: var(--sg-on-surface, #e3e1e9); }

  /* ── Error banner ── */
  .error-banner {
    padding: 0.6rem 1rem;
    border: 1px solid rgba(255,80,80,0.3);
    border-radius: 4px;
    background: rgba(255,80,80,0.08);
    font-family: "JetBrains Mono", monospace;
    font-size: 10px;
    color: #ff6b6b;
  }



  /* ── Model warning ── */
  .model-warning {
    padding: 1rem;
    background: rgba(200,120,0,0.06);
    border: 1px solid rgba(200,120,0,0.25);
    border-left: 3px solid #c87800;
    border-radius: 6px;
    display: flex;
    flex-direction: column;
    gap: 0.85rem;
  }

  .warning-title-row {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .warning-title-text {
    font-family: "JetBrains Mono", monospace;
    font-size: 11px;
    font-weight: 700;
    color: #c87800;
    flex: 1;
  }

  .warning-close {
    background: none;
    border: none;
    color: var(--sg-outline, #849495);
    font-size: 16px;
    cursor: pointer;
    line-height: 1;
    padding: 0 2px;
  }

  .model-groups {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
    gap: 0.65rem;
  }

  .model-group {
    padding: 0.65rem 0.75rem;
    border: 1px solid rgba(255,255,255,0.07);
    border-radius: 5px;
    background: rgba(255,255,255,0.02);
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .group-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 4px;
  }

  .group-label {
    font-family: "JetBrains Mono", monospace;
    font-size: 10px;
    font-weight: 700;
    color: var(--sg-on-surface, #e3e1e9);
  }

  .group-status {
    font-family: "JetBrains Mono", monospace;
    font-size: 8px;
    font-weight: 700;
  }

  .group-ok .group-status   { color: var(--sg-primary, #00f0ff); }
  .group-missing .group-status { color: var(--sg-secondary, #fe00fe); }

  .model-file {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .file-dot {
    width: 5px;
    height: 5px;
    border-radius: 50%;
    flex-shrink: 0;
  }

  .dot-ok      { background: var(--sg-primary, #00f0ff); box-shadow: 0 0 4px rgba(0,240,255,0.5); }
  .dot-missing { background: var(--sg-secondary, #fe00fe); }

  .file-label {
    font-family: "JetBrains Mono", monospace;
    font-size: 9px;
    color: var(--sg-outline, #849495);
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .file-status { font-family: "JetBrains Mono", monospace; font-size: 9px; }
  .file-ok     { color: var(--sg-primary, #00f0ff); }
  .file-missing{ color: var(--sg-secondary, #fe00fe); }

  .warning-footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
    flex-wrap: wrap;
    padding-top: 0.65rem;
    border-top: 1px solid rgba(255,255,255,0.06);
  }



  .warning-actions {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  /* ── Empty state ── */
  .empty-state {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    font-family: "JetBrains Mono", monospace;
    font-size: 11px;
    color: var(--sg-outline, #849495);
    opacity: 0.6;
  }

  /* ── Pass cards ── */
  .passes {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }

  .pass-card {
    padding: 0.85rem 1rem;
    background: var(--sg-surface-slate, #161b22);
    border: 1px solid rgba(255,255,255,0.06);
    border-radius: 6px;
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    transition: border-color 0.2s;
  }

  .pass-active {
    border-color: rgba(0,240,255,0.2);
  }

  .pass-top {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 1rem;
  }

  .pass-name-row {
    display: flex;
    align-items: center;
    gap: 7px;
  }

  .pass-accent {
    width: 3px;
    height: 14px;
    border-radius: 2px;
    flex-shrink: 0;
  }

  .pass-label {
    font-family: "JetBrains Mono", monospace;
    font-size: 11px;
    font-weight: 700;
    color: var(--sg-on-surface, #e3e1e9);
  }

  .pass-running-tag {
    font-family: "JetBrains Mono", monospace;
    font-size: 8px;
    font-weight: 700;
    letter-spacing: 0.08em;
    padding: 2px 6px;
    border-radius: 999px;
    border: 1px solid rgba(0,240,255,0.3);
    color: var(--sg-primary, #00f0ff);
    background: rgba(0,240,255,0.07);
    animation: pulse-border 1.4s ease-in-out infinite;
  }

  @keyframes pulse-border {
    0%, 100% { border-color: rgba(0,240,255,0.3); }
    50%       { border-color: rgba(0,240,255,0.7); }
  }

  .pass-desc {
    font-family: "JetBrains Mono", monospace;
    font-size: 9px;
    color: var(--sg-outline, #849495);
    margin-top: 2px;
    display: block;
  }

  .pass-right {
    display: flex;
    align-items: center;
    gap: 0.65rem;
    flex-shrink: 0;
  }

  .pass-eta {
    font-family: "JetBrains Mono", monospace;
    font-size: 10px;
    color: var(--sg-primary, #00f0ff);
  }

  .pass-avg {
    font-family: "JetBrains Mono", monospace;
    font-size: 9px;
    color: var(--sg-outline, #849495);
    opacity: 0.7;
  }

  .reset-btn {
    font-family: "JetBrains Mono", monospace;
    font-size: 9px;
    padding: 3px 8px;
    border: 1px solid rgba(255,255,255,0.08);
    border-radius: 3px;
    background: transparent;
    color: var(--sg-outline, #849495);
    cursor: pointer;
    transition: all 0.12s;
  }

  .reset-btn:hover {
    border-color: rgba(0,240,255,0.3);
    color: var(--sg-primary, #00f0ff);
  }

  /* ── Progress bar ── */
  .progress-track {
    height: 3px;
    background: rgba(255,255,255,0.06);
    border-radius: 2px;
    display: flex;
    overflow: hidden;
  }

  .progress-done    { height: 100%; transition: width 0.3s ease; }
  .progress-running { height: 100%; transition: width 0.3s ease; }
  .progress-failed  { height: 100%; background: #ff6b6b; transition: width 0.3s ease; }

  /* ── Counts ── */
  .pass-counts {
    display: flex;
    gap: 0.5rem;
    flex-wrap: wrap;
  }

  .cnt {
    font-family: "JetBrains Mono", monospace;
    font-size: 9px;
  }

  .cnt-progress { color: var(--sg-outline, #849495); }
  .cnt-failed   { color: #ff6b6b; }
  .cnt-pending  { color: var(--sg-outline, #849495); opacity: 0.6; }
  .cnt-total    { color: var(--sg-outline, #849495); opacity: 0.4; }

  /* ── Error details ── */
  .error-details {
    border-top: 1px solid rgba(255,255,255,0.05);
    padding-top: 0.4rem;
  }

  .error-details summary {
    font-family: "JetBrains Mono", monospace;
    font-size: 9px;
    color: #ff6b6b;
    cursor: pointer;
    padding: 2px 0;
  }

  .error-list {
    display: flex;
    flex-direction: column;
    gap: 3px;
    margin-top: 4px;
    padding-left: 0.5rem;
  }

  .error-row {
    display: flex;
    align-items: baseline;
    gap: 0.6rem;
    flex-wrap: wrap;
  }

  .error-path {
    font-family: "JetBrains Mono", monospace;
    font-size: 9px;
    color: var(--sg-on-surface, #e3e1e9);
    max-width: 260px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .error-log {
    font-family: "JetBrains Mono", monospace;
    font-size: 9px;
    color: #ff6b6b;
    flex: 1;
  }

  .error-dur {
    font-family: "JetBrains Mono", monospace;
    font-size: 9px;
    color: var(--sg-outline, #849495);
    flex-shrink: 0;
  }

  /* ── Update Banner Card ── */
  .update-banner-card {
    padding: 1rem;
    background: rgba(0, 240, 255, 0.05);
    border: 1px solid rgba(0, 240, 255, 0.2);
    border-left: 3px solid var(--sg-primary, #00f0ff);
    border-radius: 6px;
    display: flex;
    flex-direction: column;
    gap: 0.65rem;
  }

  .update-desc-text {
    margin: 0;
    font-family: "JetBrains Mono", monospace;
    font-size: 10px;
    color: var(--sg-on-surface, #e3e1e9);
    opacity: 0.85;
    line-height: 1.5;
  }

  .update-actions {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    flex-wrap: wrap;
    margin-top: 0.25rem;
  }

  .action-btn-danger {
    border-color: rgba(255, 80, 80, 0.3) !important;
    color: #ff6b6b !important;
    background: rgba(255, 80, 80, 0.05) !important;
  }

  .action-btn-danger:hover {
    background: rgba(255, 80, 80, 0.12) !important;
    border-color: #ff6b6b !important;
  }

  .update-close {
    background: none;
    border: none;
    color: var(--sg-outline, #849495);
    font-size: 16px;
    cursor: pointer;
    line-height: 1;
    padding: 0 2px;
    transition: color 0.12s;
  }

  .update-close:hover {
    color: var(--sg-on-surface, #e3e1e9);
  }

  /* ── Drawer ── */
  .drawer-overlay {
    position: fixed;
    top: 0;
    left: 0;
    width: 100vw;
    height: 100vh;
    background: rgba(0, 0, 0, 0.6);
    backdrop-filter: blur(4px);
    z-index: 1000;
    display: flex;
    justify-content: flex-end;
  }

  .drawer-content {
    width: 850px;
    max-width: 95vw;
    height: 100%;
    background: var(--sg-surface, #0d1117);
    border-left: 1px solid rgba(255, 255, 255, 0.08);
    display: flex;
    flex-direction: column;
    box-shadow: -10px 0 30px rgba(0, 0, 0, 0.5);
    animation: slide-in 0.25s cubic-bezier(0.16, 1, 0.3, 1) forwards;
  }

  @keyframes slide-in {
    from { transform: translateX(100%); }
    to { transform: translateX(0); }
  }

  .drawer-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 1.25rem;
    border-bottom: 1px solid rgba(255, 255, 255, 0.06);
    flex-shrink: 0;
  }

  .drawer-header-left {
    display: flex;
    flex-direction: column;
    gap: 3px;
  }

  .drawer-title {
    font-family: "JetBrains Mono", monospace;
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--sg-on-surface, #e3e1e9);
    margin: 0;
  }

  .drawer-subtitle {
    font-family: "JetBrains Mono", monospace;
    font-size: 9px;
    color: var(--sg-outline, #849495);
    margin: 0;
  }

  .drawer-close-btn {
    background: none;
    border: none;
    color: var(--sg-outline, #849495);
    font-size: 22px;
    cursor: pointer;
    line-height: 1;
    padding: 0 6px;
    transition: color 0.12s;
  }

  .drawer-close-btn:hover {
    color: var(--sg-on-surface, #e3e1e9);
  }

  .drawer-body {
    flex: 1;
    padding: 1.25rem;
    overflow-y: auto;
  }
</style>
