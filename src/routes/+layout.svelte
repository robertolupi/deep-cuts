<script lang="ts">
  import '../app.css';
  import { onMount } from 'svelte';
  import PlayerBar from '$lib/components/PlayerBar.svelte';
  import TrackDetailPane from '$lib/components/TrackDetailPane.svelte';
  import FilterSidebar from '$lib/components/FilterSidebar.svelte';
  import Toast from '$lib/components/Toast.svelte';
  import { library } from '$lib/stores/library.svelte';
  import { theme } from '$lib/stores/theme.svelte';
  import { ui } from '$lib/stores/ui.svelte';

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
  {#if ui.activeView === 'table' || ui.activeView === 'map'}
    <FilterSidebar />
  {/if}
  <div class="app-left-col">
    <div class="app-shell-content">
      {@render children()}
    </div>
    <PlayerBar />
  </div>
  <TrackDetailPane />
</div>
<Toast />

<style>
  .app-shell {
    display: flex;
    flex-direction: row;
    height: 100vh;
    overflow: hidden;
    background: var(--sg-surface);
    color: var(--sg-on-surface, #e3e1e9);
  }

  .app-left-col {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .app-shell-content {
    flex: 1;
    min-height: 0;
    height: 0;
    overflow: hidden;
  }
</style>
