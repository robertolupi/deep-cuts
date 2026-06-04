<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { library } from "$lib/stores/library.svelte";
  import { ui } from "$lib/stores/ui.svelte";
  import SettingsCard from "./SettingsCard.svelte";

  let name = $state("");
  let path = $state("");
  let isAddLoading = $state(false);

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
</script>

<SettingsCard title="Add Library Folder" subtitle="MP3 · WAV · FLAC · M4A · AIFF · OGG · OPUS">
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
</SettingsCard>

<style>
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
</style>
