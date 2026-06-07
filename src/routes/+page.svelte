<script lang="ts">
  import Navbar from "$lib/components/Navbar.svelte";
  import TrackList from "$lib/components/TrackList.svelte";
  import MusicMap from "$lib/components/MusicMap.svelte";
  import LibrarySettings from "$lib/components/LibrarySettings.svelte";
  import AnalysisPanel from "$lib/components/AnalysisPanel.svelte";
  import DuplicatesPanel from "$lib/components/DuplicatesPanel.svelte";
  import ChatPanel from "$lib/components/ChatPanel.svelte";
  import StatisticsPanel from "$lib/components/StatisticsPanel.svelte";
  import { player } from "$lib/stores/player.svelte";
  import { ui } from "$lib/stores/ui.svelte";
  import { onMount } from "svelte";
  import { invoke } from "$lib/ipc";
  import { openUrl } from "@tauri-apps/plugin-opener";

  const selectedTrack = $derived(player.selectedTrack);

  let showUpdateBanner = $state(false);
  let latestAppVersion = $state("");

  async function checkAppUpdates() {
    try {
      const response = await invoke<{ manifest: any, update_available: boolean }>("fetch_app_manifest");
      if (response && response.update_available) {
        latestAppVersion = response.manifest.min_app_version;
        showUpdateBanner = true;
      }
    } catch (e) {
      console.error("Failed to check app updates:", e);
    }
  }

  async function downloadUpdate() {
    try {
      await openUrl("https://github.com/robertolupi/deep-cuts/releases");
    } catch (e) {
      console.error("Failed to open update URL:", e);
    }
  }

  function dismissUpdateMaybeLater() {
    showUpdateBanner = false;
  }

  async function disableUpdateChecking() {
    try {
      await invoke("set_update_settings", { enabled: false });
      showUpdateBanner = false;
      ui.showToast("Update checking disabled. Re-enable in Library Settings.", "success");
    } catch (e: any) {
      console.error("Failed to disable update settings:", e);
    }
  }

  onMount(() => {
    checkAppUpdates();
  });
</script>

<div class="app-layout">
  <Navbar />

  <!-- Global Update banner -->
  {#if showUpdateBanner}
    <div class="update-banner-card global-update-banner">
      <div class="warning-title-row">
        <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="var(--sg-primary, #00f0ff)" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
          <circle cx="12" cy="12" r="10"/><polyline points="12 8 12 12 16 14"/>
        </svg>
        <span class="warning-title-text" style="color: var(--sg-primary, #00f0ff)">New update available (v{latestAppVersion})</span>
        <button class="update-close" onclick={dismissUpdateMaybeLater}>×</button>
      </div>
      <p class="update-desc-text">
        An update with performance improvements, new features, and upgraded model support is ready. Update now to ensure compatibility.
      </p>
      <div class="update-actions">
        <button class="action-btn action-btn-primary" onclick={downloadUpdate}>
          <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
            <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/>
            <polyline points="7 10 12 15 17 10"/>
            <line x1="12" y1="15" x2="12" y2="3"/>
          </svg>
          Download
        </button>
        <button class="action-btn" onclick={dismissUpdateMaybeLater}>Maybe Later</button>
        <button class="action-btn action-btn-danger" onclick={disableUpdateChecking}>Do Not Ask Again</button>
      </div>
    </div>
  {/if}

  <main class="workspace">
    {#if ui.activeView === 'table' || ui.activeView === 'map'}
      <div class="table-view-layout">
        {#if ui.activeView === 'table'}
          <TrackList
            {selectedTrack}
            isPlaying={player.isPlaying}
            onTrackSelect={(t) => player.playTrack(t)}
          />
        {:else}
          <MusicMap />
        {/if}
      </div>

    {:else if ui.activeView === 'duplicates'}
      <DuplicatesPanel />

    {:else if ui.activeView === 'analysis'}
      <AnalysisPanel />

    {:else if ui.activeView === 'settings'}
      <LibrarySettings />

    {:else if ui.activeView === 'chat'}
      <ChatPanel />

    {:else if ui.activeView === 'statistics'}
      <StatisticsPanel />
    {/if}
  </main>
</div>

<style>
  /* ── Update Banner Card ── */
  .update-banner-card {
    padding: 1rem;
    background: color-mix(in srgb, var(--sg-primary) 5%, transparent);
    border: 1px solid color-mix(in srgb, var(--sg-primary) 20%, transparent);
    border-left: 3px solid var(--sg-primary, #00f0ff);
    border-radius: 6px;
    display: flex;
    flex-direction: column;
    gap: 0.65rem;
    flex-shrink: 0;
  }

  .warning-title-row {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .warning-title-text {
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-sm);
    font-weight: 700;
    color: var(--sg-primary, #00f0ff);
    flex: 1;
  }

  .update-desc-text {
    margin: 0;
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-xs);
    color: var(--sg-on-surface, #e3e1e9);
    opacity: 0.85;
    line-height: 1.5;
  }

  .update-actions {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    flex-wrap: wrap;
    margin-top: 0.25rem;
  }

  .action-btn {
    display: flex;
    align-items: center;
    gap: 5px;
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-xs);
    font-weight: 700;
    padding: 5px 12px;
    border: 1px solid color-mix(in srgb, var(--sg-on-surface) 12%, transparent);
    border-radius: 4px;
    background: color-mix(in srgb, var(--sg-on-surface) 4%, transparent);
    color: var(--sg-outline, #849495);
    cursor: pointer;
    transition: all 0.12s;
  }

  .action-btn:hover:not(:disabled) {
    border-color: color-mix(in srgb, var(--sg-on-surface) 25%, transparent);
    color: var(--sg-on-surface);
    background: color-mix(in srgb, var(--sg-on-surface) 8%, transparent);
  }

  .action-btn-primary {
    border-color: color-mix(in srgb, var(--sg-primary) 35%, transparent);
    color: var(--sg-primary);
    background: color-mix(in srgb, var(--sg-primary) 8%, transparent);
  }

  .action-btn-primary:hover {
    background: color-mix(in srgb, var(--sg-primary) 14%, transparent) !important;
    border-color: var(--sg-primary) !important;
    color: var(--sg-primary) !important;
  }

  .action-btn-danger {
    border-color: color-mix(in srgb, var(--sg-error) 30%, transparent) !important;
    color: var(--sg-error) !important;
    background: color-mix(in srgb, var(--sg-error) 5%, transparent) !important;
  }

  .action-btn-danger:hover {
    background: color-mix(in srgb, var(--sg-error) 12%, transparent) !important;
    border-color: var(--sg-error) !important;
  }

  .update-close {
    background: none;
    border: none;
    color: var(--sg-outline, #849495);
    font-size: 16px;
    cursor: pointer;
    line-height: 1;
    padding: 0 2px;
    transition: color 0.12s;
  }

  .update-close:hover {
    color: var(--sg-on-surface, #e3e1e9);
  }
</style>
