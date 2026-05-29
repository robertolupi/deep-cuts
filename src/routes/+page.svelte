<script lang="ts">
  import { onMount, tick } from "svelte";
  import { invoke, convertFileSrc } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import WaveSurfer from "wavesurfer.js";
  import Spectrogram from "wavesurfer.js/dist/plugins/spectrogram.esm.js";

  // Import custom modular components
  import Navbar from "$lib/components/Navbar.svelte";
  import HeroPanel from "$lib/components/HeroPanel.svelte";
  import AudioPlayer from "$lib/components/AudioPlayer.svelte";
  import TrackList from "$lib/components/TrackList.svelte";
  import MusicMap from "$lib/components/MusicMap.svelte";
  import LibrarySettings from "$lib/components/LibrarySettings.svelte";
  import AnalysisPanel from "$lib/components/AnalysisPanel.svelte";
  import type { WatchedDirectory, Track } from "$lib/types";

  // State managers using Svelte 5 runes
  let currentTheme = $state("system");
  let resolvedTheme = $state("dark");
  let tauriConnected = $state(false);
  let activeTab = $state("dashboard");

  // Watched directories state
  let directories = $state<WatchedDirectory[]>([]);
  let name = $state("");
  let path = $state("");
  let errorMessage = $state("");
  let successMessage = $state("");
  let isAddLoading = $state(false);

  // Scanning States
  let isScanning = $state(false);
  let scanProgress = $state(0);
  let scanCurrentFile = $state("Idle");
  let scanProcessedCount = $state(0);
  let scanTotalCount = $state(0);

  let trackCount = $state(0);

  // Resizable Split Pane Heights
  let topPaneHeight = $state(330); // Default Top Pane height in pixels
  let isResizing = $state(false);
  let showDetails = $state(false);
  let preDetailsHeight = 330;

  function toggleDetails() {
    showDetails = !showDetails;
    if (showDetails) {
      preDetailsHeight = topPaneHeight;
      topPaneHeight = 520; // Auto-expand the pane
    } else {
      topPaneHeight = preDetailsHeight; // Restore previous height
    }
  }

  // WaveSurfer Bound States
  let wavesurfer = $state<WaveSurfer | null>(null);
  let isPlaying = $state(false);
  let currentTime = $state(0);
  let duration = $state(0);

  // DOM Container bindings
  let waveformContainer = $state<HTMLDivElement | null>(null);
  let spectrogramContainer = $state<HTMLDivElement | null>(null);

  // Track Collection States
  let tracks = $state<Track[]>([]);
  let selectedTrack = $state<Track | null>(null);
  let searchQuery = $state("");
  let selectedGenre = $state("All");

  // Derived list of filtered tracks reactively matching search box and genre selections
  // Needed for previous/next track traversal in root page context
  let filteredTracks = $derived.by(() => {
    return tracks.filter(t => {
      // 1. Genre filter
      if (selectedGenre !== "All") {
        if (!t.genre || !t.genre.toLowerCase().includes(selectedGenre.toLowerCase())) {
          return false;
        }
      }
      
      // 2. Search text filter
      if (searchQuery.trim()) {
        const query = searchQuery.toLowerCase();
        const matchesTitle = t.title?.toLowerCase().includes(query) ?? false;
        const matchesArtist = t.artist?.toLowerCase().includes(query) ?? false;
        const matchesAlbum = t.album?.toLowerCase().includes(query) ?? false;
        const matchesFilename = t.filename.toLowerCase().includes(query);
        return matchesTitle || matchesArtist || matchesAlbum || matchesFilename;
      }
      
      return true;
    });
  });

  // Retrieve track count from SQLite
  async function fetchTrackCount() {
    try {
      trackCount = await invoke<number>("get_track_count");
    } catch (err: any) {
      console.error("Failed to fetch track count:", err);
    }
  }

  // Retrieve track list from SQLite
  async function fetchTracks() {
    try {
      tracks = await invoke<Track[]>("get_tracks");
    } catch (err: any) {
      console.error("Failed to fetch tracks:", err);
    }
  }

  // Draggable Split Pane Resize Handlers
  function handleMouseDown(e: MouseEvent) {
    e.preventDefault();
    isResizing = true;
    window.addEventListener("mousemove", handleMouseMove);
    window.addEventListener("mouseup", handleMouseUp);
  }

  function handleMouseMove(e: MouseEvent) {
    if (!isResizing) return;
    const workspaceElement = document.querySelector(".workspace");
    if (!workspaceElement) return;

    const rect = workspaceElement.getBoundingClientRect();
    const relativeY = e.clientY - rect.top;

    // Constrain Top Pane height between 220px and 700px for DAWs
    if (relativeY >= 220 && relativeY <= 700) {
      topPaneHeight = relativeY;
    }
  }

  function handleMouseUp() {
    if (isResizing) {
      isResizing = false;
      window.removeEventListener("mousemove", handleMouseMove);
      window.removeEventListener("mouseup", handleMouseUp);
    }
  }

  // Theme-aware WaveSurfer playback launcher
  async function playTrack(track: Track) {
    selectedTrack = track;
    isPlaying = false;
    currentTime = 0;
    duration = 0;

    // Destroy existing WaveSurfer instance
    if (wavesurfer) {
      wavesurfer.destroy();
      wavesurfer = null;
    }

    // Convert absolute filepath to safe WebView asset URL
    const assetUrl = convertFileSrc(track.path);

    // Wait for Svelte DOM tick to bind containers
    await tick();

    if (!waveformContainer) {
      console.error("Waveform container DOM element binding not found.");
      return;
    }

    // Build the visualizer
    wavesurfer = WaveSurfer.create({
      container: waveformContainer,
      waveColor: resolvedTheme === "light" ? "rgba(15, 23, 42, 0.08)" : "rgba(255, 255, 255, 0.08)",
      cursorColor: resolvedTheme === "light" ? "var(--color-primary)" : "#00f2fe",
      cursorWidth: 2,
      barWidth: 3,
      barGap: 2.2,
      barRadius: 2,
      height: 75,
      normalize: true,
      plugins: [
        Spectrogram.create({
          container: spectrogramContainer!,
          labels: true,
          fftSamples: 512,
          height: 75,
          labelsColor: resolvedTheme === "light" ? "#4b5563" : "#00f2fe",
        })
      ]
    });

    // Theme-Aware Linear Canvas Gradients
    const ctx = document.createElement("canvas").getContext("2d");
    if (ctx) {
      const gradient = ctx.createLinearGradient(0, 0, 800, 0);
      if (resolvedTheme === "accessible") {
        wavesurfer.setOptions({ progressColor: "#ffffff" });
      } else if (resolvedTheme === "light") {
        gradient.addColorStop(0, "#4f46e5"); // Soft Indigo
        gradient.addColorStop(0.5, "#7c3aed"); // Soft Purple
        gradient.addColorStop(1, "#db2777"); // Soft Pink
        wavesurfer.setOptions({ progressColor: gradient });
      } else {
        gradient.addColorStop(0, "#00f2fe"); // Cyber Cyan
        gradient.addColorStop(0.5, "#8a2be2"); // Indigo
        gradient.addColorStop(1, "#ff007f"); // Studio Magenta
        wavesurfer.setOptions({ progressColor: gradient });
      }
    }

    wavesurfer.load(assetUrl);

    // Bind event hooks
    wavesurfer.on("play", () => { isPlaying = true; });
    wavesurfer.on("pause", () => { isPlaying = false; });
    
    wavesurfer.on("timeupdate", (time) => {
      currentTime = time;
    });

    wavesurfer.on("ready", () => {
      if (wavesurfer) {
        duration = wavesurfer.getDuration();
        wavesurfer.play(); // Autoplay
      }
    });

    wavesurfer.on("finish", () => {
      isPlaying = false;
      currentTime = 0;
      handleNextTrack(); // Auto-advance to next song!
    });
  }

  function togglePlayback() {
    if (!wavesurfer) return;
    wavesurfer.playPause();
  }

  function resetPlayer() {
    if (wavesurfer) {
      wavesurfer.destroy();
      wavesurfer = null;
    }
    selectedTrack = null;
    isPlaying = false;
    currentTime = 0;
    duration = 0;
  }

  function handlePrevTrack() {
    if (!selectedTrack || tracks.length === 0) return;
    const activeList = filteredTracks;
    const index = activeList.findIndex(t => t.id === selectedTrack!.id);
    if (index > 0) {
      playTrack(activeList[index - 1]);
    } else if (activeList.length > 0) {
      playTrack(activeList[activeList.length - 1]); // Loop back
    }
  }

  async function handleNextTrack() {
    if (!selectedTrack || tracks.length === 0) return;
    const activeList = filteredTracks;
    const index = activeList.findIndex(t => t.id === selectedTrack!.id);
    if (index !== -1 && index < activeList.length - 1) {
      playTrack(activeList[index + 1]);
    } else if (activeList.length > 0) {
      playTrack(activeList[0]); // Loop back to start
    }
  }

  function formatDuration(sec: number): string {
    const mins = Math.floor(sec / 60);
    const secs = Math.floor(sec % 60);
    return `${mins}:${secs < 10 ? "0" : ""}${secs}`;
  }

  function formatSize(bytes: number): string {
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  }

  // Retrieve directories list from SQLite
  async function fetchDirectories() {
    try {
      directories = await invoke<WatchedDirectory[]>("get_watched_directories");
    } catch (err: any) {
      showToast(err.toString(), "error");
    }
  }

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
      await invoke("add_watched_directory", { name, path });
      showToast(`Added folder "${name}" to watched lists.`, "success");
      name = "";
      path = "";
      await fetchDirectories();
    } catch (err: any) {
      showToast(err.toString(), "error");
    } finally {
      isAddLoading = false;
    }
  }

  // Executes directory removal
  async function removeDirectory(id: number, folderName: string) {
    try {
      await invoke("remove_watched_directory", { id });
      showToast(`Stopped watching "${folderName}".`, "success");
      await fetchDirectories();
      await fetchTrackCount();
      await fetchTracks();
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
    if (isScanning) return;
    if (directories.length === 0) {
      showToast("Register at least one monitored library directory first.", "error");
      return;
    }

    try {
      isScanning = true;
      scanProgress = 0;
      scanCurrentFile = "Starting library scan...";
      await invoke("scan_all_libraries");
      showToast("Library scanning initiated in background.", "success");
    } catch (err: any) {
      isScanning = false;
      showToast(err.toString(), "error");
    }
  }

  async function exportSidecars() {
    try {
      const count = await invoke<number>("export_sidecars");
      showToast(`Exported ${count} sidecar file${count === 1 ? "" : "s"}.`, "success");
    } catch (err: any) {
      showToast(err.toString(), "error");
    }
  }

  // Check Tauri connectivity and restore theme
  onMount(async () => {
    // Stage 1: Load instantly from localStorage for seamless boot
    const saved = localStorage.getItem("deep-cuts-theme") || "system";
    await setTheme(saved, false);

    let unlistenProgress: () => void;

    // Stage 2: Query database via Tauri if online
    try {
      // Query saved theme from Tauri SQLite database
      const dbTheme = await invoke<string>("get_theme");
      tauriConnected = true;
      if (dbTheme && dbTheme !== saved) {
        await setTheme(dbTheme, false);
      }

      // Fetch directories
      await fetchDirectories();
      // Fetch initial track count
      await fetchTrackCount();
      // Fetch initial track list
      await fetchTracks();

      // Listen for progress updates emitted by the parallel scanner
      unlistenProgress = await listen<any>("scan:progress", (event) => {
        const payload = event.payload;
        isScanning = payload.is_scanning;
        scanProgress = payload.progress;
        scanCurrentFile = payload.current_file;
        scanProcessedCount = payload.processed_count;
        scanTotalCount = payload.total_count;

        if (!payload.is_scanning && payload.progress === 100) {
          showToast(payload.current_file, "success");
          fetchTrackCount();
          fetchTracks();
        }
      });
    } catch (e) {
      console.warn("Tauri shell connection offline (running in browser context) or database loading.");
    }

    return () => {
      if (unlistenProgress) {
        unlistenProgress();
      }
    };
  });

  // Apply theme dynamically to HTML element and persist to storage
  async function setTheme(theme: string, saveToDb = true) {
    currentTheme = theme;
    localStorage.setItem("deep-cuts-theme", theme);

    if (theme === "system") {
      const isDark = window.matchMedia("(prefers-color-scheme: dark)").matches;
      resolvedTheme = isDark ? "dark" : "light";
      document.documentElement.setAttribute("data-theme", resolvedTheme);
    } else {
      resolvedTheme = theme;
      document.documentElement.setAttribute("data-theme", theme);
    }

    if (saveToDb && tauriConnected) {
      try {
        await invoke("save_theme", { theme });
      } catch (e) {
        console.error("Failed to save theme in Tauri database:", e);
      }
    }
  }

  // Svelte 5 effect listening for system theme changes if theme is set to 'system'
  $effect(() => {
    if (currentTheme !== "system") return;

    const mediaQuery = window.matchMedia("(prefers-color-scheme: dark)");
    const handler = (e: MediaQueryListEvent) => {
      resolvedTheme = e.matches ? "dark" : "light";
      document.documentElement.setAttribute("data-theme", resolvedTheme);
    };

    mediaQuery.addEventListener("change", handler);
    return () => mediaQuery.removeEventListener("change", handler);
  });
</script>

<div class="app-layout">
  <!-- Top Glass Navigation Bar -->
  <Navbar bind:activeTab bind:currentTheme onThemeChange={setTheme} />

  <!-- Main Workspace -->
  <main class="workspace">
    {#if activeTab === 'dashboard'}
      <div class="dashboard-split-layout">
        <!-- Top Pane: Welcome Hero or WaveSurfer audio analyzer -->
        <div class="top-pane-resizable glass-panel" style="height: {topPaneHeight}px">
          {#if selectedTrack === null}
            <HeroPanel bind:activeTab />
          {:else}
            <AudioPlayer
              {selectedTrack}
              bind:isPlaying
              bind:currentTime
              bind:duration
              bind:showDetails
              {toggleDetails}
              {formatDuration}
              {formatSize}
              bind:waveformContainer
              bind:spectrogramContainer
              {togglePlayback}
              {handlePrevTrack}
              {handleNextTrack}
            />
          {/if}
        </div>

        <!-- Draggable Resizer Dividers -->
        <div 
          class="split-pane-resizer {isResizing ? 'active' : ''}" 
          onmousedown={handleMouseDown}
          role="separator"
          aria-valuenow={topPaneHeight}
          aria-valuemin={220}
          aria-valuemax={700}
          tabindex="0"
        >
          <div class="resizer-knob"></div>
        </div>

        <!-- Bottom Pane: List of Tracks & Filters -->
        <TrackList
          {tracks}
          {selectedTrack}
          {isPlaying}
          bind:searchQuery
          bind:selectedGenre
          onTrackSelect={playTrack}
          {formatDuration}
          bind:activeTab
        />
      </div>

    {:else if activeTab === 'analysis'}
      <AnalysisPanel />

    {:else if activeTab === 'music-map'}
      <MusicMap />
      
    {:else if activeTab === 'settings'}
      <LibrarySettings
        {directories}
        {trackCount}
        {isScanning}
        {scanProgress}
        {scanCurrentFile}
        {scanProcessedCount}
        {scanTotalCount}
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
