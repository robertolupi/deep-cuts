<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { library } from "$lib/stores/library.svelte";
  import { ui } from "$lib/stores/ui.svelte";

  let name = $state("");
  let path = $state("");
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
  </div>

  <!-- Right column: folder list -->
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
</div>

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
</style>
