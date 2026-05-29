<script lang="ts">
  import { onMount, tick } from "svelte";
  import { invoke, convertFileSrc } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import WaveSurfer from "wavesurfer.js";
  import Spectrogram from "wavesurfer.js/dist/plugins/spectrogram.esm.js";

  interface WatchedDirectory {
    id: number;
    name: string;
    path: string;
  }

  interface Track {
    id: number;
    watched_directory_id: number;
    path: string;
    filename: string;
    size_bytes: number;
    last_modified: number;
    duration_seconds: number;
    sample_rate: number | null;
    bitrate: number | null;
    channels: number | null;
    bit_depth: number | null;
    title: string | null;
    artist: string | null;
    album: string | null;
    genre: string | null;
    year: number | null;
    track_number: number | null;
    track_total: number | null;
    disc_number: number | null;
    disc_total: number | null;
    album_artist: string | null;
    composer: string | null;
    comment: string | null;
    bpm: number | null;
    lyrics: string | null;
  }

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

  // Derived list of distinct genres reactively computed from tracks
  let genresList = $derived.by(() => {
    const list = new Set<string>();
    for (const t of tracks) {
      if (t.genre) {
        for (const g of t.genre.split(/[,;]/)) {
          const trimmed = g.trim();
          if (trimmed) list.add(trimmed);
        }
      }
    }
    return ["All", ...Array.from(list).sort()];
  });

  // Derived list of filtered tracks reactively matching search box and genre selections
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
  <header class="navbar glass-panel">
    <div class="brand">
      <div class="logo-circle">
        <div class="logo-cut"></div>
      </div>
      <span class="brand-name shimmer-text">DEEP CUTS</span>
    </div>

    <!-- Navigation Tabs Mockup -->
    <nav class="nav-tabs">
      <button 
        class="nav-tab {activeTab === 'dashboard' ? 'active' : ''}" 
        onclick={() => activeTab = 'dashboard'}
      >
        Dashboard
      </button>
      <button 
        class="nav-tab {activeTab === 'music-map' ? 'active' : ''}" 
        onclick={() => activeTab = 'music-map'}
      >
        Music Map
      </button>
      <button 
        class="nav-tab {activeTab === 'settings' ? 'active' : ''}" 
        onclick={() => activeTab = 'settings'}
      >
        Settings
      </button>
    </nav>

    <!-- Theme and Status Controls -->
    <div class="controls">
      <div class="theme-picker">
        <select 
          class="theme-select" 
          value={currentTheme} 
          onchange={(e) => setTheme((e.target as HTMLSelectElement).value)}
          aria-label="Theme Selection"
        >
          <option value="system">🌓 System</option>
          <option value="dark">🌙 Dark Mode</option>
          <option value="light">☀️ Light Mode</option>
          <option value="accessible">👓 High Contrast</option>
        </select>
      </div>
    </div>
  </header>

  <!-- Main Workspace -->
  <main class="workspace">
    {#if activeTab === 'dashboard'}
      <div class="dashboard-split-layout">
        <!-- Top Pane: Welcome Hero or WaveSurfer audio analyzer -->
        <div class="top-pane-resizable glass-panel" style="height: {topPaneHeight}px">
          {#if selectedTrack === null}
            <!-- Hero Welcome Panel -->
            <div class="hero-panel-content">
              <div class="hero-left">
                <h1 class="hero-title">Your Audio, Structured.</h1>
                <p class="hero-description" style="margin-top: 0rem; font-size: 1rem; line-height: 1.5; color: var(--text-secondary);">
                  Deep Cuts analyzes local audio collections, performing integrated EBU R128 loudness checks, Camelot key mapping, spectral onset BPM extraction, and offline machine-learning indexing.
                </p>

                <div class="hero-buttons">
                  <button class="btn-primary" onclick={() => activeTab = 'settings'} style="font-size: 0.9rem; padding: 0.5rem 1.25rem;">
                    Configure watched folders
                  </button>
                  <button class="btn-secondary" onclick={() => activeTab = 'settings'} style="font-size: 0.9rem; padding: 0.5rem 1.25rem;">
                    Monitored Library
                  </button>
                </div>
              </div>

              <div class="hero-right">
                <!-- Animated Crate-Digging Vinyl Waveform -->
                <div class="vinyl-display">
                  <img 
                    src="/deep_cuts_transparent.png" 
                    alt="Deep Cuts Custom Icon" 
                    class="vinyl-image" 
                    style="width: 100px; height: 100px;"
                  />
                  <!-- Mock Audio Waveform bars -->
                  <div class="waveform-animation" style="height: 40px; margin-top: 0.5rem;">
                    {#each Array(14) as _, i}
                      <div class="wave-bar" style="width: 4px; --height: {10 + Math.sin(i * 0.5) * 15 + Math.random() * 8}px; --delay: {i * 0.08}s"></div>
                    {/each}
                  </div>
                </div>
              </div>
            </div>
          {:else}
            <!-- Beautiful WaveSurfer Audio Player Pane -->
            <div class="audio-player-pane {showDetails ? 'expanded' : ''}">
              <div class="player-upper-row">
                <!-- Left side: Album cover vinyl & Track metadata -->
                <div class="player-left-col">
                  <div class="vinyl-spinner-large {isPlaying ? 'spinning' : ''}">
                    <img src="/deep_cuts_transparent.png" alt="Vinyl record center" class="vinyl-record-img" />
                  </div>
                  <div class="track-details-block">
                    <div class="track-title-row">
                      <span class="badge badge-cyan" style="font-size: 0.72rem; padding: 0.15rem 0.4rem;">{selectedTrack.path.split('.').pop()?.toUpperCase()}</span>
                      <h4>{selectedTrack.title || selectedTrack.filename}</h4>
                    </div>
                    <p class="track-credits">
                      {#if selectedTrack.artist}<span class="artist">{selectedTrack.artist}</span>{/if}
                      {#if selectedTrack.artist && selectedTrack.album}<span class="sep">—</span>{/if}
                      {#if selectedTrack.album}<span class="album">{selectedTrack.album}</span>{/if}
                    </p>
                    <p class="track-tech-specs">
                      {#if selectedTrack.sample_rate}{Math.round(selectedTrack.sample_rate / 1000)} kHz • {/if}
                      {#if selectedTrack.bit_depth}{selectedTrack.bit_depth}-bit • {/if}
                      {#if selectedTrack.bitrate}{selectedTrack.bitrate} kbps • {/if}
                      {formatSize(selectedTrack.size_bytes)}
                    </p>
                  </div>
                </div>

                <!-- Center/Main: WaveSurfer, Spectrogram & Playback controls -->
                <div class="player-main-col">
                  <!-- WaveSurfer wave wrapper -->
                  <div class="waveform-outer">
                    <div bind:this={waveformContainer} class="waveform-canvas-wrap"></div>
                  </div>
                  
                  <!-- Spectrogram wrapper -->
                  <div class="spectrogram-outer">
                    <div bind:this={spectrogramContainer} class="spectrogram-canvas-wrap"></div>
                  </div>

                  <!-- Playback controls -->
                  <div class="playback-controls-row">
                    <div style="display: flex; gap: 0.75rem; align-items: center;">
                      <!-- Skip back -->
                      <button class="player-btn" title="Previous Track" onclick={handlePrevTrack}>
                        <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="currentColor">
                          <polygon points="19 20 9 12 19 4 19 20"/>
                          <rect x="5" y="4" width="2" height="16"/>
                        </svg>
                      </button>
                      <!-- Play/Pause -->
                      <button class="btn-play-pause {isPlaying ? 'playing' : ''}" onclick={togglePlayback}>
                        {#if isPlaying}
                          <svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="currentColor">
                            <rect x="6" y="4" width="4" height="16" rx="1"/>
                            <rect x="14" y="4" width="4" height="16" rx="1"/>
                          </svg>
                        {:else}
                          <svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="currentColor">
                            <polygon points="6 4 20 12 6 20 6 4"/>
                          </svg>
                        {/if}
                      </button>
                      <!-- Skip forward -->
                      <button class="player-btn" title="Next Track" onclick={handleNextTrack}>
                        <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="currentColor">
                          <polygon points="5 4 15 12 5 20 5 4"/>
                          <rect x="17" y="4" width="2" height="16"/>
                        </svg>
                      </button>
                    </div>

                    <!-- Time counter -->
                    <div class="time-readout">
                      <span class="current-time">{formatDuration(currentTime)}</span>
                      <span class="divider">/</span>
                      <span class="total-duration">{formatDuration(duration)}</span>
                    </div>

                    <div style="display: flex; gap: 0.75rem; align-items: center;">
                      <!-- Details Toggle button -->
                      <button 
                        class="btn-secondary {showDetails ? 'pulse-glow-cyan' : ''}" 
                        onclick={toggleDetails} 
                        style="font-size: 0.75rem; padding: 0.35rem 0.8rem; border-radius: var(--radius-sm); display: flex; align-items: center; gap: 0.3rem;"
                        title="Toggle Multi-column Metadata Details"
                      >
                        <svg xmlns="http://www.w3.org/2000/svg" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" style="vertical-align: middle;">
                          <circle cx="12" cy="12" r="10"/>
                          <line x1="12" y1="16" x2="12" y2="12"/>
                          <line x1="12" y1="8" x2="12.01" y2="8"/>
                        </svg>
                        <span style="vertical-align: middle;">{showDetails ? 'Hide Details' : 'Details'}</span>
                      </button>
                    </div>
                  </div>
                </div>
              </div>

              <!-- Expanded Metadata Multicolumn Grid -->
              {#if showDetails}
                <div class="player-details-row">
                  <div class="metadata-grid">
                    <!-- Column 1: Track Details -->
                    <div class="metadata-col">
                      <div class="metadata-card">
                        <span class="metadata-label">Title</span>
                        <span class="metadata-value" title={selectedTrack.title || selectedTrack.filename}>{selectedTrack.title || '—'}</span>
                      </div>
                      <div class="metadata-card" style="margin-top: 0.75rem;">
                        <span class="metadata-label">Artist</span>
                        <span class="metadata-value" title={selectedTrack.artist || '—'}>{selectedTrack.artist || '—'}</span>
                      </div>
                      <div class="metadata-card" style="margin-top: 0.75rem;">
                        <span class="metadata-label">Album</span>
                        <span class="metadata-value" title={selectedTrack.album || '—'}>{selectedTrack.album || '—'}</span>
                      </div>
                    </div>

                    <!-- Column 2: Credits & Style -->
                    <div class="metadata-col">
                      <div class="metadata-card">
                        <span class="metadata-label">Album Artist</span>
                        <span class="metadata-value" title={selectedTrack.album_artist || '—'}>{selectedTrack.album_artist || '—'}</span>
                      </div>
                      <div class="metadata-card" style="margin-top: 0.75rem;">
                        <span class="metadata-label">Composer</span>
                        <span class="metadata-value" title={selectedTrack.composer || '—'}>{selectedTrack.composer || '—'}</span>
                      </div>
                      <div class="metadata-card" style="margin-top: 0.75rem;">
                        <span class="metadata-label">Genre</span>
                        <span class="metadata-value" title={selectedTrack.genre || '—'}>{selectedTrack.genre || '—'}</span>
                      </div>
                    </div>

                    <!-- Column 3: Tech & Tech specs -->
                    <div class="metadata-col">
                      <div class="metadata-card">
                        <span class="metadata-label">Technical Specs</span>
                        <span class="metadata-value">
                          {#if selectedTrack.sample_rate}<code>{Math.round(selectedTrack.sample_rate / 1000)} kHz</code>{/if}
                          {#if selectedTrack.bit_depth}<code> • {selectedTrack.bit_depth}-bit</code>{/if}
                          {#if selectedTrack.bitrate}<code> • {selectedTrack.bitrate}k</code>{/if}
                        </span>
                      </div>
                      <div class="metadata-card" style="margin-top: 0.75rem;">
                        <span class="metadata-label">Format / Channels</span>
                        <span class="metadata-value">
                          <code>{selectedTrack.path.split('.').pop()?.toUpperCase()}</code>
                          {#if selectedTrack.channels} • <code>{selectedTrack.channels} ch</code>{/if}
                        </span>
                      </div>
                      <div class="metadata-card" style="margin-top: 0.75rem;">
                        <span class="metadata-label">Year / BPM</span>
                        <span class="metadata-value">
                          {selectedTrack.year || '—'}
                          {#if selectedTrack.bpm} • <code>{selectedTrack.bpm} BPM</code>{/if}
                        </span>
                      </div>
                    </div>

                    <!-- Column 4: Positioning & Filesystem -->
                    <div class="metadata-col">
                      <div class="metadata-card">
                        <span class="metadata-label">Track / Disc Info</span>
                        <span class="metadata-value">
                          T: {selectedTrack.track_number || '—'}{#if selectedTrack.track_total} of {selectedTrack.track_total}{/if}
                          {#if selectedTrack.disc_number} • D: {selectedTrack.disc_number}{#if selectedTrack.disc_total} of {selectedTrack.disc_total}{/if}{/if}
                        </span>
                      </div>
                      <div class="metadata-card" style="margin-top: 0.75rem;">
                        <span class="metadata-label">Duration / Size</span>
                        <span class="metadata-value">{formatDuration(selectedTrack.duration_seconds)} • {formatSize(selectedTrack.size_bytes)}</span>
                      </div>
                      <div class="metadata-card" style="margin-top: 0.75rem;">
                        <span class="metadata-label">Indexed File</span>
                        <span class="metadata-value" title={selectedTrack.filename}>{selectedTrack.filename}</span>
                      </div>
                    </div>
                  </div>

                  <!-- Full width filepath -->
                  <div style="border-top: 1px solid var(--border-color); margin-top: 0.85rem; padding-top: 0.5rem; display: flex; flex-direction: column; gap: 0.2rem;">
                    <span class="metadata-label">Absolute File Path</span>
                    <span class="metadata-value path-value" title={selectedTrack.path}><code>{selectedTrack.path}</code></span>
                  </div>

                  <!-- Lyrics & Comments row -->
                  {#if selectedTrack.lyrics || selectedTrack.comment}
                    <div style="border-top: 1px solid var(--border-color); margin-top: 0.75rem; padding-top: 0.5rem; display: grid; grid-template-columns: 1fr 1fr; gap: 1.5rem;">
                      {#if selectedTrack.lyrics}
                        <div class="metadata-card">
                          <span class="metadata-label">Lyrics</span>
                          <p style="font-size: 0.78rem; line-height: 1.4; color: var(--text-secondary); max-height: 70px; overflow-y: auto; white-space: pre-line; margin: 0.15rem 0 0 0;">{selectedTrack.lyrics}</p>
                        </div>
                      {/if}
                      {#if selectedTrack.comment}
                        <div class="metadata-card">
                          <span class="metadata-label">Comments</span>
                          <p style="font-size: 0.78rem; line-height: 1.4; color: var(--text-secondary); max-height: 70px; overflow-y: auto; margin: 0.15rem 0 0 0;">{selectedTrack.comment}</p>
                        </div>
                      {/if}
                    </div>
                  {/if}
                </div>
              {/if}
            </div>
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
        <div class="bottom-pane-scroller glass-panel">
          <!-- Filters & search Row -->
          <div class="tracks-toolbar">
            <div style="display: flex; gap: 1rem; align-items: center; flex: 1;">
              <!-- Search box -->
              <div class="search-box-wrap">
                <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="var(--text-muted)" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round" class="search-icon">
                  <circle cx="11" cy="11" r="8"/>
                  <line x1="21" y1="21" x2="16.65" y2="16.65"/>
                </svg>
                <input 
                  type="text" 
                  placeholder="Search tracks by title, artist, album, filename..." 
                  bind:value={searchQuery}
                  class="search-input"
                />
              </div>

              <!-- Genre Filter -->
              <div class="filter-select-wrap">
                <select bind:value={selectedGenre} class="filter-select" aria-label="Genre Filter">
                  {#each genresList as genre}
                    <option value={genre}>{genre === "All" ? "🏷️ All Genres" : genre}</option>
                  {/each}
                </select>
              </div>
            </div>

            <!-- Library metadata count badge -->
            <div class="library-count-badge">
              <code>{filteredTracks.length} / {tracks.length} tracks</code>
            </div>
          </div>

          <!-- Tracks Grid List Table -->
          {#if tracks.length > 0}
            {#if filteredTracks.length > 0}
              <div class="tracks-table-wrap">
                <table class="tracks-table">
                  <thead>
                    <tr>
                      <th style="width: 40px; text-align: center;">#</th>
                      <th>Title / Filename</th>
                      <th>Artist</th>
                      <th>Album</th>
                      <th>Duration</th>
                      <th style="width: 100px;">Format</th>
                    </tr>
                  </thead>
                  <tbody>
                    {#each filteredTracks as track, index (track.id)}
                      <tr 
                        class="track-row {selectedTrack?.id === track.id ? 'active-pulse' : ''}" 
                        onclick={() => playTrack(track)}
                      >
                        <td style="text-align: center; color: var(--text-muted); font-size: 0.82rem;">
                          {#if selectedTrack?.id === track.id && isPlaying}
                            <div class="playing-bars-mini">
                              <div class="bar"></div>
                              <div class="bar"></div>
                              <div class="bar"></div>
                            </div>
                          {:else}
                            {index + 1}
                          {/if}
                        </td>
                        <td class="track-title-cell" title={track.title || track.filename}>
                          <span class="track-primary-title">{track.title || track.filename}</span>
                          {#if !track.title}
                            <span class="file-tag">file</span>
                          {/if}
                        </td>
                        <td class="track-text-cell" title={track.artist || "Unknown"}>
                          {track.artist || "—"}
                        </td>
                        <td class="track-text-cell" title={track.album || "Unknown"}>
                          {track.album || "—"}
                        </td>
                        <td style="color: var(--text-secondary); font-size: 0.88rem;">
                          {formatDuration(track.duration_seconds)}
                        </td>
                        <td>
                          <span class="format-mini-badge">{track.path.split('.').pop()?.toUpperCase()}</span>
                          {#if track.bitrate}
                            <span class="bitrate-label">{track.bitrate}k</span>
                          {/if}
                        </td>
                      </tr>
                    {/each}
                  </tbody>
                </table>
              </div>
            {:else}
              <div class="empty-search-state">
                <svg xmlns="http://www.w3.org/2000/svg" width="36" height="36" viewBox="0 0 24 24" fill="none" stroke="var(--text-muted)" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
                  <circle cx="11" cy="11" r="8"/>
                  <line x1="21" y1="21" x2="16.65" y2="16.65"/>
                </svg>
                <h5>No Matching Tracks Found</h5>
                <p>Try refining your search text or switching the active genre filter.</p>
              </div>
            {/if}
          {:else}
            <!-- Empty state: no tracks in the library -->
            <div class="empty-tracks-state">
              <div class="vinyl-display-empty">
                <img src="/deep_cuts_transparent.png" alt="Deep Cuts empty vinyl" class="vinyl-image-empty" />
              </div>
              <h5>Your Music Library is Empty</h5>
              <p>Monitored collection folders have not scanned any supported audio files yet.</p>
              <button class="btn-primary" onclick={() => activeTab = 'settings'} style="margin-top: 0.5rem;">
                Go to Library Settings
              </button>
            </div>
          {/if}
        </div>
      </div>


    {:else if activeTab === 'music-map'}
      <!-- Mock Music Map Panel -->
      <section class="map-panel glass-panel">
        <h2>The Music Map</h2>
        <p class="map-desc">Your CLAP audio embeddings will be projected in 2D space here using UMAP dimensionality reduction.</p>
        
        <div class="map-canvas-mockup">
          <div class="mock-dots">
            {#each Array(50) as _, i}
              <div 
                class="mock-dot" 
                style="
                  left: {20 + (i * 1.5 + Math.sin(i) * 15) % 65}%; 
                  top: {15 + (i * 2 + Math.cos(i) * 12) % 70}%; 
                  background-color: {i % 3 === 0 ? 'var(--color-accent-cyan)' : i % 3 === 1 ? 'var(--color-accent-magenta)' : 'var(--color-primary)'}
                "
              ></div>
            {/each}
          </div>
          <p class="map-overlay-text">Visual Map Skeleton Ready</p>
        </div>
      </section>
    {:else if activeTab === 'settings'}
      <div class="settings-panel-layout">
        <div class="settings-left-col">
          <!-- Left Side: Folder Registration -->
          <div class="glass-panel registration-card">
            <h4>Add Music Library Path</h4>
            <p class="desc">Register folders containing your MP3, WAV, FLAC, M4A, AIFF, OGG, or OPUS libraries to be monitored and indexed by our acoustic intelligence processors.</p>
            
            <div class="form-group">
              <label for="dir-path">Directory Path</label>
              <input 
                id="dir-path"
                type="text" 
                value={path} 
                placeholder="Select a folder to browse..." 
                readonly 
              />
              <button class="btn-secondary picker-btn" onclick={choosePath}>
                <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                  <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/>
                </svg>
                Select Folder
              </button>
            </div>

            <div class="form-group">
              <label for="col-name">Collection Name</label>
              <input 
                id="col-name"
                type="text" 
                bind:value={name} 
                placeholder="e.g., Hi-Res Masters, Chillout Beats" 
              />
            </div>

            {#if errorMessage}
              <div class="alert-box error-alert">
                <span class="alert-icon">⚠️</span>
                <span class="alert-text">{errorMessage}</span>
              </div>
            {/if}

            {#if successMessage}
              <div class="alert-box success-alert">
                <span class="alert-icon">✓</span>
                <span class="alert-text">{successMessage}</span>
              </div>
            {/if}

            <button 
              class="btn-primary submit-btn" 
              onclick={addDirectory} 
              disabled={isAddLoading || !path}
            >
              {#if isAddLoading}
                Registering Folder...
              {:else}
                <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
                  <line x1="12" y1="5" x2="12" y2="19"/>
                  <line x1="5" y1="12" x2="19" y2="12"/>
                </svg>
                Register Library Folder
              {/if}
            </button>
          </div>

          <!-- Collection Stats Card -->
          <div class="stat-card glass-panel">
            <h3>Collection Stats</h3>
            <div style="display: flex; gap: 1.5rem; align-items: center; margin-top: 0.5rem;">
              <div>
                <p class="stat-value" style="font-size: 2.8rem;">{directories.length}</p>
                <p class="stat-label">Folders Monitored</p>
              </div>
              <div style="width: 1px; height: 45px; background: var(--border-color); opacity: 0.6;"></div>
              <div>
                <p class="stat-value" style="font-size: 2.8rem; color: var(--color-accent-cyan);">{trackCount}</p>
                <p class="stat-label">Tracks Indexed</p>
              </div>
            </div>
          </div>
        </div>

        <!-- Right Side: Watched Folders Table -->
        <div class="glass-panel list-card">
          <div style="display: flex; justify-content: space-between; align-items: flex-start; gap: 1rem; width: 100%; border-bottom: 1px solid var(--border-color); padding-bottom: 1rem; margin-bottom: 1rem;">
            <div>
              <h4 style="margin: 0; font-size: 1.1rem; font-weight: 700;">Monitored Music Folders</h4>
              <p class="desc" style="margin: 0.25rem 0 0 0; font-size: 0.82rem;">Active music library folders monitored by Deep Cuts.</p>
            </div>
            
            {#if directories.length > 0}
              <div class="header-scan-action" style="min-width: 200px; display: flex; flex-direction: column; align-items: flex-end; gap: 0.25rem;">
                {#if isScanning}
                  <div class="scanning-status-container" style="padding: 0; gap: 0.4rem; width: 100%;">
                    <div class="scanning-spinner-row" style="justify-content: flex-end; gap: 0.5rem;">
                      <div class="vinyl-spinner-mini active" style="width: 18px; height: 18px;"></div>
                      <div class="scanning-details" style="text-align: right;">
                        <span class="scanning-title" style="font-size: 0.8rem;">Scanning ({Math.round(scanProgress)}%)</span>
                        <span class="scanning-subtitle" style="font-size: 0.65rem;">{scanProcessedCount} / {scanTotalCount}</span>
                      </div>
                    </div>
                    <div class="progress-bar-container" style="height: 4px; margin-top: 0.1rem;">
                      <div class="progress-bar-fill" style="width: {scanProgress}%"></div>
                    </div>
                    <!-- Scrolling ticker basename -->
                    <span style="font-size: 0.68rem; color: var(--color-accent-cyan); font-family: monospace; white-space: nowrap; max-width: 200px; overflow: hidden; text-overflow: ellipsis; text-align: right;" title={scanCurrentFile}>
                      {scanCurrentFile.split(/[/\\]/).pop() || ""}
                    </span>
                  </div>
                {:else}
                  <button 
                    class="btn-primary" 
                    onclick={triggerScan}
                    style="background: linear-gradient(135deg, var(--color-primary), var(--color-accent-cyan)); font-size: 0.82rem; padding: 0.4rem 1rem; border-radius: var(--radius-sm); width: auto;"
                  >
                    <svg xmlns="http://www.w3.org/2000/svg" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" style="margin-right: 0.25rem; vertical-align: middle;">
                      <circle cx="12" cy="12" r="10"/>
                      <polyline points="12 6 12 12 16 14"/>
                    </svg>
                    <span style="vertical-align: middle;">Scan Library</span>
                  </button>
                {/if}
              </div>
            {/if}
          </div>

          {#if directories.length > 0}
            <div class="dir-table-container">
              <table class="dir-table">
                <thead>
                  <tr>
                    <th>Collection</th>
                    <th>Absolute Directory Path</th>
                    <th>Actions</th>
                  </tr>
                </thead>
                <tbody>
                  {#each directories as dir (dir.id)}
                    <tr class="dir-row">
                      <td class="dir-name">
                        <span class="badge badge-cyan">{dir.name}</span>
                      </td>
                      <td class="dir-path" title={dir.path}>
                        <code>{dir.path}</code>
                      </td>
                      <td class="dir-actions">
                        <button
                          class="btn-delete"
                          title="Remove Watched Folder"
                          onclick={() => removeDirectory(dir.id, dir.name)}
                        >
                          <svg xmlns="http://www.w3.org/2000/svg" width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round">
                            <polyline points="3 6 5 6 21 6"/>
                            <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/>
                            <line x1="10" y1="11" x2="10" y2="17"/>
                            <line x1="14" y1="11" x2="14" y2="17"/>
                          </svg>
                        </button>
                      </td>
                    </tr>
                  {/each}
                </tbody>
              </table>
            </div>
          {:else}
            <div class="empty-dirs">
              <div class="empty-icon-box">
                <svg xmlns="http://www.w3.org/2000/svg" width="36" height="36" viewBox="0 0 24 24" fill="none" stroke="var(--text-muted)" stroke-width="1.5">
                  <circle cx="12" cy="12" r="10"/>
                  <path d="M12 8v8M8 12h8"/>
                </svg>
              </div>
              <h5>No Registered Libraries</h5>
              <p>Your library is empty. Select a music directory folder on the left to activate scanning features.</p>
            </div>
          {/if}
        </div>
      </div>
    {/if}
  </main>
</div>

<style>
  .app-layout {
    min-height: 100vh;
    padding: 1.5rem;
    display: flex;
    flex-direction: column;
    gap: 1.5rem;
  }

  /* Navbar Styles */
  .navbar {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 1rem 2rem;
  }

  .brand {
    display: flex;
    align-items: center;
    gap: 0.75rem;
  }

  .logo-circle {
    width: 32px;
    height: 32px;
    border-radius: 50%;
    background-color: var(--text-primary);
    position: relative;
    overflow: hidden;
  }

  .logo-cut {
    position: absolute;
    width: 100%;
    height: 3px;
    background-color: var(--bg-main);
    top: 50%;
    transform: translateY(-50%) rotate(23deg);
  }

  .brand-name {
    font-family: 'Outfit', sans-serif;
    font-size: 1.4rem;
    font-weight: 800;
    letter-spacing: 0.05em;
  }

  .nav-tabs {
    display: flex;
    gap: 0.5rem;
  }

  .nav-tab {
    background: transparent;
    border: none;
    color: var(--text-secondary);
    font-family: 'Inter', sans-serif;
    font-weight: 600;
    font-size: 1rem;
    padding: 0.5rem 1.25rem;
    border-radius: var(--radius-sm);
    cursor: pointer;
    transition: var(--transition-smooth);
  }

  .nav-tab:hover, .nav-tab.active {
    color: var(--text-primary);
    background: rgba(255, 255, 255, 0.05);
  }

  :global(html[data-theme="light"]) .nav-tab:hover, :global(html[data-theme="light"]) .nav-tab.active {
    background: rgba(15, 23, 42, 0.05);
  }

  :global(html[data-theme="accessible"]) .nav-tab.active {
    border: 2px solid #fff;
  }

  .controls {
    display: flex;
    align-items: center;
    gap: 1.5rem;
  }



  /* Workspace Styles */
  .workspace {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 1.5rem;
  }

  .hero-panel {
    display: grid;
    grid-template-columns: 1.2fr 0.8fr;
    gap: 2rem;
    padding: 3rem;
  }

  .hero-title {
    font-size: 3rem;
    font-weight: 800;
    line-height: 1.1;
    margin-top: 1rem;
    margin-bottom: 1.5rem;
    background: linear-gradient(135deg, var(--text-primary), var(--color-primary));
    -webkit-background-clip: text;
    -webkit-text-fill-color: transparent;
  }

  :global(html[data-theme="accessible"]) .hero-title {
    background: none;
    -webkit-background-clip: unset;
    -webkit-text-fill-color: unset;
    color: #fff;
  }

  .hero-description {
    font-size: 1.15rem;
    color: var(--text-secondary);
    line-height: 1.6;
    margin-bottom: 2rem;
  }

  .hero-buttons {
    display: flex;
    gap: 1rem;
  }

  /* Vinyl Display Styling */
  .vinyl-display {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 2rem;
    height: 100%;
  }

  .vinyl-image {
    width: 180px;
    height: 180px;
    object-fit: contain;
    filter: drop-shadow(0 10px 25px rgba(0, 0, 0, 0.45));
    transition: var(--transition-smooth);
  }

  .vinyl-image:hover {
    transform: rotate(6deg) scale(1.04);
  }

  :global(html[data-theme="accessible"]) .vinyl-image {
    filter: none;
    border: 2px solid #fff;
    border-radius: var(--radius-sm);
  }

  .waveform-animation {
    display: flex;
    align-items: flex-end;
    gap: 4px;
    height: 60px;
  }

  .wave-bar {
    width: 6px;
    height: var(--height);
    background-color: var(--color-accent-cyan);
    border-radius: 3px;
    animation: wave-bounce 1.5s infinite ease-in-out alternate;
    animation-delay: var(--delay);
  }

  :global(html[data-theme="accessible"]) .wave-bar {
    background-color: #fff;
    border: 1px solid #fff;
    animation: none;
  }

  @keyframes wave-bounce {
    0% { transform: scaleY(0.4); }
    100% { transform: scaleY(1); }
  }

  /* Grid Layout Styles */
  .dashboard-grid {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 1.5rem;
  }

  .stat-card {
    padding: 2rem;
    display: flex;
    flex-direction: column;
    justify-content: center;
  }

  .stat-card h3 {
    font-size: 1.1rem;
    color: var(--text-secondary);
    margin-bottom: 1rem;
  }

  .stat-value {
    font-size: 3.5rem;
    font-family: 'Outfit', sans-serif;
    font-weight: 800;
    color: var(--color-accent-cyan);
    line-height: 1;
    margin-bottom: 0.5rem;
  }

  :global(html[data-theme="accessible"]) .stat-value {
    color: #fff;
  }

  .stat-label {
    font-size: 0.9rem;
    color: var(--text-muted);
    font-weight: 500;
  }

  .model-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 0.5rem 0;
    border-bottom: 1px solid var(--border-color);
  }

  .model-row:last-child {
    border-bottom: none;
  }

  .db-path {
    font-family: monospace;
    font-size: 0.85rem;
    word-break: break-all;
    color: var(--color-accent-yellow);
    background: rgba(0,0,0,0.2);
    padding: 0.5rem;
    border-radius: var(--radius-sm);
    margin-bottom: 1rem;
  }

  :global(html[data-theme="accessible"]) .db-path {
    border: 1px solid #fff;
    background: #000;
  }

  .db-desc {
    font-size: 0.85rem;
    color: var(--text-muted);
    line-height: 1.4;
  }

  /* Map Panel Styles */
  .map-panel, .settings-panel {
    padding: 3rem;
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }

  .map-desc, .settings-desc {
    font-size: 1rem;
    color: var(--text-secondary);
    margin-bottom: 1.5rem;
  }

  .map-canvas-mockup {
    height: 400px;
    background-color: rgba(0,0,0,0.3);
    border-radius: var(--radius-lg);
    border: 2px dashed var(--border-color);
    position: relative;
    overflow: hidden;
  }

  :global(html[data-theme="accessible"]) .map-canvas-mockup {
    border: 2px dashed #fff;
    background-color: #000;
  }

  .mock-dots {
    width: 100%;
    height: 100%;
    position: relative;
  }

  .mock-dot {
    position: absolute;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    opacity: 0.6;
    transition: var(--transition-fast);
  }

  :global(html[data-theme="accessible"]) .mock-dot {
    opacity: 1;
    border: 1px solid #fff;
    background-color: #fff !important;
  }

  .map-overlay-text {
    position: absolute;
    left: 50%;
    top: 50%;
    transform: translate(-50%, -50%);
    font-family: 'Outfit', sans-serif;
    font-size: 1.5rem;
    font-weight: 700;
    color: var(--text-secondary);
    background: var(--bg-main);
    padding: 0.5rem 1.5rem;
    border: 2px solid var(--border-color);
    border-radius: var(--radius-sm);
  }

  :global(html[data-theme="accessible"]) .map-overlay-text {
    border: 2px solid #fff;
    background: #000;
    color: #fff;
  }

  /* Settings Form Styles */
  .settings-panel-layout {
    display: grid;
    grid-template-columns: 420px 1fr;
    gap: 1.5rem;
    align-items: start;
    width: 100%;
  }

  .settings-left-col {
    display: flex;
    flex-direction: column;
    gap: 1.5rem;
    width: 100%;
  }

  .registration-card, .list-card {
    padding: 2rem;
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }

  .desc {
    font-size: 0.88rem;
    color: var(--text-muted);
    line-height: 1.5;
  }

  .form-group {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    width: 100%;
  }

  .form-group label {
    font-family: 'Inter', sans-serif;
    font-size: 0.8rem;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--text-secondary);
  }



  input[type="text"] {
    background: rgba(0, 0, 0, 0.2);
    border: 2px solid var(--border-color);
    padding: 0.75rem 1rem;
    border-radius: var(--radius-md);
    color: var(--text-primary);
    font-family: 'Inter', sans-serif;
    font-size: 0.9rem;
    outline: none;
    transition: var(--transition-fast);
  }

  input[type="text"]:focus {
    border-color: var(--color-primary);
  }

  :global(html[data-theme="accessible"]) input[type="text"] {
    border-radius: 0;
    border: 2px solid #fff;
    background: #000;
    color: #fff;
  }

  .picker-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 0.5rem;
    white-space: nowrap;
    width: 100%;
  }

  .submit-btn {
    width: 100%;
    justify-content: center;
    margin-top: 1rem;
  }

  .alert-box {
    display: flex;
    gap: 0.5rem;
    padding: 0.75rem 1rem;
    border-radius: var(--radius-md);
    font-size: 0.85rem;
    line-height: 1.4;
  }

  .error-alert {
    background: rgba(255, 0, 127, 0.1);
    border: 1.5px solid rgba(255, 0, 127, 0.3);
    color: var(--color-accent-magenta);
  }

  .success-alert {
    background: rgba(0, 242, 254, 0.1);
    border: 1.5px solid rgba(0, 242, 254, 0.3);
    color: var(--color-accent-cyan);
  }

  :global(html[data-theme="accessible"]) .error-alert, :global(html[data-theme="accessible"]) .success-alert {
    border: 2px solid #fff;
    background: #000;
    color: #fff;
    border-radius: 0;
  }

  /* Monitored Folders List */
  .dir-table-container {
    overflow-x: auto;
    width: 100%;
    margin-top: 1rem;
  }

  .dir-table {
    width: 100%;
    border-collapse: collapse;
    text-align: left;
  }

  .dir-table th {
    font-size: 0.8rem;
    text-transform: uppercase;
    font-weight: 700;
    color: var(--text-muted);
    padding: 0.75rem 1rem;
    border-bottom: 2px solid var(--border-color);
  }

  .dir-row {
    border-bottom: 1px solid var(--border-color);
    transition: var(--transition-fast);
  }

  .dir-row:hover {
    background: rgba(255, 255, 255, 0.02);
  }

  :global(html[data-theme="accessible"]) .dir-row:hover {
    background: #121212;
  }

  .dir-table td {
    padding: 1rem;
    font-size: 0.9rem;
    vertical-align: middle;
  }

  .dir-name {
    font-weight: 600;
  }

  .dir-path {
    max-width: 380px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .dir-path code {
    color: var(--text-secondary);
    font-family: monospace;
    font-size: 0.85rem;
  }

  .btn-delete {
    background: transparent;
    border: none;
    color: var(--text-muted);
    cursor: pointer;
    padding: 0.5rem;
    border-radius: var(--radius-sm);
    transition: var(--transition-fast);
    display: inline-flex;
    align-items: center;
    justify-content: center;
  }

  .btn-delete:hover {
    color: var(--color-accent-magenta);
    background: rgba(255, 0, 127, 0.1);
  }

  :global(html[data-theme="accessible"]) .btn-delete:hover {
    background: #fff;
    color: #000;
    border-radius: 0;
  }

  .empty-dirs {
    text-align: center;
    padding: 3rem;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 1rem;
  }

  .empty-icon-box {
    opacity: 0.5;
  }

  /* Library Utilities & Active Scanning styles */
  .scanning-status-container {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
    padding: 0.5rem 0;
    width: 100%;
  }

  .scanning-spinner-row {
    display: flex;
    align-items: center;
    gap: 0.75rem;
  }

  .vinyl-spinner-mini {
    width: 24px;
    height: 24px;
    border-radius: 50%;
    background-color: var(--text-primary);
    position: relative;
  }

  .vinyl-spinner-mini::after {
    content: '';
    position: absolute;
    width: 100%;
    height: 2px;
    background-color: var(--bg-main);
    top: 50%;
    transform: translateY(-50%) rotate(23deg);
  }

  .vinyl-spinner-mini.active {
    animation: rotate-spinner 2s linear infinite;
  }

  @keyframes rotate-spinner {
    from { transform: rotate(0deg); }
    to { transform: rotate(360deg); }
  }

  .scanning-details {
    display: flex;
    flex-direction: column;
  }

  .scanning-title {
    font-size: 0.9rem;
    font-weight: 700;
    color: var(--text-primary);
  }

  .scanning-subtitle {
    font-size: 0.75rem;
    color: var(--text-muted);
  }

  .progress-bar-container {
    width: 100%;
    height: 6px;
    background: rgba(255, 255, 255, 0.05);
    border-radius: 3px;
    overflow: hidden;
    position: relative;
    border: 1px solid var(--border-color);
  }

  .progress-bar-fill {
    height: 100%;
    background: linear-gradient(90deg, var(--color-primary), var(--color-accent-cyan));
    border-radius: 3px;
    transition: width 0.3s ease-out;
  }

  .current-file-ticker {
    font-size: 0.78rem;
    background: rgba(0, 0, 0, 0.2);
    padding: 0.5rem;
    border-radius: var(--radius-sm);
    border: 1px solid var(--border-color);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .current-file-ticker code {
    color: var(--color-accent-cyan);
    font-family: monospace;
  }

  :global(html[data-theme="accessible"]) .vinyl-spinner-mini {
    border: 1px solid #fff;
    background: #000;
    animation: none;
    border-radius: 0;
  }
  :global(html[data-theme="accessible"]) .progress-bar-container {
    border: 1px solid #fff;
    background: #000;
    border-radius: 0;
  }
  :global(html[data-theme="accessible"]) .progress-bar-fill {
    background: #fff;
    border-radius: 0;
  }
  :global(html[data-theme="accessible"]) .current-file-ticker {
    border: 1px solid #fff;
    background: #000;
    border-radius: 0;
  }

  /* --- Main Dashboard Draggable Split-Pane Layout --- */
  .dashboard-split-layout {
    display: flex;
    flex-direction: column;
    flex: 1;
    height: 100%;
    overflow: hidden;
    gap: 0;
  }

  .top-pane-resizable {
    width: 100%;
    overflow: hidden;
    display: flex;
    flex-direction: column;
    position: relative;
    border-bottom-left-radius: 0;
    border-bottom-right-radius: 0;
  }

  .hero-panel-content {
    display: grid;
    grid-template-columns: 1.2fr 0.8fr;
    gap: 2rem;
    padding: 2rem 3rem;
    align-items: center;
    height: 100%;
    width: 100%;
  }

  .hero-left {
    display: flex;
    flex-direction: column;
    justify-content: center;
  }

  .hero-right {
    display: flex;
    align-items: center;
    justify-content: center;
  }

  /* --- Resizable Splitter Divider --- */
  .split-pane-resizer {
    height: 8px;
    background: rgba(255, 255, 255, 0.02);
    border-top: 1px solid var(--border-color);
    border-bottom: 1px solid var(--border-color);
    cursor: row-resize;
    position: relative;
    transition: background 0.15s ease, border-color 0.15s ease;
    display: flex;
    align-items: center;
    justify-content: center;
    user-select: none;
    z-index: 10;
  }

  .split-pane-resizer:hover, .split-pane-resizer.active {
    background: rgba(138, 43, 226, 0.1);
    border-top-color: rgba(138, 43, 226, 0.3);
    border-bottom-color: rgba(138, 43, 226, 0.3);
  }

  :global(html[data-theme="light"]) .split-pane-resizer:hover, :global(html[data-theme="light"]) .split-pane-resizer.active {
    background: rgba(99, 102, 241, 0.08);
    border-top-color: rgba(99, 102, 241, 0.2);
    border-bottom-color: rgba(99, 102, 241, 0.2);
  }

  :global(html[data-theme="accessible"]) .split-pane-resizer {
    height: 10px;
    background: #000;
    border-top: 2px solid #fff;
    border-bottom: 2px solid #fff;
  }

  :global(html[data-theme="accessible"]) .split-pane-resizer:hover, :global(html[data-theme="accessible"]) .split-pane-resizer.active {
    background: #fff;
  }

  .resizer-knob {
    width: 48px;
    height: 4px;
    background: var(--text-muted);
    border-radius: 2px;
    opacity: 0.5;
    transition: opacity 0.15s ease, background 0.15s ease;
  }

  .split-pane-resizer:hover .resizer-knob, .split-pane-resizer.active .resizer-knob {
    opacity: 1;
    background: var(--color-primary);
  }

  :global(html[data-theme="accessible"]) .resizer-knob {
    background: #fff;
    opacity: 1;
    height: 6px;
    border-radius: 0;
  }

  .bottom-pane-scroller {
    flex: 1;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    border-top-left-radius: 0;
    border-top-right-radius: 0;
    border-top: none;
  }

  /* --- WaveSurfer Audio Player Pane --- */
  .audio-player-pane {
    display: flex;
    flex-direction: column;
    gap: 1rem;
    padding: 1.25rem 2rem;
    height: 100%;
    width: 100%;
    overflow: hidden;
  }

  .player-left-col {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 0.85rem;
    border-right: 1px solid var(--border-color);
    padding-right: 2rem;
    overflow: hidden;
  }

  .player-main-col {
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    gap: 0.65rem;
    overflow: hidden;
    height: 100%;
  }

  .vinyl-spinner-large {
    width: 100px;
    height: 100px;
    border-radius: 50%;
    background: #030303;
    border: 4px solid #141424;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.45);
    display: flex;
    align-items: center;
    justify-content: center;
    position: relative;
    transition: transform 0.3s ease;
  }

  .vinyl-spinner-large::before {
    content: '';
    position: absolute;
    inset: 10px;
    border: 1px dashed rgba(255, 255, 255, 0.12);
    border-radius: 50%;
  }

  .vinyl-spinner-large::after {
    content: '';
    position: absolute;
    width: 16px;
    height: 16px;
    background: var(--bg-main);
    border-radius: 50%;
    border: 2px solid #000;
    z-index: 2;
  }

  .vinyl-record-img {
    width: 46px;
    height: 46px;
    object-fit: contain;
    z-index: 1;
    opacity: 0.85;
  }

  .vinyl-spinner-large.spinning {
    animation: spin-vinyl 4s linear infinite;
  }

  @keyframes spin-vinyl {
    from { transform: rotate(0deg); }
    to { transform: rotate(360deg); }
  }

  :global(html[data-theme="accessible"]) .vinyl-spinner-large {
    border: 2px solid #fff;
    background: #000;
    border-radius: 0;
    box-shadow: none;
    animation: none !important;
  }

  :global(html[data-theme="accessible"]) .vinyl-spinner-large::before,
  :global(html[data-theme="accessible"]) .vinyl-spinner-large::after {
    display: none;
  }

  :global(html[data-theme="accessible"]) .vinyl-record-img {
    width: 70px;
    height: 70px;
    border-radius: 0;
  }

  .track-details-block {
    width: 100%;
    text-align: center;
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
  }

  .track-title-row {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 0.5rem;
    margin-bottom: 0.1rem;
  }

  .track-title-row h4 {
    font-size: 1.05rem;
    font-weight: 700;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 170px;
    color: var(--text-primary);
  }

  .track-credits {
    font-size: 0.85rem;
    color: var(--text-secondary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 220px;
  }

  .track-credits .artist {
    font-weight: 600;
  }

  .track-credits .sep {
    margin: 0 0.3rem;
    color: var(--text-muted);
  }

  .track-credits .album {
    color: var(--text-muted);
  }

  .track-tech-specs {
    font-family: monospace;
    font-size: 0.74rem;
    color: var(--color-accent-cyan);
    letter-spacing: -0.01em;
  }

  :global(html[data-theme="light"]) .track-tech-specs {
    color: var(--color-primary);
  }

  :global(html[data-theme="accessible"]) .track-tech-specs {
    color: #fff;
    font-size: 0.82rem;
  }

  /* --- Waveform & Spectrogram Containers --- */
  .waveform-outer {
    background: rgba(0, 0, 0, 0.18);
    border: 1px solid var(--border-color);
    border-radius: var(--radius-md);
    padding: 0.6rem 0.85rem; /* Increased padding */
    flex: 1;
    display: flex;
    align-items: center;
    min-height: 90px; /* Accommodates 75px canvas with padding nicely */
    overflow: hidden;
  }

  .waveform-canvas-wrap {
    width: 100%;
    height: 75px; /* Aligned with WaveSurfer canvas height */
    cursor: pointer;
  }

  .spectrogram-outer {
    background: rgba(0, 0, 0, 0.18);
    border: 1px solid var(--border-color);
    border-radius: var(--radius-md);
    padding: 0.6rem 0.85rem; /* Increased padding to prevent overflow */
    flex: 1;
    display: flex;
    align-items: center;
    min-height: 90px; /* Accommodates 75px canvas with padding nicely */
    overflow: hidden;
  }

  .spectrogram-canvas-wrap {
    width: 100%;
    height: 75px; /* Aligned with WaveSurfer canvas height to avoid cropping */
    overflow: hidden;
  }

  /* --- Multi-Column Metadata Expanded Grid --- */
  .player-upper-row {
    display: grid;
    grid-template-columns: 260px 1fr;
    gap: 2rem;
    width: 100%;
    align-items: center;
  }

  .player-details-row {
    margin-top: 0.85rem;
    padding: 1.25rem;
    background: rgba(0, 0, 0, 0.22);
    border: 1px solid var(--border-color);
    border-radius: var(--radius-md);
    overflow-y: auto;
    max-height: 240px; /* Scrollable if height is constrained */
  }

  :global(html[data-theme="light"]) .player-details-row {
    background: #f1f5f9; /* Premium light Slate-100 gray background */
    border-color: rgba(15, 23, 42, 0.08);
  }

  :global(html[data-theme="accessible"]) .player-details-row {
    background: #000;
    border: 2px solid #fff;
    border-radius: 0;
  }

  .metadata-grid {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 1.25rem;
  }

  .metadata-col {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
  }

  .metadata-card {
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
  }

  .metadata-label {
    font-size: 0.7rem;
    font-weight: 700;
    text-transform: uppercase;
    color: var(--text-muted);
    letter-spacing: 0.05em;
  }

  :global(html[data-theme="accessible"]) .metadata-label {
    color: #fff;
  }

  .metadata-value {
    font-size: 0.85rem;
    font-weight: 600;
    color: var(--text-primary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .metadata-value code {
    font-family: monospace;
    font-size: 0.78rem;
    color: var(--color-accent-cyan);
    background: rgba(0, 242, 254, 0.05);
    border: 1px solid rgba(0, 242, 254, 0.15);
    padding: 0.05rem 0.25rem;
    border-radius: 3px;
  }

  :global(html[data-theme="light"]) .metadata-value code {
    color: var(--color-primary);
    background: rgba(99, 102, 241, 0.05);
    border: 1px solid rgba(99, 102, 241, 0.15);
  }

  :global(html[data-theme="accessible"]) .metadata-value code {
    color: #fff;
    background: #000;
    border: 1px solid #fff;
    border-radius: 0;
  }

  .metadata-value.path-value {
    font-size: 0.76rem;
    font-family: monospace;
    word-break: break-all;
    white-space: normal;
  }

  .audio-player-pane.expanded {
    display: flex;
    flex-direction: column;
    gap: 0.85rem;
    padding: 1.25rem 2rem;
    height: 100%;
    width: 100%;
    overflow-y: auto;
  }

  :global(html[data-theme="light"]) .waveform-outer, :global(html[data-theme="light"]) .spectrogram-outer {
    background: #f1f5f9; /* Premium light Slate-100 gray background */
    border-color: rgba(15, 23, 42, 0.08);
  }

  :global(html[data-theme="accessible"]) .waveform-outer, :global(html[data-theme="accessible"]) .spectrogram-outer {
    background: #000;
    border: 2px solid #fff;
    border-radius: 0;
  }

  /* --- Playback Controls Row --- */
  .playback-controls-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    background: rgba(255, 255, 255, 0.02);
    border: 1px solid var(--border-color);
    border-radius: var(--radius-md);
    padding: 0.4rem 0.85rem;
  }

  :global(html[data-theme="light"]) .playback-controls-row {
    background: #f8fafc; /* Premium light Slate-50 background */
    border-color: rgba(15, 23, 42, 0.08);
  }

  :global(html[data-theme="accessible"]) .playback-controls-row {
    background: #000;
    border: 2px solid #fff;
    border-radius: 0;
  }

  .player-btn {
    background: transparent;
    border: none;
    color: var(--text-secondary);
    cursor: pointer;
    width: 30px;
    height: 30px;
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: var(--transition-fast);
  }

  .player-btn:hover {
    color: var(--text-primary);
    background: rgba(255, 255, 255, 0.05);
  }

  :global(html[data-theme="accessible"]) .player-btn {
    border: 2px solid #fff;
    border-radius: 0;
    color: #fff;
  }

  :global(html[data-theme="accessible"]) .player-btn:hover {
    background: #fff;
    color: #000;
  }

  .btn-play-pause {
    background: var(--color-primary);
    color: #fff;
    border: none;
    width: 38px;
    height: 38px;
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    box-shadow: 0 4px 10px rgba(138, 43, 226, 0.3);
    transition: var(--transition-smooth);
  }

  .btn-play-pause:hover {
    transform: scale(1.08);
    box-shadow: 0 6px 14px rgba(138, 43, 226, 0.5);
  }

  :global(html[data-theme="light"]) .btn-play-pause {
    background: var(--color-primary);
    box-shadow: 0 4px 10px rgba(99, 102, 241, 0.3);
  }

  :global(html[data-theme="accessible"]) .btn-play-pause {
    background: #000;
    border: 3px solid #fff;
    border-radius: 0;
    box-shadow: none;
    color: #fff;
  }

  :global(html[data-theme="accessible"]) .btn-play-pause:hover {
    transform: none;
    background: #fff;
    color: #000;
  }

  .time-readout {
    font-family: monospace;
    font-size: 0.82rem;
    display: flex;
    align-items: center;
    gap: 0.3rem;
    color: var(--text-secondary);
  }

  .time-readout .current-time {
    color: var(--color-accent-cyan);
    font-weight: 600;
  }

  :global(html[data-theme="light"]) .time-readout .current-time {
    color: var(--color-primary);
  }

  :global(html[data-theme="accessible"]) .time-readout .current-time {
    color: #fff;
  }

  .time-readout .divider {
    color: var(--text-muted);
  }

  .time-readout .total-duration {
    color: var(--text-muted);
  }

  /* --- Tracks Library Toolbar --- */
  .tracks-toolbar {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 0.75rem 1.25rem;
    border-bottom: 1px solid var(--border-color);
    background: rgba(0, 0, 0, 0.08);
    gap: 1rem;
  }

  :global(html[data-theme="light"]) .tracks-toolbar {
    background: #f8fafc; /* Premium light Slate-50 background */
    border-bottom-color: rgba(15, 23, 42, 0.08);
  }

  :global(html[data-theme="accessible"]) .tracks-toolbar {
    background: #000;
    border-bottom: 2px solid #fff;
  }

  :global(html[data-theme="light"]) .search-input,
  :global(html[data-theme="light"]) .filter-select {
    background: #ffffff !important; /* Premium pure white inputs */
    border-color: rgba(15, 23, 42, 0.12) !important;
    color: var(--text-primary) !important;
  }

  .search-box-wrap {
    position: relative;
    flex: 1;
    max-width: 440px;
  }

  .search-icon {
    position: absolute;
    left: 0.85rem;
    top: 50%;
    transform: translateY(-50%);
    pointer-events: none;
  }

  .search-input {
    width: 100%;
    background: rgba(0, 0, 0, 0.25);
    border: 2px solid var(--border-color);
    padding: 0.5rem 1rem 0.5rem 2.8rem !important; /* Added left padding to prevent overlapping with magnifying icon */
    border-radius: var(--radius-md);
    color: var(--text-primary);
    font-family: 'Inter', sans-serif;
    font-size: 0.85rem;
    outline: none;
    transition: var(--transition-fast);
  }

  .search-input:focus {
    border-color: var(--color-primary);
  }

  :global(html[data-theme="accessible"]) .search-input {
    border-radius: 0;
    border: 2px solid #fff;
    background: #000;
    padding-left: 2.8rem !important;
  }

  .filter-select-wrap {
    position: relative;
  }

  .filter-select {
    background: rgba(0, 0, 0, 0.25);
    color: var(--text-primary);
    border: 2px solid var(--border-color);
    border-radius: var(--radius-md);
    padding: 0.5rem 2.2rem 0.5rem 1rem;
    font-family: 'Inter', sans-serif;
    font-size: 0.85rem;
    cursor: pointer;
    outline: none;
    appearance: none;
    background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='12' height='12' viewBox='0 0 24 24' fill='none' stroke='%239ca3af' stroke-width='3' stroke-linecap='round' stroke-linejoin='round'%3E%3Cpolyline points='6 9 12 15 18 9'/%3E%3C/svg%3E");
    background-repeat: no-repeat;
    background-position: right 0.85rem center;
    transition: var(--transition-fast);
    min-width: 150px;
  }

  .filter-select:focus {
    border-color: var(--color-primary);
  }

  :global(html[data-theme="accessible"]) .filter-select {
    border-radius: 0;
    border: 2px solid #fff;
    background: #000;
    background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='12' height='12' viewBox='0 0 24 24' fill='none' stroke='%23ffffff' stroke-width='3' stroke-linecap='round' stroke-linejoin='round'%3E%3Cpolyline points='6 9 12 15 18 9'/%3E%3C/svg%3E");
  }

  .library-count-badge {
    font-size: 0.8rem;
    color: var(--text-muted);
    background: rgba(255, 255, 255, 0.03);
    padding: 0.35rem 0.75rem;
    border-radius: var(--radius-sm);
    border: 1px solid var(--border-color);
  }

  :global(html[data-theme="accessible"]) .library-count-badge {
    border: 1px solid #fff;
    background: #000;
    color: #fff;
    border-radius: 0;
  }

  /* --- Tracks Data Grid Table --- */
  .tracks-table-wrap {
    flex: 1;
    overflow-y: auto;
    width: 100%;
  }

  .tracks-table {
    width: 100%;
    border-collapse: collapse;
    text-align: left;
  }

  .tracks-table th {
    font-size: 0.78rem;
    text-transform: uppercase;
    font-weight: 700;
    color: var(--text-muted);
    padding: 0.75rem 1rem;
    border-bottom: 2px solid var(--border-color);
    position: sticky;
    top: 0;
    background: var(--bg-card);
    backdrop-filter: var(--backdrop-blur);
    z-index: 2;
  }

  :global(html[data-theme="accessible"]) .tracks-table th {
    background: #000;
    border-bottom: 2px solid #fff;
  }

  .track-row {
    border-bottom: 1px solid var(--border-color);
    cursor: pointer;
    transition: var(--transition-fast);
  }

  .track-row:hover {
    background: rgba(255, 255, 255, 0.02);
  }

  :global(html[data-theme="light"]) .track-row:hover {
    background: rgba(15, 23, 42, 0.015);
  }

  :global(html[data-theme="accessible"]) .track-row:hover {
    background: #121212;
  }

  .track-row.active-pulse {
    background: rgba(138, 43, 226, 0.07);
    border-left: 3px solid var(--color-primary);
  }

  :global(html[data-theme="light"]) .track-row.active-pulse {
    background: rgba(99, 102, 241, 0.05);
    border-left: 3px solid var(--color-primary);
  }

  :global(html[data-theme="accessible"]) .track-row.active-pulse {
    background: #000;
    border-left: 5px solid #fff;
    border-top: 1.5px solid #fff;
    border-bottom: 1.5px solid #fff;
  }

  .track-row td {
    padding: 0.75rem 1rem;
    font-size: 0.85rem;
    vertical-align: middle;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .track-title-cell {
    max-width: 260px;
    font-weight: 500;
  }

  .track-primary-title {
    color: var(--text-primary);
    font-weight: 600;
  }

  .file-tag {
    font-size: 0.62rem;
    font-family: monospace;
    background: rgba(255, 255, 255, 0.05);
    color: var(--text-muted);
    padding: 0.05rem 0.25rem;
    border-radius: 3px;
    margin-left: 0.35rem;
    border: 1px solid var(--border-color);
  }

  :global(html[data-theme="accessible"]) .file-tag {
    border: 1px solid #fff;
    background: #000;
    color: #fff;
  }

  .track-text-cell {
    max-width: 170px;
    color: var(--text-secondary);
  }

  .format-mini-badge {
    font-size: 0.68rem;
    font-family: monospace;
    background: rgba(0, 242, 254, 0.08);
    color: var(--color-accent-cyan);
    padding: 0.12rem 0.3rem;
    border-radius: 4px;
    font-weight: 600;
    border: 1px solid rgba(0, 242, 254, 0.15);
  }

  :global(html[data-theme="light"]) .format-mini-badge {
    background: rgba(6, 182, 212, 0.06);
    color: var(--color-accent-cyan);
  }

  :global(html[data-theme="accessible"]) .format-mini-badge {
    border: 1px solid #fff;
    background: #000;
    color: #fff;
    border-radius: 0;
  }

  .bitrate-label {
    font-size: 0.7rem;
    font-family: monospace;
    color: var(--text-muted);
    margin-left: 0.3rem;
  }

  /* --- Mini Animated Visualizer Playing Bars --- */
  .playing-bars-mini {
    display: flex;
    align-items: flex-end;
    justify-content: center;
    gap: 2px;
    height: 11px;
    width: 13px;
    margin: 0 auto;
  }

  .playing-bars-mini .bar {
    width: 2px;
    height: 100%;
    background-color: var(--color-accent-cyan);
    border-radius: 1px;
    animation: mini-bounce 0.8s ease-in-out infinite alternate;
    transform-origin: bottom;
  }

  :global(html[data-theme="light"]) .playing-bars-mini .bar {
    background-color: var(--color-primary);
  }

  :global(html[data-theme="accessible"]) .playing-bars-mini .bar {
    background-color: #fff;
    animation: none;
  }

  .playing-bars-mini .bar:nth-child(1) {
    animation-delay: -0.6s;
  }

  .playing-bars-mini .bar:nth-child(2) {
    animation-delay: -0.2s;
  }

  .playing-bars-mini .bar:nth-child(3) {
    animation-delay: -0.4s;
  }

  @keyframes mini-bounce {
    0% { transform: scaleY(0.25); }
    100% { transform: scaleY(1); }
  }

  /* --- Empty States --- */
  .empty-search-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 0.65rem;
    padding: 3.5rem 2rem;
    text-align: center;
    color: var(--text-muted);
  }

  .empty-search-state h5 {
    font-size: 1.05rem;
    font-weight: 600;
    color: var(--text-secondary);
    margin: 0;
  }

  .empty-search-state p {
    font-size: 0.85rem;
    max-width: 300px;
    margin: 0;
  }

  .empty-tracks-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 0.65rem;
    padding: 3.5rem 2rem;
    text-align: center;
    color: var(--text-muted);
    height: 100%;
  }

  .empty-tracks-state h5 {
    font-size: 1.05rem;
    font-weight: 600;
    color: var(--text-secondary);
    margin: 0;
  }

  .empty-tracks-state p {
    font-size: 0.85rem;
    max-width: 320px;
    margin: 0;
  }

  .vinyl-display-empty {
    width: 75px;
    height: 75px;
    border-radius: 50%;
    background: rgba(255, 255, 255, 0.02);
    border: 2px dashed var(--border-color);
    display: flex;
    align-items: center;
    justify-content: center;
  }

  :global(html[data-theme="accessible"]) .vinyl-display-empty {
    border: 2px dashed #fff;
    background: #000;
    border-radius: 0;
  }

  .vinyl-image-empty {
    width: 40px;
    height: 40px;
    opacity: 0.25;
    object-fit: contain;
  }

  :global(html[data-theme="accessible"]) .vinyl-image-empty {
    opacity: 1;
  }
</style>
