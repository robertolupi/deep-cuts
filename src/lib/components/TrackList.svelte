<script lang="ts">
  import { untrack } from 'svelte';
  import { formatDuration } from '$lib/utils/format';
  import { filters } from '$lib/stores/filters.svelte';
  import { library } from '$lib/stores/library.svelte';
  import { ui } from '$lib/stores/ui.svelte';
  import { waveformBarsFromJson } from '$lib/utils/waveform';

  let {
    selectedTrack,
    isPlaying,
    onTrackSelect,
  }: {
    selectedTrack: import('../types').Track | null;
    isPlaying: boolean;
    onTrackSelect: (track: import('../types').Track) => void;
  } = $props();

  const PAGE_SIZE = 150;
  let currentPage = $state(0);

  $effect(() => {
    // Reset to page 0 whenever the filtered result set changes
    void filters.filteredTracks;
    currentPage = 0;
  });

  $effect(() => {
    // Jump to the page containing the selected track
    if (!selectedTrack) return;
    const idx = filters.filteredTracks.findIndex(t => t.id === selectedTrack!.id);
    if (idx === -1) return;
    const targetPage = Math.floor(idx / PAGE_SIZE);
    untrack(() => {
      if (targetPage !== currentPage) currentPage = targetPage;
    });
  });

  // Keep selected track smoothly scrolled into view when selected or when re-sorted due to metadata updates
  $effect(() => {
    if (selectedTrack) {
      // Reactively track changes to key identifying fields so sorting updates trigger a scroll
      const _id = selectedTrack.id;
      const _title = selectedTrack.title;
      const _artist = selectedTrack.artist;

      setTimeout(() => {
        const activeRow = document.querySelector('.track-row.active-pulse');
        if (activeRow) {
          activeRow.scrollIntoView({ behavior: 'smooth', block: 'nearest' });
        }
      }, 100);
    }
  });

  const totalPages = $derived(Math.max(1, Math.ceil(filters.filteredTracks.length / PAGE_SIZE)));
  const pageStart  = $derived(currentPage * PAGE_SIZE);
  let displayedTracks = $derived(filters.filteredTracks.slice(pageStart, pageStart + PAGE_SIZE));
  const allTracks = $derived(library.tracks);

  const isSelectedOutsideFilter = $derived(
    !!selectedTrack && !filters.filteredTracks.some(t => t.id === selectedTrack!.id)
  );
</script>

<div class="track-list-view">
  <!-- Track count badge -->
  <div class="tracks-toolbar">
    <div class="library-count-badge">
      <code>{filters.filteredTracks.length} / {allTracks.length} tracks</code>
    </div>
  </div>

  <!-- Selected track outside filter banner -->
  {#if isSelectedOutsideFilter}
    <div class="outside-filter-banner">
      <span>"{selectedTrack!.title || selectedTrack!.filename}" is hidden by active filters.</span>
      <button class="outside-filter-clear" onclick={() => filters.clearAll()}>Clear filters</button>
    </div>
  {/if}

  <!-- Tracks Grid List Table -->
  {#if allTracks.length > 0}
    {#if filters.filteredTracks.length > 0}
      <div class="tracks-table-wrap">
        <table class="tracks-table">
          <thead>
            <tr>
              <th style="width: 40px; text-align: center;">#</th>
              <th>Title / Filename</th>
              <th style="width: 140px;">Waveform</th>
              <th>Artist</th>
              <th>Album</th>
              <th>Duration</th>
              <th style="width: 60px;">BPM</th>
              <th style="width: 80px;">Key</th>
              <th style="width: 100px;">Format</th>
            </tr>
          </thead>
          <tbody>
            {#each displayedTracks as track, index (track.id)}
              <tr 
                class="track-row {selectedTrack?.id === track.id ? 'active-pulse' : ''}" 
                onclick={() => onTrackSelect(track)}
              >
                <td style="text-align: center; color: var(--sg-outline); font-size: 0.82rem;">
                  {#if selectedTrack?.id === track.id && isPlaying}
                    <div class="playing-bars-mini">
                      <div class="bar"></div>
                      <div class="bar"></div>
                      <div class="bar"></div>
                    </div>
                  {:else}
                    {pageStart + index + 1}
                  {/if}
                </td>
                <td class="track-title-cell" title={track.title || track.filename}>
                  <span class="track-primary-title">{track.title || track.filename}</span>
                  {#if filters.semanticQuery.trim() && filters.semanticTrackScores.has(track.id)}
                    {@const score = filters.semanticTrackScores.get(track.id)}
                    {#if score !== undefined}
                      <span class="semantic-score-badge">{Math.round(score)}%</span>
                    {/if}
                  {/if}
                  {#if filters.clapQuery.trim() && filters.clapTrackScores.has(track.id)}
                    {@const score = filters.clapTrackScores.get(track.id)}
                    {#if score !== undefined}
                      <span class="clap-score-badge">{Math.round(score)}%</span>
                    {/if}
                  {/if}
                </td>
                <td class="col-waveform">
                  {#if track.waveform_data}
                    {@const bars = waveformBarsFromJson(track.waveform_data)}
                    {@const peak = Math.max(...bars, 1e-6)}
                    {@const segments = track.sax_alignment_segments?.split(',') ?? []}
                    {@const sax = track.waveform_sax ?? ''}
                    {#if bars.length > 0}
                      <div class="mini-waveform">
                        {#each bars as energy, i}
                          {@const norm = energy / peak}
                          {#if segments.length > 0}
                            {@const segIdx = Math.min(Math.floor(i * segments.length / bars.length), segments.length - 1)}
                            {@const label = segments[segIdx] ?? 'unknown'}
                            <div
                               class="waveform-bar"
                               style="height: {Math.max(2, Math.round(norm * 20))}px; opacity: {norm * 0.65 + 0.35}; background: var(--label-{label});"
                            ></div>
                          {:else}
                            {@const saxIdx = Math.min(Math.floor((i * 3) / 4), sax.length - 1)}
                            {@const letter = sax[saxIdx] ?? ''}
                            <div
                               class="waveform-bar"
                               style="height: {Math.max(2, Math.round(norm * 20))}px; opacity: {norm * 0.65 + 0.35}; {letter ? `background: var(--sax-${letter});` : ''}"
                            ></div>
                          {/if}
                        {/each}
                      </div>
                    {:else}
                      <div class="mini-waveform-skeleton shimmer"></div>
                    {/if}
                  {:else}
                    <div class="mini-waveform-skeleton shimmer"></div>
                  {/if}
                </td>
                <td class="track-text-cell" title={track.artist || "Unknown"}>
                  {track.artist || "—"}
                </td>
                <td class="track-text-cell" title={track.album || "Unknown"}>
                  {track.album || "—"}
                </td>
                <td style="color: var(--sg-on-surface-variant); font-size: 0.88rem;">
                  {formatDuration(track.duration_seconds)}
                </td>
                <td style="color: var(--sg-on-surface-variant); font-size: 0.82rem; text-align: right; padding-right: 0.75rem;">
                  {#if track.bpm}{Math.round(track.bpm)}{:else}—{/if}
                </td>
                <td style="font-size: 0.82rem;">
                  {#if track.key && track.scale}
                    <span class="key-badge">{track.key} <span style="opacity: 0.6; font-size: 0.72rem;">{track.scale}</span></span>
                  {:else}—{/if}
                </td>
                <td>
                  <span class="format-mini-badge">{track.path.split('.').pop()?.toUpperCase()}</span>
                  {#if track.bitrate}
                    <span class="bitrate-label">{track.bitrate}k</span>
                  {/if}
                </td>
              </tr>
            {/each}
          </tbody>
        </table>

        {#if totalPages > 1}
          <div class="pagination-bar">
            <button
              class="page-btn"
              onclick={() => currentPage--}
              disabled={currentPage === 0}
              type="button"
            >
              <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><path d="m15 18-6-6 6-6"/></svg>
              Prev
            </button>
            <span class="page-info">
              {pageStart + 1}–{Math.min(pageStart + PAGE_SIZE, filters.filteredTracks.length)}
              <span class="page-of">of {filters.filteredTracks.length}</span>
            </span>
            <button
              class="page-btn"
              onclick={() => currentPage++}
              disabled={currentPage >= totalPages - 1}
              type="button"
            >
              Next
              <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><path d="m9 18 6-6-6-6"/></svg>
            </button>
          </div>
        {/if}
      </div>
    {:else}
      <div class="empty-search-state">
        <svg xmlns="http://www.w3.org/2000/svg" width="36" height="36" viewBox="0 0 24 24" fill="none" stroke="var(--sg-outline)" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
          <circle cx="11" cy="11" r="8"/>
          <line x1="21" y1="21" x2="16.65" y2="16.65"/>
        </svg>
        <h5>No Matching Tracks Found</h5>
        <p>Try refining your search text or switching the active genre filter.</p>
      </div>
    {/if}
  {:else}
    <!-- Empty state: no tracks in the library -->
    <div class="empty-tracks-state">
      <div class="vinyl-display-empty">
        <img src="/deep_cuts_transparent.png" alt="Deep Cuts empty vinyl" class="vinyl-image-empty" />
      </div>
      <h5>Your Music Library is Empty</h5>
      <p>Monitored collection folders have not scanned any supported audio files yet.</p>
      <button class="btn-primary" onclick={() => ui.activeView = 'settings'} style="margin-top: 0.5rem;">
        Go to Library Settings
      </button>
    </div>
  {/if}
</div>

<style>
  .track-list-view {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    background: var(--sg-surface, #0d1117);
  }

  .key-badge {
    font-family: var(--sg-font-mono);
    color: var(--sg-on-surface);
  }

  .col-waveform {
    padding: 0.6rem 1rem;
    vertical-align: middle;
  }

  .mini-waveform {
    display: flex;
    align-items: flex-end;
    gap: 1px;
    height: 22px;
    width: 120px;
  }

  /* SAX letter colours: a=very quiet … e=very loud */
  :global(:root) {
    --sax-a: #4a7fa5; /* TODO: map to --sg-* token */
    --sax-b: #5ba3c9; /* TODO: map to --sg-* token */
    --sax-c: var(--sg-primary);
    --sax-d: #f0a030; /* TODO: map to --sg-* token */
    --sax-e: #ff5533; /* TODO: map to --sg-* token */
  }

  .waveform-bar {
    flex: 1;
    border-radius: 1px;
    background: var(--sg-primary, #00f0ff);
    transition: opacity 0.15s ease;
  }

  .mini-waveform-skeleton {
    height: 8px;
    width: 108px;
    background: rgba(255, 255, 255, 0.05);
    border-radius: 4px;
    position: relative;
    overflow: hidden;
  }

  .shimmer::after {
    content: '';
    position: absolute;
    inset: 0;
    background: linear-gradient(
      90deg,
      transparent,
      rgba(255, 255, 255, 0.08),
      transparent
    );
    animation: shimmer-sweep 1.6s infinite;
  }

  @keyframes shimmer-sweep {
    0%   { transform: translateX(-100%); }
    100% { transform: translateX(100%); }
  }

  .pagination-bar {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 0.75rem;
    padding: 0.75rem 1rem;
    border-top: 1px solid var(--sg-surface-high);
  }

  .page-btn {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    background: rgba(255, 255, 255, 0.03);
    border: 1px solid rgba(255, 255, 255, 0.1);
    color: var(--sg-on-surface-variant, #a0a0b0);
    padding: 0.3rem 0.75rem;
    font-family: var(--sg-font-mono);
    font-size: 0.78rem;
    font-weight: 600;
    border-radius: 4px;
    cursor: pointer;
    transition: all 0.12s;
  }

  .page-btn:hover:not(:disabled) {
    border-color: var(--sg-primary, #00f0ff);
    color: var(--sg-primary, #00f0ff);
    background: rgba(0, 240, 255, 0.06);
  }

  .page-btn:disabled {
    opacity: 0.3;
    cursor: not-allowed;
  }

  .page-info {
    font-family: var(--sg-font-mono);
    font-size: 0.78rem;
    color: var(--sg-on-surface, #e3e1e9);
    min-width: 120px;
    text-align: center;
  }

  .page-of {
    color: var(--sg-outline, #849495);
  }

  .outside-filter-banner {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    padding: 0.5rem 1rem;
    background: color-mix(in srgb, var(--sg-warning) 7%, transparent);
    border-bottom: 1px solid color-mix(in srgb, var(--sg-warning) 20%, transparent);
    font-size: 0.8rem;
    color: var(--sg-on-surface-variant, #a0a0b0);
  }

  .outside-filter-banner span {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .outside-filter-clear {
    flex-shrink: 0;
    background: none;
    border: 1px solid color-mix(in srgb, var(--sg-warning) 35%, transparent);
    color: color-mix(in srgb, var(--sg-warning) 85%, transparent);
    border-radius: 4px;
    padding: 0.2rem 0.6rem;
    font-size: 0.75rem;
    cursor: pointer;
    transition: background 0.15s;
  }

  .outside-filter-clear:hover {
    background: color-mix(in srgb, var(--sg-warning) 12%, transparent);
  }

  .semantic-score-badge {
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-2xs);
    font-weight: 700;
    padding: 2px 6px;
    border-radius: 999px;
    border: 1px solid color-mix(in srgb, var(--sg-primary) 30%, transparent);
    color: var(--sg-primary);
    background: color-mix(in srgb, var(--sg-primary) 8%, transparent);
    margin-left: 6px;
    vertical-align: middle;
    display: inline-block;
  }

  .clap-score-badge {
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-2xs);
    font-weight: 700;
    padding: 2px 6px;
    border-radius: 999px;
    border: 1px solid color-mix(in srgb, var(--sg-secondary) 30%, transparent);
    color: var(--sg-secondary);
    background: color-mix(in srgb, var(--sg-secondary) 8%, transparent);
    margin-left: 6px;
    vertical-align: middle;
    display: inline-block;
  }

</style>
