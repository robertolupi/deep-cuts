<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { ui } from "$lib/stores/ui.svelte";
  import { player } from "$lib/stores/player.svelte";
  import SettingsCard from "./SettingsCard.svelte";

  interface Props {}
  let {}: Props = $props();

  let sidecarEnabled = $state(false);

  export async function loadSidecarSetting() {
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

  async function openLogDir() {
    try {
      await invoke("open_log_dir");
      ui.showToast("Log directory opened.", "success");
    } catch (err: any) {
      ui.showToast(err.toString(), "error");
    }
  }
</script>

<SettingsCard
  title="Analysis Settings"
  subtitle="Control how analysis results are stored"
>
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

  <div class="action-section">
    <button class="sg-btn action-btn" onclick={openLogDir}>
      <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
        <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/>
      </svg>
      Open Application Logs
    </button>
  </div>
</SettingsCard>

<style>
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

  .action-section {
    border-top: 1px solid rgba(255,255,255,0.06);
    padding-top: 0.85rem;
    margin-top: 0.25rem;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

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

  .action-btn {
    width: 100%;
    justify-content: center;
  }
</style>
