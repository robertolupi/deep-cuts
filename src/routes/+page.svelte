<script lang="ts">
  import { onMount } from "svelte";

  // Import custom modular components
  import Navbar from "$lib/components/Navbar.svelte";
  import HeroPanel from "$lib/components/HeroPanel.svelte";
  import AudioPlayer from "$lib/components/AudioPlayer.svelte";
  import TrackList from "$lib/components/TrackList.svelte";
  import MusicMap from "$lib/components/MusicMap.svelte";
  import LibrarySettings from "$lib/components/LibrarySettings.svelte";
  import AnalysisPanel from "$lib/components/AnalysisPanel.svelte";
  import { library } from "$lib/stores/library.svelte";
  import { player } from "$lib/stores/player.svelte";
  import { filters } from "$lib/stores/filters.svelte";
  import { theme } from "$lib/stores/theme.svelte";
  import { ui } from "$lib/stores/ui.svelte";

  // Resizable Split Pane Heights (removed in Phase 1.7)
  let tauriConnected = $state(false);
  let topPaneHeight = $state(330);
  let isResizing = $state(false);

  const selectedTrack = $derived(player.selectedTrack);

  // Check Tauri connectivity, restore theme, initialize library
  onMount(() => {
    async function init() {
      await library.init();
      tauriConnected = library.tauriConnected;
      await theme.init(tauriConnected);
    }

    const cleanup = theme.initSystemListener();
    init();
    return cleanup;
  });
</script>

<div class="app-layout">
  <!-- Top Glass Navigation Bar -->
  <Navbar />

  <!-- Main Workspace -->
  <main class="workspace">
    {#if ui.activeView === 'table'}
      <div class="dashboard-split-layout">
        <!-- Top Pane: Welcome Hero or WaveSurfer audio analyzer -->
        <div class="top-pane-resizable glass-panel" style="height: {topPaneHeight}px">
          {#if selectedTrack === null}
            <HeroPanel />
          {:else}
            <AudioPlayer />
          {/if}
        </div>

        <!-- Draggable Resizer Dividers (mouse handlers moved to Phase 1.7) -->
        <div
          class="split-pane-resizer {isResizing ? 'active' : ''}"
          role="separator"
          aria-valuenow={topPaneHeight}
          aria-valuemin={220}
          aria-valuemax={700}
        >
          <div class="resizer-knob"></div>
        </div>

        <!-- Bottom Pane: List of Tracks & Filters -->
        <TrackList
          tracks={library.tracks}
          {selectedTrack}
          isPlaying={player.isPlaying}
          onTrackSelect={(t) => player.playTrack(t, theme.resolvedTheme, filters.filteredTracks)}
        />
      </div>

    {:else if ui.activeView === 'analysis'}
      <AnalysisPanel />

    {:else if ui.activeView === 'map'}
      <MusicMap bind:focusTrackId={ui.mapFocusTrackId} />

    {:else if ui.activeView === 'settings'}
      <LibrarySettings />
    {/if}
  </main>
</div>
