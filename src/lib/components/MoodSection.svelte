<script lang="ts">
  import { filters } from "$lib/stores/filters.svelte";
  import MoodRadar, { type MoodValues } from "./MoodRadar.svelte";

  let {} = $props();

  const emptyMood: MoodValues = {
    happy: null, sad: null, aggressive: null, relaxed: null,
    party: null, acoustic: null, electronic: null,
  };

  let thresholds = $derived<Partial<MoodValues>>({
    happy:      filters.moodHappyMin      > 0 ? filters.moodHappyMin      : undefined,
    sad:        filters.moodSadMin        > 0 ? filters.moodSadMin        : undefined,
    aggressive: filters.moodAggressiveMin > 0 ? filters.moodAggressiveMin : undefined,
    relaxed:    filters.moodRelaxedMin    > 0 ? filters.moodRelaxedMin    : undefined,
    party:      filters.moodPartyMin      > 0 ? filters.moodPartyMin      : undefined,
    acoustic:   filters.moodAcousticMin   > 0 ? filters.moodAcousticMin   : undefined,
    electronic: filters.moodElectronicMin > 0 ? filters.moodElectronicMin : undefined,
  });

  function handleAxisClick(key: keyof MoodValues, value: number | null) {
    const minKey = `mood${key.charAt(0).toUpperCase()}${key.slice(1)}Min` as keyof typeof filters;
    const maxKey = `mood${key.charAt(0).toUpperCase()}${key.slice(1)}Max` as keyof typeof filters;
    (filters as any)[minKey] = value ?? 0;
    (filters as any)[maxKey] = 1;
  }

  function handleClear() {
    const keys: (keyof MoodValues)[] = ['happy', 'sad', 'aggressive', 'relaxed', 'party', 'acoustic', 'electronic'];
    for (const key of keys) {
      const minKey = `mood${key.charAt(0).toUpperCase()}${key.slice(1)}Min` as keyof typeof filters;
      const maxKey = `mood${key.charAt(0).toUpperCase()}${key.slice(1)}Max` as keyof typeof filters;
      (filters as any)[minKey] = 0;
      (filters as any)[maxKey] = 1;
    }
  }
</script>

<div class="sidebar-section">
  <span class="section-label">MOOD</span>
  <div class="mood-radar-wrap">
    <MoodRadar
      moodA={emptyMood}
      interactive={true}
      {thresholds}
      onAxisClick={handleAxisClick}
      onClear={handleClear}
    />
  </div>
</div>

<style>
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
</style>
