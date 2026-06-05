<script lang="ts">
  import { library } from "$lib/stores/library.svelte";
  import { ui } from "$lib/stores/ui.svelte";

  const directories        = $derived(library.directories);
  const isScanning         = $derived(library.isScanning);
  const scanProgress       = $derived(library.scanProgress);
  const scanCurrentFile    = $derived(library.scanCurrentFile);
  const scanProcessedCount = $derived(library.scanProcessedCount);
  const scanTotalCount     = $derived(library.scanTotalCount);

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

<style>
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

  .list-header {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 1rem;
    padding-bottom: 0.85rem;
    border-bottom: 1px solid rgba(255,255,255,0.06);
  }

  .card-title {
    display: block;
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-sm);
    font-weight: 700;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--sg-on-surface, #e3e1e9);
  }

  .card-subtitle {
    display: block;
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-2xs);
    color: var(--sg-outline, #849495);
    letter-spacing: 0.04em;
    margin-top: 3px;
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
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-xs);
    font-weight: 700;
    color: var(--sg-primary, #00f0ff);
  }

  .scan-counts {
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-2xs);
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
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-2xs);
    color: var(--sg-outline, #849495);
    max-width: 180px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .sg-btn {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-xs);
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
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-xs);
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
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-xs);
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
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-sm);
    margin: 0;
  }

  .empty-sub { font-size: var(--sg-text-xs) !important; opacity: 0.7; }
</style>
