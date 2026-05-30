<script lang="ts">
  import { onDestroy } from "svelte";
  import { player, formatDuration } from "$lib/stores/player.svelte";
  import { ui } from "$lib/stores/ui.svelte";

  let waveformContainer    = $state<HTMLDivElement | null>(null);
  let spectrogramContainer = $state<HTMLDivElement | null>(null);

  const SPECTROGRAM_KEY = "deep-cuts-spectrogram";
  let showSpectrogram = $state(
    typeof localStorage !== "undefined"
      ? localStorage.getItem(SPECTROGRAM_KEY) === "true"
      : false
  );

  function toggleSpectrogram() {
    showSpectrogram = !showSpectrogram;
    localStorage.setItem(SPECTROGRAM_KEY, String(showSpectrogram));
  }

  $effect(() => {
    if (waveformContainer && spectrogramContainer) {
      player.register(waveformContainer, spectrogramContainer);
    }
  });

  onDestroy(() => {
    player.unregister();
  });

  const selectedTrack = $derived(player.selectedTrack);
  const isPlaying     = $derived(player.isPlaying);
  const currentTime   = $derived(player.currentTime);
  const duration      = $derived(player.duration);

  function getRevealLabel(): string {
    if (typeof navigator !== "undefined") {
      const ua = navigator.userAgent.toLowerCase();
      if (ua.includes("mac")) return "Reveal in Finder";
      if (ua.includes("win")) return "Show in Explorer";
    }
    return "Show in Files";
  }
  const revealLabel = getRevealLabel();
</script>

<footer class="player-bar" class:expanded={showSpectrogram}>
  {#if selectedTrack}
  <div class="player-main-col">
    <div class="waveform-outer">
      <div bind:this={waveformContainer} class="waveform-canvas-wrap"></div>
    </div>
    <div class="spectrogram-outer">
      <div bind:this={spectrogramContainer} class="spectrogram-canvas-wrap"></div>
    </div>

    <div class="playback-controls-row">
      <div style="display: flex; gap: 0.75rem; align-items: center;">
        <button class="player-btn" title="Previous Track" onclick={() => player.handlePrevTrack()}>
          <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="currentColor">
            <polygon points="19 20 9 12 19 4 19 20"/>
            <rect x="5" y="4" width="2" height="16"/>
          </svg>
        </button>
        <button class="btn-play-pause {isPlaying ? 'playing' : ''}" onclick={() => player.togglePlayback()}>
          {#if isPlaying}
            <svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="currentColor">
              <rect x="6" y="4" width="4" height="16" rx="1"/>
              <rect x="14" y="4" width="4" height="16" rx="1"/>
            </svg>
          {:else}
            <svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="currentColor">
              <polygon points="6 4 20 12 6 20 6 4"/>
            </svg>
          {/if}
        </button>
        <button class="player-btn" title="Next Track" onclick={() => player.handleNextTrack()}>
          <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="currentColor">
            <polygon points="5 4 15 12 5 20 5 4"/>
            <rect x="17" y="4" width="2" height="16"/>
          </svg>
        </button>
      </div>

      <div class="time-readout">
        <span class="current-time">{formatDuration(currentTime)}</span>
        <span class="divider">/</span>
        <span class="total-duration">{formatDuration(duration)}</span>
      </div>

      <div style="display: flex; gap: 0.75rem; align-items: center;">
        <button
          class="btn-secondary"
          onclick={() => ui.focusMapTrack(selectedTrack.id)}
          style="font-size: 0.75rem; padding: 0.35rem 0.8rem; border-radius: var(--sg-radius-sm); display: flex; align-items: center; gap: 0.3rem;"
          title="Find similar tracks on the Music Map"
        >
          <svg xmlns="http://www.w3.org/2000/svg" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" style="vertical-align: middle;">
            <circle cx="11" cy="11" r="8"/>
            <line x1="21" y1="21" x2="16.65" y2="16.65"/>
            <line x1="11" y1="8" x2="11" y2="14"/>
            <line x1="8" y1="11" x2="14" y2="11"/>
          </svg>
          <span style="vertical-align: middle;">Similar</span>
        </button>

        <button
          class="btn-secondary"
          onclick={() => selectedTrack && player.revealInFinder(selectedTrack.path)}
          style="font-size: 0.75rem; padding: 0.35rem 0.8rem; border-radius: var(--sg-radius-sm); display: flex; align-items: center; gap: 0.3rem;"
          title={revealLabel}
        >
          <svg xmlns="http://www.w3.org/2000/svg" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" style="vertical-align: middle;">
            <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/>
          </svg>
          <span style="vertical-align: middle;">{revealLabel}</span>
        </button>

        <button
          class="btn-secondary {showSpectrogram ? 'pulse-glow-cyan' : ''}"
          onclick={toggleSpectrogram}
          style="font-size: 0.75rem; padding: 0.35rem 0.8rem; border-radius: var(--sg-radius-sm); display: flex; align-items: center; gap: 0.3rem;"
          title="Toggle Spectrogram"
        >
          <svg xmlns="http://www.w3.org/2000/svg" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" style="vertical-align: middle;">
            <rect x="3" y="3" width="18" height="18" rx="2"/>
            <path d="M3 9h18M3 15h18M9 3v18M15 3v18"/>
          </svg>
          <span style="vertical-align: middle;">Spectrogram</span>
        </button>
      </div>
    </div>
  </div>
  {:else}
  <div class="player-idle">
    <span class="idle-label">No track selected</span>
  </div>
  {/if}
</footer>

<style>
  .player-bar {
    flex-shrink: 0;
    background: var(--bg-player, var(--sg-waveform-bg, #0d1117));
    border-top: 1px solid var(--sg-surface-high);
  }

  /* Full-width when no left column */
  :global(.player-main-col) {
    flex: 1;
    min-width: 0;
  }

  .player-idle {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    padding: 0.75rem 1.5rem;
    opacity: 0.45;
  }

  .idle-label {
    font-size: 0.85rem;
    color: var(--sg-outline);
  }

  /* Hide spectrogram section when toggled off */
  .player-bar:not(.expanded) :global(.spectrogram-outer) {
    display: none;
  }
</style>
