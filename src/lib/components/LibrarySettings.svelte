<script lang="ts">
  import type { WatchedDirectory } from '../types';

  let {
    directories,
    trackCount,
    isScanning,
    scanProgress,
    scanCurrentFile,
    scanProcessedCount,
    scanTotalCount,
    path = $bindable(),
    name = $bindable(),
    isAddLoading,
    errorMessage,
    successMessage,
    choosePath,
    addDirectory,
    removeDirectory,
    triggerScan
  }: {
    directories: WatchedDirectory[];
    trackCount: number;
    isScanning: boolean;
    scanProgress: number;
    scanCurrentFile: string;
    scanProcessedCount: number;
    scanTotalCount: number;
    path: string;
    name: string;
    isAddLoading: boolean;
    errorMessage: string;
    successMessage: string;
    choosePath: () => Promise<void>;
    addDirectory: () => Promise<void>;
    removeDirectory: (id: number, folderName: string) => Promise<void>;
    triggerScan: () => Promise<void>;
  } = $props();
</script>

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
