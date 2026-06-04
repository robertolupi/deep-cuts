<script lang="ts">
  import { curation } from "$lib/stores/curation.svelte";
  import type { SavedSearch } from "$lib/types";

  let { onopen }: { onopen: (search: SavedSearch) => void } = $props();

  let deleteSearchId = $state<number | null>(null);
</script>

<div class="sidebar-section">
  <span class="section-label">🔍 Saved Searches</span>
  <div class="curation-list" style="display: flex; flex-direction: column; gap: 4px;">
    {#each curation.savedSearches as search}
      <div class="curation-item-row" style="display: flex; align-items: center; justify-content: space-between; padding: 4px 6px; border-radius: 4px; background: rgba(255,255,255,0.02);">
        <button
          class="curation-item-name-btn"
          style="background: none; border: none; text-align: left; padding: 0; cursor: pointer; display: flex; align-items: center; gap: 4px;"
          onclick={() => onopen(search)}
        >
          <span class="curation-item-name" style="font-family: 'JetBrains Mono', monospace; font-size: 11px; color: {curation.activeSavedSearch?.id === search.id ? 'var(--sg-primary, #00f0ff)' : 'var(--sg-on-surface, #e3e1e9)'};">🔍 {search.name}</span>
        </button>
        {#if deleteSearchId === search.id}
          <div style="display: flex; gap: 4px; align-items: center;">
            <button class="mini-confirm-btn" style="color: #ff5555; background: none; border: none; font-size: 10px; cursor: pointer;" onclick={() => { curation.deleteSavedSearch(search.id); deleteSearchId = null; }}>Confirm</button>
            <button class="mini-confirm-btn" style="color: var(--sg-outline); background: none; border: none; font-size: 10px; cursor: pointer;" onclick={() => deleteSearchId = null}>Cancel</button>
          </div>
        {:else}
          <button class="mini-delete-btn" style="background: none; border: none; color: var(--sg-outline); cursor: pointer; font-size: 11px; padding: 2px;" onclick={() => deleteSearchId = search.id} title="Delete Saved Search">🗑️</button>
        {/if}
      </div>
    {/each}
    {#if curation.savedSearches.length === 0}
      <p class="empty-list-text" style="font-family: 'JetBrains Mono', monospace; font-size: 10px; color: var(--sg-outline, #849495); text-align: center; margin: 0.5rem 0;">No saved searches created yet.</p>
    {/if}
  </div>
</div>
