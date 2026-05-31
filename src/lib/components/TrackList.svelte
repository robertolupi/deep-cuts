<script lang="ts">
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

  let displayLimit = $state(150);

  $effect(() => {
    filters.searchQuery;
    filters.genreFilter;
    filters.selectedKeys;
    filters.selectedScale;
    filters.minBpm;
    filters.maxBpm;
    filters.musicOnly;
    filters.vocalFilter;
    filters.similarToTrack;
    const idx = selectedTrack
      ? filters.filteredTracks.findIndex(t => t.id === selectedTrack!.id)
      : -1;
    displayLimit = idx >= 150 ? idx + 1 : 150;
  });

  let displayedTracks = $derived(filters.filteredTracks.slice(0, displayLimit));
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
                    {index + 1}
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
                    {#if bars.length > 0}
                      <div class="mini-waveform">
                        {#each bars as energy}
                          {@const norm = energy / peak}
                          <div
                             class="waveform-bar"
                             style="height: {Math.max(2, Math.round(norm * 20))}px; opacity: {norm * 0.65 + 0.35};"
                          ></div>
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

        {#if filters.filteredTracks.length > displayLimit}
          <div class="load-more-container">
            <button
              class="load-more-btn"
              onclick={() => displayLimit += 150}
              type="button"
            >
              <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" class="load-more-icon">
                <path d="m6 9 6 6 6-6"/>
              </svg>
              Load More Tracks ({filters.filteredTracks.length - displayLimit} remaining)
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
    font-family: "JetBrains Mono", monospace;
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

  .waveform-bar {
    flex: 1;
    border-radius: 1px;
    background: linear-gradient(
      180deg,
      var(--sg-primary, #00f0ff) 0%,
      var(--sg-primary) 100%
    );
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

.load-more-container {
    display: flex;
    justify-content: center;
    padding: 1.5rem;
    border-top: 1px solid var(--sg-surface-high);
    background: linear-gradient(180deg, transparent 0%, rgba(10, 11, 16, 0.2) 100%);
  }

  .load-more-btn {
    background: rgba(255, 255, 255, 0.03);
    border: 1px solid var(--sg-surface-high);
    color: var(--sg-on-surface);
    padding: 0.6rem 1.5rem;
    font-size: 0.85rem;
    font-weight: 600;
    border-radius: 6px;
    cursor: pointer;
    display: flex;
    align-items: center;
    gap: 0.5rem;
    transition: all 0.2s ease-in-out;
    backdrop-filter: blur(8px);
  }

  .load-more-btn:hover {
    background: rgba(255, 255, 255, 0.08);
    border-color: var(--sg-primary, #00f0ff);
    color: var(--sg-primary, #00f0ff);
    box-shadow: 0 0 12px color-mix(in srgb, var(--sg-primary, #00f0ff) 15%, transparent);
    transform: translateY(-1px);
  }

  .load-more-btn:active {
    transform: translateY(0);
  }

  .load-more-icon {
    transition: transform 0.2s ease;
  }

  .load-more-btn:hover .load-more-icon {
    transform: translateY(1px);
  }

  .outside-filter-banner {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    padding: 0.5rem 1rem;
    background: rgba(255, 200, 0, 0.07);
    border-bottom: 1px solid rgba(255, 200, 0, 0.2);
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
    border: 1px solid rgba(255, 200, 0, 0.35);
    color: rgba(255, 200, 0, 0.85);
    border-radius: 4px;
    padding: 0.2rem 0.6rem;
    font-size: 0.75rem;
    cursor: pointer;
    transition: background 0.15s;
  }

  .outside-filter-clear:hover {
    background: rgba(255, 200, 0, 0.12);
  }

  .semantic-score-badge {
    font-family: "JetBrains Mono", monospace;
    font-size: 9px;
    font-weight: 700;
    padding: 2px 6px;
    border-radius: 999px;
    border: 1px solid rgba(0, 240, 255, 0.3);
    color: var(--sg-primary, #00f0ff);
    background: rgba(0, 240, 255, 0.08);
    margin-left: 6px;
    vertical-align: middle;
    display: inline-block;
  }

  .clap-score-badge {
    font-family: "JetBrains Mono", monospace;
    font-size: 9px;
    font-weight: 700;
    padding: 2px 6px;
    border-radius: 999px;
    border: 1px solid rgba(254, 0, 254, 0.3);
    color: var(--sg-secondary, #fe00fe);
    background: rgba(254, 0, 254, 0.08);
    margin-left: 6px;
    vertical-align: middle;
    display: inline-block;
  }

</style>
