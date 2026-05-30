<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";

  // Import custom modular components
  import Navbar from "$lib/components/Navbar.svelte";
  import HeroPanel from "$lib/components/HeroPanel.svelte";
  import AudioPlayer from "$lib/components/AudioPlayer.svelte";
  import TrackList from "$lib/components/TrackList.svelte";
  import MusicMap from "$lib/components/MusicMap.svelte";
  import LibrarySettings from "$lib/components/LibrarySettings.svelte";
  import AnalysisPanel from "$lib/components/AnalysisPanel.svelte";
  import type { WatchedDirectory, Track } from "$lib/types";
  import { library } from "$lib/stores/library.svelte";
  import { player } from "$lib/stores/player.svelte";
  import { filters } from "$lib/stores/filters.svelte";
  import { theme } from "$lib/stores/theme.svelte";

  // State managers using Svelte 5 runes
  let tauriConnected = $state(false);
  let activeTab = $state("dashboard");
  let mapFocusTrackId = $state<number | null>(null);

  function findSimilar(trackId: number) {
    mapFocusTrackId = trackId;
    activeTab = 'music-map';
  }

  // Local Form / Settings States
  let name = $state("");
  let path = $state("");
  let errorMessage = $state("");
  let successMessage = $state("");
  let isAddLoading = $state(false);

  // Resizable Split Pane Heights
  let topPaneHeight = $state(330);
  let isResizing = $state(false);
  let showDetails = $state(false);
  let preDetailsHeight = 330;

  function toggleDetails() {
    showDetails = !showDetails;
    if (showDetails) {
      preDetailsHeight = topPaneHeight;
      topPaneHeight = 520;
    } else {
      topPaneHeight = preDetailsHeight;
    }
  }

  // selectedTrack now lives in the player store
  const selectedTrack = $derived(player.selectedTrack);

  // Trigger native RFD directory selector in Rust
  async function choosePath() {
    try {
      const selected = await invoke<string | null>("select_directory");
      if (selected) {
        path = selected;
        // Autofill a friendly collection name from the folder basename
        if (!name) {
          const parts = selected.split(/[/\\]/);
          const baseName = parts[parts.length - 1] || parts[parts.length - 2] || "Music Library";
          name = baseName;
        }
        showToast("Path selected successfully.", "success");
      }
    } catch (err: any) {
      showToast(err.toString(), "error");
    }
  }

  // Submit and save new directory configuration
  async function addDirectory() {
    if (!name.trim() || !path.trim()) {
      showToast("Collection Name and Directory Path are required.", "error");
      return;
    }

    isAddLoading = true;
    try {
      await library.addDirectory(name, path);
      showToast(`Added folder "${name}" to monitored lists.`, "success");
      name = "";
      path = "";
    } catch (err: any) {
      showToast(err.toString(), "error");
    } finally {
      isAddLoading = false;
    }
  }

  // Executes directory removal
  async function removeDirectory(id: number, folderName: string) {
    try {
      await library.removeDirectory(id);
      showToast(`Stopped watching "${folderName}".`, "success");
    } catch (err: any) {
      showToast(err.toString(), "error");
    }
  }

  // Toast notifier helper
  let toastTimeout: any;
  function showToast(msg: string, type: "success" | "error") {
    clearTimeout(toastTimeout);
    if (type === "error") {
      errorMessage = msg;
      successMessage = "";
    } else {
      successMessage = msg;
      errorMessage = "";
    }
    toastTimeout = setTimeout(() => {
      errorMessage = "";
      successMessage = "";
    }, 4500);
  }

  // Trigger all library monitoring index scan
  async function triggerScan() {
    if (library.isScanning) return;
    if (library.directories.length === 0) {
      showToast("Register at least one monitored library directory first.", "error");
      return;
    }

    try {
      await library.triggerScan();
      showToast("Library scanning initiated in background.", "success");
    } catch (err: any) {
      showToast(err.toString(), "error");
    }
  }

  async function exportSidecars() {
    try {
      const count = await library.exportSidecars();
      showToast(`Exported ${count} sidecar file${count === 1 ? "" : "s"}.`, "success");
    } catch (err: any) {
      showToast(err.toString(), "error");
    }
  }

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
  <Navbar bind:activeTab />

  <!-- Main Workspace -->
  <main class="workspace">
    {#if activeTab === 'dashboard'}
      <div class="dashboard-split-layout">
        <!-- Top Pane: Welcome Hero or WaveSurfer audio analyzer -->
        <div class="top-pane-resizable glass-panel" style="height: {topPaneHeight}px">
          {#if selectedTrack === null}
            <HeroPanel bind:activeTab />
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
          bind:activeTab
        />
      </div>

    {:else if activeTab === 'analysis'}
      <AnalysisPanel />

    {:else if activeTab === 'music-map'}
      <MusicMap bind:focusTrackId={mapFocusTrackId} />
      
    {:else if activeTab === 'settings'}
      <LibrarySettings
        directories={library.directories}
        trackCount={library.trackCount}
        isScanning={library.isScanning}
        scanProgress={library.scanProgress}
        scanCurrentFile={library.scanCurrentFile}
        scanProcessedCount={library.scanProcessedCount}
        scanTotalCount={library.scanTotalCount}
        bind:path
        bind:name
        {isAddLoading}
        {errorMessage}
        {successMessage}
        {choosePath}
        {addDirectory}
        {removeDirectory}
        {triggerScan}
        {exportSidecars}
      />
    {/if}
  </main>
</div>
