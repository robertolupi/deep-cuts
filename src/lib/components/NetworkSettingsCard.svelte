<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { ui } from "$lib/stores/ui.svelte";
  import SettingsCard from "./SettingsCard.svelte";

  let checkUpdatesEnabled = $state(true);
  let acoustidEnabled = $state(true);

  export async function loadSettings() {
    try {
      checkUpdatesEnabled = await invoke<boolean>("get_update_settings");
    } catch (e) {
      console.error("Failed to load update settings:", e);
    }
    try {
      const mode = await invoke<string>("get_acoustid_setting");
      acoustidEnabled = mode === "silent";
    } catch (e) {
      console.error("Failed to load AcoustID settings:", e);
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

  export async function toggleAcoustidSettings(enabled: boolean) {
    try {
      const mode = enabled ? "silent" : "never";
      await invoke("save_acoustid_setting", { value: mode });
      acoustidEnabled = enabled;
      ui.showToast(`MusicBrainz metadata enrichment ${enabled ? "enabled (silent)" : "disabled"}.`, "success");
    } catch (err: any) {
      ui.showToast(err.toString(), "error");
    }
  }
</script>

<SettingsCard
  title="Network Settings"
  subtitle="Control network access and online metadata fetches"
>
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
</style>
