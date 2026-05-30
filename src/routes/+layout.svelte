<script lang="ts">
  import '../app.css';
  import { onMount } from 'svelte';
  import PlayerBar from '$lib/components/PlayerBar.svelte';
  import { library } from '$lib/stores/library.svelte';
  import { theme } from '$lib/stores/theme.svelte';

  let { children } = $props();

  onMount(() => {
    async function init() {
      await library.init();
      await theme.init(library.tauriConnected);
    }
    const cleanup = theme.initSystemListener();
    init();
    return cleanup;
  });
</script>

<div class="app-shell" data-theme={theme.currentTheme}>
  <div class="app-shell-content">
    {@render children()}
  </div>
  <PlayerBar />
</div>

<style>
  .app-shell {
    display: flex;
    flex-direction: column;
    height: 100vh;
    overflow: hidden;
    background: var(--sg-surface);
    color: var(--sg-on-surface, #e3e1e9);
  }

  .app-shell-content {
    flex: 1;
    min-height: 0;
    overflow: hidden;
  }
</style>
