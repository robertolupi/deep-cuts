<script lang="ts">
  import { ui } from "$lib/stores/ui.svelte";
</script>

<div class="hero-panel-content">
  <div class="hero-left">
    <h1 class="hero-title">Your Audio, Structured.</h1>
    <p class="hero-description" style="margin-top: 0rem; font-size: 1rem; line-height: 1.5; color: var(--text-secondary);">
      Deep Cuts analyzes local audio collections, performing integrated EBU R128 loudness checks, Camelot key mapping, spectral onset BPM extraction, and offline machine-learning indexing.
    </p>

    <div class="hero-buttons">
      <button class="btn-primary" onclick={() => ui.activeView = 'settings'} style="font-size: 0.9rem; padding: 0.5rem 1.25rem;">
        Configure watched folders
      </button>
      <button class="btn-secondary" onclick={() => ui.activeView = 'settings'} style="font-size: 0.9rem; padding: 0.5rem 1.25rem;">
        Monitored Library
      </button>
    </div>
  </div>

  <div class="hero-right">
    <!-- Animated Crate-Digging Vinyl Waveform -->
    <div class="vinyl-display">
      <img 
        src="/deep_cuts_transparent.png" 
        alt="Deep Cuts Custom Icon" 
        class="vinyl-image" 
        style="width: 100px; height: 100px;"
      />
      <!-- Mock Audio Waveform bars -->
      <div class="waveform-animation" style="height: 40px; margin-top: 0.5rem;">
        {#each Array(14) as _, i}
          <div class="wave-bar" style="width: 4px; --height: {10 + Math.sin(i * 0.5) * 15 + Math.random() * 8}px; --delay: {i * 0.08}s"></div>
        {/each}
      </div>
    </div>
  </div>
</div>
