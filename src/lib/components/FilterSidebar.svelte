<script lang="ts">
  import { filters } from "$lib/stores/filters.svelte";
import { library } from "$lib/stores/library.svelte";
import { ui } from "$lib/stores/ui.svelte";
import { curation } from "$lib/stores/curation.svelte";
import RangeSlider from "./RangeSlider.svelte";
import ActiveFilterChips from "./ActiveFilterChips.svelte";
import MoodSection from "./MoodSection.svelte";
import SavedSearchList from "./SavedSearchList.svelte";
import { onMount } from "svelte";
import { invoke } from "$lib/ipc";
import GenreAutocomplete from "./GenreAutocomplete.svelte";
import TagsAutocomplete from "./TagsAutocomplete.svelte";
import Autocomplete from "./Autocomplete.svelte";
import CollapsiblePane from "./CollapsiblePane.svelte";

  // Tag autocomplete
  let tagInput = $state("");
  let structureFocused = $state(false);

  let folderSearchInput = $state("");
  let playlistSearchInput = $state("");

  $effect(() => {
    if (!curation.activePlaylist) {
      playlistSearchInput = "";
    } else if (curation.activePlaylist.name !== playlistSearchInput) {
      playlistSearchInput = curation.activePlaylist.name;
    }
  });

  const playlistSuggestions = $derived.by(() => {
    const q = playlistSearchInput.trim().toLowerCase();
    if (!q) {
      return curation.playlists;
    }
    return curation.playlists.filter(pl => pl.name.toLowerCase().includes(q)).slice(0, 12);
  });

  function addTag(tag: string) {
    filters.toggleTag(tag);
    tagInput = "";
  }
  function onTagKeydown(e: KeyboardEvent) {
    if (e.key === "Enter" && tagInput.trim()) {
      addTag(tagInput.trim());
      e.preventDefault();
    }
    if (e.key === "Escape") { tagInput = ""; }
  }
  let newPlaylistName = $state("");
  let isCreatingPlaylist = $state(false);
  let deletePlaylistId = $state<number | null>(null);

  function applySerializedFilterState(queryJson: string) {
    try {
      const data = JSON.parse(queryJson);
      filters.clearAll();
      if (data.searchQuery) filters.searchQuery = data.searchQuery;
      if (data.semanticQuery) filters.semanticQuery = data.semanticQuery;
      if (data.clapQuery) filters.clapQuery = data.clapQuery;
      if (data.genreFilter) filters.genreFilter = data.genreFilter;
      if (data.minBpm != null) filters.minBpm = data.minBpm;
      if (data.maxBpm != null) filters.maxBpm = data.maxBpm;
      if (data.selectedKeys) filters.selectedKeys = data.selectedKeys;
      if (data.selectedScale) filters.selectedScale = data.selectedScale;
      if (data.musicOnly != null) filters.musicOnly = data.musicOnly;
      if (data.vocalFilter) filters.vocalFilter = data.vocalFilter;
      if (data.selectedDirectoryIds) {
        for (const id of data.selectedDirectoryIds) {
          filters.toggleDirectoryId(id);
        }
      }
    } catch (e) {
      console.error("Failed to parse saved search query:", e);
    }
  }

  function handleOpenSavedSearch(search: import('$lib/types').SavedSearch) {
    curation.activePlaylist = null;
    curation.activePlaylistTracks = [];
    curation.activeSavedSearch = search;
    applySerializedFilterState(search.query_json);
    ui.sidebarTab = "filters";
  }

  onMount(() => {
    curation.loadAll();
  });

  const NOTE_NAMES = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "Bb", "B"];



  function makeHistogram(values: (number | null | undefined)[], bins: number, lo: number, hi: number): number[] {
    const counts = new Array<number>(bins).fill(0);
    const range = hi - lo;
    for (const v of values) {
      if (v == null) continue;
      const idx = Math.min(bins - 1, Math.floor(((v - lo) / range) * bins));
      counts[idx]++;
    }
    const max = Math.max(1, ...counts);
    return counts.map(c => c / max);
  }

  const bpmDistribution = $derived(makeHistogram(library.tracks.map(t => t.bpm), 40, 20, 250));

  const hasMoodData = $derived(library.tracks.some(t => t.mood_happy != null));

  const moodDistributions = $derived({
    happy:      makeHistogram(library.tracks.map(t => t.mood_happy),      20, 0, 1),
    sad:        makeHistogram(library.tracks.map(t => t.mood_sad),        20, 0, 1),
    aggressive: makeHistogram(library.tracks.map(t => t.mood_aggressive), 20, 0, 1),
    relaxed:    makeHistogram(library.tracks.map(t => t.mood_relaxed),    20, 0, 1),
    party:      makeHistogram(library.tracks.map(t => t.mood_party),      20, 0, 1),
    acoustic:   makeHistogram(library.tracks.map(t => t.mood_acoustic),   20, 0, 1),
    electronic: makeHistogram(library.tracks.map(t => t.mood_electronic), 20, 0, 1),
  });



  const hasActiveFilters = $derived(
    filters.searchQuery !== "" ||
    filters.semanticQuery !== "" ||
    filters.clapQuery !== "" ||
    filters.structureFilter !== "" ||
    filters.structureClusterFilter !== null ||
    filters.structureSimilarToTrack !== null ||
    filters.genreFilter !== "" ||
    filters.selectedDirectoryIds.length > 0 ||
    filters.selectedKeys.length > 0 ||
    filters.selectedScale !== "all" ||
    filters.minBpm !== 20 ||
    filters.maxBpm !== 250 ||
    filters.moodHappyMin > 0      || filters.moodHappyMax < 1 ||
    filters.moodSadMin > 0        || filters.moodSadMax < 1 ||
    filters.moodAggressiveMin > 0 || filters.moodAggressiveMax < 1 ||
    filters.moodRelaxedMin > 0    || filters.moodRelaxedMax < 1 ||
    filters.moodPartyMin > 0      || filters.moodPartyMax < 1 ||
    filters.moodAcousticMin > 0   || filters.moodAcousticMax < 1 ||
    filters.moodElectronicMin > 0 || filters.moodElectronicMax < 1 ||
    filters.musicOnly ||
    filters.vocalFilter !== "all" ||
    filters.similarToTrack !== null ||
    filters.selectedTags.length > 0
  );

  function clearAll() {
    filters.searchQuery      = "";
    filters.semanticQuery    = "";
    filters.clapQuery        = "";
    filters.structureFilter  = "";
    filters.structureClusterFilter = null;
    filters.clearStructureSimilar();
    filters.genreFilter   = "";
    filters.clearDirectories();
    filters.clearKeys();
    filters.selectedScale = "all";
    filters.minBpm        = 20;
    filters.maxBpm        = 250;
    filters.musicOnly     = false;
    filters.vocalFilter   = "all";
    filters.clearSimilar();
    filters.clearTags();
    curation.activeSavedSearch = null;
  }
</script>

<CollapsiblePane side="left" width="var(--sg-sidebar-width, 260px)" hasIndicator={hasActiveFilters}>
  {#snippet children({ collapse })}
  <div class="sidebar-inner">
    <!-- Header -->
    <div class="sidebar-header">
      <div>
        <span class="sidebar-title">Library Filter</span>
        <span class="sidebar-count">
          {library.trackCount.toLocaleString()} tracks indexed{#if library.staleCount > 0}&thinsp;·&thinsp;<span class="stale-badge">{library.staleCount} updated</span>{/if}
        </span>
      </div>
      <button class="collapse-btn" onclick={collapse} title="Collapse sidebar">
        <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
          <polyline points="15 18 9 12 15 6"/>
        </svg>
      </button>
    </div>

    <!-- Segmented Tab Selector -->
    <div class="sidebar-tabs">
      <button 
        class="tab-btn" 
        class:active={ui.sidebarTab === 'filters'} 
        onclick={() => ui.sidebarTab = 'filters'}
      >
        🎛️ Filters
      </button>
      <button 
        class="tab-btn" 
        class:active={ui.sidebarTab === 'curations'} 
        onclick={() => ui.sidebarTab = 'curations'}
      >
        📂 Curations
      </button>
    </div>

    {#if ui.sidebarTab === 'filters'}
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

        <!-- Song Structure filter -->
        {#if filters.structureSimilarToTrack}
          <div class="structure-similar-badge">
            <span>≈ {filters.structureSimilarToTrack.title}</span>
            <button class="clear-x" onclick={() => filters.clearStructureSimilar()}>×</button>
          </div>
        {/if}
        <div class="search-wrap structure-wrap">
          <!-- Waveform icon -->
          <svg xmlns="http://www.w3.org/2000/svg" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" class="search-icon structure-icon">
            <line x1="2"  y1="12" x2="2"  y2="12"/>
            <line x1="5"  y1="8"  x2="5"  y2="16"/>
            <line x1="8"  y1="5"  x2="8"  y2="19"/>
            <line x1="11" y1="9"  x2="11" y2="15"/>
            <line x1="14" y1="4"  x2="14" y2="20"/>
            <line x1="17" y1="8"  x2="17" y2="16"/>
            <line x1="20" y1="11" x2="20" y2="13"/>
          </svg>
          <input
            type="text"
            placeholder="song structure regex"
            bind:value={filters.structureFilter}
            class="search-input structure-search-input"
            onfocus={() => structureFocused = true}
            onblur={() => structureFocused = false}
          />
          {#if filters.structureFilter}
            <button class="clear-x" onclick={() => filters.structureFilter = ""}>×</button>
          {/if}
        </div>
        {#if structureFocused}
        <div class="structure-help">
          <div class="structure-legend">
            <code>I</code>=intro <code>V</code>=verse <code>P</code>=pre-chorus <code>C</code>=chorus <code>B</code>=bridge <code>O</code>=outro <code>E</code>=end <code>U</code>=unknown
          </div>
          <div class="structure-examples">
            <code>.</code> any label &nbsp;·&nbsp;
            <code>*</code> zero or more &nbsp;·&nbsp;
            <code>+</code> one or more &nbsp;·&nbsp;
            <code>^</code> start &nbsp;·&nbsp;
            <code>$</code> end
          </div>
          <div class="structure-examples">
            <code>B</code> any bridge &nbsp;·&nbsp;
            <code>^I</code> starts with intro &nbsp;·&nbsp;
            <code>O$</code> ends with outro &nbsp;·&nbsp;
            <code>^I.*O$</code> intro→outro &nbsp;·&nbsp;
            <code>B.*O</code> bridge before outro &nbsp;·&nbsp;
            <code>^[^I]</code> no intro &nbsp;·&nbsp;
            <code>VC</code> verse straight into chorus &nbsp;·&nbsp;
            <code>CC</code> two chorus runs
          </div>
        </div>
        {/if}

        <!-- Genre filter -->
        <div class="genre-wrap">
          <GenreAutocomplete
            bind:value={filters.genreFilter}
            placeholder="Filter by genre…"
          />
        </div>

        <!-- Tag filter -->
        <div class="tag-filter-container">
          {#if filters.selectedTags.length > 0}
            <div class="tag-chips-wrap" style="margin-bottom: 6px;">
              {#each filters.selectedTags as tag}
                <button class="tag-filter-chip tag-filter-chip-active" onclick={() => filters.toggleTag(tag)}>
                  <span class="tfc-label">{tag.split(':').slice(1).join(':')}</span>
                  <span class="tfc-ns">{tag.split(':')[0]}</span>
                  <span class="tfc-remove">×</span>
                </button>
              {/each}
            </div>
          {/if}

          <div class="tag-input-wrap" style="display: flex; align-items: center; gap: 4px; position: relative; width: 100%;">
            <svg xmlns="http://www.w3.org/2000/svg" width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" class="tag-input-icon" style="flex-shrink: 0; margin-left: 8px;">
              <path d="M20.59 13.41l-7.17 7.17a2 2 0 0 1-2.83 0L2 12V2h10l8.59 8.59a2 2 0 0 1 0 2.82z"/>
              <line x1="7" y1="7" x2="7.01" y2="7"/>
            </svg>
            <TagsAutocomplete
              bind:value={tagInput}
              placeholder="Filter by tag..."
              excludeTags={filters.selectedTags}
              onselect={addTag}
              onkeydown={onTagKeydown}
              borderless={true}
            />
          </div>
        </div>

        <!-- Watched directory filter -->
        {#if library.directories.length > 1}
          <div class="directory-filter-container">
            {#if filters.selectedDirectoryIds.length > 0}
              <div class="section-label-row" style="justify-content: flex-end; margin-bottom: 4px;">
                <button class="label-clear" onclick={() => filters.clearDirectories()}>Clear</button>
              </div>
            {/if}

            {#if filters.selectedDirectoryIds.length > 0}
              <div class="tag-chips-wrap" style="margin-bottom: 6px;">
                {#each library.directories.filter(d => filters.selectedDirectoryIds.includes(d.id)) as dir}
                  <button 
                    class="tag-filter-chip tag-filter-chip-active" 
                    onclick={() => filters.toggleDirectoryId(dir.id)}
                    title={dir.path}
                  >
                    <span class="tfc-label">{dir.name}</span>
                    <span class="tfc-remove">×</span>
                  </button>
                {/each}
              </div>
            {/if}

            {#snippet dirItemSnippet(dir: { id: number; name: string; path: string })}
              <span style="font-family: var(--sg-font-mono); font-size: var(--sg-text-xs);">{dir.name}</span>
            {/snippet}

            <div class="tag-input-wrap" style="display: flex; align-items: center; gap: 4px; position: relative; width: 100%;">
              <svg xmlns="http://www.w3.org/2000/svg" width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" class="tag-input-icon" style="flex-shrink: 0; margin-left: 8px; color: var(--sg-outline);">
                <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/>
              </svg>
              <Autocomplete
                bind:value={folderSearchInput}
                placeholder="Filter by folder…"
                options={library.directories.filter(d => !filters.selectedDirectoryIds.includes(d.id) && d.name.toLowerCase().includes(folderSearchInput.toLowerCase()))}
                onselect={(dir) => {
                  filters.toggleDirectoryId(dir.id);
                  folderSearchInput = "";
                }}
                itemSnippet={dirItemSnippet}
                borderless={true}
              />
            </div>
          </div>
        {/if}

        <!-- Playlist Filter -->
        <div class="playlist-filter-container">
          {#snippet playlistItemSnippet(pl: import('$lib/types').Playlist)}
            <span style="font-family: var(--sg-font-mono); font-size: var(--sg-text-xs);">{pl.name}</span>
          {/snippet}

          {#snippet playlistClearButtonSnippet()}
            {#if curation.activePlaylist || playlistSearchInput}
              <button type="button" class="clear-x" onclick={() => {
                playlistSearchInput = "";
                curation.activePlaylist = null;
                curation.activePlaylistTracks = [];
              }}>×</button>
            {/if}
          {/snippet}

          <div class="tag-input-wrap" style="display: flex; align-items: center; gap: 4px; position: relative; width: 100%;">
            <svg xmlns="http://www.w3.org/2000/svg" width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" class="tag-input-icon" style="flex-shrink: 0; margin-left: 8px;">
              <path d="M12 2A10 10 0 0 0 2 12a10 10 0 0 0 10 10 10 10 0 0 0 10-10A10 10 0 0 0 12 2zm0 15a5 5 0 1 1 0-10 5 5 0 0 1 0 10z"/>
              <circle cx="12" cy="12" r="2"/>
            </svg>
            <Autocomplete
              bind:value={playlistSearchInput}
              options={playlistSuggestions}
              placeholder="Filter by playlist…"
              onselect={async (pl) => {
                playlistSearchInput = pl.name;
                curation.activePlaylist = pl;
                await curation.loadPlaylistTracks(pl.id);
              }}
              itemSnippet={playlistItemSnippet}
              buttonSnippet={playlistClearButtonSnippet}
              borderless={true}
            />
          </div>
        </div>
      </div>
    </div>

    <!-- Active filter chips & Saved Search actions -->
    <ActiveFilterChips {hasActiveFilters} {clearAll} />


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
        distribution={bpmDistribution}
      />
      <div class="bpm-presets">
        <button class="preset-btn" class:active={filters.minBpm===60&&filters.maxBpm===90}    onclick={() => { filters.minBpm=60;  filters.maxBpm=90;  }}>Slow</button>
        <button class="preset-btn" class:active={filters.minBpm===90&&filters.maxBpm===125}   onclick={() => { filters.minBpm=90;  filters.maxBpm=125; }}>Mid</button>
        <button class="preset-btn" class:active={filters.minBpm===125&&filters.maxBpm===150}  onclick={() => { filters.minBpm=125; filters.maxBpm=150; }}>Fast</button>
        <button class="preset-btn" class:active={filters.minBpm===150&&filters.maxBpm===250}  onclick={() => { filters.minBpm=150; filters.maxBpm=250; }}>V.Fast</button>
        <button class="preset-btn" class:active={filters.minBpm===20&&filters.maxBpm===250}   onclick={() => { filters.minBpm=20;  filters.maxBpm=250; }}>All</button>
      </div>
    </div>
    <!-- Mood sliders -->
    {#if hasMoodData}
    <MoodSection />
    {/if}

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
  {/if}

  {#if ui.sidebarTab === 'curations'}
    <!-- Playlists Section -->
    <div class="sidebar-section">
      <div class="section-label-row" style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 6px;">
        <span class="section-label" style="margin-bottom: 0;">🟣 Playlists</span>
        <button class="label-clear" style="text-decoration:none; color: var(--sg-primary, #00f0ff);" onclick={() => isCreatingPlaylist = !isCreatingPlaylist}>
          {isCreatingPlaylist ? 'Cancel' : '[+ New]'}
        </button>
      </div>

      {#if isCreatingPlaylist}
        <div class="inline-save-form" style="background: rgba(255,255,255,0.02); border: 1px solid rgba(255,255,255,0.06); padding: 8px; border-radius:4px; margin-bottom: 12px; display: flex; flex-direction: column; gap: 6px;">
          <input 
            type="text" 
            placeholder="New playlist name..." 
            bind:value={newPlaylistName} 
            class="search-input"
            style="padding-left: 8px; font-size: var(--sg-text-sm);"
          />
          <button class="action-btn action-btn-primary" style="width: 100%; justify-content: center;" onclick={async () => {
            if (newPlaylistName.trim()) {
              await curation.createPlaylist(newPlaylistName.trim());
              newPlaylistName = "";
              isCreatingPlaylist = false;
            }
          }}>Create Playlist</button>
        </div>
      {/if}

      <div class="curation-list" style="display: flex; flex-direction: column; gap: 4px;">
        {#each curation.playlists as pl}
          <div class="curation-item-row" style="display: flex; align-items: center; justify-content: space-between; padding: 4px 6px; border-radius: 4px; background: rgba(255,255,255,0.02);">
            <button 
              class="curation-item-name-btn" 
              style="background: none; border: none; text-align: left; padding: 0; cursor: pointer; display: flex; align-items: center; gap: 4px;"
              onclick={async () => {
                curation.activeSavedSearch = null;
                curation.activePlaylist = pl;
                await curation.loadPlaylistTracks(pl.id);
                ui.sidebarTab = "filters";
              }}
            >
              <span class="curation-item-name" style="font-family: var(--sg-font-mono); font-size: var(--sg-text-sm); color: {curation.activePlaylist?.id === pl.id ? 'var(--sg-primary, #00f0ff)' : 'var(--sg-on-surface, #e3e1e9)'};">🟣 {pl.name}</span>
            </button>
            {#if deletePlaylistId === pl.id}
              <div style="display: flex; gap: 4px; align-items: center;">
                <button class="mini-confirm-btn" style="color: #ff5555; background: none; border: none; font-size: var(--sg-text-xs); cursor: pointer;" onclick={() => { curation.deletePlaylist(pl.id); deletePlaylistId = null; }}>Confirm</button>
                <button class="mini-confirm-btn" style="color: var(--sg-outline); background: none; border: none; font-size: var(--sg-text-xs); cursor: pointer;" onclick={() => deletePlaylistId = null}>Cancel</button>
              </div>
            {:else}
              <button class="mini-delete-btn" style="background: none; border: none; color: var(--sg-outline); cursor: pointer; font-size: var(--sg-text-sm); padding: 2px;" onclick={() => deletePlaylistId = pl.id} title="Delete Playlist">🗑️</button>
            {/if}
          </div>
        {/each}
        {#if curation.playlists.length === 0}
          <p class="empty-list-text" style="font-family: var(--sg-font-mono); font-size: var(--sg-text-xs); color: var(--sg-outline, #849495); text-align: center; margin: 0.5rem 0;">No playlists created yet.</p>
        {/if}
      </div>
    </div>

    <!-- Saved Searches Section -->
    <SavedSearchList onopen={handleOpenSavedSearch} />
  {/if}
  </div>
  {/snippet}
</CollapsiblePane>

<style>
  .sidebar-inner {
    display: flex;
    flex-direction: column;
    gap: 0;
    height: 100%;
    overflow-y: auto;
    overflow-x: hidden;
    padding: 1rem 0.75rem;
    scrollbar-width: thin;
    scrollbar-color: color-mix(in srgb, var(--sg-on-surface) 10%, transparent) transparent;
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
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-xs);
    font-weight: 700;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--sg-on-surface, #e3e1e9);
  }

  .sidebar-count {
    display: block;
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-xs);
    color: var(--sg-outline, #849495);
    margin-top: 2px;
  }

  .stale-badge {
    color: var(--sg-secondary, #fe00fe);
    font-weight: 700;
  }

  .collapse-btn {
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

  .collapse-btn:hover {
    color: var(--sg-on-surface, #e3e1e9);
    background: color-mix(in srgb, var(--sg-on-surface) 5%, transparent);
  }

  /* ── Sections ── */
  .sidebar-section {
    padding: 0.65rem 0;
    border-top: 1px solid color-mix(in srgb, var(--sg-on-surface) 6%, transparent);
  }

  .section-label {
    display: block;
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-2xs);
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
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-xs);
    color: var(--sg-primary, #00f0ff);
  }

  .label-clear {
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-2xs);
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
    border: 1px solid var(--sg-surface-high);
    border-radius: 4px;
    padding: 0.4rem 0.5rem 0.4rem 2rem;
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-sm);
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
    font-size: var(--sg-text-md);
    line-height: 1;
    padding: 0 2px;
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
    border: 1px solid color-mix(in srgb, var(--sg-on-surface) 12%, transparent);
    border-radius: 4px;
    z-index: 100;
    display: flex;
    flex-direction: column;
    scrollbar-width: thin;
    scrollbar-color: color-mix(in srgb, var(--sg-on-surface) 10%, transparent) transparent;
  }

  .genre-suggestion-item {
    background: none;
    border: none;
    text-align: left;
    padding: 6px 10px;
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-sm);
    color: var(--sg-outline, #849495);
    cursor: pointer;
    transition: background 0.1s, color 0.1s;
  }

  .genre-suggestion-item:hover {
    background: color-mix(in srgb, var(--sg-primary) 8%, transparent);
    color: var(--sg-primary, #00f0ff);
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
    border: 1px solid color-mix(in srgb, var(--sg-on-surface) 6%, transparent);
    border-radius: 3px;
    background: color-mix(in srgb, var(--sg-on-surface) 2%, transparent);
    color: var(--sg-outline, #849495);
    cursor: pointer;
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-xs);
    transition: all 0.12s;
  }

  .dir-btn:hover {
    border-color: color-mix(in srgb, var(--sg-primary) 30%, transparent);
    color: var(--sg-on-surface, #e3e1e9);
    background: color-mix(in srgb, var(--sg-primary) 4%, transparent);
  }

  .dir-btn.dir-active {
    border-color: var(--sg-primary, #00f0ff);
    background: color-mix(in srgb, var(--sg-primary) 10%, transparent);
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

  .search-inputs-container {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .semantic-wrap .search-icon {
    color: var(--sg-primary, #00f0ff);
  }

  .semantic-search-input {
    border-color: color-mix(in srgb, var(--sg-primary) 15%, transparent) !important;
  }

  .semantic-search-input:focus {
    border-color: var(--sg-primary, #00f0ff) !important;
    box-shadow: 0 0 8px color-mix(in srgb, var(--sg-primary) 15%, transparent);
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

  .clap-wrap .search-icon {
    color: var(--sg-secondary, #fe00fe);
  }

  .clap-search-input {
    border-color: color-mix(in srgb, var(--sg-secondary) 15%, transparent) !important;
  }

  .clap-search-input:focus {
    border-color: var(--sg-secondary, #fe00fe) !important;
    box-shadow: 0 0 8px color-mix(in srgb, var(--sg-secondary) 15%, transparent);
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
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-xs);
    padding: 5px 2px;
    text-align: center;
    border: 1px solid var(--sg-surface-high);
    background: color-mix(in srgb, var(--sg-on-surface) 2%, transparent);
    color: var(--sg-outline, #849495);
    cursor: pointer;
    border-radius: 2px;
    transition: all 0.12s;
  }

  .key-btn:hover {
    border-color: color-mix(in srgb, var(--sg-primary) 30%, transparent);
    color: var(--sg-on-surface, #e3e1e9);
    background: color-mix(in srgb, var(--sg-primary) 5%, transparent);
  }

  .key-active {
    border-color: var(--sg-primary, #00f0ff);
    background: color-mix(in srgb, var(--sg-primary) 12%, transparent);
    color: var(--sg-primary, #00f0ff);
  }

  .scale-toggle {
    display: flex;
    gap: 4px;
    margin-top: 4px;
  }

  .scale-btn {
    flex: 1;
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-xs);
    padding: 5px 4px;
    text-align: center;
    border: 1px solid var(--sg-surface-high);
    background: color-mix(in srgb, var(--sg-on-surface) 2%, transparent);
    color: var(--sg-outline, #849495);
    cursor: pointer;
    border-radius: 2px;
    transition: all 0.12s;
  }

  .scale-btn:hover {
    color: var(--sg-on-surface, #e3e1e9);
    border-color: color-mix(in srgb, var(--sg-primary) 30%, transparent);
  }

  .scale-active {
    border-color: var(--sg-primary, #00f0ff);
    background: color-mix(in srgb, var(--sg-primary) 12%, transparent);
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
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-2xs);
    padding: 3px 7px;
    border: 1px solid color-mix(in srgb, var(--sg-on-surface) 10%, transparent);
    background: color-mix(in srgb, var(--sg-on-surface) 3%, transparent);
    color: var(--sg-outline, #849495);
    cursor: pointer;
    border-radius: 3px;
    transition: all 0.12s;
  }

  .preset-btn:hover { color: var(--sg-on-surface, #e3e1e9); }
  .preset-btn.active {
    border-color: var(--sg-primary, #00f0ff);
    color: var(--sg-primary, #00f0ff);
    background: color-mix(in srgb, var(--sg-primary) 8%, transparent);
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
    border: 1px solid color-mix(in srgb, var(--sg-on-surface) 15%, transparent);
    background: color-mix(in srgb, var(--sg-on-surface) 6%, transparent);
    cursor: pointer;
    padding: 0;
    transition: background 0.2s, border-color 0.2s;
    flex-shrink: 0;
  }

  .toggle-btn.toggle-on {
    background: color-mix(in srgb, var(--sg-primary) 20%, transparent);
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
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-2xs);
    color: var(--sg-outline, #849495);
    margin: 4px 0 0;
    opacity: 0.7;
    line-height: 1.4;
  }

  /* ── Tabbed Sidebar ── */
  .sidebar-tabs {
    display: flex;
    background: color-mix(in srgb, var(--sg-on-surface) 3%, transparent);
    border: 1px solid color-mix(in srgb, var(--sg-on-surface) 6%, transparent);
    border-radius: 6px;
    padding: 2px;
    margin: 0.5rem 0.75rem 0.25rem 0.75rem;
    gap: 2px;
  }

  .tab-btn {
    flex: 1;
    background: none;
    border: none;
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-xs);
    font-weight: 700;
    color: var(--sg-outline, #849495);
    padding: 6px 12px;
    cursor: pointer;
    border-radius: 4px;
    transition: all 0.15s ease-in-out;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 4px;
  }

  .tab-btn:hover {
    color: var(--sg-on-surface, #e3e1e9);
    background: color-mix(in srgb, var(--sg-on-surface) 4%, transparent);
  }

  .tab-btn.active {
    background: color-mix(in srgb, var(--sg-on-surface) 8%, transparent);
    color: var(--sg-primary, #00f0ff);
    box-shadow: 0 1px 3px color-mix(in srgb, var(--sg-surface) 30%, transparent);
  }

  /* ── Playlist Autocomplete Suggestions ── */
  .playlist-suggestions {
    position: absolute;
    top: calc(100% + 4px);
    left: 0;
    right: 0;
    max-height: 200px;
    overflow-y: auto;
    background: var(--sg-surface-container, #1e1f25);
    border: 1px solid color-mix(in srgb, var(--sg-on-surface) 12%, transparent);
    border-radius: 4px;
    z-index: 100;
    display: flex;
    flex-direction: column;
    scrollbar-width: thin;
    scrollbar-color: color-mix(in srgb, var(--sg-on-surface) 10%, transparent) transparent;
  }

  .playlist-suggestion-item {
    background: none;
    border: none;
    text-align: left;
    padding: 6px 10px;
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-sm);
    color: var(--sg-outline, #849495);
    cursor: pointer;
    transition: background 0.1s, color 0.1s;
  }

  .playlist-suggestion-item:hover {
    background: color-mix(in srgb, var(--sg-primary) 8%, transparent);
    color: var(--sg-primary, #00f0ff);
  }

  /* ── Tag filter ── */
  .tag-chips-wrap {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    margin-bottom: 6px;
  }

  .tag-filter-chip {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-2xs);
    font-weight: 600;
    padding: 2px 7px;
    border-radius: 999px;
    border: 1px solid color-mix(in srgb, var(--sg-primary) 35%, transparent);
    background: color-mix(in srgb, var(--sg-primary) 8%, transparent);
    color: var(--sg-primary, #00f0ff);
    cursor: pointer;
    transition: filter 0.12s;
  }

  .tag-filter-chip:hover { filter: brightness(1.3); }

  .tfc-ns {
    font-size: var(--sg-text-3xs);
    opacity: 0.5;
    font-weight: 400;
  }

  .tfc-remove {
    font-size: var(--sg-text-xs);
    opacity: 0.6;
    margin-left: 1px;
  }

  .tag-input-wrap {
    position: relative;
    display: flex;
    align-items: center;
    gap: 6px;
    background: var(--sg-surface-container);
    border: 1px solid var(--sg-surface-high);
    border-radius: 5px;
    padding: 0 8px;
    height: 28px;
    transition: border-color 0.15s;
  }

  .tag-input-wrap:focus-within {
    border-color: color-mix(in srgb, var(--sg-primary) 40%, transparent);
  }

  .tag-input-icon { color: var(--sg-outline, #849495); flex-shrink: 0; }

  .tag-search-input {
    flex: 1;
    background: transparent;
    border: none;
    outline: none;
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-xs);
    color: var(--sg-on-surface, #e3e1e9);
    padding: 0;
    height: 100%;
  }

  .tag-search-input::placeholder { color: var(--sg-outline, #849495); }

  /* ── Structure filter ── */
  .structure-icon { color: var(--sg-warning); }

  .structure-similar-badge {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 6px;
    padding: 4px 8px;
    margin-bottom: 4px;
    border-radius: 4px;
    border: 1px solid color-mix(in srgb, var(--sg-warning) 40%, transparent);
    background: color-mix(in srgb, var(--sg-warning) 8%, transparent);
    font-family: var(--sg-font-mono);
    font-size: 10px;
    font-weight: 700;
    color: var(--sg-warning);
    letter-spacing: 0.04em;
  }

  .structure-similar-badge .clear-x {
    color: var(--sg-warning);
    opacity: 0.7;
  }

  .structure-similar-badge .clear-x:hover {
    opacity: 1;
  }

  .structure-search-input {
    border-color: color-mix(in srgb, var(--sg-warning) 15%, transparent) !important;
  }

  .structure-search-input:focus {
    border-color: color-mix(in srgb, var(--sg-warning) 50%, transparent) !important;
    box-shadow: 0 0 8px color-mix(in srgb, var(--sg-warning) 12%, transparent);
  }

  .structure-help {
    padding: 0.4rem 0.5rem 0.2rem;
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
  }

  .structure-legend {
    font-family: var(--sg-font-mono);
    font-size: 10px;
    color: var(--sg-outline);
    letter-spacing: 0.03em;
    line-height: 1.8;
  }

  .structure-examples {
    font-size: 10px;
    color: var(--sg-outline);
    line-height: 1.7;
  }

  .structure-examples code {
    font-family: var(--sg-font-mono);
    font-size: 10px;
    color: var(--sg-warning);
    background: color-mix(in srgb, var(--sg-warning) 8%, transparent);
    padding: 0 3px;
    border-radius: 2px;
  }
</style>
