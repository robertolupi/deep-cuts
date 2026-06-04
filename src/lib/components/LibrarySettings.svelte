<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { getVersion } from "@tauri-apps/api/app";
  import { library } from "$lib/stores/library.svelte";
  import { ui } from "$lib/stores/ui.svelte";
  import { player } from "$lib/stores/player.svelte";
  import ModelDownloader from "./ModelDownloader.svelte";
  import TelemetryInspector from "./TelemetryInspector.svelte";
  import { onMount } from "svelte";
  import licenseText from "../../../LICENSE.md?raw";

  let name = $state("");
  let path = $state("");
  let appVersion = $state("");
  let isAddLoading = $state(false);

  const directories    = $derived(library.directories);
  const trackCount     = $derived(library.trackCount);
  const isScanning     = $derived(library.isScanning);
  const scanProgress   = $derived(library.scanProgress);
  const scanCurrentFile      = $derived(library.scanCurrentFile);
  const scanProcessedCount   = $derived(library.scanProcessedCount);
  const scanTotalCount       = $derived(library.scanTotalCount);

  async function choosePath() {
    try {
      const selected = await invoke<string | null>("select_directory");
      if (selected) {
        path = selected;
        if (!name) {
          const parts = selected.split(/[/\\]/);
          name = parts[parts.length - 1] || parts[parts.length - 2] || "Music Library";
        }
        ui.showToast("Path selected successfully.", "success");
      }
    } catch (err: any) { ui.showToast(err.toString(), "error"); }
  }

  async function addDirectory() {
    if (!name.trim() || !path.trim()) {
      ui.showToast("Collection Name and Directory Path are required.", "error");
      return;
    }
    isAddLoading = true;
    try {
      await library.addDirectory(name, path);
      ui.showToast(`Added folder "${name}" to monitored list.`, "success");
      name = ""; path = "";
    } catch (err: any) { ui.showToast(err.toString(), "error"); }
    finally { isAddLoading = false; }
  }

  async function removeDirectory(id: number, folderName: string) {
    try {
      await library.removeDirectory(id);
      ui.showToast(`Stopped watching "${folderName}".`, "success");
    } catch (err: any) { ui.showToast(err.toString(), "error"); }
  }

  async function triggerScan() {
    if (library.isScanning) return;
    if (library.directories.length === 0) {
      ui.showToast("Register at least one monitored library directory first.", "error");
      return;
    }
    try {
      await library.triggerScan();
      ui.showToast("Library scanning initiated in background.", "success");
    } catch (err: any) { ui.showToast(err.toString(), "error"); }
  }

  async function exportSidecars() {
    try {
      const count = await library.exportSidecars();
      ui.showToast(`Exported ${count} sidecar file${count === 1 ? "" : "s"}.`, "success");
    } catch (err: any) { ui.showToast(err.toString(), "error"); }
  }

  let configuredModelPath = $state<string | null>(null);
  let modelPathMessage = $state("");
  let checkUpdatesEnabled = $state(true);
  let acoustidEnabled = $state(true);
  let sidecarEnabled = $state(false);
  let showModelDownloaderDrawer = $state(false);
  let showLicenseDrawer = $state(false);
  let showTelemetryDrawer = $state(false);


  async function loadModelPathSetting() {
    try {
      configuredModelPath = await invoke<string | null>("get_model_path_setting");
    } catch (e) {
      console.error("Failed to load model path setting:", e);
    }
  }

  async function loadUpdateSettings() {
    try {
      checkUpdatesEnabled = await invoke<boolean>("get_update_settings");
    } catch (e) {
      console.error("Failed to load update settings:", e);
    }
  }

  async function toggleUpdateSettings(enabled: boolean) {
    try {
      await invoke("set_update_settings", { enabled });
      checkUpdatesEnabled = enabled;
      ui.showToast(`Startup update checking ${enabled ? "enabled" : "disabled"}.`, "success");
    } catch (err: any) {
      ui.showToast(err.toString(), "error");
    }
  }

  async function loadAcoustidSettings() {
    try {
      const mode = await invoke<string>("get_acoustid_setting");
      acoustidEnabled = mode === "silent";
    } catch (e) {
      console.error("Failed to load AcoustID settings:", e);
    }
  }

  async function toggleAcoustidSettings(enabled: boolean) {
    try {
      const mode = enabled ? "silent" : "never";
      await invoke("save_acoustid_setting", { value: mode });
      acoustidEnabled = enabled;
      ui.showToast(`MusicBrainz metadata enrichment ${enabled ? "enabled (silent)" : "disabled"}.`, "success");
    } catch (err: any) {
      ui.showToast(err.toString(), "error");
    }
  }

  async function loadSidecarSetting() {
    try {
      sidecarEnabled = await invoke<boolean>("get_sidecar_setting");
    } catch (e) {
      console.error("Failed to load sidecar setting:", e);
    }
  }

  async function toggleSidecarSetting(enabled: boolean) {
    try {
      await invoke("save_sidecar_setting", { enabled });
      sidecarEnabled = enabled;
      ui.showToast(`Sidecar file writing ${enabled ? "enabled" : "disabled"}.`, "success");
    } catch (err: any) {
      ui.showToast(err.toString(), "error");
    }
  }

  async function chooseModelPath() {
    modelPathMessage = "";
    try {
      const path = await invoke<string | null>("select_directory");
      if (!path) return;
      await invoke("save_model_path_setting", { path });
      configuredModelPath = path;
      modelPathMessage = "Model folder saved.";
    } catch (e: any) {
      modelPathMessage = e?.toString() ?? "Failed to save model folder.";
    }
  }

  async function clearModelPath() {
    modelPathMessage = "";
    try {
      await invoke("save_model_path_setting", { path: null });
      configuredModelPath = null;
      modelPathMessage = "Using default model locations.";
    } catch (e: any) {
      modelPathMessage = e?.toString() ?? "Failed to clear model folder.";
    }
  }

  async function openLogDir() {
    try {
      await invoke("open_log_dir");
      ui.showToast("Log directory opened.", "success");
    } catch (err: any) {
      ui.showToast(err.toString(), "error");
    }
  }

  onMount(async () => {
    loadModelPathSetting();
    loadUpdateSettings();
    loadAcoustidSettings();
    loadSidecarSetting();
    appVersion = await getVersion();
  });
</script>

<div class="settings-layout">

  <!-- Left column -->
  <div class="settings-left">

    <!-- Add folder card -->
    <div class="sg-card">
      <div class="card-header">
        <span class="card-title">Add Library Folder</span>
        <span class="card-subtitle">MP3 · WAV · FLAC · M4A · AIFF · OGG · OPUS</span>
      </div>

      <div class="field-group">
        <span class="field-label">DIRECTORY PATH</span>
        <div class="path-row">
          <input
            type="text"
            value={path}
            placeholder="Select a folder…"
            readonly
            class="sg-input path-input"
          />
          <button class="sg-btn" onclick={choosePath}>
            <svg xmlns="http://www.w3.org/2000/svg" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
              <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/>
            </svg>
            Browse
          </button>
        </div>
      </div>

      <div class="field-group">
        <span class="field-label">COLLECTION NAME</span>
        <input
          type="text"
          bind:value={name}
          placeholder="e.g. Hi-Res Masters, Chillout Beats"
          class="sg-input"
        />
      </div>

      <button
        class="sg-btn sg-btn-primary submit-btn"
        onclick={addDirectory}
        disabled={isAddLoading || !path}
      >
        {#if isAddLoading}
          <span class="spin-icon">
            <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><path d="M21 12a9 9 0 1 1-6.219-8.56"/></svg>
          </span>
          Registering…
        {:else}
          <svg xmlns="http://www.w3.org/2000/svg" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
            <line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/>
          </svg>
          Register Folder
        {/if}
      </button>
    </div>

    <!-- Stats card -->
    <div class="sg-card stats-card">
      <span class="card-title">Collection</span>
      <div class="stats-row">
        <div class="stat-item">
          <span class="stat-value">{directories.length}</span>
          <span class="stat-label">Folders</span>
        </div>
        <div class="stat-divider"></div>
        <div class="stat-item">
          <span class="stat-value stat-cyan">{trackCount.toLocaleString()}</span>
          <span class="stat-label">Tracks indexed</span>
        </div>
      </div>
    </div>

    <!-- Model folder card -->
    <div class="sg-card">
      <div class="card-header">
        <span class="card-title">Model Folder</span>
        <span class="card-subtitle">Location of neural network model files (requires ~6.3 GB of disk space)</span>
      </div>

      <div class="field-group">
        <div class="model-path-value" title={configuredModelPath ?? 'Default locations'}>
          {configuredModelPath ?? 'Default locations'}
        </div>
      </div>

      <div class="model-path-actions">
        <button class="sg-btn sg-btn-primary" onclick={chooseModelPath}>Choose Folder</button>
        {#if configuredModelPath}
          <button class="sg-btn" onclick={clearModelPath}>Clear</button>
        {/if}
        <button class="sg-btn sg-btn-primary" onclick={() => { showModelDownloaderDrawer = true; }}>
          <svg xmlns="http://www.w3.org/2000/svg" width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
            <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/>
          </svg>
          Manage Models
        </button>
      </div>

      {#if modelPathMessage}
        <div class="model-path-message">{modelPathMessage}</div>
      {/if}
    </div>

    <!-- Network Settings card -->
    <div class="sg-card">
      <div class="card-header">
        <span class="card-title">Network Settings</span>
        <span class="card-subtitle">Control network access and online metadata fetches</span>
      </div>

      <div class="update-toggle-row">
        <label class="update-checkbox-label">
          <input
            type="checkbox"
            checked={acoustidEnabled}
            onchange={(e) => toggleAcoustidSettings(e.currentTarget.checked)}
            class="update-checkbox"
          />
          <span class="checkbox-text">Fetch metadata from MusicBrainz (AcoustID)</span>
        </label>
      </div>

      <div class="update-toggle-row">
        <label class="update-checkbox-label">
          <input
            type="checkbox"
            checked={checkUpdatesEnabled}
            onchange={(e) => toggleUpdateSettings(e.currentTarget.checked)}
            class="update-checkbox"
          />
          <span class="checkbox-text">Check for updates on startup</span>
        </label>
      </div>
    </div>

    <!-- Analysis Settings card -->
    <div class="sg-card">
      <div class="card-header">
        <span class="card-title">Analysis Settings</span>
        <span class="card-subtitle">Control how analysis results are stored</span>
      </div>

      <div class="update-toggle-row">
        <label class="update-checkbox-label">
          <input
            type="checkbox"
            checked={sidecarEnabled}
            onchange={(e) => toggleSidecarSetting(e.currentTarget.checked)}
            class="update-checkbox"
          />
          <span class="checkbox-text">Write .dc.json sidecar files after analysis</span>
        </label>
      </div>

      <div class="update-toggle-row">
        <label class="update-checkbox-label">
          <input
            type="checkbox"
            checked={player.showLoudestMarker}
            onchange={(e) => player.setShowLoudestMarker(e.currentTarget.checked)}
            class="update-checkbox"
          />
          <span class="checkbox-text">Show loudest analysis windows on player</span>
        </label>
      </div>

      <div style="border-top: 1px solid rgba(255,255,255,0.06); padding-top: 0.85rem; margin-top: 0.25rem; display: flex; flex-direction: column; gap: 8px;">
        <button class="sg-btn sg-btn-primary" onclick={() => showTelemetryDrawer = true} style="width: 100%; justify-content: center;">
          <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
            <line x1="18" y1="20" x2="18" y2="10"/><line x1="12" y1="20" x2="12" y2="4"/><line x1="6" y1="20" x2="6" y2="14"/>
          </svg>
          Inspect Telemetry & Traces
        </button>
        <button class="sg-btn" onclick={openLogDir} style="width: 100%; justify-content: center;">
          <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
            <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/>
          </svg>
          Open Application Logs
        </button>
      </div>
    </div>
  </div>

  <!-- Right column: folder list + about -->
  <div class="right-col">
  <div class="sg-card list-card">
    <div class="list-header">
      <div>
        <span class="card-title">Monitored Folders</span>
        <span class="card-subtitle">Folders Deep Cuts watches for audio files</span>
      </div>

      {#if directories.length > 0}
        <div class="scan-actions">
          {#if isScanning}
            <div class="scan-progress">
              <div class="scan-top-row">
                <span class="scan-label">Scanning {Math.round(scanProgress)}%</span>
                <span class="scan-counts">{scanProcessedCount} / {scanTotalCount}</span>
              </div>
              <div class="scan-bar-track">
                <div class="scan-bar-fill" style="width:{scanProgress}%"></div>
              </div>
              <span class="scan-file" title={scanCurrentFile}>
                {scanCurrentFile.split(/[/\\]/).pop() ?? ""}
              </span>
            </div>
          {:else}
            <button class="sg-btn sg-btn-primary" onclick={triggerScan}>
              <svg xmlns="http://www.w3.org/2000/svg" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
                <circle cx="12" cy="12" r="10"/><polyline points="12 6 12 12 16 14"/>
              </svg>
              Scan Library
            </button>
            <button class="sg-btn" onclick={exportSidecars} title="Write .dc.json sidecar files next to each audio file">
              <svg xmlns="http://www.w3.org/2000/svg" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
                <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/>
                <polyline points="7 10 12 15 17 10"/>
                <line x1="12" y1="15" x2="12" y2="3"/>
              </svg>
              Export Sidecars
            </button>
          {/if}
        </div>
      {/if}
    </div>

    {#if directories.length > 0}
      <div class="dir-list">
        {#each directories as dir (dir.id)}
          <div class="dir-row">
            <div class="dir-name-badge">{dir.name}</div>
            <code class="dir-path" title={dir.path}>{dir.path}</code>
            <button
              class="delete-btn"
              title="Remove folder"
              onclick={() => removeDirectory(dir.id, dir.name)}
            >
              <svg xmlns="http://www.w3.org/2000/svg" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round">
                <polyline points="3 6 5 6 21 6"/>
                <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/>
                <line x1="10" y1="11" x2="10" y2="17"/>
                <line x1="14" y1="11" x2="14" y2="17"/>
              </svg>
            </button>
          </div>
        {/each}
      </div>
    {:else}
      <div class="empty-dirs">
        <svg xmlns="http://www.w3.org/2000/svg" width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
          <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/>
        </svg>
        <p>No folders registered yet.</p>
        <p class="empty-sub">Use the form on the left to add a music directory.</p>
      </div>
    {/if}
  </div>

  <!-- About card -->
  <div class="sg-card about-card">
    <div class="card-header">
      <span class="card-title">About Deep Cuts</span>
      {#if appVersion}<span class="card-subtitle">v{appVersion}</span>{/if}
    </div>
    <p class="about-desc">
      A local-first music intelligence desktop app that analyzes your audio library with machine
      learning — BPM, key, genre, mood, and semantic embeddings — so producers can filter, search,
      and discover reference tracks by sonic characteristics. Everything runs on your machine,
      with no cloud dependency.
    </p>
    <div class="about-meta">
      <span class="about-copyright">© 2025 <a class="about-link" href="https://www.rlupi.com" target="_blank" rel="noopener noreferrer">Roberto Lupi</a></span>
      <span class="about-sep">·</span>
      <span class="about-license-sep">·</span>
      <button class="about-license" onclick={() => showLicenseDrawer = true}>GNU AGPL v3</button>
    </div>

    <div class="model-credits-list">
      <div class="credit-item">
        <div class="credit-header">
          <span class="credit-name">LAION CLAP</span>
          <span class="credit-badge badge-apache">Apache 2.0</span>
        </div>
        <p class="credit-desc">Contrastive Language-Audio Pretraining model for audio-text semantic similarity.</p>
        <div class="credit-links">
          <a href="https://huggingface.co/laion/clap-htsat-unfused" target="_blank" rel="noopener noreferrer" class="credit-link">laion/clap-htsat-unfused</a>
          <span class="credit-link-sep">·</span>
          <span class="credit-citation">Wu et al. (ICASSP 2023)</span>
        </div>
      </div>

      <div class="credit-item">
        <div class="credit-header">
          <span class="credit-name">all-MiniLM-L6-v2</span>
          <span class="credit-badge badge-apache">Apache 2.0</span>
        </div>
        <p class="credit-desc">MiniLM sentence transformer model for robust text semantic embeddings.</p>
        <div class="credit-links">
          <a href="https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2" target="_blank" rel="noopener noreferrer" class="credit-link">sentence-transformers/all-MiniLM-L6-v2</a>
        </div>
      </div>

      <div class="credit-item">
        <div class="credit-header">
          <span class="credit-name">Essentia Classifiers</span>
          <span class="credit-badge badge-cc">CC BY-NC-ND 4.0</span>
        </div>
        <p class="credit-desc">Discogs-Effnet feature extractor and classifier heads for genre, mood, and vocal classification.</p>
        <div class="credit-links">
          <a href="https://essentia.upf.edu/models.html" target="_blank" rel="noopener noreferrer" class="credit-link">Essentia Models Hub</a>
          <span class="credit-link-sep">·</span>
          <span class="credit-citation">Bogdanov et al. (ISMIR 2013)</span>
        </div>
        <div class="credit-warning">
          Strictly for non-commercial use. Original checkpoints copyright © Music Technology Group (MTG), Universitat Pompeu Fabra.
        </div>
      </div>

      <div class="credit-item">
        <div class="credit-header">
          <span class="credit-name">llama.cpp</span>
          <span class="credit-badge badge-mit">MIT</span>
        </div>
        <p class="credit-desc">LLM inference in C/C++ supporting the local Qwen2-Audio server lifecycle.</p>
        <div class="credit-links">
          <a href="https://github.com/ggerganov/llama.cpp" target="_blank" rel="noopener noreferrer" class="credit-link">ggerganov/llama.cpp</a>
          <span class="credit-link-sep">·</span>
          <span class="credit-citation">Georgi Gerganov</span>
        </div>
      </div>
    </div>
  </div>

  </div>
</div>

{#if showModelDownloaderDrawer}
  <div class="drawer-overlay" onclick={() => { showModelDownloaderDrawer = false; }}>
    <div class="drawer-content" onclick={(e) => e.stopPropagation()}>
      <div class="drawer-header">
        <div class="drawer-header-left">
          <h3 class="drawer-title">Manage Neural Models</h3>
          <p class="drawer-subtitle">Download, verify, or resume AI models required for local-first audio indexing</p>
        </div>
        <button class="drawer-close-btn" onclick={() => { showModelDownloaderDrawer = false; }}>×</button>
      </div>
      <div class="drawer-body">
        <ModelDownloader onComplete={() => {}} />
      </div>
    </div>
  </div>
{/if}

{#if showLicenseDrawer}
  <div class="drawer-overlay" onclick={() => { showLicenseDrawer = false; }}>
    <div class="drawer-content" onclick={(e) => e.stopPropagation()} style="width: 550px; max-width: 90vw;">
      <div class="drawer-header">
        <div class="drawer-header-left">
          <h3 class="drawer-title">Application License</h3>
          <p class="drawer-subtitle">GNU Affero General Public License Version 3</p>
        </div>
        <button class="drawer-close-btn" onclick={() => { showLicenseDrawer = false; }}>×</button>
      </div>
      <div class="drawer-body" style="overflow-y: auto; max-height: calc(100vh - 120px);">
        <pre class="license-text">{licenseText}</pre>
      </div>
    </div>
  </div>
{/if}

{#if showTelemetryDrawer}
  <div class="drawer-overlay" onclick={() => { showTelemetryDrawer = false; }}>
    <div class="drawer-content" onclick={(e) => e.stopPropagation()} style="width: 850px; max-width: 95vw;">
      <div class="drawer-header">
        <div class="drawer-header-left">
          <h3 class="drawer-title">Pipeline Diagnostics & Telemetry</h3>
          <p class="drawer-subtitle">Inspect performance traces, latency statistics, and diagnostic logs</p>
        </div>
        <button class="drawer-close-btn" onclick={() => { showTelemetryDrawer = false; }}>×</button>
      </div>
      <div class="drawer-body" style="overflow-y: auto;">
        <TelemetryInspector />
      </div>
    </div>
  </div>
{/if}

<style>
  .settings-layout {
    display: grid;
    grid-template-columns: 300px 1fr;
    gap: 1rem;
    padding: 1rem 1.25rem;
    height: 100%;
    overflow-y: auto;
    background: var(--sg-surface, #0d1117);
    scrollbar-width: thin;
    scrollbar-color: rgba(255,255,255,0.1) transparent;
    align-content: start;
  }

  .settings-left {
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }

  .model-path-value {
    min-height: 24px;
    padding: 7px 10px;
    border: 1px solid rgba(255,255,255,0.08);
    border-radius: 4px;
    background: rgba(0,0,0,0.22);
    color: var(--sg-on-surface, #e3e1e9);
    font-family: "JetBrains Mono", monospace;
    font-size: 10px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    box-sizing: border-box;
  }

  .model-path-actions {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
  }

  .model-path-message {
    font-family: "JetBrains Mono", monospace;
    font-size: 9px;
    color: var(--sg-primary, #00f0ff);
  }

  /* ── Card ── */
  .sg-card {
    background: var(--sg-surface-slate, #161b22);
    border: 1px solid rgba(255,255,255,0.07);
    border-radius: 6px;
    padding: 1rem;
    display: flex;
    flex-direction: column;
    gap: 0.85rem;
  }

  .list-card {
    height: fit-content;
  }

  .card-header {
    display: flex;
    flex-direction: column;
    gap: 3px;
    padding-bottom: 0.65rem;
    border-bottom: 1px solid rgba(255,255,255,0.06);
  }

  .card-title {
    font-family: "JetBrains Mono", monospace;
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--sg-on-surface, #e3e1e9);
  }

  .card-subtitle {
    font-family: "JetBrains Mono", monospace;
    font-size: 9px;
    color: var(--sg-outline, #849495);
    letter-spacing: 0.04em;
  }

  /* ── Form fields ── */
  .field-group {
    display: flex;
    flex-direction: column;
    gap: 5px;
  }

  .field-label {
    font-family: "JetBrains Mono", monospace;
    font-size: 8px;
    font-weight: 700;
    letter-spacing: 0.12em;
    color: var(--sg-outline, #849495);
  }

  .sg-input {
    width: 100%;
    background: rgba(255,255,255,0.03);
    border: 1px solid rgba(255,255,255,0.08);
    border-radius: 4px;
    padding: 7px 10px;
    font-family: "JetBrains Mono", monospace;
    font-size: 11px;
    color: var(--sg-on-surface, #e3e1e9);
    outline: none;
    box-sizing: border-box;
    transition: border-color 0.15s;
  }

  .sg-input::placeholder { color: var(--sg-outline, #849495); opacity: 0.6; }
  .sg-input:focus { border-color: rgba(0,240,255,0.4); }
  .sg-input[readonly] { cursor: default; opacity: 0.7; }

  .path-row {
    display: flex;
    gap: 6px;
    align-items: center;
  }

  .path-input { flex: 1; min-width: 0; }

  /* ── Buttons ── */
  .sg-btn {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    font-family: "JetBrains Mono", monospace;
    font-size: 10px;
    font-weight: 700;
    padding: 6px 12px;
    border: 1px solid rgba(255,255,255,0.12);
    border-radius: 4px;
    background: rgba(255,255,255,0.04);
    color: var(--sg-outline, #849495);
    cursor: pointer;
    white-space: nowrap;
    transition: all 0.12s;
    flex-shrink: 0;
  }

  .sg-btn:hover:not(:disabled) {
    border-color: rgba(255,255,255,0.25);
    color: var(--sg-on-surface, #e3e1e9);
    background: rgba(255,255,255,0.08);
  }

  .sg-btn:disabled { opacity: 0.4; cursor: not-allowed; }

  .sg-btn-primary {
    border-color: rgba(0,240,255,0.35);
    color: var(--sg-primary, #00f0ff);
    background: rgba(0,240,255,0.08);
  }

  .sg-btn-primary:hover:not(:disabled) {
    background: rgba(0,240,255,0.14);
    border-color: var(--sg-primary, #00f0ff);
    color: var(--sg-primary, #00f0ff);
  }

  .submit-btn { width: 100%; justify-content: center; }

  .spin-icon {
    display: inline-flex;
    animation: spin 1s linear infinite;
  }

  @keyframes spin {
    from { transform: rotate(0deg); }
    to   { transform: rotate(360deg); }
  }

  /* ── Stats card ── */
  .stats-card { padding: 0.85rem 1rem; }

  .stats-row {
    display: flex;
    align-items: center;
    gap: 1.25rem;
    margin-top: 0.25rem;
  }

  .stat-item {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .stat-value {
    font-family: "JetBrains Mono", monospace;
    font-size: 28px;
    font-weight: 700;
    color: var(--sg-on-surface, #e3e1e9);
    line-height: 1;
  }

  .stat-cyan { color: var(--sg-primary, #00f0ff); text-shadow: 0 0 12px rgba(0,240,255,0.3); }

  .stat-label {
    font-family: "JetBrains Mono", monospace;
    font-size: 9px;
    color: var(--sg-outline, #849495);
    letter-spacing: 0.06em;
  }

  .stat-divider {
    width: 1px;
    height: 36px;
    background: rgba(255,255,255,0.08);
  }

  /* ── List card ── */
  .list-header {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 1rem;
    padding-bottom: 0.85rem;
    border-bottom: 1px solid rgba(255,255,255,0.06);
  }

  .scan-actions {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 0.4rem;
    flex-shrink: 0;
  }

  .scan-progress {
    display: flex;
    flex-direction: column;
    gap: 3px;
    min-width: 160px;
    align-items: flex-end;
  }

  .scan-top-row {
    display: flex;
    gap: 8px;
    align-items: center;
  }

  .scan-label {
    font-family: "JetBrains Mono", monospace;
    font-size: 10px;
    font-weight: 700;
    color: var(--sg-primary, #00f0ff);
  }

  .scan-counts {
    font-family: "JetBrains Mono", monospace;
    font-size: 9px;
    color: var(--sg-outline, #849495);
  }

  .scan-bar-track {
    width: 100%;
    height: 3px;
    background: rgba(255,255,255,0.06);
    border-radius: 2px;
    overflow: hidden;
  }

  .scan-bar-fill {
    height: 100%;
    background: var(--sg-primary, #00f0ff);
    border-radius: 2px;
    transition: width 0.3s ease;
  }

  .scan-file {
    font-family: "JetBrains Mono", monospace;
    font-size: 9px;
    color: var(--sg-outline, #849495);
    max-width: 180px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  /* ── Directory rows ── */
  .dir-list {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .dir-row {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    padding: 8px 10px;
    border: 1px solid rgba(255,255,255,0.05);
    border-radius: 4px;
    background: rgba(255,255,255,0.02);
    transition: border-color 0.15s;
  }

  .dir-row:hover { border-color: rgba(255,255,255,0.1); }

  .dir-name-badge {
    font-family: "JetBrains Mono", monospace;
    font-size: 10px;
    font-weight: 700;
    padding: 3px 8px;
    border: 1px solid rgba(0,240,255,0.3);
    color: var(--sg-primary, #00f0ff);
    background: rgba(0,240,255,0.07);
    border-radius: 3px;
    white-space: nowrap;
    flex-shrink: 0;
  }

  .dir-path {
    font-family: "JetBrains Mono", monospace;
    font-size: 10px;
    color: var(--sg-outline, #849495);
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .delete-btn {
    background: none;
    border: 1px solid transparent;
    border-radius: 3px;
    color: var(--sg-outline, #849495);
    cursor: pointer;
    padding: 4px;
    display: flex;
    align-items: center;
    flex-shrink: 0;
    margin-left: auto;
    transition: all 0.12s;
  }

  .delete-btn:hover {
    color: #ff6b6b;
    border-color: rgba(255,107,107,0.3);
    background: rgba(255,107,107,0.07);
  }

  /* ── Empty state ── */
  .empty-dirs {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 0.5rem;
    padding: 2.5rem 1rem;
    color: var(--sg-outline, #849495);
    opacity: 0.5;
    text-align: center;
  }

  .empty-dirs p {
    font-family: "JetBrains Mono", monospace;
    font-size: 11px;
    margin: 0;
  }

  .empty-sub { font-size: 10px !important; opacity: 0.7; }

  /* ── Right column wrapper ── */
  .right-col {
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }

  /* ── About card ── */
  .about-card {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
  }

  .about-desc {
    margin: 0;
    font-size: 11px;
    line-height: 1.65;
    color: var(--sg-on-surface, #e3e1e9);
    opacity: 0.75;
  }

  .about-meta {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    font-family: "JetBrains Mono", monospace;
    font-size: 10px;
    color: var(--sg-outline, #849495);
  }

  .about-link {
    color: inherit;
    text-decoration: underline;
    text-underline-offset: 2px;
    text-decoration-color: rgba(132, 148, 149, 0.4);
    transition: color 0.15s, text-decoration-color 0.15s;
  }
  .about-link:hover {
    color: var(--sg-primary, #00f0ff);
    text-decoration-color: var(--sg-primary, #00f0ff);
  }

  .about-sep { opacity: 0.4; }

  .about-license {
    background: none;
    border: none;
    padding: 0;
    font-family: inherit;
    font-size: inherit;
    color: var(--sg-primary, #00f0ff);
    opacity: 0.75;
    cursor: pointer;
    text-decoration: underline;
    text-underline-offset: 2px;
    text-decoration-color: rgba(0, 240, 255, 0.4);
    transition: opacity 0.15s, text-decoration-color 0.15s;
  }
  .about-license:hover {
    opacity: 1;
    text-decoration-color: var(--sg-primary, #00f0ff);
  }

  .license-text {
    font-family: "JetBrains Mono", monospace;
    font-size: 10px;
    line-height: 1.6;
    color: var(--sg-on-surface, #e3e1e9);
    white-space: pre-wrap;
    background: rgba(0, 0, 0, 0.22);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 4px;
    padding: 1rem;
    margin: 0;
    text-align: left;
  }

  .update-toggle-row {
    display: flex;
    align-items: center;
    padding: 4px 0;
  }

  .update-checkbox-label {
    display: flex;
    align-items: center;
    gap: 8px;
    cursor: pointer;
    user-select: none;
    font-family: "JetBrains Mono", monospace;
    font-size: 10px;
    color: var(--sg-on-surface, #e3e1e9);
  }

  .update-checkbox {
    appearance: none;
    width: 14px;
    height: 14px;
    border: 1px solid rgba(255, 255, 255, 0.15);
    border-radius: 3px;
    background: rgba(255, 255, 255, 0.03);
    cursor: pointer;
    display: grid;
    place-content: center;
    transition: all 0.12s ease;
    margin: 0;
  }

  .update-checkbox:hover {
    border-color: rgba(0, 240, 255, 0.4);
    background: rgba(0, 240, 255, 0.03);
  }

  .update-checkbox:checked {
    border-color: var(--sg-primary, #00f0ff);
    background: rgba(0, 240, 255, 0.1);
  }

  .update-checkbox:checked::before {
    content: "";
    width: 6px;
    height: 6px;
    background: var(--sg-primary, #00f0ff);
    border-radius: 1px;
    box-shadow: 0 0 4px rgba(0, 240, 255, 0.5);
  }

  .checkbox-text {
    line-height: 1;
    font-size: 11px;
    color: var(--sg-outline, #849495);
    transition: color 0.12s;
  }

  .update-checkbox-label:hover .checkbox-text {
    color: var(--sg-on-surface, #e3e1e9);
  }

  /* ── Drawer Overlay & Panel ── */
  .drawer-overlay {
    position: fixed;
    top: 0;
    left: 0;
    width: 100vw;
    height: 100vh;
    background: rgba(0, 0, 0, 0.6);
    backdrop-filter: blur(4px);
    z-index: 1000;
    display: flex;
    justify-content: flex-end;
  }

  .drawer-content {
    width: 420px;
    height: 100%;
    background: var(--sg-surface, #0d1117);
    border-left: 1px solid rgba(255, 255, 255, 0.08);
    display: flex;
    flex-direction: column;
    box-shadow: -10px 0 30px rgba(0, 0, 0, 0.5);
    animation: slide-in 0.25s cubic-bezier(0.16, 1, 0.3, 1) forwards;
  }

  @keyframes slide-in {
    from { transform: translateX(100%); }
    to { transform: translateX(0); }
  }

  .drawer-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 1.25rem;
    border-bottom: 1px solid rgba(255, 255, 255, 0.06);
    flex-shrink: 0;
  }

  .drawer-header-left {
    display: flex;
    flex-direction: column;
    gap: 3px;
  }

  .drawer-title {
    font-family: "JetBrains Mono", monospace;
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--sg-on-surface, #e3e1e9);
    margin: 0;
  }

  .drawer-subtitle {
    font-family: "JetBrains Mono", monospace;
    font-size: 9px;
    color: var(--sg-outline, #849495);
    margin: 0;
  }

  .drawer-close-btn {
    background: none;
    border: none;
    color: var(--sg-outline, #849495);
    font-size: 22px;
    cursor: pointer;
    line-height: 1;
    padding: 0 6px;
    transition: color 0.12s;
  }

  .drawer-close-btn:hover {
    color: var(--sg-on-surface, #e3e1e9);
  }

  .drawer-body {
    flex: 1;
    padding: 1.25rem;
    overflow-y: auto;
  }

  .about-btn {
    background: none;
    border: none;
    padding: 0;
    font-family: "JetBrains Mono", monospace;
    font-size: 10px;
    color: var(--sg-outline, #849495);
    cursor: pointer;
    text-decoration: underline;
    text-underline-offset: 2px;
    text-decoration-color: rgba(132, 148, 149, 0.4);
    transition: color 0.15s, text-decoration-color 0.15s;
  }

  .about-btn:hover, .about-btn.active {
    color: var(--sg-primary, #00f0ff);
    text-decoration-color: var(--sg-primary, #00f0ff);
  }

  .model-credits-list {
    display: flex;
    flex-direction: column;
    gap: 12px;
    margin-top: 8px;
    padding-top: 12px;
    border-top: 1px solid rgba(255, 255, 255, 0.06);
    animation: fade-in 0.2s ease-out;
  }

  @keyframes fade-in {
    from { opacity: 0; transform: translateY(-4px); }
    to { opacity: 1; transform: translateY(0); }
  }

  .credit-item {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .credit-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }

  .credit-name {
    font-family: "JetBrains Mono", monospace;
    font-size: 10px;
    font-weight: 700;
    color: var(--sg-on-surface, #e3e1e9);
  }

  .credit-badge {
    font-family: "JetBrains Mono", monospace;
    font-size: 8px;
    font-weight: 700;
    padding: 1px 5px;
    border-radius: 3px;
    white-space: nowrap;
  }

  .badge-apache {
    border: 1px solid rgba(0, 240, 255, 0.3);
    color: var(--sg-primary, #00f0ff);
    background: rgba(0, 240, 255, 0.05);
  }

  .badge-cc {
    border: 1px solid rgba(255, 170, 0, 0.3);
    color: #ffaa00;
    background: rgba(255, 170, 0, 0.05);
  }

  .badge-mit {
    border: 1px solid rgba(188, 19, 254, 0.35);
    color: #bc13fe;
    background: rgba(188, 19, 254, 0.05);
  }

  .credit-desc {
    margin: 0;
    font-size: 10px;
    line-height: 1.4;
    color: var(--sg-on-surface, #e3e1e9);
    opacity: 0.7;
  }

  .credit-links {
    display: flex;
    align-items: center;
    gap: 6px;
    font-family: "JetBrains Mono", monospace;
    font-size: 9px;
    color: var(--sg-outline, #849495);
  }

  .credit-link {
    color: inherit;
    text-decoration: underline;
    text-underline-offset: 2px;
    text-decoration-color: rgba(132, 148, 149, 0.3);
    transition: color 0.15s, text-decoration-color 0.15s;
  }

  .credit-link:hover {
    color: var(--sg-primary, #00f0ff);
    text-decoration-color: var(--sg-primary, #00f0ff);
  }

  .credit-link-sep {
    opacity: 0.4;
  }

  .credit-citation {
    opacity: 0.8;
  }

  .credit-warning {
    font-family: "JetBrains Mono", monospace;
    font-size: 8px;
    color: rgba(255, 170, 0, 0.8);
    margin-top: 2px;
    line-height: 1.3;
  }
</style>
