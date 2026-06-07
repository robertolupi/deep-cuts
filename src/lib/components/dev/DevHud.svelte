<script lang="ts">
  import { filters } from '$lib/stores/filters.svelte';
  import { library } from '$lib/stores/library.svelte';
  import { player } from '$lib/stores/player.svelte';

  let { totalPending, onOpen }: {
    totalPending: number;
    onOpen: () => void;
  } = $props();

  const trackLabel = $derived(() => {
    const t = player.selectedTrack;
    if (!t) return null;
    const name = t.title ?? t.filename ?? '?';
    return name.length > 22 ? name.slice(0, 22) + '…' : name;
  });
</script>

<button class="hud" onclick={onOpen} title="Open dev inspector">
  <span class="seg">{library.trackCount} tracks</span>
  <span class="sep">·</span>
  <span class="seg" class:hot={filters.filteredTracks.length < library.trackCount}>
    {filters.filteredTracks.length} shown
  </span>
  {#if trackLabel()}
    <span class="sep">·</span>
    <span class="seg track">▶ {trackLabel()}</span>
  {/if}
  {#if totalPending > 0}
    <span class="sep">·</span>
    <span class="seg pending">{totalPending} pending</span>
  {/if}
  {#if library.isScanning}
    <span class="sep">·</span>
    <span class="seg scanning">scanning</span>
  {/if}
</button>

<style>
  .hud {
    display: flex;
    align-items: center;
    gap: 0;
    font-family: var(--sg-font-mono);
    font-size: 10px;
    background: rgba(255,255,255,0.04);
    border: 1px solid rgba(255,255,255,0.08);
    border-radius: 4px;
    padding: 3px 8px;
    cursor: pointer;
    color: var(--sg-outline, #849495);
    flex-shrink: 0;
    transition: border-color 0.12s, color 0.12s;
    white-space: nowrap;
  }

  .hud:hover {
    border-color: rgba(0,240,255,0.3);
    color: var(--sg-on-surface, #e3e1e9);
  }

  .sep {
    margin: 0 5px;
    opacity: 0.3;
  }

  .seg.hot      { color: var(--sg-primary, #00f0ff); }
  .seg.track    { color: var(--sg-on-surface, #e3e1e9); }
  .seg.pending  { color: var(--sg-warning); }
  .seg.scanning { color: var(--sg-primary, #00f0ff); animation: blink 1.4s ease-in-out infinite; }

  @keyframes blink {
    0%, 100% { opacity: 1; }
    50%       { opacity: 0.4; }
  }
</style>
