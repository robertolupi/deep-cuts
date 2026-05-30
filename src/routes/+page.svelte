<script lang="ts">
  import Navbar from "$lib/components/Navbar.svelte";
  import HeroPanel from "$lib/components/HeroPanel.svelte";
  import AudioPlayer from "$lib/components/AudioPlayer.svelte";
  import TrackList from "$lib/components/TrackList.svelte";
  import MusicMap from "$lib/components/MusicMap.svelte";
  import LibrarySettings from "$lib/components/LibrarySettings.svelte";
  import AnalysisPanel from "$lib/components/AnalysisPanel.svelte";
  import { library } from "$lib/stores/library.svelte";
  import { player } from "$lib/stores/player.svelte";
  import { ui } from "$lib/stores/ui.svelte";

  const selectedTrack = $derived(player.selectedTrack);
</script>

<div class="app-layout">
  <Navbar />

  <main class="workspace">
    {#if ui.activeView === 'table'}
      <div class="dashboard-split-layout">
        <div class="top-pane-resizable glass-panel">
          {#if selectedTrack === null}
            <HeroPanel />
          {:else}
            <AudioPlayer />
          {/if}
        </div>

        <div class="split-pane-resizer" role="separator" aria-valuenow={330} aria-valuemin={220} aria-valuemax={700}>
          <div class="resizer-knob"></div>
        </div>

        <TrackList
          tracks={library.tracks}
          {selectedTrack}
          isPlaying={player.isPlaying}
          onTrackSelect={(t) => player.playTrack(t)}
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
