<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { filters } from '$lib/stores/filters.svelte';
  import { library } from '$lib/stores/library.svelte';
  import { player } from '$lib/stores/player.svelte';
  import { ui } from '$lib/stores/ui.svelte';
  import { devInspector } from '$lib/stores/devInspector.svelte';
  import DevPane from './DevPane.svelte';
  import DevKV from './DevKV.svelte';

  const open = $derived(devInspector.open);

  interface PassStats {
    pass_name: string;
    pending: number;
    in_progress: number;
    done: number;
    failed: number;
    total: number;
    avg_duration_ms: number | null;
    concurrency: number;
  }

  let passStats = $state<PassStats[]>([]);
  let rawData = $state<Record<string, any> | null>(null);
  let rawLoading = $state(false);
  let rawError = $state('');
  let lastTrackId = $state<number | null>(null);
  
  // Dynamic filter fields derived from the filters store
  const filterFields = $derived.by(() => {
    const skip = [
      'filteredTracks', 'autoName', 'toggleDirectoryId', 'clearDirectories',
      'setSimilarTo', 'setSimilarBlend', 'clearSimilar', 'toggleKey',
      'clearKeys', 'toggleTag', 'clearTags', 'clearAll', 'isSimilarLoading',
      'isSemanticLoading', 'isClapLoading', 'semanticTrackScores', 'clapTrackScores'
    ];
    return Object.entries(filters)
      .filter(([k, v]) => typeof v !== 'function' && !skip.includes(k))
      .map(([k, v]) => {
        if (k === 'minBpm') return ['bpm', `${filters.minBpm} – ${filters.maxBpm}`];
        if (k === 'maxBpm') return null;
        return [k, v];
      })
      .filter((entry): entry is [string, unknown] => entry !== null);
  });

  // Keep shared store in sync so HUD can read pending count without mounting drawer
  $effect(() => {
    devInspector.totalPending = passStats.reduce((s, p) => s + p.pending + p.in_progress, 0);
  });

  // Poll pass stats every 3s while analysis running
  $effect(() => {
    if (!open && !library.analysisRunning) return;
    const id = setInterval(async () => {
      try {
        passStats = await invoke<PassStats[]>('get_pass_stats');
      } catch {}
    }, 3000);
    // Initial fetch
    invoke<PassStats[]>('get_pass_stats').then(s => passStats = s).catch(() => {});
    return () => clearInterval(id);
  });

  // Fetch raw SQL data when current track changes or drawer opens
  $effect(() => {
    const track = player.selectedTrack;
    if (!track) { rawData = null; lastTrackId = null; return; }
    if (track.id === lastTrackId && rawData) return; // already loaded
    fetchRaw(track.id);
  });

  // Re-fetch when AcoustID enriches the track
  $effect(() => {
    let unlisten: (() => void) | undefined;
    listen<number>('track-enriched', (e) => {
      if (player.selectedTrack?.id === e.payload) fetchRaw(e.payload);
    }).then(fn => { unlisten = fn; });
    return () => unlisten?.();
  });

  async function fetchRaw(trackId: number) {
    rawLoading = true;
    rawError = '';
    try {
      rawData = await invoke<Record<string, any>>('debug_track_raw', { trackId });
      lastTrackId = trackId;
    } catch (e) {
      rawError = String(e);
      rawData = null;
    } finally {
      rawLoading = false;
    }
  }

  function isDefault(key: string, value: unknown): boolean {
    if (value === null || value === undefined || value === '') return true;
    if (key === 'minBpm' && value === 20) return true;
    if (key === 'maxBpm' && value === 250) return true;
    if (key === 'similarBlend' && value === 0.5) return true;
    if (key.endsWith('Min') && value === 0) return true;
    if (key.endsWith('Max') && value === 1) return true;
    if (Array.isArray(value) && value.length === 0) return true;
    if (value instanceof Set && value.size === 0) return true;
    if (value instanceof Map && value.size === 0) return true;
    if (value === 'all' || value === false) return true;
    return false;
  }

  async function dumpToConsole() {
    let freshStats: PassStats[] = [];
    try { freshStats = await invoke<PassStats[]>('get_pass_stats'); } catch {}

    console.group('%c🎛 Deep Cuts state snapshot', 'color:#00f0ff;font-weight:bold');
    console.log('filters', {
      searchQuery: filters.searchQuery,
      semanticQuery: filters.semanticQuery,
      clapQuery: filters.clapQuery,
      genreFilter: filters.genreFilter,
      bpm: [filters.minBpm, filters.maxBpm],
      selectedKeys: filters.selectedKeys,
      selectedScale: filters.selectedScale,
      vocalFilter: filters.vocalFilter,
      musicOnly: filters.musicOnly,
      selectedTags: filters.selectedTags,
      similarToTrack: filters.similarToTrack,
      similarBlend: filters.similarBlend,
      filteredCount: filters.filteredTracks.length,
      semanticScores: filters.semanticTrackScores,
      clapScores: filters.clapTrackScores,
    });
    console.log('player', {
      selectedTrack: player.selectedTrack,
      isPlaying: player.isPlaying,
      currentTime: player.currentTime,
      duration: player.duration,
    });
    console.log('library', {
      trackCount: library.trackCount,
      tracksLoaded: library.tracks.length,
      tracks: library.tracks,  // full array for ad-hoc queries
      directories: library.directories,
      isScanning: library.isScanning,
      analysisRunning: library.analysisRunning,
      analysisPaused: library.analysisPaused,
    });
    console.log('ui', {
      activeView: ui.activeView,
      sidebarTab: ui.sidebarTab,
    });
    console.log('analysis passes', freshStats);
    if (rawData) console.log('raw SQL (current track)', rawData);
    console.groupEnd();
  }

  function fmt(ms: number | null): string {
    if (ms === null) return '—';
    return ms >= 1000 ? (ms / 1000).toFixed(1) + 's' : ms.toFixed(0) + 'ms';
  }

  function statusColor(status: number): string {
    // 0=pending 1=in_progress 2=done 3=failed
    return ['#849495', '#00f0ff', '#4caf50', '#ff6b6b'][status] ?? '#849495';
  }
</script>

{#if open}
  <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
  <div class="backdrop" onclick={() => devInspector.open = false}></div>
  <aside class="drawer">
    <!-- Header -->
    <div class="drawer-header">
      <span class="drawer-title">⚙ Dev Inspector</span>
      <div class="drawer-actions">
        <button class="hdr-btn" onclick={dumpToConsole}>Dump to console</button>
        <button class="hdr-close" onclick={() => devInspector.open = false}>×</button>
      </div>
    </div>

    <div class="drawer-body">
      <!-- Pane 1: Filters -->
      <DevPane title="Filters">
        {#each filterFields as [k, v]}
          <DevKV label={k} value={v} dim={isDefault(k, v)} />
        {/each}
        <div class="divider"></div>
        <DevKV label="filteredTracks" value={`${filters.filteredTracks.length} / ${library.trackCount}`} />
        <DevKV label="semanticIds" value={filters.semanticTrackScores.size} dim={filters.semanticTrackScores.size === 0} />
        <DevKV label="clapIds" value={filters.clapTrackScores.size} dim={filters.clapTrackScores.size === 0} />
      </DevPane>
 
      <!-- Pane 2: Current Track -->
      <DevPane title="Current Track">
        {#if player.selectedTrack}
          {@const t = player.selectedTrack}
          <div class="highlight-block">
            <DevKV label="id"       value={t.id} />
            <DevKV label="title"    value={t.title} />
            <DevKV label="artist"   value={t.artist} />
            <DevKV label="bpm"      value={t.bpm} />
            <DevKV label="key/scale" value={`${t.key ?? '—'} / ${t.scale ?? '—'}`} />
            <DevKV label="duration" value={t.duration ? `${Math.round(t.duration)}s` : '—'} />
            <DevKV label="acoustid" value={t.acoustid_status} />
            <DevKV label="waveform_sax" value={t.waveform_sax} />
            <DevKV label="waveform_fingerprint" value={t.waveform_fingerprint} />
          </div>
          <div class="divider"></div>
          <DevKV label="isPlaying"   value={player.isPlaying} />
          <DevKV label="currentTime" value={`${Math.round(player.currentTime)}s`} />
          <div class="divider"></div>
          <!-- All remaining fields -->
          {#each Object.entries(t) as [k, v]}
            {#if !['id','title','artist','bpm','key','scale','duration','acoustid_status','waveform_sax','waveform_fingerprint'].includes(k)}
              <DevKV label={k} value={v} dim={v === null || v === undefined || v === ''} truncate={80} />
            {/if}
          {/each}
        {:else}
          <p class="empty">No track selected</p>
        {/if}
      </DevPane>

      <!-- Pane 3: Analysis Pipeline -->
      <DevPane title="Analysis Pipeline">
        {#if passStats.length > 0}
          <table class="pass-table">
            <thead>
              <tr>
                <th>Pass</th>
                <th>pend</th>
                <th>run</th>
                <th>done</th>
                <th>fail</th>
                <th>avg</th>
              </tr>
            </thead>
            <tbody>
              {#each passStats as p}
                {@const active = p.pending > 0 || p.in_progress > 0 || p.failed > 0}
                <tr class:active>
                  <td class="pass-name">{p.pass_name}</td>
                  <td class:hot={p.pending > 0}>{p.pending}</td>
                  <td class:hot={p.in_progress > 0}>{p.in_progress}</td>
                  <td>{p.done}</td>
                  <td class:err={p.failed > 0}>{p.failed || '—'}</td>
                  <td>{fmt(p.avg_duration_ms)}</td>
                </tr>
              {/each}
            </tbody>
          </table>
          <button class="small-btn" onclick={() => invoke('recover_stuck_passes').catch(() => {})}>
            Recover stuck
          </button>
        {:else}
          <p class="empty">No pass data yet</p>
        {/if}
      </DevPane>

      <!-- Pane 4: Library / Scan -->
      <DevPane title="Library & Scan">
        <DevKV label="tauriConnected"  value={library.tauriConnected} />
        <DevKV label="trackCount"      value={library.trackCount} />
        <DevKV label="tracksLoaded"    value={library.tracks.length} />
        <DevKV label="directories"     value={library.directories.length} />
        <div class="divider"></div>
        <DevKV label="isScanning"      value={library.isScanning} />
        <DevKV label="scanProgress"    value={`${library.scanProgress}%`} dim={!library.isScanning} />
        <DevKV label="scanCurrentFile" value={library.scanCurrentFile} dim={!library.isScanning} truncate={50} />
        <div class="divider"></div>
        <DevKV label="analysisRunning" value={library.analysisRunning} />
        <DevKV label="analysisPaused"  value={library.analysisPaused} />
        <DevKV label="  manual"        value={library.analysisManuallyPaused} dim={!library.analysisPaused} />
        <DevKV label="  auto"          value={library.analysisAutoPaused}     dim={!library.analysisPaused} />
        <div class="divider"></div>
        <DevKV label="activeView"  value={ui.activeView} />
        <DevKV label="sidebarTab"  value={ui.sidebarTab} />
      </DevPane>

      <!-- Pane 5: Raw SQL -->
      <DevPane title="Raw SQL — Current Track" open={false}>
        {#if !player.selectedTrack}
          <p class="empty">No track selected</p>
        {:else if rawLoading}
          <p class="empty">Loading…</p>
        {:else if rawError}
          <p class="error">{rawError}</p>
        {:else if rawData}
          <div class="raw-actions">
            <button class="small-btn" onclick={() => fetchRaw(player.selectedTrack!.id)}>Refresh</button>
          </div>

          <!-- tracks row -->
          <details open class="sql-section">
            <summary>tracks</summary>
            {#each Object.entries(rawData.track ?? {}) as [k, v]}
              <DevKV label={k} value={v} dim={v === null} truncate={80} />
            {/each}
          </details>

          <!-- track_passes -->
          <details open class="sql-section">
            <summary>track_passes ({rawData.passes?.length ?? 0})</summary>
            {#if rawData.passes?.length > 0}
              <table class="pass-table">
                <thead>
                  <tr>
                    <th>pass</th><th>status</th><th>ver</th><th>dur</th><th>result</th>
                  </tr>
                </thead>
                <tbody>
                  {#each rawData.passes as p}
                    <tr>
                      <td class="pass-name">{p.pass_name}</td>
                      <td style="color:{statusColor(p.status)}">{p.status}</td>
                      <td>{p.pass_version ?? '—'}</td>
                      <td>{fmt(p.duration_ms)}</td>
                      <td class="raw-result">{p.raw_result ? JSON.stringify(p.raw_result).slice(0,60) : '—'}</td>
                    </tr>
                  {/each}
                </tbody>
              </table>
            {:else}
              <p class="empty">No passes</p>
            {/if}
          </details>

          <!-- track_coords -->
          <details class="sql-section">
            <summary>track_coords</summary>
            {#if rawData.coords && Object.keys(rawData.coords).length > 0}
              {#each Object.entries(rawData.coords) as [k, v]}
                <DevKV label={k} value={v} />
              {/each}
            {:else}
              <p class="empty">No coordinates</p>
            {/if}
          </details>

          <!-- tags -->
          <details class="sql-section">
            <summary>tags ({rawData.tags?.length ?? 0})</summary>
            {#if rawData.tags?.length > 0}
              <table class="pass-table">
                <thead><tr><th>name</th><th>score</th><th>discard</th></tr></thead>
                <tbody>
                  {#each rawData.tags as tag}
                    <tr class:dim-row={tag.discard}>
                      <td>{tag.name}</td>
                      <td>{tag.score?.toFixed(3) ?? '—'}</td>
                      <td>{tag.discard ? '✕' : ''}</td>
                    </tr>
                  {/each}
                </tbody>
              </table>
            {:else}
              <p class="empty">No tags</p>
            {/if}
          </details>

          <!-- suppressions -->
          <details class="sql-section">
            <summary>suppressions ({rawData.suppressions?.length ?? 0})</summary>
            {#if rawData.suppressions?.length > 0}
              {#each rawData.suppressions as s}
                <div class="chip">{s}</div>
              {/each}
            {:else}
              <p class="empty">None</p>
            {/if}
          </details>

          <!-- chat sessions -->
          <details class="sql-section">
            <summary>chat_sessions ({rawData.chat_sessions?.length ?? 0})</summary>
            {#if rawData.chat_sessions?.length > 0}
              {#each rawData.chat_sessions as sess}
                <DevKV label={sess.id} value={sess.title} />
              {/each}
            {:else}
              <p class="empty">None</p>
            {/if}
          </details>
        {/if}
      </DevPane>

    </div>
  </aside>
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 490;
    background: rgba(0,0,0,0.3);
  }

  .drawer {
    position: fixed;
    top: 0;
    right: 0;
    bottom: 0;
    width: 420px;
    z-index: 500;
    background: #111520;
    border-left: 1px solid rgba(0,240,255,0.15);
    display: flex;
    flex-direction: column;
    box-shadow: -8px 0 32px rgba(0,0,0,0.5);
  }

  .drawer-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 10px 14px;
    border-bottom: 1px solid rgba(255,255,255,0.08);
    flex-shrink: 0;
  }

  .drawer-title {
    font-family: var(--sg-font-mono);
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.1em;
    color: var(--sg-primary, #00f0ff);
  }

  .drawer-actions {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .hdr-btn {
    font-family: var(--sg-font-mono);
    font-size: 10px;
    padding: 3px 9px;
    background: rgba(0,240,255,0.08);
    border: 1px solid rgba(0,240,255,0.2);
    border-radius: 4px;
    color: var(--sg-primary, #00f0ff);
    cursor: pointer;
    transition: background 0.12s;
  }

  .hdr-btn:hover { background: rgba(0,240,255,0.16); }

  .hdr-close {
    background: none;
    border: none;
    color: var(--sg-outline, #849495);
    font-size: 18px;
    line-height: 1;
    cursor: pointer;
    padding: 0 2px;
    transition: color 0.12s;
  }

  .hdr-close:hover { color: var(--sg-on-surface, #e3e1e9); }

  .drawer-body {
    flex: 1;
    overflow-y: auto;
    overscroll-behavior: contain;
  }

  .divider {
    border-top: 1px solid rgba(255,255,255,0.05);
    margin: 6px 0;
  }

  .highlight-block {
    background: rgba(0,240,255,0.03);
    border-left: 2px solid rgba(0,240,255,0.3);
    padding: 4px 8px;
    margin-bottom: 6px;
    border-radius: 0 3px 3px 0;
  }

  .empty {
    font-family: var(--sg-font-mono);
    font-size: 10px;
    color: var(--sg-outline, #849495);
    margin: 4px 0;
    opacity: 0.6;
  }

  .error {
    font-family: var(--sg-font-mono);
    font-size: 10px;
    color: #ff6b6b;
    margin: 4px 0;
  }

  /* Pass stats table */
  .pass-table {
    width: 100%;
    border-collapse: collapse;
    font-family: var(--sg-font-mono);
    font-size: 10px;
    margin-bottom: 8px;
  }

  .pass-table th {
    text-align: left;
    color: var(--sg-outline, #849495);
    padding: 2px 4px;
    border-bottom: 1px solid rgba(255,255,255,0.06);
    font-weight: 600;
  }

  .pass-table td {
    padding: 2px 4px;
    color: var(--sg-on-surface, #e3e1e9);
    opacity: 0.5;
  }

  .pass-table tr.active td { opacity: 1; }
  .pass-table tr.dim-row td { opacity: 0.35; }
  .pass-table td.pass-name { color: var(--sg-outline, #849495); opacity: 1; }
  .pass-table td.hot  { color: var(--sg-primary, #00f0ff); opacity: 1; }
  .pass-table td.err  { color: #ff6b6b; opacity: 1; }
  .pass-table td.raw-result { opacity: 0.5; font-size: 9px; }

  /* Raw SQL sections */
  .raw-actions {
    margin-bottom: 8px;
  }

  .sql-section {
    margin-bottom: 8px;
  }

  .sql-section summary {
    font-family: var(--sg-font-mono);
    font-size: 10px;
    color: var(--sg-outline, #849495);
    cursor: pointer;
    padding: 3px 0;
    user-select: none;
    list-style: none;
  }

  .sql-section summary::before {
    content: '▶ ';
    font-size: 8px;
  }

  .sql-section[open] summary::before {
    content: '▼ ';
  }

  .small-btn {
    font-family: var(--sg-font-mono);
    font-size: 10px;
    padding: 3px 8px;
    background: rgba(255,255,255,0.04);
    border: 1px solid rgba(255,255,255,0.1);
    border-radius: 4px;
    color: var(--sg-outline, #849495);
    cursor: pointer;
    transition: all 0.12s;
  }

  .small-btn:hover {
    border-color: rgba(255,255,255,0.2);
    color: var(--sg-on-surface, #e3e1e9);
  }

  .chip {
    display: inline-block;
    font-family: var(--sg-font-mono);
    font-size: 9px;
    padding: 1px 6px;
    background: rgba(255,80,80,0.1);
    border: 1px solid rgba(255,80,80,0.2);
    border-radius: 10px;
    color: #ff9999;
    margin: 2px 2px 2px 0;
  }
</style>
