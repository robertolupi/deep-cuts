<script lang="ts">
  import { filters } from "$lib/stores/filters.svelte";
  import { library } from "$lib/stores/library.svelte";
  import { curation } from "$lib/stores/curation.svelte";
  import { ui } from "$lib/stores/ui.svelte";

  let { hasActiveFilters, clearAll }: { hasActiveFilters: boolean; clearAll: () => void } = $props();

  let isSavingSearch = $state(false);
  let newSavedSearchName = $state("");
  let saveToPlaylistOpen = $state(false);
  let saveToNewPlaylistName = $state("");
  let saveToNewPlaylistMode = $state(false);

  function getSerializedFilterState(): string {
    return JSON.stringify({
      searchQuery: filters.searchQuery,
      semanticQuery: filters.semanticQuery,
      clapQuery: filters.clapQuery,
      genreFilter: filters.genreFilter,
      minBpm: filters.minBpm,
      maxBpm: filters.maxBpm,
      selectedKeys: filters.selectedKeys,
      selectedScale: filters.selectedScale,
      musicOnly: filters.musicOnly,
      vocalFilter: filters.vocalFilter,
      selectedDirectoryIds: filters.selectedDirectoryIds,
    });
  }

  async function handleCreateSavedSearch() {
    const finalName = newSavedSearchName.trim() || filters.autoName;
    if (!finalName) return;
    const q = getSerializedFilterState();
    const id = await curation.createSavedSearch(finalName, q);
    if (id) {
      newSavedSearchName = "";
      isSavingSearch = false;
    }
  }

  async function handleUpdateActiveSavedSearch() {
    if (!curation.activeSavedSearch) return;
    const q = getSerializedFilterState();
    await curation.updateSavedSearch(curation.activeSavedSearch.id, q);
  }

  async function handleExportM3U() {
    const list = filters.filteredTracks;
    if (list.length === 0) {
      ui.showToast("No tracks to export", "error");
      return;
    }
    const tracksPayload = list.map(t => ({
      path: t.path,
      title: t.title ?? null,
      artist: t.artist ?? null,
      duration_seconds: t.duration_seconds ?? null
    }));
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      const exported = await invoke<boolean>('export_m3u_playlist', { tracks: tracksPayload });
      if (exported) {
        ui.showToast(`Exported ${list.length} tracks to M3U successfully!`, "success");
      }
    } catch (e: any) {
      console.error("Failed to export M3U playlist:", e);
      ui.showToast(e, "error");
    }
  }
</script>

{#if hasActiveFilters || curation.activeSavedSearch}
  <div class="sidebar-section active-chips">
    {#if hasActiveFilters}
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
      {#if filters.moodHappyMin > 0 || filters.moodHappyMax < 1 || filters.moodSadMin > 0 || filters.moodSadMax < 1 || filters.moodAggressiveMin > 0 || filters.moodAggressiveMax < 1 || filters.moodRelaxedMin > 0 || filters.moodRelaxedMax < 1 || filters.moodPartyMin > 0 || filters.moodPartyMax < 1 || filters.moodAcousticMin > 0 || filters.moodAcousticMax < 1 || filters.moodElectronicMin > 0 || filters.moodElectronicMax < 1}
        <button class="chip chip-active" onclick={() => { filters.moodHappyMin=0; filters.moodHappyMax=1; filters.moodSadMin=0; filters.moodSadMax=1; filters.moodAggressiveMin=0; filters.moodAggressiveMax=1; filters.moodRelaxedMin=0; filters.moodRelaxedMax=1; filters.moodPartyMin=0; filters.moodPartyMax=1; filters.moodAcousticMin=0; filters.moodAcousticMax=1; filters.moodElectronicMin=0; filters.moodElectronicMax=1; }}>
          Mood filter ×
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
      {#if filters.similarToTrack}
        <div class="blend-slider-row">
          <span class="blend-label">Feels</span>
          <input
            type="range"
            min="0" max="1" step="0.05"
            value={filters.similarBlend}
            oninput={(e) => filters.setSimilarBlend(parseFloat((e.target as HTMLInputElement).value))}
            class="blend-slider"
          />
          <span class="blend-label">Sounds</span>
        </div>
      {/if}
    {/if}

    <!-- Saved Search actions -->
    <div class="save-search-actions" style="margin-top: 10px; width: 100%; display: flex; flex-direction: column; gap: 6px;">
      {#if curation.activeSavedSearch}
        {#if isSavingSearch}
          <div class="inline-save-form" style="background: rgba(255,255,255,0.02); border: 1px solid rgba(255,255,255,0.06); padding: 8px; border-radius:4px; width: 100%; display: flex; flex-direction: column; gap: 6px; box-sizing: border-box;">
            <input
              type="text"
              placeholder={filters.autoName}
              bind:value={newSavedSearchName}
              class="search-input"
              style="padding-left: 8px; font-size: var(--sg-text-sm); box-sizing: border-box;"
            />
            <div style="display: flex; gap: 4px; width: 100%;">
              <button class="action-btn action-btn-primary" style="flex: 1; justify-content: center;" onclick={handleCreateSavedSearch}>Save</button>
              <button class="action-btn" style="flex: 1; justify-content: center;" onclick={() => isSavingSearch = false}>Cancel</button>
            </div>
          </div>
        {:else}
          <div style="display: flex; gap: 6px; width: 100%;">
            <button class="action-btn action-btn-primary" style="flex: 1; justify-content: center;" onclick={handleUpdateActiveSavedSearch}>
              💾 Update Smart Search
            </button>
            <button class="action-btn" style="flex: 1; justify-content: center;" onclick={() => isSavingSearch = true}>
              💾 Save as New Search
            </button>
          </div>
        {/if}
      {:else if hasActiveFilters}
        {#if isSavingSearch}
          <div class="inline-save-form" style="background: rgba(255,255,255,0.02); border: 1px solid rgba(255,255,255,0.06); padding: 8px; border-radius:4px; width: 100%; display: flex; flex-direction: column; gap: 6px; box-sizing: border-box;">
            <input
              type="text"
              placeholder={filters.autoName}
              bind:value={newSavedSearchName}
              class="search-input"
              style="padding-left: 8px; font-size: var(--sg-text-sm); box-sizing: border-box;"
            />
            <div style="display: flex; gap: 4px; width: 100%;">
              <button class="action-btn action-btn-primary" style="flex: 1; justify-content: center;" onclick={handleCreateSavedSearch}>Save</button>
              <button class="action-btn" style="flex: 1; justify-content: center;" onclick={() => isSavingSearch = false}>Cancel</button>
            </div>
          </div>
        {:else}
          <button class="action-btn action-btn-primary" style="width: 100%; justify-content: center;" onclick={() => isSavingSearch = true}>
            💾 Save as Smart Search
          </button>
        {/if}
      {/if}

      {#if filters.filteredTracks.length > 0}
        <button class="action-btn" style="width: 100%; justify-content: center; border-color: rgba(254, 0, 254, 0.35); color: var(--sg-secondary, #fe00fe); background: rgba(254, 0, 254, 0.08);" onclick={handleExportM3U}>
          📤 Export results as M3U ({filters.filteredTracks.length})
        </button>
        <!-- Save filtered results to a playlist -->
        <div class="save-to-playlist-wrap">
          <button class="action-btn" style="width: 100%; justify-content: center;" onclick={() => { saveToPlaylistOpen = !saveToPlaylistOpen; saveToNewPlaylistMode = false; saveToNewPlaylistName = ""; }}>
            🟣 Save {filters.filteredTracks.length} tracks to playlist ▾
          </button>
          {#if saveToPlaylistOpen}
            <div class="playlist-dropdown">
              {#if saveToNewPlaylistMode}
                <div style="display: flex; flex-direction: column; gap: 6px; padding: 6px;">
                  <input
                    type="text"
                    placeholder="New playlist name..."
                    bind:value={saveToNewPlaylistName}
                    class="search-input"
                    style="padding-left: 8px; font-size: var(--sg-text-sm);"
                  />
                  <div style="display: flex; gap: 4px;">
                    <button class="action-btn action-btn-primary" style="flex: 1; justify-content: center;" onclick={async () => {
                      if (!saveToNewPlaylistName.trim()) return;
                      const id = await curation.createPlaylist(saveToNewPlaylistName.trim());
                      if (id) {
                        await curation.addTracksToPlaylist(id, filters.filteredTracks.map(t => t.id));
                      }
                      saveToPlaylistOpen = false;
                      saveToNewPlaylistMode = false;
                      saveToNewPlaylistName = "";
                    }}>Create & Add</button>
                    <button class="action-btn" style="flex: 1; justify-content: center;" onclick={() => saveToNewPlaylistMode = false}>Back</button>
                  </div>
                </div>
              {:else}
                {#each curation.playlists as pl}
                  <button class="playlist-dropdown-item" onclick={async () => {
                    await curation.addTracksToPlaylist(pl.id, filters.filteredTracks.map(t => t.id));
                    saveToPlaylistOpen = false;
                  }}>
                    🟣 {pl.name}
                  </button>
                {/each}
                <button class="playlist-dropdown-item playlist-dropdown-new" onclick={() => saveToNewPlaylistMode = true}>
                  + New playlist…
                </button>
              {/if}
            </div>
          {/if}
        </div>
      {/if}
    </div>
  </div>
{/if}

<style>
  .active-chips {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }

  .chip {
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-xs);
    padding: 3px 8px;
    border-radius: 999px;
    border: 1px solid color-mix(in srgb, var(--sg-on-surface) 12%, transparent);
    background: color-mix(in srgb, var(--sg-on-surface) 4%, transparent);
    color: var(--sg-outline, #849495);
    cursor: pointer;
    transition: all 0.15s;
    white-space: nowrap;
  }

  .chip:hover {
    border-color: color-mix(in srgb, var(--sg-primary) 40%, transparent);
    color: var(--sg-on-surface);
  }

  .chip-active {
    border-color: var(--sg-primary);
    background: color-mix(in srgb, var(--sg-primary) 10%, transparent);
    color: var(--sg-primary);
  }

  .chip-clear {
    border-color: color-mix(in srgb, var(--sg-on-surface) 8%, transparent);
    color: var(--sg-outline, #849495);
    font-style: italic;
  }

  .chip-similar {
    border-color: color-mix(in srgb, var(--sg-secondary) 45%, transparent) !important;
    background: color-mix(in srgb, var(--sg-secondary) 8%, transparent) !important;
    color: var(--sg-secondary) !important;
    max-width: 180px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .chip-semantic {
    border-color: color-mix(in srgb, var(--sg-primary) 45%, transparent) !important;
    background: color-mix(in srgb, var(--sg-primary) 8%, transparent) !important;
    color: var(--sg-primary) !important;
    max-width: 180px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .chip-clap {
    border-color: color-mix(in srgb, var(--sg-secondary) 45%, transparent) !important;
    background: color-mix(in srgb, var(--sg-secondary) 8%, transparent) !important;
    color: var(--sg-secondary) !important;
    max-width: 180px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .blend-slider-row {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 2px 2px;
  }

  .blend-label {
    font-size: var(--sg-text-xs);
    color: var(--text-muted, rgba(255,255,255,0.4));
    white-space: nowrap;
    flex-shrink: 0;
  }

  .blend-slider {
    flex: 1;
    height: 3px;
    accent-color: var(--sg-primary, #00f0ff);
    cursor: pointer;
  }

  .action-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 5px;
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-xs);
    font-weight: 700;
    padding: 5px 12px;
    border: 1px solid color-mix(in srgb, var(--sg-on-surface) 12%, transparent);
    border-radius: 4px;
    background: color-mix(in srgb, var(--sg-on-surface) 4%, transparent);
    color: var(--sg-outline, #849495);
    cursor: pointer;
    transition: all 0.12s;
  }

  .action-btn:hover:not(:disabled) {
    border-color: color-mix(in srgb, var(--sg-on-surface) 25%, transparent);
    color: var(--sg-on-surface);
    background: color-mix(in srgb, var(--sg-on-surface) 8%, transparent);
  }

  .action-btn-primary {
    border-color: color-mix(in srgb, var(--sg-primary) 35%, transparent);
    color: var(--sg-primary);
    background: color-mix(in srgb, var(--sg-primary) 8%, transparent);
  }

  .action-btn-primary:hover {
    background: color-mix(in srgb, var(--sg-primary) 14%, transparent) !important;
    border-color: var(--sg-primary) !important;
    color: var(--sg-primary) !important;
  }

  .search-input {
    width: 100%;
    background: var(--sg-surface-container, #1e1f25);
    border: 1px solid rgba(255,255,255,0.08);
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

  .save-to-playlist-wrap {
    position: relative;
    width: 100%;
  }

  .playlist-dropdown {
    position: absolute;
    top: calc(100% + 4px);
    left: 0;
    right: 0;
    background: var(--sg-surface-container, #1e1f25);
    border: 1px solid rgba(255,255,255,0.12);
    border-radius: 4px;
    z-index: 200;
    display: flex;
    flex-direction: column;
    max-height: 200px;
    overflow-y: auto;
    scrollbar-width: thin;
    scrollbar-color: rgba(255,255,255,0.1) transparent;
  }

  .playlist-dropdown-item {
    background: none;
    border: none;
    text-align: left;
    padding: 7px 10px;
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-sm);
    color: var(--sg-outline, #849495);
    cursor: pointer;
    transition: background 0.1s, color 0.1s;
  }

  .playlist-dropdown-item:hover {
    background: rgba(0,240,255,0.08);
    color: var(--sg-primary, #00f0ff);
  }

  .playlist-dropdown-new {
    border-top: 1px solid rgba(255,255,255,0.06);
    color: var(--sg-primary, #00f0ff);
    font-style: italic;
  }
</style>
