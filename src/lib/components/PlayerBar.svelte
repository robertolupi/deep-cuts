<script lang="ts">
  import { onMount, onDestroy } from "svelte";
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

  onMount(() => {
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

  function getOS(): "mac" | "win" | "other" {
    if (typeof navigator === "undefined") return "other";
    const ua = navigator.userAgent.toLowerCase();
    if (ua.includes("mac")) return "mac";
    if (ua.includes("win")) return "win";
    return "other";
  }
  const os = getOS();
</script>

<footer class="player-bar" class:spectrogram-open={showSpectrogram}>
  <!-- Left: track info -->
  <div class="player-left">
    <div class="vinyl-thumb" class:spinning={isPlaying}>
      <img src="/deep_cuts_transparent.png" alt="Now playing" />
    </div>
    {#if selectedTrack}
      <div class="track-info">
        <span class="track-name">{selectedTrack.title || selectedTrack.filename}</span>
        <span class="track-artist">{selectedTrack.artist || '—'}</span>
      </div>
      <button
        class="icon-btn"
        title="Find Similar on Map"
        onclick={() => ui.focusMapTrack(selectedTrack.id)}
      >
        <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <circle cx="11" cy="11" r="8"/>
          <line x1="21" y1="21" x2="16.65" y2="16.65"/>
          <line x1="11" y1="8" x2="11" y2="14"/>
          <line x1="8" y1="11" x2="14" y2="11"/>
        </svg>
      </button>
    {:else}
      <div class="track-info idle">
        <span class="track-name">No track selected</span>
        <span class="track-artist">Select a track to begin</span>
      </div>
    {/if}
  </div>

  <!-- Center: waveform + controls -->
  <div class="player-center">
    <div class="transport-row">
      <button class="icon-btn" title="Previous" onclick={() => player.handlePrevTrack()}>
        <svg xmlns="http://www.w3.org/2000/svg" width="15" height="15" viewBox="0 0 24 24" fill="currentColor">
          <polygon points="19 20 9 12 19 4 19 20"/>
          <rect x="5" y="4" width="2" height="16"/>
        </svg>
      </button>

      <button
        class="play-btn"
        class:playing={isPlaying}
        onclick={() => player.togglePlayback()}
        disabled={!selectedTrack}
        title={isPlaying ? "Pause" : "Play"}
      >
        {#if isPlaying}
          <svg xmlns="http://www.w3.org/2000/svg" width="18" height="18" viewBox="0 0 24 24" fill="currentColor">
            <rect x="6" y="4" width="4" height="16" rx="1"/>
            <rect x="14" y="4" width="4" height="16" rx="1"/>
          </svg>
        {:else}
          <svg xmlns="http://www.w3.org/2000/svg" width="18" height="18" viewBox="0 0 24 24" fill="currentColor">
            <polygon points="6 4 20 12 6 20 6 4"/>
          </svg>
        {/if}
      </button>

      <button class="icon-btn" title="Next" onclick={() => player.handleNextTrack()}>
        <svg xmlns="http://www.w3.org/2000/svg" width="15" height="15" viewBox="0 0 24 24" fill="currentColor">
          <polygon points="5 4 15 12 5 20 5 4"/>
          <rect x="17" y="4" width="2" height="16"/>
        </svg>
      </button>
    </div>

    <div class="waveform-row">
      <span class="time-mono">{formatDuration(currentTime)}</span>

      <div class="waveform-well">
        <div bind:this={waveformContainer} class="waveform-slot"></div>
        <div bind:this={spectrogramContainer} class="spectrogram-slot" class:hidden={!showSpectrogram}></div>
      </div>

      <span class="time-mono">{formatDuration(duration)}</span>
    </div>
  </div>

  <!-- Right: utilities -->
  <div class="player-right">
    {#if selectedTrack}
      <button
        class="icon-btn"
        title={os === "mac" ? "Reveal in Finder" : os === "win" ? "Show in Explorer" : "Show in Files"}
        onclick={() => selectedTrack && player.revealInFinder(selectedTrack.path)}
      >
        <svg xmlns="http://www.w3.org/2000/svg" width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/>
        </svg>
      </button>
    {/if}

    <button
      class="icon-btn"
      class:active={showSpectrogram}
      title="Toggle Spectrogram"
      onclick={toggleSpectrogram}
    >
      <svg xmlns="http://www.w3.org/2000/svg" width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <rect x="3" y="3" width="18" height="18" rx="2"/>
        <path d="M3 9h18M3 15h18M9 3v18M15 3v18"/>
      </svg>
    </button>
  </div>
</footer>

<style>
  .player-bar {
    display: grid;
    grid-template-columns: 25% 1fr 25%;
    align-items: center;
    height: var(--sg-player-bar-height);
    background: var(--sg-waveform-bg);
    border-top: 1px solid rgba(255, 255, 255, 0.08);
    padding: 0 var(--sg-spacing-md, 16px);
    transition: height 0.25s cubic-bezier(0.34, 1.56, 0.64, 1);
    overflow: hidden;
    flex-shrink: 0;
  }

  .player-bar.spectrogram-open {
    height: var(--sg-player-bar-expanded);
  }

  /* ── Left ── */
  .player-left {
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 0;
  }

  .vinyl-thumb {
    width: 40px;
    height: 40px;
    border-radius: 50%;
    overflow: hidden;
    flex-shrink: 0;
    background: var(--sg-surface-high, #292a2f);
  }

  .vinyl-thumb img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .vinyl-thumb.spinning img {
    animation: spin 4s linear infinite;
  }

  @keyframes spin {
    from { transform: rotate(0deg); }
    to   { transform: rotate(360deg); }
  }

  .track-info {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .track-info.idle {
    opacity: 0.4;
  }

  .track-name {
    font-family: "JetBrains Mono", monospace;
    font-size: 12px;
    font-weight: 600;
    color: var(--sg-on-surface, #e3e1e9);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 180px;
  }

  .track-artist {
    font-family: "JetBrains Mono", monospace;
    font-size: 10px;
    color: var(--sg-outline, #849495);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 180px;
  }

  /* ── Center ── */
  .player-center {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    width: 100%;
  }

  .transport-row {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .play-btn {
    width: 36px;
    height: 36px;
    border-radius: 50%;
    background: var(--sg-primary, #00f0ff);
    color: var(--sg-on-primary, #003a3f);
    border: none;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: transform 0.15s ease, box-shadow 0.15s ease;
  }

  .play-btn:hover:not(:disabled) {
    transform: scale(1.08);
    box-shadow: 0 0 12px rgba(0, 240, 255, 0.5);
  }

  .play-btn:disabled {
    opacity: 0.35;
    cursor: not-allowed;
  }

  .play-btn.playing {
    background: var(--sg-secondary, #fe00fe);
    color: #fff;
  }

  .waveform-row {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
  }

  .time-mono {
    font-family: "JetBrains Mono", monospace;
    font-size: 10px;
    color: var(--sg-outline, #849495);
    white-space: nowrap;
    flex-shrink: 0;
    min-width: 36px;
  }

  .waveform-well {
    flex: 1;
    background: var(--sg-waveform-bg, #0d1117);
    border-radius: 2px;
    overflow: hidden;
    display: flex;
    flex-direction: column;
  }

  .waveform-slot {
    height: 48px;
  }

  .spectrogram-slot {
    height: 48px;
    transition: height 0.25s ease;
  }

  .spectrogram-slot.hidden {
    height: 0;
    overflow: hidden;
  }

  /* ── Right ── */
  .player-right {
    display: flex;
    align-items: center;
    gap: 8px;
    justify-content: flex-end;
  }

  /* ── Shared: icon buttons ── */
  .icon-btn {
    background: none;
    border: none;
    cursor: pointer;
    color: var(--sg-outline, #849495);
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 6px;
    border-radius: 4px;
    transition: color 0.15s ease, background 0.15s ease;
  }

  .icon-btn:hover {
    color: var(--sg-on-surface, #e3e1e9);
    background: rgba(255, 255, 255, 0.05);
  }

  .icon-btn.active {
    color: var(--sg-primary, #00f0ff);
  }
</style>
