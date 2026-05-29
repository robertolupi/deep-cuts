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

  async function loadStats() {
    try {
      stats = await invoke<PassStats[]>("get_pass_stats");
    } catch (e: any) {
      console.error("Failed to load pass stats:", e);
    }
  }

  async function checkRunning() {
    isRunning = await invoke<boolean>("is_analysis_running");
  }

  async function startAnalysis() {
    errorMessage = "";
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

  onMount(() => {
    checkRunning();
    loadStats();

    listen("analysis-progress", () => { loadStats(); }).then(u => unlisteners.push(u));
    listen("analysis-complete", () => { isRunning = false; loadStats(); }).then(u => unlisteners.push(u));
  });

  onDestroy(() => {
    unlisteners.forEach(u => u());
  });
</script>

<div class="analysis-panel">
  <div class="analysis-header glass-panel">
    <div>
      <h2 class="panel-title">Audio Analysis</h2>
      <p class="panel-subtitle">Compute BPM, key, loudness, waveforms, and neural embeddings for your library.</p>
    </div>
    <div class="header-actions">
      {#if isRunning}
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
</style>
