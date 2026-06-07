<script lang="ts">
  interface Props {
    title: string;
    subtitle?: string;
    class?: string;
  }
  let { title, subtitle, class: extraClass = "" }: Props = $props();
</script>

<div class="sg-card {extraClass}">
  <div class="card-header">
    <span class="card-title">{title}</span>
    {#if subtitle}
      <span class="card-subtitle">{subtitle}</span>
    {/if}
  </div>
  <slot />
</div>

<style>
  .sg-card {
    background: var(--sg-surface-slate, #161b22);
    border: 1px solid var(--sg-surface-high);
    border-radius: 6px;
    padding: 1rem;
    display: flex;
    flex-direction: column;
    gap: 0.85rem;
  }

  .card-header {
    display: flex;
    flex-direction: column;
    gap: 3px;
    padding-bottom: 0.65rem;
    border-bottom: 1px solid color-mix(in srgb, var(--sg-on-surface) 6%, transparent);
  }

  .card-title {
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-sm);
    font-weight: 700;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--sg-on-surface, #e3e1e9);
  }

  .card-subtitle {
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-2xs);
    color: var(--sg-outline, #849495);
    letter-spacing: 0.04em;
  }
</style>
