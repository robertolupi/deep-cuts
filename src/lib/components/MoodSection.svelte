<script lang="ts">
  import { filters } from "$lib/stores/filters.svelte";
  import RangeSlider from "./RangeSlider.svelte";

  let { distributions }: {
    distributions: {
      happy: number[];
      sad: number[];
      aggressive: number[];
      relaxed: number[];
      party: number[];
      acoustic: number[];
      electronic: number[];
    }
  } = $props();

  let open = $state(false);
</script>

<div class="sidebar-section">
  <button class="section-label-row mood-toggle" onclick={() => open = !open}>
    <span class="section-label" style="margin-bottom:0;">MOOD</span>
    <span class="mood-chevron" class:open>▸</span>
  </button>
  {#if open}
    <div class="mood-sliders">
      {#snippet moodSlider(label: string)}
        <span class="mood-dim-label">{label}</span>
      {/snippet}
      <div class="mood-dim">{@render moodSlider('Happy')}
        <RangeSlider min={0} max={1} step={0.01} bind:minValue={filters.moodHappyMin}      bind:maxValue={filters.moodHappyMax}      distribution={distributions.happy}      formatValue={(v) => (v*100).toFixed(0)+'%'} />
      </div>
      <div class="mood-dim">{@render moodSlider('Sad')}
        <RangeSlider min={0} max={1} step={0.01} bind:minValue={filters.moodSadMin}        bind:maxValue={filters.moodSadMax}        distribution={distributions.sad}        formatValue={(v) => (v*100).toFixed(0)+'%'} />
      </div>
      <div class="mood-dim">{@render moodSlider('Aggressive')}
        <RangeSlider min={0} max={1} step={0.01} bind:minValue={filters.moodAggressiveMin} bind:maxValue={filters.moodAggressiveMax} distribution={distributions.aggressive} formatValue={(v) => (v*100).toFixed(0)+'%'} />
      </div>
      <div class="mood-dim">{@render moodSlider('Relaxed')}
        <RangeSlider min={0} max={1} step={0.01} bind:minValue={filters.moodRelaxedMin}    bind:maxValue={filters.moodRelaxedMax}    distribution={distributions.relaxed}    formatValue={(v) => (v*100).toFixed(0)+'%'} />
      </div>
      <div class="mood-dim">{@render moodSlider('Party')}
        <RangeSlider min={0} max={1} step={0.01} bind:minValue={filters.moodPartyMin}      bind:maxValue={filters.moodPartyMax}      distribution={distributions.party}      formatValue={(v) => (v*100).toFixed(0)+'%'} />
      </div>
      <div class="mood-dim">{@render moodSlider('Acoustic')}
        <RangeSlider min={0} max={1} step={0.01} bind:minValue={filters.moodAcousticMin}   bind:maxValue={filters.moodAcousticMax}   distribution={distributions.acoustic}   formatValue={(v) => (v*100).toFixed(0)+'%'} />
      </div>
      <div class="mood-dim">{@render moodSlider('Electronic')}
        <RangeSlider min={0} max={1} step={0.01} bind:minValue={filters.moodElectronicMin} bind:maxValue={filters.moodElectronicMax} distribution={distributions.electronic} formatValue={(v) => (v*100).toFixed(0)+'%'} />
      </div>
    </div>
  {/if}
</div>

<style>
  .mood-toggle {
    display: flex;
    width: 100%;
    background: none;
    border: none;
    cursor: pointer;
    padding: 0;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 0;
  }

  .mood-chevron {
    font-size: 9px;
    color: var(--sg-outline, #849495);
    transition: transform 0.15s;
    display: inline-block;
  }

  .mood-chevron.open {
    transform: rotate(90deg);
  }

  .mood-sliders {
    display: flex;
    flex-direction: column;
    gap: 0.65rem;
    margin-top: 0.55rem;
  }

  .mood-dim {
    display: flex;
    flex-direction: column;
    gap: 3px;
  }

  .section-label {
    display: block;
    font-family: "JetBrains Mono", monospace;
    font-size: 9px;
    font-weight: 700;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--sg-outline, #849495);
    margin-bottom: 0;
  }

  .mood-dim-label {
    font-family: "JetBrains Mono", monospace;
    font-size: 8px;
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--sg-outline, #849495);
  }
</style>
