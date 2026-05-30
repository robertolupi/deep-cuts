<script lang="ts">
  import Navbar from "$lib/components/Navbar.svelte";
  import FilterSidebar from "$lib/components/FilterSidebar.svelte";
  import TrackList from "$lib/components/TrackList.svelte";
  import MusicMap from "$lib/components/MusicMap.svelte";
  import LibrarySettings from "$lib/components/LibrarySettings.svelte";
  import AnalysisPanel from "$lib/components/AnalysisPanel.svelte";
  import { player } from "$lib/stores/player.svelte";
  import { ui } from "$lib/stores/ui.svelte";

  const selectedTrack = $derived(player.selectedTrack);
</script>

<div class="app-layout">
  <Navbar />

  <main class="workspace">
    {#if ui.activeView === 'table' || ui.activeView === 'map'}
      <div class="table-view-layout">
        <FilterSidebar />
        {#if ui.activeView === 'table'}
          <TrackList
            {selectedTrack}
            isPlaying={player.isPlaying}
            onTrackSelect={(t) => player.playTrack(t)}
          />
        {:else}
          <MusicMap />
        {/if}
      </div>

    {:else if ui.activeView === 'analysis'}
      <AnalysisPanel />

    {:else if ui.activeView === 'settings'}
      <LibrarySettings />
    {/if}
  </main>
</div>
