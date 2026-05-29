<script lang="ts">
  import type { Track } from '../types';

  let {
    tracks,
    selectedTrack,
    isPlaying,
    searchQuery = $bindable(),
    selectedGenre = $bindable(),
    onTrackSelect,
    formatDuration,
    activeTab = $bindable()
  }: {
    tracks: Track[];
    selectedTrack: Track | null;
    isPlaying: boolean;
    searchQuery: string;
    selectedGenre: string;
    onTrackSelect: (track: Track) => void;
    formatDuration: (sec: number) => string;
    activeTab?: string;
  } = $props();

  // Derived list of distinct genres reactively computed from tracks
  let genresList = $derived.by(() => {
    const list = new Set<string>();
    for (const t of tracks) {
      if (t.genre) {
        for (const g of t.genre.split(/[,;]/)) {
          const trimmed = g.trim();
          if (trimmed) list.add(trimmed);
        }
      }
    }
    return ["All", ...Array.from(list).sort()];
  });

  // Derived list of filtered tracks reactively matching search box and genre selections
  let filteredTracks = $derived.by(() => {
    return tracks.filter(t => {
      // 1. Genre filter
      if (selectedGenre !== "All") {
        if (!t.genre || !t.genre.toLowerCase().includes(selectedGenre.toLowerCase())) {
          return false;
        }
      }
      
      // 2. Search text filter
      if (searchQuery.trim()) {
        const query = searchQuery.toLowerCase();
        const matchesTitle = t.title?.toLowerCase().includes(query) ?? false;
        const matchesArtist = t.artist?.toLowerCase().includes(query) ?? false;
        const matchesAlbum = t.album?.toLowerCase().includes(query) ?? false;
        const matchesFilename = t.filename.toLowerCase().includes(query);
        return matchesTitle || matchesArtist || matchesAlbum || matchesFilename;
      }
      
      return true;
    });
  });
</script>

<div class="bottom-pane-scroller glass-panel">
  <!-- Filters & search Row -->
  <div class="tracks-toolbar">
    <div style="display: flex; gap: 1rem; align-items: center; flex: 1;">
      <!-- Search box -->
      <div class="search-box-wrap">
        <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="var(--text-muted)" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round" class="search-icon">
          <circle cx="11" cy="11" r="8"/>
          <line x1="21" y1="21" x2="16.65" y2="16.65"/>
        </svg>
        <input 
          type="text" 
          placeholder="Search tracks by title, artist, album, filename..." 
          bind:value={searchQuery}
          class="search-input"
        />
      </div>

      <!-- Genre Filter -->
      <div class="filter-select-wrap">
        <select bind:value={selectedGenre} class="filter-select" aria-label="Genre Filter">
          {#each genresList as genre}
            <option value={genre}>{genre === "All" ? "🏷️ All Genres" : genre}</option>
          {/each}
        </select>
      </div>
    </div>

    <!-- Library metadata count badge -->
    <div class="library-count-badge">
      <code>{filteredTracks.length} / {tracks.length} tracks</code>
    </div>
  </div>

  <!-- Tracks Grid List Table -->
  {#if tracks.length > 0}
    {#if filteredTracks.length > 0}
      <div class="tracks-table-wrap">
        <table class="tracks-table">
          <thead>
            <tr>
              <th style="width: 40px; text-align: center;">#</th>
              <th>Title / Filename</th>
              <th>Artist</th>
              <th>Album</th>
              <th>Duration</th>
              <th style="width: 60px;">BPM</th>
              <th style="width: 80px;">Key</th>
              <th style="width: 100px;">Format</th>
            </tr>
          </thead>
          <tbody>
            {#each filteredTracks as track, index (track.id)}
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
                  {#if !track.title}
                    <span class="file-tag">file</span>
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
      {#if activeTab !== undefined}
        <button class="btn-primary" onclick={() => activeTab = 'settings'} style="margin-top: 0.5rem;">
          Go to Library Settings
        </button>
      {/if}
    </div>
  {/if}
</div>

<style>
  .key-badge {
    font-family: var(--font-mono, monospace);
    color: var(--text-primary);
  }
</style>
