<script lang="ts">
  import { filters } from "$lib/stores/filters.svelte";
  import { library } from "$lib/stores/library.svelte";
  import RangeSlider from "./RangeSlider.svelte";

  let collapsed = $state(false);

  const NOTE_NAMES = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "Bb", "B"];

  // All distinct genres for autocomplete
  const allGenres = $derived.by(() => {
    const set = new Set<string>();
    for (const t of library.tracks) {
      if (t.genre) {
        for (const g of t.genre.split(/[,;]/)) {
          const s = g.trim();
          if (s) set.add(s);
        }
      }
      if (t.detected_genre) set.add(t.detected_genre);
    }
    return Array.from(set).sort();
  });

  let isGenreFocused = $state(false);
  let genreInputEl = $state<HTMLInputElement | null>(null);

  const genreSuggestions = $derived.by(() => {
    const q = filters.genreFilter.trim().toLowerCase();
    if (!q) return [];
    return allGenres.filter(g => g.toLowerCase().includes(q)).slice(0, 12);
  });

  function selectGenre(genre: string) {
    filters.genreFilter = genre;
    isGenreFocused = false;
    genreInputEl?.blur();
  }

  const hasActiveFilters = $derived(
    filters.searchQuery !== "" ||
    filters.semanticQuery !== "" ||
    filters.clapQuery !== "" ||
    filters.genreFilter !== "" ||
    filters.selectedDirectoryIds.length > 0 ||
    filters.selectedKeys.length > 0 ||
    filters.selectedScale !== "all" ||
    filters.minBpm !== 20 ||
    filters.maxBpm !== 250 ||
    filters.musicOnly ||
    filters.vocalFilter !== "all" ||
    filters.similarToTrack !== null
  );

  function clearAll() {
    filters.searchQuery   = "";
    filters.semanticQuery = "";
    filters.clapQuery     = "";
    filters.genreFilter   = "";
    filters.clearDirectories();
    filters.clearKeys();
    filters.selectedScale = "all";
    filters.minBpm        = 20;
    filters.maxBpm        = 250;
    filters.musicOnly     = false;
    filters.vocalFilter   = "all";
    filters.clearSimilar();
  }
</script>

<aside class="filter-sidebar" class:collapsed>
  {#if !collapsed}
  <div class="sidebar-inner">
    <!-- Header -->
    <div class="sidebar-header">
      <div>
        <span class="sidebar-title">Library Filter</span>
        <span class="sidebar-count">
          {library.trackCount.toLocaleString()} tracks indexed{#if library.staleCount > 0}&thinsp;·&thinsp;<span class="stale-badge">{library.staleCount} updated</span>{/if}
        </span>
      </div>
      <button class="collapse-btn" onclick={() => collapsed = true} title="Collapse sidebar">
        <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
          <polyline points="15 18 9 12 15 6"/>
        </svg>
      </button>
    </div>

    <!-- Search -->
    <div class="sidebar-section">
      <span class="section-label">SEARCH</span>
      <div class="search-inputs-container">
        <!-- Keyword Search -->
        <div class="search-wrap">
          <svg xmlns="http://www.w3.org/2000/svg" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" class="search-icon">
            <circle cx="11" cy="11" r="8"/>
            <line x1="21" y1="21" x2="16.65" y2="16.65"/>
          </svg>
          <input
            type="text"
            placeholder="Keyword: Title, artist, album…"
            bind:value={filters.searchQuery}
            class="search-input"
          />
          {#if filters.searchQuery}
            <button class="clear-x" onclick={() => filters.searchQuery = ""}>×</button>
          {/if}
        </div>

        <!-- Semantic Search (AI Vibes) -->
        <div class="search-wrap semantic-wrap">
          {#if filters.isSemanticLoading}
            <!-- Spinner -->
            <svg class="search-icon ai-spinner" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" xmlns="http://www.w3.org/2000/svg">
              <circle cx="12" cy="12" r="10" stroke="currentColor" stroke-width="3" stroke-dasharray="32" class="spinner-circle" />
            </svg>
          {:else}
            <!-- Sparkles -->
            <svg xmlns="http://www.w3.org/2000/svg" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" class="search-icon">
              <path d="M12 3v1m0 16v1m9-9h-1M4 12H3m15.364-6.364l-.707.707M6.343 17.657l-.707.707m0-12.728l.707.707m11.314 11.314l.707.707M12 7a5 5 0 0 0 0 10 5 5 0 0 0 0-10z" />
            </svg>
          {/if}
          <input
            type="text"
            placeholder="AI Vibe: description, mood…"
            bind:value={filters.semanticQuery}
            class="search-input semantic-search-input"
          />
          {#if filters.semanticQuery}
            <button class="clear-x" onclick={() => filters.semanticQuery = ""}>×</button>
          {/if}
        </div>

        <!-- Sonic Search (AI Sound) -->
        <div class="search-wrap clap-wrap">
          {#if filters.isClapLoading}
            <!-- Spinner -->
            <svg class="search-icon ai-spinner-clap" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" xmlns="http://www.w3.org/2000/svg">
              <circle cx="12" cy="12" r="10" stroke="currentColor" stroke-width="3" stroke-dasharray="32" class="spinner-circle" />
            </svg>
          {:else}
            <!-- Music note -->
            <svg xmlns="http://www.w3.org/2000/svg" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" class="search-icon">
              <path d="M9 18V5l12-2v13" />
              <circle cx="6" cy="18" r="3" />
              <circle cx="18" cy="16" r="3" />
            </svg>
          {/if}
          <input
            type="text"
            placeholder="AI Sound: acoustic texture, genre…"
            bind:value={filters.clapQuery}
            class="search-input clap-search-input"
          />
          {#if filters.clapQuery}
            <button class="clear-x" onclick={() => filters.clapQuery = ""}>×</button>
          {/if}
        </div>
      </div>
    </div>

    <!-- Active filter chips -->
    {#if hasActiveFilters}
      <div class="sidebar-section active-chips">
        {#each filters.selectedDirectoryIds as id}
          {@const dir = library.directories.find(d => d.id === id)}
          {#if dir}
            <button class="chip chip-active" onclick={() => filters.toggleDirectoryId(id)}>
              {dir.name} ×
            </button>
          {/if}
        {/each}
        {#if filters.genreFilter}
          <button class="chip chip-active" onclick={() => filters.genreFilter = ""}>
            {filters.genreFilter} ×
          </button>
        {/if}
        {#if filters.semanticQuery}
          <button class="chip chip-active chip-semantic" onclick={() => filters.semanticQuery = ""}>
            ✨ {filters.semanticQuery} ×
          </button>
        {/if}
        {#if filters.clapQuery}
          <button class="chip chip-active chip-clap" onclick={() => filters.clapQuery = ""}>
            🎵 {filters.clapQuery} ×
          </button>
        {/if}
        {#each filters.selectedKeys as k}
          <button class="chip chip-active" onclick={() => filters.toggleKey(k)}>
            {k} ×
          </button>
        {/each}
        {#if filters.selectedScale !== "all"}
          <button class="chip chip-active" onclick={() => filters.selectedScale = "all"}>
            {filters.selectedScale} ×
          </button>
        {/if}
        {#if filters.minBpm !== 20 || filters.maxBpm !== 250}
          <button class="chip chip-active" onclick={() => { filters.minBpm = 20; filters.maxBpm = 250; }}>
            {Math.round(filters.minBpm)}–{Math.round(filters.maxBpm)} BPM ×
          </button>
        {/if}
        {#if filters.musicOnly}
          <button class="chip chip-active" onclick={() => filters.musicOnly = false}>
            Music only ×
          </button>
        {/if}
        {#if filters.vocalFilter !== "all"}
          <button class="chip chip-active" onclick={() => filters.vocalFilter = "all"}>
            {filters.vocalFilter === "voice" ? "Vocals" : "Instrumental"} ×
          </button>
        {/if}
        {#if filters.similarToTrack}
          <button class="chip chip-active chip-similar" onclick={() => filters.clearSimilar()}>
            ≈ {filters.similarToTrack.title} ×
          </button>
        {/if}
        <button class="chip chip-clear" onclick={clearAll}>Clear all</button>
      </div>
    {/if}

    <!-- Watched directory filter -->
    {#if library.directories.length > 1}
    <div class="sidebar-section">
      <div class="section-label-row">
        <span class="section-label">FOLDERS</span>
        {#if filters.selectedDirectoryIds.length > 0}
          <button class="label-clear" onclick={() => filters.clearDirectories()}>Clear</button>
        {/if}
      </div>
      <div class="dir-list">
        {#each library.directories as dir}
          <button
            class="dir-btn"
            class:dir-active={filters.selectedDirectoryIds.includes(dir.id)}
            onclick={() => filters.toggleDirectoryId(dir.id)}
            title={dir.path}
          >
            <svg xmlns="http://www.w3.org/2000/svg" width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" class="dir-icon">
              <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/>
            </svg>
            <span class="dir-name">{dir.name}</span>
            {#if filters.selectedDirectoryIds.includes(dir.id)}
              <svg xmlns="http://www.w3.org/2000/svg" width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round" class="dir-check">
                <polyline points="20 6 9 17 4 12"/>
              </svg>
            {/if}
          </button>
        {/each}
      </div>
    </div>
    {/if}

    <!-- Genre filter -->
    <div class="sidebar-section">
      <span class="section-label">GENRE</span>
      <div class="genre-wrap">
        <div class="search-wrap">
          <svg xmlns="http://www.w3.org/2000/svg" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" class="search-icon">
            <path d="M20.59 13.41l-7.17 7.17a2 2 0 0 1-2.83 0L2 12V2h10l8.59 8.59a2 2 0 0 1 0 2.82z"/>
            <line x1="7" y1="7" x2="7.01" y2="7"/>
          </svg>
          <input
            type="text"
            placeholder="Filter by genre…"
            bind:value={filters.genreFilter}
            bind:this={genreInputEl}
            class="search-input"
            onfocus={() => isGenreFocused = true}
            onblur={() => setTimeout(() => { isGenreFocused = false; }, 150)}
          />
          {#if filters.genreFilter}
            <button class="clear-x" onclick={() => filters.genreFilter = ""}>×</button>
          {/if}
        </div>
        {#if isGenreFocused && genreSuggestions.length > 0}
          <div class="genre-suggestions">
            {#each genreSuggestions as suggestion}
              <button class="genre-suggestion-item" onmousedown={() => selectGenre(suggestion)}>
                {suggestion}
              </button>
            {/each}
          </div>
        {/if}
      </div>
    </div>

    <!-- Key filter -->
    <div class="sidebar-section">
      <div class="section-label-row">
        <span class="section-label">KEY</span>
        {#if filters.selectedKeys.length > 0}
          <button class="label-clear" onclick={() => filters.clearKeys()}>Clear</button>
        {/if}
      </div>
      <div class="key-note-grid">
        {#each NOTE_NAMES as note}
          <button
            class="key-btn"
            class:key-active={filters.selectedKeys.includes(note)}
            onclick={() => filters.toggleKey(note)}
          >{note}</button>
        {/each}
      </div>

      <div class="scale-toggle">
        {#each [["all", "All"], ["major", "Maj"], ["minor", "Min"]] as [val, label]}
          <button
            class="scale-btn"
            class:scale-active={filters.selectedScale === val}
            onclick={() => filters.selectedScale = val as "all" | "major" | "minor"}
          >{label}</button>
        {/each}
      </div>
    </div>

    <!-- BPM Range -->
    <div class="sidebar-section">
      <div class="section-label-row">
        <span class="section-label">BPM RANGE</span>
        <span class="section-value">{Math.round(filters.minBpm)} – {Math.round(filters.maxBpm)}</span>
      </div>
      <RangeSlider
        min={20}
        max={250}
        step={1}
        bind:minValue={filters.minBpm}
        bind:maxValue={filters.maxBpm}
        unit="BPM"
      />
      <div class="bpm-presets">
        <button class="preset-btn" class:active={filters.minBpm===60&&filters.maxBpm===90}    onclick={() => { filters.minBpm=60;  filters.maxBpm=90;  }}>Slow</button>
        <button class="preset-btn" class:active={filters.minBpm===90&&filters.maxBpm===125}   onclick={() => { filters.minBpm=90;  filters.maxBpm=125; }}>Mid</button>
        <button class="preset-btn" class:active={filters.minBpm===125&&filters.maxBpm===150}  onclick={() => { filters.minBpm=125; filters.maxBpm=150; }}>Fast</button>
        <button class="preset-btn" class:active={filters.minBpm===150&&filters.maxBpm===250}  onclick={() => { filters.minBpm=150; filters.maxBpm=250; }}>V.Fast</button>
        <button class="preset-btn" class:active={filters.minBpm===20&&filters.maxBpm===250}   onclick={() => { filters.minBpm=20;  filters.maxBpm=250; }}>All</button>
      </div>
    </div>
    <!-- Vocal / Instrumental -->
    <div class="sidebar-section">
      <span class="section-label">VOCALS</span>
      <div class="scale-toggle">
        {#each [["all", "All"], ["voice", "Vocals"], ["instrumental", "Instrumental"]] as [val, label]}
          <button
            class="scale-btn"
            class:scale-active={filters.vocalFilter === val}
            onclick={() => filters.vocalFilter = val as "all" | "voice" | "instrumental"}
          >{label}</button>
        {/each}
      </div>
    </div>

    <!-- Music only toggle -->
    <div class="sidebar-section">
      <label class="toggle-row">
        <span class="section-label" style="margin-bottom:0;">MUSIC ONLY</span>
        <button
          class="toggle-btn"
          class:toggle-on={filters.musicOnly}
          onclick={() => filters.musicOnly = !filters.musicOnly}
          title="Hide tracks classified as non-music"
          role="switch"
          aria-checked={filters.musicOnly}
        >
          <span class="toggle-knob"></span>
        </button>
      </label>
      <p class="toggle-hint">Hides tracks Essentia classified as Non-Music (audiobooks, spoken word, etc.)</p>
    </div>
  </div>
  {:else}
  <!-- Collapsed tab -->
  <button class="expand-btn" onclick={() => collapsed = false} title="Expand sidebar">
    <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
      <polyline points="9 18 15 12 9 6"/>
    </svg>
    {#if hasActiveFilters}<span class="active-dot"></span>{/if}
  </button>
  {/if}
</aside>

<style>
  .filter-sidebar {
    display: flex;
    flex-direction: column;
    width: var(--sg-sidebar-width, 260px);
    height: 100%;
    background: var(--sg-surface-slate, #161b22);
    border-right: 1px solid rgba(255,255,255,0.08);
    overflow: hidden;
    flex-shrink: 0;
    transition: width 0.2s ease;
  }

  .filter-sidebar.collapsed {
    width: 32px;
  }

  .sidebar-inner {
    display: flex;
    flex-direction: column;
    gap: 0;
    height: 100%;
    overflow-y: auto;
    overflow-x: hidden;
    padding: 1rem 0.75rem;
    scrollbar-width: thin;
    scrollbar-color: rgba(255,255,255,0.1) transparent;
  }

  /* ── Header ── */
  .sidebar-header {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    margin-bottom: 1rem;
  }

  .sidebar-title {
    display: block;
    font-family: "JetBrains Mono", monospace;
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--sg-on-surface, #e3e1e9);
  }

  .sidebar-count {
    display: block;
    font-family: "JetBrains Mono", monospace;
    font-size: 10px;
    color: var(--sg-outline, #849495);
    margin-top: 2px;
  }

  .stale-badge {
    color: var(--sg-secondary, #fe00fe);
    font-weight: 700;
  }

  .collapse-btn, .expand-btn {
    background: none;
    border: none;
    cursor: pointer;
    color: var(--sg-outline, #849495);
    padding: 2px;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 3px;
    flex-shrink: 0;
  }

  .collapse-btn:hover, .expand-btn:hover {
    color: var(--sg-on-surface, #e3e1e9);
    background: rgba(255,255,255,0.05);
  }

  .expand-btn {
    width: 32px;
    height: 100%;
    position: relative;
  }

  .active-dot {
    position: absolute;
    top: 12px;
    right: 6px;
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--sg-primary, #00f0ff);
  }

  /* ── Sections ── */
  .sidebar-section {
    padding: 0.65rem 0;
    border-top: 1px solid rgba(255,255,255,0.06);
  }

  .section-label {
    display: block;
    font-family: "JetBrains Mono", monospace;
    font-size: 9px;
    font-weight: 700;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--sg-outline, #849495);
    margin-bottom: 0.5rem;
  }

  .section-label-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 0.5rem;
  }

  .section-label-row .section-label {
    margin-bottom: 0;
  }

  .section-value {
    font-family: "JetBrains Mono", monospace;
    font-size: 10px;
    color: var(--sg-primary, #00f0ff);
  }

  .label-clear {
    font-family: "JetBrains Mono", monospace;
    font-size: 9px;
    color: var(--sg-outline, #849495);
    background: none;
    border: none;
    cursor: pointer;
    padding: 0;
    text-decoration: underline;
  }

  .label-clear:hover { color: var(--sg-on-surface, #e3e1e9); }

  /* ── Search ── */
  .search-wrap {
    position: relative;
    display: flex;
    align-items: center;
  }

  .search-icon {
    position: absolute;
    left: 8px;
    color: var(--sg-outline, #849495);
    pointer-events: none;
  }

  .search-input {
    width: 100%;
    background: var(--sg-surface-container, #1e1f25);
    border: 1px solid rgba(255,255,255,0.08);
    border-radius: 4px;
    padding: 0.4rem 0.5rem 0.4rem 2rem;
    font-family: "JetBrains Mono", monospace;
    font-size: 11px;
    color: var(--sg-on-surface, #e3e1e9);
    outline: none;
    transition: border-color 0.15s;
  }

  .search-input::placeholder { color: var(--sg-outline, #849495); }
  .search-input:focus { border-color: var(--sg-primary, #00f0ff); }

  .clear-x {
    position: absolute;
    right: 6px;
    background: none;
    border: none;
    cursor: pointer;
    color: var(--sg-outline, #849495);
    font-size: 14px;
    line-height: 1;
    padding: 0 2px;
  }

  /* ── Chips ── */
  .active-chips {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }

  .genre-wrap {
    position: relative;
  }

  .genre-suggestions {
    position: absolute;
    top: calc(100% + 4px);
    left: 0;
    right: 0;
    max-height: 200px;
    overflow-y: auto;
    background: var(--sg-surface-container, #1e1f25);
    border: 1px solid rgba(255,255,255,0.12);
    border-radius: 4px;
    z-index: 100;
    display: flex;
    flex-direction: column;
    scrollbar-width: thin;
    scrollbar-color: rgba(255,255,255,0.1) transparent;
  }

  .genre-suggestion-item {
    background: none;
    border: none;
    text-align: left;
    padding: 6px 10px;
    font-family: "JetBrains Mono", monospace;
    font-size: 11px;
    color: var(--sg-outline, #849495);
    cursor: pointer;
    transition: background 0.1s, color 0.1s;
  }

  .genre-suggestion-item:hover {
    background: rgba(0,240,255,0.08);
    color: var(--sg-primary, #00f0ff);
  }

  .chip {
    font-family: "JetBrains Mono", monospace;
    font-size: 10px;
    padding: 3px 8px;
    border-radius: 999px;
    border: 1px solid rgba(255,255,255,0.12);
    background: rgba(255,255,255,0.04);
    color: var(--sg-outline, #849495);
    cursor: pointer;
    transition: all 0.15s;
    white-space: nowrap;
  }

  .chip:hover {
    border-color: rgba(0,240,255,0.4);
    color: var(--sg-on-surface, #e3e1e9);
  }

  .chip-active {
    border-color: var(--sg-primary, #00f0ff);
    background: rgba(0,240,255,0.1);
    color: var(--sg-primary, #00f0ff);
  }

  .chip-clear {
    border-color: rgba(255,255,255,0.08);
    color: var(--sg-outline, #849495);
    font-style: italic;
  }

  /* ── Watched directory list ── */
  .dir-list {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .dir-btn {
    display: flex;
    align-items: center;
    gap: 6px;
    width: 100%;
    text-align: left;
    padding: 5px 7px;
    border: 1px solid rgba(255,255,255,0.06);
    border-radius: 3px;
    background: rgba(255,255,255,0.02);
    color: var(--sg-outline, #849495);
    cursor: pointer;
    font-family: "JetBrains Mono", monospace;
    font-size: 10px;
    transition: all 0.12s;
  }

  .dir-btn:hover {
    border-color: rgba(0,240,255,0.3);
    color: var(--sg-on-surface, #e3e1e9);
    background: rgba(0,240,255,0.04);
  }

  .dir-btn.dir-active {
    border-color: var(--sg-primary, #00f0ff);
    background: rgba(0,240,255,0.1);
    color: var(--sg-primary, #00f0ff);
  }

  .dir-icon { flex-shrink: 0; }

  .dir-name {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .dir-check { flex-shrink: 0; margin-left: auto; }

  .chip-similar {
    border-color: rgba(254,0,254,0.45) !important;
    background: rgba(254,0,254,0.08) !important;
    color: var(--sg-secondary, #fe00fe) !important;
    max-width: 180px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .chip-semantic {
    border-color: rgba(0, 240, 255, 0.45) !important;
    background: rgba(0, 240, 255, 0.08) !important;
    color: var(--sg-primary, #00f0ff) !important;
    max-width: 180px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .search-inputs-container {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .semantic-wrap .search-icon {
    color: var(--sg-primary, #00f0ff);
  }

  .semantic-search-input {
    border-color: rgba(0, 240, 255, 0.15) !important;
  }

  .semantic-search-input:focus {
    border-color: var(--sg-primary, #00f0ff) !important;
    box-shadow: 0 0 8px rgba(0, 240, 255, 0.15);
  }

  /* Spinner Animation */
  @keyframes spin {
    to { transform: rotate(360deg); }
  }

  .ai-spinner {
    animation: spin 0.8s linear infinite;
    color: var(--sg-primary, #00f0ff) !important;
  }

  .spinner-circle {
    stroke-linecap: round;
    opacity: 0.75;
  }

  .chip-clap {
    border-color: rgba(254, 0, 254, 0.45) !important;
    background: rgba(254, 0, 254, 0.08) !important;
    color: var(--sg-secondary, #fe00fe) !important;
    max-width: 180px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .clap-wrap .search-icon {
    color: var(--sg-secondary, #fe00fe);
  }

  .clap-search-input {
    border-color: rgba(254, 0, 254, 0.15) !important;
  }

  .clap-search-input:focus {
    border-color: var(--sg-secondary, #fe00fe) !important;
    box-shadow: 0 0 8px rgba(254, 0, 254, 0.15);
  }

  .ai-spinner-clap {
    animation: spin 0.8s linear infinite;
    color: var(--sg-secondary, #fe00fe) !important;
  }

  /* ── Key filter ── */
  .key-note-grid {
    display: grid;
    grid-template-columns: repeat(6, 1fr);
    gap: 3px;
    margin-bottom: 6px;
  }

  .key-btn {
    font-family: "JetBrains Mono", monospace;
    font-size: 10px;
    padding: 5px 2px;
    text-align: center;
    border: 1px solid rgba(255,255,255,0.08);
    background: rgba(255,255,255,0.02);
    color: var(--sg-outline, #849495);
    cursor: pointer;
    border-radius: 2px;
    transition: all 0.12s;
  }

  .key-btn:hover {
    border-color: rgba(0,240,255,0.3);
    color: var(--sg-on-surface, #e3e1e9);
    background: rgba(0,240,255,0.05);
  }

  .key-active {
    border-color: var(--sg-primary, #00f0ff);
    background: rgba(0,240,255,0.12);
    color: var(--sg-primary, #00f0ff);
  }

  .scale-toggle {
    display: flex;
    gap: 4px;
    margin-top: 4px;
  }

  .scale-btn {
    flex: 1;
    font-family: "JetBrains Mono", monospace;
    font-size: 10px;
    padding: 5px 4px;
    text-align: center;
    border: 1px solid rgba(255,255,255,0.08);
    background: rgba(255,255,255,0.02);
    color: var(--sg-outline, #849495);
    cursor: pointer;
    border-radius: 2px;
    transition: all 0.12s;
  }

  .scale-btn:hover {
    color: var(--sg-on-surface, #e3e1e9);
    border-color: rgba(0,240,255,0.3);
  }

  .scale-active {
    border-color: var(--sg-primary, #00f0ff);
    background: rgba(0,240,255,0.12);
    color: var(--sg-primary, #00f0ff);
  }

  /* ── BPM presets ── */
  .bpm-presets {
    display: flex;
    gap: 4px;
    margin-top: 0.5rem;
    flex-wrap: wrap;
  }

  .preset-btn {
    font-family: "JetBrains Mono", monospace;
    font-size: 9px;
    padding: 3px 7px;
    border: 1px solid rgba(255,255,255,0.1);
    background: rgba(255,255,255,0.03);
    color: var(--sg-outline, #849495);
    cursor: pointer;
    border-radius: 3px;
    transition: all 0.12s;
  }

  .preset-btn:hover { color: var(--sg-on-surface, #e3e1e9); }
  .preset-btn.active {
    border-color: var(--sg-primary, #00f0ff);
    color: var(--sg-primary, #00f0ff);
    background: rgba(0,240,255,0.08);
  }

  /* ── Music only toggle ── */
  .toggle-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    cursor: pointer;
  }

  .toggle-btn {
    position: relative;
    width: 32px;
    height: 18px;
    border-radius: 999px;
    border: 1px solid rgba(255,255,255,0.15);
    background: rgba(255,255,255,0.06);
    cursor: pointer;
    padding: 0;
    transition: background 0.2s, border-color 0.2s;
    flex-shrink: 0;
  }

  .toggle-btn.toggle-on {
    background: rgba(0,240,255,0.2);
    border-color: var(--sg-primary, #00f0ff);
  }

  .toggle-knob {
    position: absolute;
    top: 2px;
    left: 2px;
    width: 12px;
    height: 12px;
    border-radius: 50%;
    background: var(--sg-outline, #849495);
    transition: transform 0.2s, background 0.2s;
  }

  .toggle-on .toggle-knob {
    transform: translateX(14px);
    background: var(--sg-primary, #00f0ff);
  }

  .toggle-hint {
    font-family: "JetBrains Mono", monospace;
    font-size: 9px;
    color: var(--sg-outline, #849495);
    margin: 4px 0 0;
    opacity: 0.7;
    line-height: 1.4;
  }
</style>
