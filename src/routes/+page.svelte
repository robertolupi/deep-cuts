<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";

  // State managers using Svelte 5 runes
  let currentTheme = $state("system");
  let resolvedTheme = $state("dark");
  let tauriConnected = $state(false);
  let activeTab = $state("dashboard");

  // Check Tauri connectivity and query version
  onMount(async () => {
    // Restore theme from localStorage
    const saved = localStorage.getItem("deep-cuts-theme") || "system";
    setTheme(saved);

    try {
      // Basic ping test to confirm Tauri shell connection
      tauriConnected = true;
    } catch (e) {
      console.warn("Tauri shell connection offline (likely running in standard browser context).");
    }
  });

  // Apply theme dynamically to HTML element
  function setTheme(theme: string) {
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

      <!-- Grid Cards -->
      <section class="dashboard-grid">
        <div class="stat-card glass-panel">
          <h3>Collection Stats</h3>
          <p class="stat-value">0</p>
          <p class="stat-label">Tracks Analyzed</p>
        </div>

        <div class="stat-card glass-panel">
          <h3>Local Storage</h3>
          <p class="db-path">~/Library/Application Support/com.rlupi.deep-cuts/</p>
          <p class="db-desc">Your database and logs reside locally in standard macOS sandbox folders.</p>
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
      <!-- Mock Settings Panel -->
      <section class="settings-panel glass-panel">
        <h2>Settings</h2>
        <p class="settings-desc">Manage collection directories and processing parameters.</p>

        <div class="settings-form">
          <div class="form-group">
            <label for="watched-folders">Watched Directories</label>
            <div class="folder-list">
              <div class="empty-folders">No watched directories configured yet.</div>
            </div>
            <button class="btn-primary" onclick={() => alert("Directory pickers will load via RFD in the next phase!")}>
              Add Directory
            </button>
          </div>
          
          <div class="form-group">
            <label for="concurrency">Analysis Concurrency</label>
            <input type="number" value="4" min="1" max="16" class="theme-select" style="width: 80px;" disabled />
            <p class="help-text">Number of worker threads running DSP and ONNX models concurrently.</p>
          </div>
        </div>
      </section>
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
  .settings-form {
    display: flex;
    flex-direction: column;
    gap: 2rem;
    max-width: 600px;
  }

  .form-group {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
  }

  .form-group label {
    font-family: 'Outfit', sans-serif;
    font-weight: 600;
    font-size: 1.1rem;
  }

  .folder-list {
    background: rgba(0,0,0,0.15);
    border: 2px solid var(--border-color);
    border-radius: var(--radius-md);
    padding: 1.5rem;
    margin-bottom: 0.5rem;
  }

  html[data-theme="accessible"] .folder-list {
    border: 2px solid #fff;
    background: #000;
  }

  .empty-folders {
    font-size: 0.95rem;
    color: var(--text-muted);
    text-align: center;
  }

  .help-text {
    font-size: 0.85rem;
    color: var(--text-muted);
  }
</style>
