<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { onDestroy } from 'svelte';
  import { library } from '$lib/stores/library.svelte';
  import { player } from '$lib/stores/player.svelte';
  import { ui } from '$lib/stores/ui.svelte';

  interface DuplicatePair {
    id_a: number;
    id_b: number;
    title_a: string | null;
    title_b: string | null;
    artist_a: string | null;
    artist_b: string | null;
    filename_a: string;
    filename_b: string;
    distance: number;
  }

  interface ProgressPayload {
    stage: string;
    done?: number;
    total?: number;
    n?: number;
  }

  let threshold = $state(0.15);
  let pairs = $state<DuplicatePair[]>([]);
  let isScanning = $state(false);
  let hasScanned = $state(false);
  let progressDone = $state(0);
  let progressTotal = $state(0);

  // similarity % = (1 − distance / sqrt(2)) × 100, clamped to [0,100]
  function similarity(distance: number): number {
    return Math.round(Math.max(0, Math.min(100, (1 - distance / Math.SQRT2) * 100)));
  }

  function displayName(pair: DuplicatePair, side: 'a' | 'b'): string {
    const title  = side === 'a' ? pair.title_a  : pair.title_b;
    const artist = side === 'a' ? pair.artist_a : pair.artist_b;
    const file   = side === 'a' ? pair.filename_a : pair.filename_b;
    if (title && artist) return `${artist} — ${title}`;
    if (title) return title;
    return file.split('/').pop() ?? file;
  }

  function playTrack(id: number) {
    const track = library.tracks.find(t => t.id === id);
    if (track) player.playTrack(track);
  }

  let unlistenProgress: (() => void) | null = null;
  let unlistenDone: (() => void) | null = null;

  async function scan() {
    isScanning = true;
    hasScanned = false;
    pairs = [];
    progressDone = 0;
    progressTotal = 0;

    unlistenProgress?.();
    unlistenDone?.();

    unlistenProgress = await listen<ProgressPayload>('duplicate-scan-progress', (e) => {
      if (e.payload.total) progressTotal = e.payload.total;
      if (e.payload.done)  progressDone  = e.payload.done;
      else if (e.payload.n) progressTotal = e.payload.n;
    });

    unlistenDone = await listen('duplicate-scan-done', () => {
      isScanning = false;
      hasScanned = true;
      unlistenProgress?.();
      unlistenDone?.();
    });

    try {
      pairs = await invoke<DuplicatePair[]>('find_duplicate_pairs', { threshold });
      // done event may arrive before invoke resolves; ensure we mark finished
      isScanning = false;
      hasScanned = true;
    } catch (err: any) {
      ui.showToast(err.toString(), 'error');
      isScanning = false;
    }
  }

  onDestroy(() => {
    unlistenProgress?.();
    unlistenDone?.();
  });
</script>

<div class="duplicates-panel">
  <!-- Toolbar -->
  <div class="dup-toolbar">
    <span class="dup-title">DUPLICATE DETECTION</span>

    <div class="dup-control">
      <label class="dup-label" for="threshold-slider">THRESHOLD</label>
      <input
        id="threshold-slider"
        type="range"
        min="0.05"
        max="0.50"
        step="0.01"
        bind:value={threshold}
        class="dup-slider"
        disabled={isScanning}
      />
      <span class="dup-threshold-val">{threshold.toFixed(2)}</span>
    </div>

    <button class="dup-scan-btn" onclick={scan} disabled={isScanning}>
      {#if isScanning}
        <span class="spin-icon">
          <svg xmlns="http://www.w3.org/2000/svg" width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round">
            <path d="M21 12a9 9 0 1 1-6.219-8.56"/>
          </svg>
        </span>
        Scanning…
      {:else}
        Scan
      {/if}
    </button>

    {#if hasScanned}
      <span class="dup-result-count">
        {pairs.length} pair{pairs.length !== 1 ? 's' : ''} found
      </span>
    {/if}

    <span class="dup-hint">Lower threshold = stricter matching</span>
  </div>

  <!-- Progress bar -->
  {#if isScanning && progressTotal > 0}
    <div class="dup-progress-bar">
      <div
        class="dup-progress-fill"
        style="width: {Math.round((progressDone / progressTotal) * 100)}%"
      ></div>
    </div>
  {/if}

  <!-- Results -->
  <div class="dup-results">
    {#if !hasScanned && !isScanning}
      <div class="dup-empty">
        <p>Set a threshold and click <strong>Scan</strong> to find similar tracks.</p>
        <p class="dup-hint-text">A threshold of 0.10–0.15 catches near-exact duplicates.<br>0.25–0.40 catches similar-sounding tracks.</p>
      </div>

    {:else if isScanning}
      <div class="dup-empty">
        <span class="spin-icon large">
          <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M21 12a9 9 0 1 1-6.219-8.56"/>
          </svg>
        </span>
        <span>
          {#if progressTotal > 0}
            Analysing {progressDone} / {progressTotal} tracks…
          {:else}
            Loading embeddings…
          {/if}
        </span>
      </div>

    {:else if pairs.length === 0}
      <div class="dup-empty">
        <p>No duplicates found at threshold {threshold.toFixed(2)}.</p>
        <p class="dup-hint-text">Try raising the threshold to broaden the search.</p>
      </div>

    {:else}
      <table class="dup-table">
        <thead>
          <tr>
            <th>Match</th>
            <th>Track A</th>
            <th></th>
            <th>Track B</th>
            <th></th>
          </tr>
        </thead>
        <tbody>
          {#each pairs as pair (pair.id_a + '-' + pair.id_b)}
            {@const sim = similarity(pair.distance)}
            <tr class="dup-row">
              <td class="dup-sim">
                <span class="sim-badge" style="--sim: {sim}%">{sim}%</span>
              </td>
              <td class="dup-track">
                <span class="dup-track-name">{displayName(pair, 'a')}</span>
              </td>
              <td class="dup-play">
                <button class="play-btn" onclick={() => playTrack(pair.id_a)} title="Play">
                  <svg xmlns="http://www.w3.org/2000/svg" width="10" height="10" viewBox="0 0 24 24" fill="currentColor">
                    <polygon points="5 3 19 12 5 21 5 3"/>
                  </svg>
                </button>
              </td>
              <td class="dup-track">
                <span class="dup-track-name">{displayName(pair, 'b')}</span>
              </td>
              <td class="dup-play">
                <button class="play-btn" onclick={() => playTrack(pair.id_b)} title="Play">
                  <svg xmlns="http://www.w3.org/2000/svg" width="10" height="10" viewBox="0 0 24 24" fill="currentColor">
                    <polygon points="5 3 19 12 5 21 5 3"/>
                  </svg>
                </button>
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}
  </div>
</div>

<style>
  .duplicates-panel {
    display: flex;
    flex-direction: column;
    height: 100%;
    overflow: hidden;
    background: var(--sg-surface, #0d1117);
  }

  /* ── Toolbar ── */
  .dup-toolbar {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: 1rem;
    padding: 0.6rem 1rem;
    background: var(--sg-surface-slate, #161b22);
    border-bottom: 1px solid rgba(255,255,255,0.07);
    flex-wrap: wrap;
  }

  .dup-title {
    font-family: "JetBrains Mono", monospace;
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.12em;
    color: var(--sg-primary, #00f0ff);
  }

  .dup-control {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .dup-label {
    font-family: "JetBrains Mono", monospace;
    font-size: 8px;
    font-weight: 700;
    letter-spacing: 0.1em;
    color: var(--sg-outline, #849495);
  }

  .dup-slider {
    width: 120px;
    accent-color: var(--sg-primary, #00f0ff);
    cursor: pointer;
  }

  .dup-slider:disabled { opacity: 0.4; cursor: not-allowed; }

  .dup-threshold-val {
    font-family: "JetBrains Mono", monospace;
    font-size: 10px;
    color: var(--sg-on-surface, #e3e1e9);
    min-width: 2.8ch;
  }

  .dup-scan-btn {
    display: flex;
    align-items: center;
    gap: 6px;
    font-family: "JetBrains Mono", monospace;
    font-size: 10px;
    font-weight: 600;
    padding: 5px 14px;
    border: 1px solid rgba(0,240,255,0.35);
    border-radius: 4px;
    background: rgba(0,240,255,0.08);
    color: var(--sg-primary, #00f0ff);
    cursor: pointer;
    transition: background 0.12s, border-color 0.12s;
  }

  .dup-scan-btn:hover:not(:disabled) {
    background: rgba(0,240,255,0.15);
    border-color: var(--sg-primary, #00f0ff);
  }

  .dup-scan-btn:disabled { opacity: 0.45; cursor: not-allowed; }

  .dup-result-count {
    font-family: "JetBrains Mono", monospace;
    font-size: 10px;
    color: var(--sg-outline, #849495);
  }

  .dup-hint {
    font-family: "JetBrains Mono", monospace;
    font-size: 9px;
    color: var(--sg-outline, #849495);
    opacity: 0.5;
    margin-left: auto;
  }

  /* ── Progress bar ── */
  .dup-progress-bar {
    flex-shrink: 0;
    height: 2px;
    background: rgba(255,255,255,0.06);
  }

  .dup-progress-fill {
    height: 100%;
    background: var(--sg-primary, #00f0ff);
    transition: width 0.3s ease;
  }

  /* ── Results ── */
  .dup-results {
    flex: 1;
    overflow-y: auto;
    padding: 0;
  }

  .dup-empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    height: 100%;
    gap: 0.5rem;
    color: var(--sg-outline, #849495);
    font-family: "JetBrains Mono", monospace;
    font-size: 12px;
    text-align: center;
    padding: 2rem;
  }

  .dup-hint-text {
    font-size: 10px;
    opacity: 0.6;
    line-height: 1.6;
  }

  /* ── Table ── */
  .dup-table {
    width: 100%;
    border-collapse: collapse;
    font-family: "JetBrains Mono", monospace;
    font-size: 11px;
  }

  .dup-table thead tr {
    border-bottom: 1px solid rgba(255,255,255,0.07);
    position: sticky;
    top: 0;
    background: var(--sg-surface-slate, #161b22);
    z-index: 1;
  }

  .dup-table th {
    padding: 8px 12px;
    text-align: left;
    font-size: 8px;
    font-weight: 700;
    letter-spacing: 0.1em;
    color: var(--sg-outline, #849495);
  }

  .dup-row {
    border-bottom: 1px solid rgba(255,255,255,0.04);
    transition: background 0.1s;
  }

  .dup-row:hover { background: rgba(255,255,255,0.03); }

  .dup-sim {
    padding: 8px 12px;
    width: 60px;
  }

  .sim-badge {
    display: inline-block;
    font-size: 10px;
    font-weight: 700;
    padding: 2px 8px;
    border-radius: 999px;
    background: rgba(0,240,255,0.1);
    border: 1px solid rgba(0,240,255,0.3);
    color: var(--sg-primary, #00f0ff);
  }

  .dup-track {
    padding: 8px 8px;
    max-width: 320px;
  }

  .dup-track-name {
    display: block;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    color: var(--sg-on-surface, #e3e1e9);
  }

  .dup-play {
    padding: 8px 4px;
    width: 28px;
  }

  .play-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 22px;
    height: 22px;
    border: none;
    border-radius: 50%;
    background: rgba(255,255,255,0.06);
    color: var(--sg-outline, #849495);
    cursor: pointer;
    transition: background 0.12s, color 0.12s;
  }

  .play-btn:hover {
    background: rgba(0,240,255,0.15);
    color: var(--sg-primary, #00f0ff);
  }

  /* ── Spinner ── */
  .spin-icon {
    display: inline-flex;
    animation: spin 1s linear infinite;
  }

  .spin-icon.large { display: flex; }

  @keyframes spin {
    from { transform: rotate(0deg); }
    to   { transform: rotate(360deg); }
  }
</style>
