<script lang="ts">
  import { filters } from "$lib/stores/filters.svelte";
  import MoodRadar, { type MoodValues } from "./MoodRadar.svelte";

  let {} = $props();

  const MOOD_KEYS: (keyof MoodValues)[] = ['happy', 'sad', 'aggressive', 'relaxed', 'party', 'acoustic', 'electronic'];

  function moodMinKey(key: keyof MoodValues) {
    return `mood${key.charAt(0).toUpperCase()}${key.slice(1)}Min` as keyof typeof filters;
  }
  function moodMaxKey(key: keyof MoodValues) {
    return `mood${key.charAt(0).toUpperCase()}${key.slice(1)}Max` as keyof typeof filters;
  }

  // A mood axis is "active" if its range is narrower than [0, 1]
  function isAxisActive(key: keyof MoodValues): boolean {
    return (filters as any)[moodMinKey(key)] > 0 || (filters as any)[moodMaxKey(key)] < 1;
  }

  // Center = midpoint of the active range
  function axisCenter(key: keyof MoodValues): number {
    const lo = (filters as any)[moodMinKey(key)] as number;
    const hi = (filters as any)[moodMaxKey(key)] as number;
    return (lo + hi) / 2;
  }

  const thresholds = $derived<Partial<MoodValues>>(
    Object.fromEntries(
      MOOD_KEYS
        .filter(k => isAxisActive(k))
        .map(k => [k, axisCenter(k)])
    )
  );

  const emptyMood: MoodValues = {
    happy: null, sad: null, aggressive: null, relaxed: null,
    party: null, acoustic: null, electronic: null,
  };

  function handleAxisClick(key: keyof MoodValues, value: number | null) {
    const tol = filters.moodTolerance;
    const clamp = (v: number) => Math.max(0, Math.min(1, v));
    if (value == null) {
      (filters as any)[moodMinKey(key)] = 0;
      (filters as any)[moodMaxKey(key)] = 1;
    } else {
      (filters as any)[moodMinKey(key)] = clamp(value - tol);
      (filters as any)[moodMaxKey(key)] = clamp(value + tol);
    }
  }

  function handleClear() {
    for (const key of MOOD_KEYS) {
      (filters as any)[moodMinKey(key)] = 0;
      (filters as any)[moodMaxKey(key)] = 1;
    }
  }

  function applyTolerance(tol: number) {
    const clamp = (v: number) => Math.max(0, Math.min(1, v));
    for (const key of MOOD_KEYS) {
      if (!isAxisActive(key)) continue;
      const center = axisCenter(key);
      (filters as any)[moodMinKey(key)] = clamp(center - tol);
      (filters as any)[moodMaxKey(key)] = clamp(center + tol);
    }
  }
</script>

<div class="sidebar-section">
  <div class="section-header">
    <span class="section-label">MOOD</span>
    {#if Object.keys(thresholds).length > 0}
      <button class="clear-link" onclick={handleClear}>clear</button>
    {/if}
  </div>
  <div class="mood-radar-wrap">
    <MoodRadar
      moodA={emptyMood}
      interactive={true}
      {thresholds}
      tolerance={filters.moodTolerance}
      onAxisClick={handleAxisClick}
      onClear={handleClear}
    />
  </div>
  <div class="tolerance-row">
    <span class="tol-label">TOLERANCE</span>
    <input
      type="range"
      min="0.02"
      max="0.40"
      step="0.01"
      value={filters.moodTolerance}
      oninput={(e) => {
        const tol = parseFloat((e.target as HTMLInputElement).value);
        filters.moodTolerance = tol;
        applyTolerance(tol);
      }}
      class="tol-slider"
    />
    <span class="tol-value">±{Math.round(filters.moodTolerance * 100)}%</span>
  </div>
</div>

<style>
  .section-header {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    margin-bottom: 0.5rem;
  }

  .section-header .section-label {
    margin-bottom: 0;
  }

  .clear-link {
    background: none;
    border: none;
    padding: 0;
    cursor: pointer;
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-3xs);
    font-weight: 700;
    letter-spacing: 0.05em;
    color: var(--sg-outline, #849495);
  }

  .clear-link:hover {
    color: var(--sg-error);
  }

  .section-label {
    display: block;
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-2xs);
    font-weight: 700;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--sg-outline, #849495);
    margin-bottom: 0.5rem;
  }

  .mood-radar-wrap {
    width: 100%;
    aspect-ratio: 1;
  }

  .tolerance-row {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-top: 6px;
  }

  .tol-label {
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-3xs);
    font-weight: 700;
    letter-spacing: 0.08em;
    color: var(--sg-outline, #849495);
    flex-shrink: 0;
  }

  .tol-slider {
    -webkit-appearance: none;
    appearance: none;
    flex: 1;
    height: 3px;
    border-radius: 2px;
    background: color-mix(in srgb, var(--sg-on-surface) 12%, transparent);
    outline: none;
    cursor: pointer;
  }

  .tol-slider::-webkit-slider-thumb {
    -webkit-appearance: none;
    appearance: none;
    width: 11px;
    height: 11px;
    border-radius: 50%;
    background: #bc13fe; /* TODO: map to --sg-* token */
    cursor: pointer;
    box-shadow: 0 0 4px rgba(188,19,254,0.5); /* TODO: map to --sg-* token */
  }

  .tol-slider::-moz-range-thumb {
    width: 11px;
    height: 11px;
    border-radius: 50%;
    border: none;
    background: #bc13fe; /* TODO: map to --sg-* token */
    cursor: pointer;
  }

  .tol-slider::-moz-range-track {
    height: 3px;
    border-radius: 2px;
    background: color-mix(in srgb, var(--sg-on-surface) 12%, transparent);
  }

  .tol-value {
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-3xs);
    color: #bc13fe; /* TODO: map to --sg-* token */
    width: 32px;
    text-align: right;
    flex-shrink: 0;
  }
</style>
