<script lang="ts">
  import { invoke } from "$lib/ipc";
  import SettingsCard from "./SettingsCard.svelte";

  interface Props {
    onOpenModelDownloader: () => void;
  }
  let { onOpenModelDownloader }: Props = $props();

  let configuredModelPath = $state<string | null>(null);
  let modelPathMessage = $state("");

  export async function loadModelPathSetting() {
    try {
      configuredModelPath = await invoke<string | null>("get_model_path_setting");
    } catch (e) {
      console.error("Failed to load model path setting:", e);
    }
  }

  async function chooseModelPath() {
    modelPathMessage = "";
    try {
      const selected = await invoke<string | null>("select_directory");
      if (!selected) return;
      await invoke("save_model_path_setting", { path: selected });
      configuredModelPath = selected;
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
</script>

<SettingsCard
  title="Model Folder"
  subtitle="Location of neural network model files (requires ~6.3 GB of disk space)"
>
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
    <button class="sg-btn sg-btn-primary" onclick={onOpenModelDownloader}>
      <svg xmlns="http://www.w3.org/2000/svg" width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
        <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/>
      </svg>
      Manage Models
    </button>
  </div>

  {#if modelPathMessage}
    <div class="model-path-message">{modelPathMessage}</div>
  {/if}
</SettingsCard>

<style>
  .field-group {
    display: flex;
    flex-direction: column;
    gap: 5px;
  }

  .model-path-value {
    min-height: 24px;
    padding: 7px 10px;
    border: 1px solid rgba(255,255,255,0.08);
    border-radius: 4px;
    background: rgba(0,0,0,0.22);
    color: var(--sg-on-surface, #e3e1e9);
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-xs);
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
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-2xs);
    color: var(--sg-primary, #00f0ff);
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
</style>
