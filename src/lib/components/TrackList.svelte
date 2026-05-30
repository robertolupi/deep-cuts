<script lang="ts">
  import type { Track } from '../types';
  import RangeSlider from './RangeSlider.svelte';
  import { formatDuration } from '$lib/utils/format';

  let {
    tracks,
    selectedTrack,
    isPlaying,
    searchQuery = $bindable(),
    genreFilter = $bindable(),
    minBpm = $bindable(20),
    maxBpm = $bindable(250),
    selectedKey = $bindable("All"),
    onTrackSelect,
    activeTab = $bindable()
  }: {
    tracks: Track[];
    selectedTrack: Track | null;
    isPlaying: boolean;
    searchQuery: string;
    genreFilter: string;
    minBpm: number;
    maxBpm: number;
    selectedKey: string;
    onTrackSelect: (track: Track) => void;
    activeTab?: string;
  } = $props();

  // BPM filter popup state
  let isBpmPopupOpen = $state(false);
  let bpmContainer = $state<HTMLDivElement | null>(null);

  // Genre autocomplete state
  let isGenreFocused = $state(false);
  let genreInputEl = $state<HTMLInputElement | null>(null);

  // All distinct genres from both metadata and essentia, sorted
  let allGenres = $derived.by(() => {
    const set = new Set<string>();
    for (const t of tracks) {
      if (t.genre) {
        for (const g of t.genre.split(/[,;]/)) {
          const trimmed = g.trim();
          if (trimmed) set.add(trimmed);
        }
      }
      if (t.detected_genre) set.add(t.detected_genre);
    }
    return Array.from(set).sort();
  });

  // Suggestions: genres matching the current filter text (max 12)
  let genreSuggestions = $derived.by(() => {
    const q = genreFilter.trim().toLowerCase();
    if (!q) return [];
    return allGenres.filter(g => g.toLowerCase().includes(q)).slice(0, 12);
  });

  function selectGenreSuggestion(genre: string) {
    genreFilter = genre;
    isGenreFocused = false;
    genreInputEl?.blur();
  }

  // Derived list of distinct keys reactively computed from tracks
  let keysList = $derived.by(() => {
    const list = new Set<string>();
    for (const t of tracks) {
      if (t.key && t.scale) {
        const scaleLabel = t.scale.toLowerCase() === 'minor' ? 'minor' : 'major';
        list.add(`${t.key} ${scaleLabel}`);
      }
    }
    return ["All", ...Array.from(list).sort()];
  });

  // Derived list of filtered tracks reactively matching search box and genre/key/BPM selections
  let filteredTracks = $derived.by(() => {
    return tracks.filter(t => {
      // 1. Genre filter — partial case-insensitive match against metadata genre or detected_genre
      if (genreFilter.trim()) {
        const q = genreFilter.trim().toLowerCase();
        const metaMatch = t.genre?.toLowerCase().includes(q) ?? false;
        const detectedMatch = t.detected_genre?.toLowerCase().includes(q) ?? false;
        if (!metaMatch && !detectedMatch) return false;
      }
      
      // 2. Key filter
      if (selectedKey !== "All") {
        if (!t.key || !t.scale) return false;
        const keyLabel = `${t.key} ${t.scale.toLowerCase()}`;
        if (keyLabel.toLowerCase() !== selectedKey.toLowerCase()) {
          return false;
        }
      }

      // 3. BPM filter
      if (minBpm > 20 || maxBpm < 250) {
        if (t.bpm === null || t.bpm === undefined) return false;
        if (t.bpm < minBpm || t.bpm > maxBpm) return false;
      }
      
      // 4. Search text filter
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

  let displayLimit = $state(150);

  $effect(() => {
    // Reactively reset limit to 150 when filters or search query change
    searchQuery;
    genreFilter;
    selectedKey;
    minBpm;
    maxBpm;
    displayLimit = 150;
  });

  let displayedTracks = $derived(filteredTracks.slice(0, displayLimit));

  function setBpmPreset(minVal: number, maxVal: number) {
    minBpm = minVal;
    maxBpm = maxVal;
  }

  function handleWindowClick(e: MouseEvent) {
    if (isBpmPopupOpen && bpmContainer && !bpmContainer.contains(e.target as Node)) {
      isBpmPopupOpen = false;
    }
  }
</script>

<svelte:window onclick={handleWindowClick} />

<div class="bottom-pane-scroller glass-panel">
  <!-- Filters & search Row -->
  <div class="tracks-toolbar">
    <div style="display: flex; gap: 1rem; align-items: center; flex: 1; position: relative;">
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

      <!-- Genre Filter with autocomplete -->
      <div class="genre-filter-container">
        <div class="search-box-wrap" style="max-width: 200px;">
          <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="var(--text-muted)" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round" class="search-icon">
            <path d="M20.59 13.41l-7.17 7.17a2 2 0 0 1-2.83 0L2 12V2h10l8.59 8.59a2 2 0 0 1 0 2.82z"/>
            <line x1="7" y1="7" x2="7.01" y2="7"/>
          </svg>
          <input
            type="text"
            placeholder="Filter by genre…"
            bind:value={genreFilter}
            bind:this={genreInputEl}
            class="search-input"
            onfocus={() => isGenreFocused = true}
            onblur={() => setTimeout(() => { isGenreFocused = false; }, 150)}
          />
        </div>
        {#if isGenreFocused && genreSuggestions.length > 0}
          <div class="genre-suggestions glass-panel">
            {#each genreSuggestions as suggestion}
              <button
                class="genre-suggestion-item"
                type="button"
                onmousedown={() => selectGenreSuggestion(suggestion)}
              >{suggestion}</button>
            {/each}
          </div>
        {/if}
      </div>

      <!-- Key Filter -->
      <div class="filter-select-wrap">
        <select bind:value={selectedKey} class="filter-select" aria-label="Key Filter">
          <option value="All">🎹 All Keys</option>
          {#each keysList.filter(k => k !== "All") as keyItem}
            <option value={keyItem}>{keyItem}</option>
          {/each}
        </select>
      </div>

      <!-- BPM Filter Container -->
      <div class="bpm-filter-container" bind:this={bpmContainer}>
        <button 
          class="bpm-filter-btn {minBpm > 20 || maxBpm < 250 ? 'active' : ''}" 
          onclick={() => isBpmPopupOpen = !isBpmPopupOpen}
          type="button"
        >
          ⏱️ BPM: {minBpm === 20 && maxBpm === 250 ? 'All' : `${Math.round(minBpm)}-${Math.round(maxBpm)}`}
        </button>
        
        {#if isBpmPopupOpen}
          <div class="bpm-popup glass-panel">
            <div class="bpm-popup-header">
              <span class="bpm-popup-title">BPM Range</span>
              <button class="btn-close-sm" onclick={() => isBpmPopupOpen = false} type="button">&times;</button>
            </div>
            <div class="bpm-slider-wrapper">
              <RangeSlider
                min={20}
                max={250}
                step={1}
                bind:minValue={minBpm}
                bind:maxValue={maxBpm}
                unit="BPM"
              />
            </div>
            <div class="bpm-presets">
              <button class="preset-btn {minBpm === 60 && maxBpm === 90 ? 'active' : ''}" onclick={() => setBpmPreset(60, 90)} type="button">Slow</button>
              <button class="preset-btn {minBpm === 90 && maxBpm === 125 ? 'active' : ''}" onclick={() => setBpmPreset(90, 125)} type="button">Mid</button>
              <button class="preset-btn {minBpm === 125 && maxBpm === 150 ? 'active' : ''}" onclick={() => setBpmPreset(125, 150)} type="button">Fast</button>
              <button class="preset-btn {minBpm === 150 && maxBpm === 250 ? 'active' : ''}" onclick={() => setBpmPreset(150, 250)} type="button">V. Fast</button>
              <button class="preset-btn preset-btn-full {minBpm === 20 && maxBpm === 250 ? 'active' : ''}" onclick={() => setBpmPreset(20, 250)} type="button">All</button>
            </div>
          </div>
        {/if}
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
                  {#if !track.title}
                    <span class="file-tag">file</span>
                  {/if}
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

        {#if filteredTracks.length > displayLimit}
          <div class="load-more-container">
            <button 
              class="load-more-btn" 
              onclick={() => displayLimit += 150}
              type="button"
            >
              <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" class="load-more-icon">
                <path d="m6 9 6 6 6-6"/>
              </svg>
              Load More Tracks ({filteredTracks.length - displayLimit} remaining)
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

  .bpm-filter-container {
    position: relative;
    display: inline-block;
  }

  .bpm-filter-btn {
    background: var(--bg-card);
    border: 1px solid var(--border-color);
    color: var(--text-primary);
    padding: 0.35rem 0.75rem;
    font-size: 0.85rem;
    font-family: inherit;
    border-radius: 4px;
    cursor: pointer;
    display: flex;
    align-items: center;
    gap: 0.35rem;
    transition: all 0.2s ease;
    height: 38px; /* aligns perfectly with filter-select */
    box-sizing: border-box;
  }

  .bpm-filter-btn:hover {
    border-color: var(--color-primary);
    background: color-mix(in srgb, var(--color-primary) 5%, var(--bg-card));
  }

  .bpm-filter-btn.active {
    border-color: var(--color-accent-cyan);
    color: var(--color-accent-cyan);
    background: color-mix(in srgb, var(--color-accent-cyan) 8%, var(--bg-card));
    box-shadow: 0 0 8px color-mix(in srgb, var(--color-accent-cyan) 15%, transparent);
  }

  .bpm-popup {
    position: absolute;
    top: calc(100% + 8px);
    left: 0;
    width: 250px;
    background: var(--bg-card);
    border: 1px solid var(--border-color);
    border-radius: 6px;
    padding: 1rem;
    z-index: 1000;
    box-shadow: 0 10px 25px rgba(0, 0, 0, 0.25);
    display: flex;
    flex-direction: column;
    gap: 0.85rem;
    backdrop-filter: blur(12px);
  }

  .bpm-popup-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    border-bottom: 1px solid var(--border-color);
    padding-bottom: 0.45rem;
  }

  .bpm-popup-title {
    font-size: 0.75rem;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    font-weight: 700;
    color: var(--text-muted);
  }

  .btn-close-sm {
    background: transparent;
    border: none;
    color: var(--text-muted);
    font-size: 1.1rem;
    cursor: pointer;
    padding: 0;
    line-height: 1;
  }

  .btn-close-sm:hover {
    color: var(--text-primary);
  }

  .bpm-slider-wrapper {
    padding: 0.25rem 0.5rem;
  }

  .bpm-presets {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 0.35rem;
    border-top: 1px solid var(--border-color);
    padding-top: 0.6rem;
  }

  .preset-btn {
    background: color-mix(in srgb, var(--text-muted) 6%, transparent);
    border: 1px solid var(--border-color);
    color: var(--text-secondary);
    padding: 0.25rem 0;
    font-size: 0.68rem;
    font-weight: 600;
    border-radius: 3px;
    cursor: pointer;
    transition: all 0.15s ease;
    text-align: center;
  }

  .preset-btn:hover {
    border-color: var(--color-primary);
    color: var(--text-primary);
    background: color-mix(in srgb, var(--color-primary) 8%, transparent);
  }

  .preset-btn.active {
    background: color-mix(in srgb, var(--color-accent-cyan) 10%, transparent);
    border-color: var(--color-accent-cyan);
    color: var(--color-accent-cyan);
  }

  .preset-btn-full {
    grid-column: span 4;
    margin-top: 0.15rem;
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

  .genre-filter-container {
    position: relative;
    display: inline-block;
  }

  .genre-suggestions {
    position: absolute;
    top: calc(100% + 6px);
    left: 0;
    min-width: 220px;
    max-height: 260px;
    overflow-y: auto;
    background: var(--bg-card);
    border: 1px solid var(--border-color);
    border-radius: 6px;
    padding: 0.3rem 0;
    z-index: 1000;
    box-shadow: 0 10px 25px rgba(0, 0, 0, 0.3);
    backdrop-filter: blur(12px);
    display: flex;
    flex-direction: column;
  }

  .genre-suggestion-item {
    background: transparent;
    border: none;
    color: var(--text-secondary);
    text-align: left;
    padding: 0.35rem 0.75rem;
    font-size: 0.82rem;
    font-family: inherit;
    cursor: pointer;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    transition: background 0.1s ease, color 0.1s ease;
  }

  .genre-suggestion-item:hover {
    background: color-mix(in srgb, var(--color-accent-cyan) 10%, transparent);
    color: var(--color-accent-cyan);
  }
</style>
