<script lang="ts">
  import type { Snippet } from 'svelte';

  let {
    side = 'left',
    width = '260px',
    hasIndicator = false,
    children,
  }: {
    side?: 'left' | 'right';
    width?: string;
    hasIndicator?: boolean;
    children: Snippet<[{ collapse: () => void }]>;
  } = $props();

  let collapsed = $state(false);
  let hovered   = $state(false);

  // Left pane: collapse = point left, expand = point right. Right pane: vice-versa.
  const collapsePoints = $derived(side === 'left' ? '15 18 9 12 15 6' : '9 18 15 12 9 6');
  const expandPoints   = $derived(side === 'left' ? '9 18 15 12 9 6'  : '15 18 9 12 15 6');

  function collapse() { collapsed = true; }
  function expand()   { collapsed = false; hovered = false; }
</script>

{#snippet chevron(points: string)}
  <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24"
    fill="none" stroke="currentColor" stroke-width="2.5"
    stroke-linecap="round" stroke-linejoin="round">
    <polyline points={points} />
  </svg>
{/snippet}

<aside
  class="collapsible-pane"
  class:collapsed
  class:pane-left={side === 'left'}
  class:pane-right={side === 'right'}
  style="--pane-width: {width};"
  onmouseenter={() => { if (collapsed) hovered = true; }}
  onmouseleave={() => hovered = false}
>
  {#if collapsed}
    <button class="expand-strip" onclick={expand} title="Click to pin open">
      {@render chevron(expandPoints)}
      {#if hasIndicator}<span class="indicator-dot"></span>{/if}
    </button>
    {#if hovered}
      <div class="preview-overlay">
        {@render children({ collapse })}
      </div>
    {/if}
  {:else}
    {@render children({ collapse })}
  {/if}
</aside>

<style>
  .collapsible-pane {
    height: 100%;
    width: var(--pane-width);
    flex-shrink: 0;
    overflow: hidden;
    background: var(--sg-surface-slate, #161b22);
    transition: width 0.2s ease;
  }

  .collapsible-pane.collapsed { width: 32px; }

  .pane-left  { border-right: 1px solid rgba(255,255,255,0.08); }
  .pane-right { border-left:  1px solid rgba(255,255,255,0.08); }

  /* ── Expand strip (collapsed state) ── */
  .expand-strip {
    width: 32px;
    height: 100%;
    background: none;
    border: none;
    cursor: pointer;
    color: var(--sg-outline, #849495);
    display: flex;
    align-items: center;
    justify-content: center;
    position: relative;
  }

  .expand-strip:hover {
    color: var(--sg-on-surface, #e3e1e9);
    background: rgba(255,255,255,0.05);
  }

  .indicator-dot {
    position: absolute;
    top: 12px;
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--sg-primary, #00f0ff);
  }

  .pane-left  .indicator-dot { right: 6px; }
  .pane-right .indicator-dot { left:  6px; }

  /* ── Hover preview overlay ── */
  .preview-overlay {
    position: fixed;
    top: 0;
    bottom: 0;
    width: var(--pane-width);
    background: var(--sg-surface-slate, #161b22);
    overflow-y: auto;
    overflow-x: hidden;
    scrollbar-width: thin;
    scrollbar-color: rgba(255,255,255,0.1) transparent;
    z-index: 200;
    pointer-events: auto;
  }

  .pane-left  .preview-overlay {
    left: 32px;
    border-right: 1px solid rgba(255,255,255,0.08);
    box-shadow: 6px 0 24px rgba(0,0,0,0.4);
  }

  .pane-right .preview-overlay {
    right: 32px;
    border-left: 1px solid rgba(255,255,255,0.08);
    box-shadow: -6px 0 24px rgba(0,0,0,0.4);
  }
</style>
