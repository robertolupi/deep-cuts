<script lang="ts">
  import { getVersion } from "@tauri-apps/api/app";
  import { library } from "$lib/stores/library.svelte";
  import ModelDownloader from "./ModelDownloader.svelte";
  import { onMount } from "svelte";
  import licenseText from "../../../LICENSE.md?raw";
  import AddFolderCard from "./AddFolderCard.svelte";
  import ModelPathCard from "./ModelPathCard.svelte";
  import NetworkSettingsCard from "./NetworkSettingsCard.svelte";
  import AnalysisSettingsCard from "./AnalysisSettingsCard.svelte";
  import WatchedFoldersCard from "./WatchedFoldersCard.svelte";
  import AboutCard from "./AboutCard.svelte";

  const directories = $derived(library.directories);
  const trackCount  = $derived(library.trackCount);

  let appVersion = $state("");
  let showModelDownloaderDrawer = $state(false);
  let showLicenseDrawer = $state(false);

  let modelPathCard: ModelPathCard;
  let networkCard: NetworkSettingsCard;
  let analysisCard: AnalysisSettingsCard;

  onMount(async () => {
    modelPathCard.loadModelPathSetting();
    networkCard.loadSettings();
    analysisCard.loadSidecarSetting();
    appVersion = await getVersion();
  });
</script>

<div class="settings-layout">

  <!-- Left column -->
  <div class="settings-left">

    <AddFolderCard />

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

    <ModelPathCard
      bind:this={modelPathCard}
      onOpenModelDownloader={() => { showModelDownloaderDrawer = true; }}
    />

    <NetworkSettingsCard bind:this={networkCard} />

    <AnalysisSettingsCard
      bind:this={analysisCard}
    />

  </div>

  <!-- Right column: folder list + about -->
  <div class="right-col">
    <WatchedFoldersCard />

    <AboutCard
      {appVersion}
      onOpenLicense={() => { showLicenseDrawer = true; }}
    />
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

  .right-col {
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }

  /* ── Stats card ── */
  .sg-card {
    background: var(--sg-surface-slate, #161b22);
    border: 1px solid rgba(255,255,255,0.07);
    border-radius: 6px;
    padding: 1rem;
    display: flex;
    flex-direction: column;
    gap: 0.85rem;
  }

  .card-title {
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-sm);
    font-weight: 700;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--sg-on-surface, #e3e1e9);
  }

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
    font-family: var(--sg-font-mono);
    font-size: 28px;
    font-weight: 700;
    color: var(--sg-on-surface, #e3e1e9);
    line-height: 1;
  }

  .stat-cyan { color: var(--sg-primary, #00f0ff); text-shadow: 0 0 12px rgba(0,240,255,0.3); }

  .stat-label {
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-2xs);
    color: var(--sg-outline, #849495);
    letter-spacing: 0.06em;
  }

  .stat-divider {
    width: 1px;
    height: 36px;
    background: rgba(255,255,255,0.08);
  }

  /* ── Drawer ── */
  .drawer-overlay {
    position: fixed;
    top: 0;
    left: 0;
    width: 100vw;
    height: 100vh;
    background: rgba(0, 0, 0, 0.6); /* TODO: map to --sg-* token */
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
    box-shadow: -10px 0 30px rgba(0, 0, 0, 0.5); /* TODO: map to --sg-* token */
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
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-sm);
    font-weight: 700;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--sg-on-surface, #e3e1e9);
    margin: 0;
  }

  .drawer-subtitle {
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-2xs);
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

  .license-text {
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-xs);
    line-height: 1.6;
    color: var(--sg-on-surface, #e3e1e9);
    white-space: pre-wrap;
    background: rgba(0, 0, 0, 0.22); /* TODO: map to --sg-* token */
    border: 1px solid var(--sg-glass-border);
    border-radius: 4px;
    padding: 1rem;
    margin: 0;
    text-align: left;
  }
</style>
