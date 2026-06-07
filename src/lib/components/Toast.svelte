<script lang="ts">
  import { ui } from "$lib/stores/ui.svelte";
</script>

{#if ui.successMessage || ui.errorMessage}
  <div class="toast-container">
    {#if ui.successMessage}
      <div class="toast toast-success">
        <svg xmlns="http://www.w3.org/2000/svg" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
          <polyline points="20 6 9 17 4 12"/>
        </svg>
        {ui.successMessage}
      </div>
    {/if}
    {#if ui.errorMessage}
      <div class="toast toast-error">
        <svg xmlns="http://www.w3.org/2000/svg" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
          <circle cx="12" cy="12" r="10"/><line x1="12" y1="8" x2="12" y2="12"/><line x1="12" y1="16" x2="12.01" y2="16"/>
        </svg>
        {ui.errorMessage}
      </div>
    {/if}
  </div>
{/if}

<style>
  .toast-container {
    position: fixed;
    bottom: calc(var(--sg-player-bar-height, 80px) + 12px);
    left: 50%;
    transform: translateX(-50%);
    display: flex;
    flex-direction: column;
    gap: 6px;
    z-index: 1000;
    pointer-events: none;
  }

  .toast {
    display: flex;
    align-items: center;
    gap: 8px;
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-sm);
    padding: 8px 14px;
    border-radius: 5px;
    backdrop-filter: blur(12px);
    animation: slide-up 0.2s ease-out;
    white-space: nowrap;
  }

  .toast-success {
    background: color-mix(in srgb, var(--sg-primary) 12%, transparent);
    border: 1px solid color-mix(in srgb, var(--sg-primary) 30%, transparent);
    color: var(--sg-primary);
  }

  .toast-error {
    background: color-mix(in srgb, var(--sg-error) 12%, transparent);
    border: 1px solid color-mix(in srgb, var(--sg-error) 35%, transparent);
    color: var(--sg-error);
  }

  @keyframes slide-up {
    from { opacity: 0; transform: translateY(8px); }
    to   { opacity: 1; transform: translateY(0); }
  }
</style>
