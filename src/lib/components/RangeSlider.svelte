<script lang="ts">
  let {
    min = 0,
    max = 100,
    step = 1,
    minValue = $bindable(min),
    maxValue = $bindable(max),
    unit = '',
    formatValue = (v: number) => String(v)
  }: {
    min?: number;
    max?: number;
    step?: number;
    minValue?: number;
    maxValue?: number;
    unit?: string;
    formatValue?: (v: number) => string;
  } = $props();

  let minPct = $derived(((minValue - min) / (max - min)) * 100);
  let maxPct = $derived(((maxValue - min) / (max - min)) * 100);
  
  // Lift min thumb on top when handles overlap so user can separate them
  let minOnTop = $derived(minValue >= maxValue);

  function onMinInput(e: Event) {
    const val = Number((e.target as HTMLInputElement).value);
    minValue = Math.min(val, maxValue);
  }

  function onMaxInput(e: Event) {
    const val = Number((e.target as HTMLInputElement).value);
    maxValue = Math.max(val, minValue);
  }
</script>

<div class="dual-range-slider">
  <div class="track-wrap">
    <div class="track-bg"></div>
    <div
      class="track-fill"
      style="left: {minPct}%; right: {100 - maxPct}%"
    ></div>
    <input
      type="range"
      {min} {max} {step}
      value={minValue}
      oninput={onMinInput}
      class="range-input"
      style="z-index: {minOnTop ? 5 : 3}"
      aria-label="Minimum{unit ? ' ' + unit : ''}"
    />
    <input
      type="range"
      {min} {max} {step}
      value={maxValue}
      oninput={onMaxInput}
      class="range-input"
      style="z-index: {minOnTop ? 3 : 5}"
      aria-label="Maximum{unit ? ' ' + unit : ''}"
    />
  </div>
  <div class="range-labels">
    <span class="range-label">{formatValue(minValue)}{unit ? ' ' + unit : ''}</span>
    <span class="range-label">{formatValue(maxValue)}{unit ? ' ' + unit : ''}</span>
  </div>
</div>

<style>
  .dual-range-slider {
    display: flex;
    flex-direction: column;
    gap: 0.55rem;
    width: 100%;
    user-select: none;
  }

  .track-wrap {
    position: relative;
    height: 20px;
    display: flex;
    align-items: center;
  }

  .track-bg {
    position: absolute;
    left: 0;
    right: 0;
    height: 4px;
    border-radius: 2px;
    background: var(--sg-surface-high);
    opacity: 0.8;
    pointer-events: none;
  }

  .track-fill {
    position: absolute;
    height: 4px;
    border-radius: 2px;
    background: var(--sg-primary);
    box-shadow: 0 0 6px color-mix(in srgb, var(--sg-primary) 40%, transparent);
    pointer-events: none;
  }

  /* Shared base for both range inputs — they overlap on the same track */
  .range-input {
    position: absolute;
    left: 0;
    width: 100%;
    height: 100%;
    background: transparent;
    -webkit-appearance: none;
    appearance: none;
    outline: none;
    cursor: pointer;
    pointer-events: none; /* only the thumb captures events */
    margin: 0;
    padding: 0;
  }

  .range-input::-webkit-slider-runnable-track {
    background: transparent;
    height: 4px;
    border: none;
  }

  .range-input::-moz-range-track {
    background: transparent;
    height: 4px;
    border: none;
  }

  .range-input::-webkit-slider-thumb {
    -webkit-appearance: none;
    appearance: none;
    width: 14px;
    height: 14px;
    border-radius: 50%;
    background: var(--sg-primary);
    pointer-events: all;
    cursor: grab;
    box-shadow: 0 0 8px color-mix(in srgb, var(--sg-primary) 50%, transparent);
    border: 2px solid var(--sg-surface-container);
    transition: transform 0.1s, box-shadow 0.1s;
  }

  .range-input::-moz-range-thumb {
    width: 14px;
    height: 14px;
    border-radius: 50%;
    background: var(--sg-primary);
    pointer-events: all;
    cursor: grab;
    box-shadow: 0 0 8px color-mix(in srgb, var(--sg-primary) 50%, transparent);
    border: 2px solid var(--sg-surface-container);
    transition: transform 0.1s, box-shadow 0.1s;
  }

  .range-input::-webkit-slider-thumb:hover {
    transform: scale(1.25);
    box-shadow: 0 0 12px color-mix(in srgb, var(--sg-primary) 70%, transparent);
  }

  .range-input:active::-webkit-slider-thumb {
    cursor: grabbing;
    transform: scale(1.15);
  }

  .range-input:focus::-webkit-slider-thumb {
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--sg-primary) 20%, transparent), 0 0 8px color-mix(in srgb, var(--sg-primary) 50%, transparent);
  }

  .range-labels {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .range-label {
    font-size: 0.7rem;
    font-weight: 700;
    color: var(--sg-primary);
    font-family: 'Outfit', sans-serif;
    background: color-mix(in srgb, var(--sg-primary) 8%, transparent);
    border: 1px solid color-mix(in srgb, var(--sg-primary) 22%, transparent);
    padding: 0.1rem 0.42rem;
    border-radius: 3px;
    letter-spacing: 0.02em;
  }
</style>
