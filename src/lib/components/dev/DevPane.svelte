<script lang="ts">
  let { title, open = true, children }: {
    title: string;
    open?: boolean;
    children: any;
  } = $props();

  let isOpen = $state(open);
</script>

<section class="pane">
  <button class="pane-header" onclick={() => isOpen = !isOpen}>
    <svg class="chevron" class:open={isOpen} xmlns="http://www.w3.org/2000/svg"
      width="10" height="10" viewBox="0 0 24 24" fill="none"
      stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round">
      <polyline points="9 18 15 12 9 6"/>
    </svg>
    <span class="pane-title">{title}</span>
  </button>
  {#if isOpen}
    <div class="pane-body">
      {@render children()}
    </div>
  {/if}
</section>

<style>
  .pane {
    border-bottom: 1px solid rgba(255,255,255,0.06);
  }

  .pane-header {
    display: flex;
    align-items: center;
    gap: 7px;
    width: 100%;
    background: transparent;
    border: none;
    padding: 8px 14px;
    cursor: pointer;
    color: var(--sg-outline, #849495);
    font-family: var(--sg-font-mono);
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    text-align: left;
    transition: color 0.12s, background 0.12s;
  }

  .pane-header:hover {
    color: var(--sg-on-surface, #e3e1e9);
    background: rgba(255,255,255,0.03);
  }

  .chevron {
    flex-shrink: 0;
    transition: transform 0.15s;
  }

  .chevron.open {
    transform: rotate(90deg);
  }

  .pane-body {
    padding: 0 14px 12px;
  }
</style>
