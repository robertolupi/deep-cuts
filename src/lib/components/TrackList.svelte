<script lang="ts">
  import { formatDuration } from '$lib/utils/format';
  import { filters } from '$lib/stores/filters.svelte';
  import { library } from '$lib/stores/library.svelte';
  import { ui } from '$lib/stores/ui.svelte';

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
    displayLimit = 150;
  });

  let displayedTracks = $derived(filters.filteredTracks.slice(0, displayLimit));
  const allTracks = $derived(library.tracks);
</script>

<div class="track-list-view">
  <!-- Track count badge -->
  <div class="tracks-toolbar">
    <div class="library-count-badge">
      <code>{filters.filteredTracks.length} / {allTracks.length} tracks</code>
    </div>
  </div>

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
                <td style="text-align: center; color: var(--text-muted); font-size: 0.82rem;">
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
                </td>
                <td class="col-waveform">
                  {#if track.waveform_data}
                    {@const bars = (JSON.parse(track.waveform_data) as number[]).filter((_, i) => i % 3 === 0)}
                    {@const peak = Math.max(...bars, 1e-6)}
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
                </td>
                <td class="track-text-cell" title={track.artist || "Unknown"}>
                  {track.artist || "—"}
                </td>
                <td class="track-text-cell" title={track.album || "Unknown"}>
                  {track.album || "—"}
                </td>
                <td style="color: var(--text-secondary); font-size: 0.88rem;">
                  {formatDuration(track.duration_seconds)}
                </td>
                <td style="color: var(--text-secondary); font-size: 0.82rem; text-align: right; padding-right: 0.75rem;">
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
        <svg xmlns="http://www.w3.org/2000/svg" width="36" height="36" viewBox="0 0 24 24" fill="none" stroke="var(--text-muted)" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
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
    font-family: var(--font-mono, monospace);
    color: var(--text-primary);
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
      var(--color-accent-cyan, #00f2fe) 0%,
      var(--color-primary, #8a2be2) 100%
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
    border-top: 1px solid var(--border-color);
    background: linear-gradient(180deg, transparent 0%, rgba(10, 11, 16, 0.2) 100%);
  }

  .load-more-btn {
    background: rgba(255, 255, 255, 0.03);
    border: 1px solid var(--border-color);
    color: var(--text-primary);
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
    border-color: var(--color-accent-cyan, #00f2fe);
    color: var(--color-accent-cyan, #00f2fe);
    box-shadow: 0 0 12px color-mix(in srgb, var(--color-accent-cyan, #00f2fe) 15%, transparent);
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

</style>
