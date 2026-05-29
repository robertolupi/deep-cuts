<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import type { Track } from '../types';

  let {
    selectedTrack,
    isPlaying = $bindable(),
    currentTime = $bindable(),
    duration = $bindable(),
    showDetails = $bindable(),
    toggleDetails,
    formatDuration,
    formatSize,
    waveformContainer = $bindable(),
    spectrogramContainer = $bindable(),
    togglePlayback,
    handlePrevTrack,
    handleNextTrack
  }: {
    selectedTrack: Track;
    isPlaying: boolean;
    currentTime: number;
    duration: number;
    showDetails: boolean;
    toggleDetails: () => void;
    formatDuration: (sec: number) => string;
    formatSize: (bytes: number) => string;
    waveformContainer: HTMLDivElement | null;
    spectrogramContainer: HTMLDivElement | null;
    togglePlayback: () => void;
    handlePrevTrack: () => void;
    handleNextTrack: () => void;
  } = $props();

  function getRevealLabel(): string {
    if (typeof navigator !== 'undefined') {
      const ua = navigator.userAgent.toLowerCase();
      if (ua.includes('mac')) return 'Reveal in Finder';
      if (ua.includes('win')) return 'Show in Explorer';
    }
    return 'Show in Files';
  }

  const revealLabel = getRevealLabel();

  async function handleReveal() {
    try {
      await invoke("reveal_in_finder", { path: selectedTrack.path });
    } catch (e: any) {
      console.error("Failed to reveal file in system explorer:", e);
    }
  }
</script>

<div class="audio-player-pane {showDetails ? 'expanded' : ''}">
  <div class="player-upper-row">
    <!-- Left side: Album cover vinyl & Track metadata -->
    <div class="player-left-col">
      <div class="vinyl-spinner-large {isPlaying ? 'spinning' : ''}">
        <img src="/deep_cuts_transparent.png" alt="Vinyl record center" class="vinyl-record-img" />
      </div>
      <div class="track-details-block">
        <div class="track-title-row">
          <span class="badge badge-cyan" style="font-size: 0.72rem; padding: 0.15rem 0.4rem;">{selectedTrack.path.split('.').pop()?.toUpperCase()}</span>
          <h4>{selectedTrack.title || selectedTrack.filename}</h4>
        </div>
        <p class="track-credits">
          {#if selectedTrack.artist}<span class="artist">{selectedTrack.artist}</span>{/if}
          {#if selectedTrack.artist && selectedTrack.album}<span class="sep">—</span>{/if}
          {#if selectedTrack.album}<span class="album">{selectedTrack.album}</span>{/if}
        </p>
        <p class="track-tech-specs">
          {#if selectedTrack.sample_rate}{Math.round(selectedTrack.sample_rate / 1000)} kHz • {/if}
          {#if selectedTrack.bit_depth}{selectedTrack.bit_depth}-bit • {/if}
          {#if selectedTrack.bitrate}{selectedTrack.bitrate} kbps • {/if}
          {formatSize(selectedTrack.size_bytes)}
        </p>
      </div>
    </div>

    <!-- Center/Main: WaveSurfer, Spectrogram & Playback controls -->
    <div class="player-main-col">
      <!-- WaveSurfer wave wrapper -->
      <div class="waveform-outer">
        <div bind:this={waveformContainer} class="waveform-canvas-wrap"></div>
      </div>
      
      <!-- Spectrogram wrapper -->
      <div class="spectrogram-outer">
        <div bind:this={spectrogramContainer} class="spectrogram-canvas-wrap"></div>
      </div>

      <!-- Playback controls -->
      <div class="playback-controls-row">
        <div style="display: flex; gap: 0.75rem; align-items: center;">
          <!-- Skip back -->
          <button class="player-btn" title="Previous Track" onclick={handlePrevTrack}>
            <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="currentColor">
              <polygon points="19 20 9 12 19 4 19 20"/>
              <rect x="5" y="4" width="2" height="16"/>
            </svg>
          </button>
          <!-- Play/Pause -->
          <button class="btn-play-pause {isPlaying ? 'playing' : ''}" onclick={togglePlayback}>
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
          <!-- Skip forward -->
          <button class="player-btn" title="Next Track" onclick={handleNextTrack}>
            <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="currentColor">
              <polygon points="5 4 15 12 5 20 5 4"/>
              <rect x="17" y="4" width="2" height="16"/>
            </svg>
          </button>
        </div>

        <!-- Time counter -->
        <div class="time-readout">
          <span class="current-time">{formatDuration(currentTime)}</span>
          <span class="divider">/</span>
          <span class="total-duration">{formatDuration(duration)}</span>
        </div>

        <div style="display: flex; gap: 0.75rem; align-items: center;">
          <!-- Reveal in system file explorer -->
          <button 
            class="btn-secondary" 
            onclick={handleReveal} 
            style="font-size: 0.75rem; padding: 0.35rem 0.8rem; border-radius: var(--radius-sm); display: flex; align-items: center; gap: 0.3rem;"
            title={revealLabel}
          >
            <svg xmlns="http://www.w3.org/2000/svg" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" style="vertical-align: middle;">
              <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/>
            </svg>
            <span style="vertical-align: middle;">{revealLabel}</span>
          </button>

          <!-- Details Toggle button -->
          <button 
            class="btn-secondary {showDetails ? 'pulse-glow-cyan' : ''}" 
            onclick={toggleDetails} 
            style="font-size: 0.75rem; padding: 0.35rem 0.8rem; border-radius: var(--radius-sm); display: flex; align-items: center; gap: 0.3rem;"
            title="Toggle Multi-column Metadata Details"
          >
            <svg xmlns="http://www.w3.org/2000/svg" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" style="vertical-align: middle;">
              <circle cx="12" cy="12" r="10"/>
              <line x1="12" y1="16" x2="12" y2="12"/>
              <line x1="12" y1="8" x2="12.01" y2="8"/>
            </svg>
            <span style="vertical-align: middle;">{showDetails ? 'Hide Details' : 'Details'}</span>
          </button>
        </div>
      </div>
    </div>
  </div>

  <!-- Expanded Metadata Multicolumn Grid -->
  {#if showDetails}
    <div class="player-details-row">
      <div class="metadata-grid">
        <!-- Column 1: Track Details -->
        <div class="metadata-col">
          <div class="metadata-card">
            <span class="metadata-label">Title</span>
            <span class="metadata-value" title={selectedTrack.title || selectedTrack.filename}>{selectedTrack.title || '—'}</span>
          </div>
          <div class="metadata-card" style="margin-top: 0.75rem;">
            <span class="metadata-label">Artist</span>
            <span class="metadata-value" title={selectedTrack.artist || '—'}>{selectedTrack.artist || '—'}</span>
          </div>
          <div class="metadata-card" style="margin-top: 0.75rem;">
            <span class="metadata-label">Album</span>
            <span class="metadata-value" title={selectedTrack.album || '—'}>{selectedTrack.album || '—'}</span>
          </div>
        </div>

        <!-- Column 2: Credits & Style -->
        <div class="metadata-col">
          <div class="metadata-card">
            <span class="metadata-label">Album Artist</span>
            <span class="metadata-value" title={selectedTrack.album_artist || '—'}>{selectedTrack.album_artist || '—'}</span>
          </div>
          <div class="metadata-card" style="margin-top: 0.75rem;">
            <span class="metadata-label">Composer</span>
            <span class="metadata-value" title={selectedTrack.composer || '—'}>{selectedTrack.composer || '—'}</span>
          </div>
          <div class="metadata-card" style="margin-top: 0.75rem;">
            <span class="metadata-label">Genre</span>
            <span class="metadata-value" title={selectedTrack.genre || '—'}>{selectedTrack.genre || '—'}</span>
          </div>
        </div>

        <!-- Column 3: Tech Specs -->
        <div class="metadata-col">
          <div class="metadata-card">
            <span class="metadata-label">Technical Specs</span>
            <span class="metadata-value">
              {#if selectedTrack.sample_rate}<code>{Math.round(selectedTrack.sample_rate / 1000)} kHz</code>{/if}
              {#if selectedTrack.bit_depth}<code> • {selectedTrack.bit_depth}-bit</code>{/if}
              {#if selectedTrack.bitrate}<code> • {selectedTrack.bitrate}k</code>{/if}
            </span>
          </div>
          <div class="metadata-card" style="margin-top: 0.75rem;">
            <span class="metadata-label">Format / Channels</span>
            <span class="metadata-value">
              <code>{selectedTrack.path.split('.').pop()?.toUpperCase()}</code>
              {#if selectedTrack.channels} • <code>{selectedTrack.channels} ch</code>{/if}
            </span>
          </div>
          <div class="metadata-card" style="margin-top: 0.75rem;">
            <span class="metadata-label">Year / BPM</span>
            <span class="metadata-value">
              {selectedTrack.year || '—'}
              {#if selectedTrack.bpm} • <code>{selectedTrack.bpm} BPM</code>{/if}
            </span>
          </div>
        </div>

        <!-- Column 4: Positioning & Filesystem -->
        <div class="metadata-col">
          <div class="metadata-card">
            <span class="metadata-label">Track / Disc Info</span>
            <span class="metadata-value">
              T: {selectedTrack.track_number || '—'}{#if selectedTrack.track_total} of {selectedTrack.track_total}{/if}
              {#if selectedTrack.disc_number} • D: {selectedTrack.disc_number}{#if selectedTrack.disc_total} of {selectedTrack.disc_total}{/if}{/if}
            </span>
          </div>
          <div class="metadata-card" style="margin-top: 0.75rem;">
            <span class="metadata-label">Duration / Size</span>
            <span class="metadata-value">{formatDuration(selectedTrack.duration_seconds)} • {formatSize(selectedTrack.size_bytes)}</span>
          </div>
          <div class="metadata-card" style="margin-top: 0.75rem;">
            <span class="metadata-label">Indexed File</span>
            <span class="metadata-value" title={selectedTrack.filename}>{selectedTrack.filename}</span>
          </div>
        </div>
      </div>

      <!-- Full width filepath -->
      <div style="border-top: 1px solid var(--border-color); margin-top: 0.85rem; padding-top: 0.5rem; display: flex; flex-direction: column; gap: 0.2rem;">
        <span class="metadata-label">Absolute File Path</span>
        <span class="metadata-value path-value" title={selectedTrack.path}><code>{selectedTrack.path}</code></span>
      </div>

      <!-- Lyrics & Comments row -->
      {#if selectedTrack.lyrics || selectedTrack.comment}
        <div style="border-top: 1px solid var(--border-color); margin-top: 0.75rem; padding-top: 0.5rem; display: grid; grid-template-columns: 1fr 1fr; gap: 1.5rem;">
          {#if selectedTrack.lyrics}
            <div class="metadata-card">
              <span class="metadata-label">Lyrics</span>
              <p style="font-size: 0.78rem; line-height: 1.4; color: var(--text-secondary); white-space: pre-line; margin: 0.15rem 0 0 0;">{selectedTrack.lyrics}</p>
            </div>
          {/if}
          {#if selectedTrack.comment}
            <div class="metadata-card">
              <span class="metadata-label">Comments</span>
              <p style="font-size: 0.78rem; line-height: 1.4; color: var(--text-secondary); margin: 0.15rem 0 0 0;">{selectedTrack.comment}</p>
            </div>
          {/if}
        </div>
      {/if}
    </div>
  {/if}
</div>
