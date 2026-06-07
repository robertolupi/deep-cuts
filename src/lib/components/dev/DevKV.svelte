<script lang="ts">
  let { label, value, dim = false, mono = true, truncate = 60 }: {
    label: string;
    value: unknown;
    dim?: boolean;
    mono?: boolean;
    truncate?: number;
  } = $props();

  function format(v: unknown): string {
    if (v === null || v === undefined) return '—';
    if (typeof v === 'boolean') return v ? 'true' : 'false';
    if (typeof v === 'object') return JSON.stringify(v);
    return String(v);
  }

  const formatted = $derived(format(value));
  const isTruncated = $derived(formatted.length > truncate);
  const display = $derived(isTruncated ? formatted.slice(0, truncate) + '…' : formatted);

  let expanded = $state(false);

  async function copyFull() {
    await navigator.clipboard.writeText(formatted);
  }
</script>

<div class="kv" class:dim>
  <span class="kv-label">{label}</span>
  <span class="kv-value" class:mono>
    {#if expanded}
      {formatted}
    {:else}
      {display}
    {/if}
    {#if isTruncated}
      <button class="kv-action" onclick={() => expanded = !expanded}>
        {expanded ? 'less' : 'more'}
      </button>
      <button class="kv-action" onclick={copyFull}>copy</button>
    {/if}
  </span>
</div>

<style>
  .kv {
    display: grid;
    grid-template-columns: 130px 1fr;
    gap: 6px;
    padding: 2px 0;
    align-items: baseline;
  }

  .kv.dim {
    opacity: 0.35;
  }

  .kv-label {
    font-family: var(--sg-font-mono);
    font-size: 10px;
    color: var(--sg-outline, #849495);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    flex-shrink: 0;
  }

  .kv-value {
    font-size: 10px;
    color: var(--sg-on-surface, #e3e1e9);
    word-break: break-all;
    line-height: 1.5;
  }

  .kv-value.mono {
    font-family: var(--sg-font-mono);
  }

  .kv-action {
    font-family: var(--sg-font-mono);
    font-size: 9px;
    background: color-mix(in srgb, var(--sg-primary) 8%, transparent);
    border: 1px solid color-mix(in srgb, var(--sg-primary) 20%, transparent);
    border-radius: 3px;
    color: var(--sg-primary, #00f0ff);
    padding: 0 4px;
    margin-left: 4px;
    cursor: pointer;
    line-height: 1.6;
    transition: background 0.1s;
  }

  .kv-action:hover {
    background: color-mix(in srgb, var(--sg-primary) 16%, transparent);
  }
</style>
