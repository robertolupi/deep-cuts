<script lang="ts">
  import { curation } from "$lib/stores/curation.svelte";

  interface Props {
    placeholder?: string;
    value?: string;
    activePlaylist?: import('$lib/types').Playlist | null;
    showAllOnFocus?: boolean;
    onselect?: (pl: import('$lib/types').Playlist) => void;
    onclear?: () => void;
  }

  let {
    placeholder = "Filter by playlist...",
    value = $bindable(""),
    activePlaylist = $bindable(null),
    showAllOnFocus = true,
    onselect,
    onclear
  }: Props = $props();

  let isFocused = $state(false);

  // Sync value when activePlaylist changes
  $effect(() => {
    if (!activePlaylist) {
      value = "";
    } else if (activePlaylist.name !== value) {
      value = activePlaylist.name;
    }
  });

  const suggestions = $derived.by(() => {
    const q = value.trim().toLowerCase();
    if (!q) {
      return showAllOnFocus ? curation.playlists : [];
    }
    return curation.playlists.filter(pl => pl.name.toLowerCase().includes(q)).slice(0, 12);
  });

  function selectPlaylist(pl: import('$lib/types').Playlist) {
    value = pl.name;
    activePlaylist = pl;
    isFocused = false;
    if (onselect) onselect(pl);
  }

  function handleClear() {
    value = "";
    activePlaylist = null;
    if (onclear) onclear();
  }
</script>

<div class="playlist-wrap">
  <div class="search-wrap">
    <!-- Vinyl record icon -->
    <svg xmlns="http://www.w3.org/2000/svg" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" class="search-icon">
      <path d="M12 2A10 10 0 0 0 2 12a10 10 0 0 0 10 10 10 10 0 0 0 10-10A10 10 0 0 0 12 2zm0 15a5 5 0 1 1 0-10 5 5 0 0 1 0 10z"/>
      <circle cx="12" cy="12" r="2"/>
    </svg>
    <input
      type="text"
      {placeholder}
      bind:value
      class="search-input"
      onfocus={() => isFocused = true}
      onblur={() => setTimeout(() => { isFocused = false; }, 150)}
    />
    {#if activePlaylist || value}
      <button type="button" class="clear-x" onclick={handleClear}>×</button>
    {/if}
  </div>
  {#if isFocused && suggestions.length > 0}
    <div class="playlist-suggestions">
      {#each suggestions as suggestion}
        <button type="button" class="playlist-suggestion-item" onmousedown={() => selectPlaylist(suggestion)}>
          {suggestion.name}
        </button>
      {/each}
    </div>
  {/if}
</div>

<style>
  .playlist-wrap {
    position: relative;
    width: 100%;
  }

  .search-wrap {
    position: relative;
    display: flex;
    align-items: center;
    width: 100%;
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

  .playlist-suggestions {
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

  .playlist-suggestion-item {
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

  .playlist-suggestion-item:hover {
    background: rgba(0, 240, 255, 0.08);
    color: var(--sg-primary, #00f0ff);
  }
</style>
