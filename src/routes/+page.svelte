<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";

  interface WatchedDirectory {
    id: number;
    name: string;
    path: string;
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

  // Check Tauri connectivity and restore theme
  onMount(async () => {
    // Stage 1: Load instantly from localStorage for seamless boot
    const saved = localStorage.getItem("deep-cuts-theme") || "system";
    await setTheme(saved, false);

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
    } catch (e) {
      console.warn("Tauri shell connection offline (running in browser context) or database loading.");
    }
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
      <!-- Hero Welcome Panel -->
      <section class="hero-panel glass-panel">
        <div class="hero-left">
          <h1 class="hero-title">Your Audio, Structured.</h1>
          <p class="hero-description">
            Deep Cuts analyzes local audio collections, performing integrated EBU R128 loudness checks, Camelot key mapping, spectral onset BPM extraction, and offline machine-learning indexing.
          </p>

          <div class="hero-buttons">
            <button class="btn-primary" onclick={() => activeTab = 'settings'}>
              Configure watched folders
            </button>
            <button class="btn-secondary" onclick={() => alert("Ready for implementation!")}>
              Learn more
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
            />
            <!-- Mock Audio Waveform bars -->
            <div class="waveform-animation">
              {#each Array(20) as _, i}
                <div class="wave-bar" style="--height: {15 + Math.sin(i * 0.5) * 25 + Math.random() * 10}px; --delay: {i * 0.08}s"></div>
              {/each}
            </div>
          </div>
        </div>
      </section>


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
            <p class="stat-value">{directories.length}</p>
            <p class="stat-label">Folders Monitored</p>
          </div>
        </div>

        <!-- Right Side: Watched Folders Table -->
        <div class="glass-panel list-card">
          <h4>Monitored Music Folders</h4>
          <p class="desc">Active music library folders monitored by Deep Cuts.</p>

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

  html[data-theme="light"] .nav-tab:hover, html[data-theme="light"] .nav-tab.active {
    background: rgba(15, 23, 42, 0.05);
  }

  html[data-theme="accessible"] .nav-tab.active {
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

  html[data-theme="accessible"] .hero-title {
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

  html[data-theme="accessible"] .vinyl-image {
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

  html[data-theme="accessible"] .wave-bar {
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

  html[data-theme="accessible"] .stat-value {
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

  html[data-theme="accessible"] .db-path {
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

  html[data-theme="accessible"] .map-canvas-mockup {
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

  html[data-theme="accessible"] .mock-dot {
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

  html[data-theme="accessible"] .map-overlay-text {
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

  html[data-theme="accessible"] input[type="text"] {
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

  html[data-theme="accessible"] .error-alert, html[data-theme="accessible"] .success-alert {
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

  html[data-theme="accessible"] .dir-row:hover {
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

  html[data-theme="accessible"] .btn-delete:hover {
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
</style>
