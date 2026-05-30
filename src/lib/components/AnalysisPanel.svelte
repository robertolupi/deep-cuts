<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { onMount, onDestroy } from "svelte";

  interface PassError {
    path: string;
    log: string | null;
    duration_ms: number | null;
    last_run_at: string | null;
  }

  // Mirrors pass_status constants in database.rs
  const PassStatus = { PENDING: 0, IN_PROGRESS: 1, DONE: 2, FAILED: 3 } as const;

  interface PassStats {
    pass_name: string;
    pending: number;
    in_progress: number;
    done: number;
    failed: number;
    total: number;
    avg_duration_ms: number | null;
    errors: PassError[];
  }

  let stats = $state<PassStats[]>([]);
  let isRunning = $state(false);
  let errorMessage = $state("");
  let unlisteners: Array<() => void> = [];

  // Model existence states
  interface ModelExistence {
    qwen_model: boolean;
    qwen_mmproj: boolean;
    sentence_model: boolean;
    sentence_tok: boolean;
    clap_model: boolean;
    clap_mel: boolean;
    essentia_base: boolean;
    essentia_base_json: boolean;
    essentia_heads: boolean;
    qwen_exists: boolean;
    sentence_exists: boolean;
    clap_exists: boolean;
    essentia_exists: boolean;
    all_exist: boolean;
  }

  let modelStatus = $state<ModelExistence | null>(null);
  let isCheckingModels = $state(false);
  let showModelWarning = $state(false);
  let hasCopiedCommand = $state(false);
  let warningDismissed = $state(false);

  async function checkModels() {
    isCheckingModels = true;
    try {
      const status = await invoke<ModelExistence>("check_models_exist");
      modelStatus = status;
      if (!status.all_exist && !warningDismissed) {
        showModelWarning = true;
      } else if (status.all_exist) {
        showModelWarning = false;
      }
    } catch (e) {
      console.error("Failed to check model existence:", e);
    } finally {
      isCheckingModels = false;
    }
  }

  function copyCommand() {
    navigator.clipboard.writeText("python3 tools/download_models.py");
    hasCopiedCommand = true;
    setTimeout(() => {
      hasCopiedCommand = false;
    }, 2000);
  }

  function dismissWarning() {
    showModelWarning = false;
    warningDismissed = true;
  }

  // Per-pass throughput tracking: records the done count and wall-clock time at the moment
  // a pass first starts completing tracks, so we can compute actual completions/ms.
  interface ThroughputSample { time: number; done: number; }
  let throughputBaseline = new Map<string, ThroughputSample>();

  function updateThroughput(newStats: PassStats[]) {
    const now = Date.now();
    for (const pass of newStats) {
      const existing = throughputBaseline.get(pass.pass_name);
      if (pass.done > 0 && pass.in_progress > 0) {
        // Pass is actively running — seed baseline on first completion
        if (!existing) {
          throughputBaseline.set(pass.pass_name, { time: now, done: pass.done });
        }
      } else if (pass.in_progress === 0) {
        // Pass finished or not started — clear baseline so it reseeds next run
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
      if (completed > 0 && elapsedMs > 0) {
        const throughputPerMs = completed / elapsedMs;
        return remaining / throughputPerMs;
      }
    }

    // Fallback to avg_duration_ms before any completions arrive
    if (pass.avg_duration_ms) return remaining * pass.avg_duration_ms;
    return 0;
  }

  // Derived state to compute total remaining ETA across all active/pending passes
  let estimatedTimeRemaining = $derived.by(() => {
    let totalMs = 0;
    for (const pass of stats) {
      totalMs += etaForPass(pass);
    }
    return totalMs;
  });

  function formatEta(ms: number | null): string {
    if (ms === null || ms <= 0) return "";
    const seconds = Math.ceil(ms / 1000);
    if (seconds < 60) return `${seconds}s`;
    const mins = Math.floor(seconds / 60);
    const secs = seconds % 60;
    if (secs === 0) return `${mins}m`;
    return `${mins}m ${secs}s`;
  }

  async function loadStats() {
    try {
      const newStats = await invoke<PassStats[]>("get_pass_stats");
      updateThroughput(newStats);
      stats = newStats;
    } catch (e: any) {
      console.error("Failed to load pass stats:", e);
    }
  }

  async function checkRunning() {
    isRunning = await invoke<boolean>("is_analysis_running");
  }

  async function startAnalysis() {
    errorMessage = "";
    throughputBaseline.clear();
    try {
      await invoke("run_analysis_pipeline");
      isRunning = true;
    } catch (e: any) {
      errorMessage = e?.toString() ?? "Unknown error";
    }
  }

  async function resetAll() {
    try {
      await invoke("reset_all_passes");
      await loadStats();
    } catch (e: any) {
      errorMessage = e?.toString() ?? "Unknown error";
    }
  }

  async function resetPass(passName: string) {
    try {
      await invoke("reset_pass", { passName });
      await loadStats();
    } catch (e: any) {
      errorMessage = e?.toString() ?? "Unknown error";
    }
  }

  function formatMs(ms: number | null): string {
    if (ms === null) return "—";
    if (ms < 1000) return `${Math.round(ms)}ms`;
    return `${(ms / 1000).toFixed(1)}s`;
  }

  let checkInterval: any;

  onMount(() => {
    checkRunning();
    loadStats();
    checkModels();

    // Auto-check models every 5 seconds if warning is visible and no active analysis is running
    checkInterval = setInterval(() => {
      if (showModelWarning && !isCheckingModels && !isRunning) {
        checkModels();
      }
    }, 5000);

    listen("analysis-progress", () => { loadStats(); }).then(u => unlisteners.push(u));
    listen("analysis-complete", () => { isRunning = false; loadStats(); }).then(u => unlisteners.push(u));
  });

  onDestroy(() => {
    unlisteners.forEach(u => u());
    if (checkInterval) {
      clearInterval(checkInterval);
    }
  });
</script>

<div class="analysis-panel">
  <div class="analysis-header glass-panel">
    <div>
      <h2 class="panel-title">Audio Analysis</h2>
      <p class="panel-subtitle">Compute BPM, key, loudness, waveforms, and neural embeddings for your library.</p>
    </div>
    <div class="header-actions" style="display: flex; align-items: center; gap: 0.75rem;">
      {#if isRunning}
        {#if estimatedTimeRemaining > 0}
          <span class="eta-global" style="font-size: 0.8rem; color: var(--text-secondary); font-family: var(--font-mono, monospace);">
            ⏱️ {formatEta(estimatedTimeRemaining)} remaining
          </span>
        {/if}
        <span class="badge badge-cyan pulse-glow-cyan">Running…</span>
      {:else}
        <button class="btn-primary" onclick={startAnalysis}>
          Run Analysis
        </button>
        {#if stats.length > 0}
          <button class="btn-secondary" onclick={resetAll} style="margin-left: 0.5rem;">
            Reset All
          </button>
        {/if}
      {/if}
    </div>
  </div>

  {#if showModelWarning && modelStatus}
    <div class="model-warning-pane glass-panel">
      <div class="warning-pane-header">
        <div class="warning-pane-title-area">
          <h3 class="warning-title">
            <span class="warning-icon">⚠️</span>
            Neural Network Models Check — Missing Files Detected
          </h3>
          <p class="warning-desc">
            Deep Cuts relies on locally executed neural network models to run classification, acoustic mapping, and audio description passes. Some required files are missing from your local directory structure.
          </p>
        </div>
        <button class="warning-dismiss-btn" onclick={dismissWarning} title="Dismiss Warning">
          ✕
        </button>
      </div>

      <div class="model-groups-grid">
        <!-- GROUP 1: Essentia Models -->
        <div class="model-group-card">
          <div class="model-group-header">
            <div class="model-group-info">
              <span class="model-group-name">🎧 Essentia Acoustic Classifier</span>
              <span class="model-group-feature">Enables BPM, genre, mood, & vocal state detection</span>
            </div>
            <span class="badge-status {modelStatus.essentia_exists ? 'badge-status-ok' : 'badge-status-missing'}">
              {modelStatus.essentia_exists ? '● READY' : '▲ INCOMPLETE'}
            </span>
          </div>
          <div class="model-files-list">
            <div class="model-file-item">
              <span class="model-file-name" title="discogs-effnet-bsdynamic-1.onnx">discogs-effnet base model</span>
              <span class="file-status-dot">
                <span class="dot {modelStatus.essentia_base ? 'dot-ok' : 'dot-missing'}"></span>
                <span class="{modelStatus.essentia_base ? 'text-ok' : 'text-missing'}">
                  {modelStatus.essentia_base ? 'Found' : 'Missing'}
                </span>
              </span>
            </div>
            <div class="model-file-item">
              <span class="model-file-name" title="discogs-effnet-bsdynamic-1.json">discogs-effnet labels</span>
              <span class="file-status-dot">
                <span class="dot {modelStatus.essentia_base_json ? 'dot-ok' : 'dot-missing'}"></span>
                <span class="{modelStatus.essentia_base_json ? 'text-ok' : 'text-missing'}">
                  {modelStatus.essentia_base_json ? 'Found' : 'Missing'}
                </span>
              </span>
            </div>
            <div class="model-file-item">
              <span class="model-file-name" title="9 classification head model files">9 task heads & labels</span>
              <span class="file-status-dot">
                <span class="dot {modelStatus.essentia_heads ? 'dot-ok' : 'dot-missing'}"></span>
                <span class="{modelStatus.essentia_heads ? 'text-ok' : 'text-missing'}">
                  {modelStatus.essentia_heads ? 'Found' : 'Missing'}
                </span>
              </span>
            </div>
          </div>
        </div>

        <!-- GROUP 2: CLAP Models -->
        <div class="model-group-card">
          <div class="model-group-header">
            <div class="model-group-info">
              <span class="model-group-name">🗺️ CLAP Acoustic Embedder</span>
              <span class="model-group-feature">Enables acoustic mapping on UMAP music projection</span>
            </div>
            <span class="badge-status {modelStatus.clap_exists ? 'badge-status-ok' : 'badge-status-missing'}">
              {modelStatus.clap_exists ? '● READY' : '▲ INCOMPLETE'}
            </span>
          </div>
          <div class="model-files-list">
            <div class="model-file-item">
              <span class="model-file-name" title="clap_audio_encoder.onnx">clap_audio_encoder.onnx</span>
              <span class="file-status-dot">
                <span class="dot {modelStatus.clap_model ? 'dot-ok' : 'dot-missing'}"></span>
                <span class="{modelStatus.clap_model ? 'text-ok' : 'text-missing'}">
                  {modelStatus.clap_model ? 'Found' : 'Missing'}
                </span>
              </span>
            </div>
            <div class="model-file-item">
              <span class="model-file-name" title="clap_mel_weights.bin">clap_mel_weights.bin</span>
              <span class="file-status-dot">
                <span class="dot {modelStatus.clap_mel ? 'dot-ok' : 'dot-missing'}"></span>
                <span class="{modelStatus.clap_mel ? 'text-ok' : 'text-missing'}">
                  {modelStatus.clap_mel ? 'Found' : 'Missing'}
                </span>
              </span>
            </div>
          </div>
        </div>

        <!-- GROUP 3: Qwen Listener -->
        <div class="model-group-card">
          <div class="model-group-header">
            <div class="model-group-info">
              <span class="model-group-name">🤖 Qwen2-Audio Listener</span>
              <span class="model-group-feature">Enables prose descriptive text generation</span>
            </div>
            <span class="badge-status {modelStatus.qwen_exists ? 'badge-status-ok' : 'badge-status-missing'}">
              {modelStatus.qwen_exists ? '● READY' : '▲ INCOMPLETE'}
            </span>
          </div>
          <div class="model-files-list">
            <div class="model-file-item">
              <span class="model-file-name" title="Qwen2-Audio-7B-Instruct.Q4_K_M.gguf">Audio LLM GGUF (4.7GB)</span>
              <span class="file-status-dot">
                <span class="dot {modelStatus.qwen_model ? 'dot-ok' : 'dot-missing'}"></span>
                <span class="{modelStatus.qwen_model ? 'text-ok' : 'text-missing'}">
                  {modelStatus.qwen_model ? 'Found' : 'Missing'}
                </span>
              </span>
            </div>
            <div class="model-file-item">
              <span class="model-file-name" title="Qwen2-Audio-7B-Instruct.mmproj-Q8_0.gguf">mmproj projection (0.3GB)</span>
              <span class="file-status-dot">
                <span class="dot {modelStatus.qwen_mmproj ? 'dot-ok' : 'dot-missing'}"></span>
                <span class="{modelStatus.qwen_mmproj ? 'text-ok' : 'text-missing'}">
                  {modelStatus.qwen_mmproj ? 'Found' : 'Missing'}
                </span>
              </span>
            </div>
          </div>
        </div>

        <!-- GROUP 4: Description Embedder -->
        <div class="model-group-card">
          <div class="model-group-header">
            <div class="model-group-info">
              <span class="model-group-name">📝 MiniLM Text Embedder</span>
              <span class="model-group-feature">Enables prose description embedding vector indexing</span>
            </div>
            <span class="badge-status {modelStatus.sentence_exists ? 'badge-status-ok' : 'badge-status-missing'}">
              {modelStatus.sentence_exists ? '● READY' : '▲ INCOMPLETE'}
            </span>
          </div>
          <div class="model-files-list">
            <div class="model-file-item">
              <span class="model-file-name" title="all-minilm-l6-v2.onnx">all-minilm-l6-v2.onnx</span>
              <span class="file-status-dot">
                <span class="dot {modelStatus.sentence_model ? 'dot-ok' : 'dot-missing'}"></span>
                <span class="{modelStatus.sentence_model ? 'text-ok' : 'text-missing'}">
                  {modelStatus.sentence_model ? 'Found' : 'Missing'}
                </span>
              </span>
            </div>
            <div class="model-file-item">
              <span class="model-file-name" title="all-minilm-l6-v2-tokenizer.json">all-minilm tokenizer</span>
              <span class="file-status-dot">
                <span class="dot {modelStatus.sentence_tok ? 'dot-ok' : 'dot-missing'}"></span>
                <span class="{modelStatus.sentence_tok ? 'text-ok' : 'text-missing'}">
                  {modelStatus.sentence_tok ? 'Found' : 'Missing'}
                </span>
              </span>
            </div>
          </div>
        </div>
      </div>

      <div class="warning-actions-row">
        <div class="terminal-command-box">
          <span class="command-text">python3 tools/download_models.py</span>
          <button class="btn-copy-cmd {hasCopiedCommand ? 'copied' : ''}" onclick={copyCommand}>
            {hasCopiedCommand ? '✓ Copied' : '❐ Copy Command'}
          </button>
        </div>

        <div class="warning-control-buttons">
          <button class="btn-check-again" onclick={checkModels} disabled={isCheckingModels}>
            {#if isCheckingModels}
              <span class="spin-icon">⏳</span> Checking...
            {:else}
              <span>🔄 Check Status</span>
            {/if}
          </button>
          <button class="btn-dismiss-warn" onclick={dismissWarning}>
            Proceed Anyway
          </button>
        </div>
      </div>
    </div>
  {/if}

  {#if errorMessage}
    <div class="error-banner">{errorMessage}</div>
  {/if}

  {#if stats.length === 0}
    <div class="empty-state glass-panel">
      <p>No analysis data yet. Run analysis to get started.</p>
    </div>
  {:else}
    {#each stats as pass (pass.pass_name)}
      <div class="pass-card glass-panel">
        <div class="pass-header">
          <div class="pass-title-row">
            <span class="pass-name">{pass.pass_name}</span>
            <span class="pass-counts">
              <span class="count-done">{pass.done} done</span>
              {#if pass.in_progress > 0}<span class="count-progress"> · {pass.in_progress} running</span>{/if}
              {#if pass.failed > 0}<span class="count-failed"> · {pass.failed} failed</span>{/if}
              {#if pass.pending > 0}<span class="count-pending"> · {pass.pending} pending</span>{/if}
              <span class="count-total"> / {pass.total}</span>
              {#if isRunning && (pass.pending > 0 || pass.in_progress > 0)}
                {@const eta = etaForPass(pass)}
                {#if eta > 0}
                  <span class="count-eta" style="color: var(--accent-cyan); font-weight: 500;">
                    · {formatEta(eta)} remaining
                  </span>
                {/if}
              {/if}
            </span>
            {#if pass.avg_duration_ms !== null}
              <span class="avg-duration">avg {formatMs(pass.avg_duration_ms)}</span>
            {/if}
          </div>
          <div class="progress-bar-track">
            <div
              class="progress-bar-done"
              style="width: {pass.total > 0 ? (pass.done / pass.total) * 100 : 0}%"
            ></div>
            <div
              class="progress-bar-running"
              style="width: {pass.total > 0 ? (pass.in_progress / pass.total) * 100 : 0}%"
            ></div>
            <div
              class="progress-bar-failed"
              style="width: {pass.total > 0 ? (pass.failed / pass.total) * 100 : 0}%"
            ></div>
          </div>
          {#if !isRunning}
            <button class="btn-ghost-sm" onclick={() => resetPass(pass.pass_name)}>Reset</button>
          {/if}
        </div>

        {#if pass.errors.length > 0}
          <details class="error-details">
            <summary>{pass.errors.length} failed track{pass.errors.length !== 1 ? 's' : ''}</summary>
            <div class="error-list">
              {#each pass.errors as err}
                <div class="error-row">
                  <code class="error-path" title={err.path}>{err.path.split('/').pop()}</code>
                  {#if err.log}
                    <span class="error-log">{err.log}</span>
                  {/if}
                  {#if err.duration_ms !== null}
                    <span class="error-dur">{formatMs(err.duration_ms)}</span>
                  {/if}
                </div>
              {/each}
            </div>
          </details>
        {/if}
      </div>
    {/each}
  {/if}
</div>

<style>
  .analysis-panel {
    display: flex;
    flex-direction: column;
    gap: 1rem;
    padding: 1.25rem;
    height: 100%;
    overflow-y: auto;
  }

  .analysis-header {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    padding: 1rem 1.25rem;
  }

  .panel-title {
    font-size: 1.1rem;
    font-weight: 600;
    margin: 0 0 0.25rem 0;
    color: var(--text-primary);
  }

  .panel-subtitle {
    font-size: 0.8rem;
    color: var(--text-secondary);
    margin: 0;
  }

  .header-actions {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    flex-shrink: 0;
  }

  .error-banner {
    background: rgba(255, 80, 80, 0.12);
    border: 1px solid rgba(255, 80, 80, 0.3);
    border-radius: var(--radius-sm);
    padding: 0.6rem 1rem;
    font-size: 0.8rem;
    color: var(--text-secondary);
  }

  .empty-state {
    padding: 2rem;
    text-align: center;
    color: var(--text-secondary);
    font-size: 0.85rem;
  }

  .pass-card {
    padding: 1rem 1.25rem;
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
  }

  .pass-header {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
  }

  .pass-title-row {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    flex-wrap: wrap;
  }

  .pass-name {
    font-size: 0.85rem;
    font-weight: 600;
    color: var(--text-primary);
    font-family: var(--font-mono, monospace);
  }

  .pass-counts {
    font-size: 0.78rem;
    color: var(--text-secondary);
  }

  .count-done { color: var(--accent-cyan); }
  .count-progress { color: var(--text-secondary); }
  .count-failed { color: #ff6b6b; }
  .count-pending { color: var(--text-secondary); }
  .count-total { color: var(--text-secondary); }

  .avg-duration {
    font-size: 0.72rem;
    color: var(--text-secondary);
    margin-left: auto;
  }

  .progress-bar-track {
    height: 4px;
    background: var(--border-color);
    border-radius: 2px;
    overflow: hidden;
    position: relative;
    display: flex;
  }

  .progress-bar-done {
    background: var(--accent-cyan);
    height: 100%;
    transition: width 0.3s ease;
  }

  .progress-bar-running {
    background: rgba(var(--accent-cyan-rgb, 0, 200, 200), 0.4);
    height: 100%;
    transition: width 0.3s ease;
  }

  .progress-bar-failed {
    background: #ff6b6b;
    height: 100%;
    transition: width 0.3s ease;
  }

  .btn-ghost-sm {
    font-size: 0.72rem;
    padding: 0.2rem 0.6rem;
    border-radius: var(--radius-sm);
    background: transparent;
    border: 1px solid var(--border-color);
    color: var(--text-secondary);
    cursor: pointer;
    align-self: flex-end;
    transition: border-color 0.15s, color 0.15s;
  }

  .btn-ghost-sm:hover {
    border-color: var(--accent-cyan);
    color: var(--accent-cyan);
  }

  .error-details {
    font-size: 0.78rem;
    color: var(--text-secondary);
  }

  .error-details summary {
    cursor: pointer;
    color: #ff6b6b;
    padding: 0.2rem 0;
  }

  .error-list {
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
    margin-top: 0.4rem;
    padding-left: 0.5rem;
  }

  .error-row {
    display: flex;
    align-items: flex-start;
    gap: 0.75rem;
    flex-wrap: wrap;
  }

  .error-path {
    font-size: 0.75rem;
    color: var(--text-primary);
    max-width: 280px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .error-log {
    font-size: 0.72rem;
    color: #ff6b6b;
    flex: 1;
  }

  .error-dur {
    font-size: 0.72rem;
    color: var(--text-secondary);
    flex-shrink: 0;
  }

  /* Model warning panel styles */
  .model-warning-pane {
    padding: 1.5rem;
    background: rgba(255, 110, 0, 0.04);
    border: 2px solid rgba(255, 110, 0, 0.15);
    position: relative;
    overflow: hidden;
  }

  .model-warning-pane::before {
    content: '';
    position: absolute;
    top: 0;
    left: 0;
    width: 4px;
    height: 100%;
    background: linear-gradient(to bottom, var(--color-accent-yellow), var(--color-accent-magenta));
  }

  .warning-pane-header {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    margin-bottom: 1rem;
    gap: 1rem;
  }

  .warning-pane-title-area {
    display: flex;
    flex-direction: column;
  }

  .warning-title {
    font-size: 1.05rem;
    font-weight: 700;
    color: var(--text-primary);
    display: flex;
    align-items: center;
    gap: 0.5rem;
    margin: 0;
  }

  .warning-title span.warning-icon {
    font-size: 1.2rem;
    color: var(--color-accent-yellow);
    filter: drop-shadow(0 0 4px rgba(249, 217, 118, 0.4));
  }

  .warning-desc {
    font-size: 0.82rem;
    color: var(--text-secondary);
    line-height: 1.5;
    margin-top: 0.35rem;
    max-width: 800px;
  }

  .warning-dismiss-btn {
    background: transparent;
    border: none;
    color: var(--text-muted);
    font-size: 1.1rem;
    cursor: pointer;
    padding: 0.25rem;
    line-height: 1;
    border-radius: var(--radius-sm);
    transition: var(--transition-fast);
  }

  .warning-dismiss-btn:hover {
    color: var(--text-primary);
    background: rgba(255, 255, 255, 0.05);
  }

  .model-groups-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(240px, 1fr));
    gap: 1rem;
    margin: 1.25rem 0;
  }

  .model-group-card {
    background: rgba(255, 255, 255, 0.02);
    border: 1px solid var(--border-color);
    border-radius: var(--radius-md);
    padding: 1rem;
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
    transition: var(--transition-smooth);
  }

  .model-group-card:hover {
    background: rgba(255, 255, 255, 0.04);
    border-color: rgba(255, 255, 255, 0.15);
  }

  .model-group-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 0.5rem;
  }

  .model-group-info {
    display: flex;
    flex-direction: column;
    gap: 0.1rem;
  }

  .model-group-name {
    font-size: 0.82rem;
    font-weight: 700;
    color: var(--text-primary);
  }

  .model-group-feature {
    font-size: 0.72rem;
    color: var(--text-muted);
  }

  /* Compact status badges for groups */
  .badge-status {
    font-size: 0.65rem;
    padding: 0.15rem 0.45rem;
    border-radius: 4px;
    font-weight: 700;
    letter-spacing: 0.02em;
  }

  .badge-status-ok {
    background: rgba(0, 242, 254, 0.08);
    color: var(--color-accent-cyan);
    border: 1px solid rgba(0, 242, 254, 0.2);
    box-shadow: 0 0 10px rgba(0, 242, 254, 0.1);
  }

  .badge-status-missing {
    background: rgba(255, 0, 127, 0.08);
    color: var(--color-accent-magenta);
    border: 1px solid rgba(255, 0, 127, 0.2);
    box-shadow: 0 0 10px rgba(255, 0, 127, 0.1);
  }

  .model-files-list {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
    border-top: 1px solid rgba(255, 255, 255, 0.04);
    padding-top: 0.6rem;
  }

  .model-file-item {
    display: flex;
    align-items: center;
    justify-content: space-between;
    font-size: 0.75rem;
  }

  .model-file-name {
    font-family: var(--font-mono, monospace);
    color: var(--text-secondary);
    max-width: 190px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .file-status-dot {
    display: flex;
    align-items: center;
    gap: 0.35rem;
    font-weight: 500;
  }

  .dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
  }

  .dot-ok {
    background-color: var(--color-accent-cyan);
    box-shadow: 0 0 6px var(--color-accent-cyan);
  }

  .dot-missing {
    background-color: var(--color-accent-magenta);
    box-shadow: 0 0 6px var(--color-accent-magenta);
  }

  .text-ok {
    color: var(--color-accent-cyan);
  }

  .text-missing {
    color: var(--color-accent-magenta);
  }

  /* Action container in warning pane */
  .warning-actions-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    flex-wrap: wrap;
    gap: 1rem;
    border-top: 1px solid rgba(255, 255, 255, 0.06);
    padding-top: 1rem;
    margin-top: 0.5rem;
  }

  .terminal-command-box {
    display: flex;
    align-items: center;
    background: rgba(0, 0, 0, 0.25);
    border: 1px solid var(--border-color);
    border-radius: var(--radius-sm);
    padding: 0.4rem 0.75rem;
    font-family: var(--font-mono, monospace);
    font-size: 0.78rem;
    color: var(--color-accent-yellow);
    max-width: 500px;
    flex: 1;
    min-width: 280px;
    justify-content: space-between;
  }

  .command-text {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    user-select: all;
  }

  .btn-copy-cmd {
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.1);
    color: var(--text-secondary);
    padding: 0.2rem 0.5rem;
    border-radius: 4px;
    font-size: 0.7rem;
    font-family: 'Inter', sans-serif;
    font-weight: 500;
    cursor: pointer;
    transition: var(--transition-fast);
    display: flex;
    align-items: center;
    gap: 0.3rem;
  }

  .btn-copy-cmd:hover {
    color: var(--text-primary);
    background: rgba(255, 255, 255, 0.1);
    border-color: rgba(255, 255, 255, 0.2);
  }

  .btn-copy-cmd.copied {
    color: var(--color-accent-cyan);
    background: rgba(0, 242, 254, 0.08);
    border-color: rgba(0, 242, 254, 0.3);
  }

  .warning-control-buttons {
    display: flex;
    align-items: center;
    gap: 0.75rem;
  }

  .btn-check-again {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    background: rgba(255, 255, 255, 0.04);
    border: 1px solid var(--border-color);
    color: var(--text-primary);
    font-size: 0.78rem;
    font-weight: 600;
    padding: 0.45rem 0.9rem;
    border-radius: var(--radius-sm);
    cursor: pointer;
    transition: var(--transition-fast);
  }

  .btn-check-again:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.08);
    border-color: var(--border-color-hover);
  }

  .btn-check-again:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }

  .spin-icon {
    display: inline-block;
    animation: icon-spin 1s infinite linear;
  }

  @keyframes icon-spin {
    0% { transform: rotate(0deg); }
    100% { transform: rotate(360deg); }
  }

  .btn-dismiss-warn {
    background: transparent;
    border: 1px solid transparent;
    color: var(--text-secondary);
    font-size: 0.78rem;
    font-weight: 500;
    padding: 0.45rem 0.9rem;
    border-radius: var(--radius-sm);
    cursor: pointer;
    transition: var(--transition-fast);
  }

  .btn-dismiss-warn:hover {
    color: var(--text-primary);
    background: rgba(255, 255, 255, 0.04);
  }
</style>
