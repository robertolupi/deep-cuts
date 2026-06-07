<script lang="ts">
  import { invoke } from "$lib/ipc";
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
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-xs);
    color: var(--sg-on-surface, #e3e1e9);
  }

  .update-checkbox {
    appearance: none;
    width: 14px;
    height: 14px;
    border: 1px solid color-mix(in srgb, var(--sg-on-surface) 15%, transparent);
    border-radius: 3px;
    background: color-mix(in srgb, var(--sg-on-surface) 3%, transparent);
    cursor: pointer;
    display: grid;
    place-content: center;
    transition: all 0.12s ease;
    margin: 0;
  }

  .update-checkbox:hover {
    border-color: color-mix(in srgb, var(--sg-primary) 40%, transparent);
    background: color-mix(in srgb, var(--sg-primary) 3%, transparent);
  }

  .update-checkbox:checked {
    border-color: var(--sg-primary, #00f0ff);
    background: color-mix(in srgb, var(--sg-primary) 10%, transparent);
  }

  .update-checkbox:checked::before {
    content: "";
    width: 6px;
    height: 6px;
    background: var(--sg-primary, #00f0ff);
    border-radius: 1px;
    box-shadow: 0 0 4px color-mix(in srgb, var(--sg-primary) 50%, transparent);
  }

  .checkbox-text {
    line-height: 1;
    font-size: var(--sg-text-sm);
    color: var(--sg-outline, #849495);
    transition: color 0.12s;
  }

  .update-checkbox-label:hover .checkbox-text {
    color: var(--sg-on-surface, #e3e1e9);
  }
</style>
