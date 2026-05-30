<script lang="ts">
  import { player, formatDuration, formatSize } from "$lib/stores/player.svelte";

  const track     = $derived(player.selectedTrack);
  const isPlaying = $derived(player.isPlaying);

  const moods = $derived(track ? [
    { label: "Happy",      value: track.mood_happy,      color: "var(--sg-primary, #00f0ff)" },
    { label: "Sad",        value: track.mood_sad,         color: "var(--sg-outline, #849495)" },
    { label: "Aggressive", value: track.mood_aggressive,  color: "var(--sg-secondary, #fe00fe)" },
    { label: "Relaxed",    value: track.mood_relaxed,     color: "var(--sg-primary, #00f0ff)" },
    { label: "Party",      value: track.mood_party,       color: "var(--sg-secondary, #fe00fe)" },
    { label: "Acoustic",   value: track.mood_acoustic,    color: "var(--sg-outline, #849495)" },
    { label: "Electronic", value: track.mood_electronic,  color: "var(--sg-primary, #00f0ff)" },
  ].filter(m => m.value != null) : []);

  const hasMoods = $derived(moods.length > 0);
  const hasAi    = $derived(!!track?.description || !!track?.ai_genre || !!track?.ai_mood);
  const ext      = $derived(track?.path.split('.').pop()?.toUpperCase() ?? '');
</script>

<aside class="detail-pane">
  {#if !track}
    <!-- Empty state -->
    <div class="empty-state">
      <div class="empty-vinyl">
        <img src="/deep_cuts_transparent.png" alt="No track" />
      </div>
      <p class="empty-label">Select a track</p>
      <p class="empty-sub">Details appear here</p>
    </div>
  {:else}
    <div class="pane-inner">

      <!-- Vinyl + title -->
      <div class="track-header">
        <div class="vinyl-wrap" class:spinning={isPlaying}>
          <img src="/deep_cuts_transparent.png" alt="Now playing" />
        </div>
        <div class="track-title-block">
          <span class="format-badge">{ext}</span>
          <h3 class="track-title">{track.title || track.filename}</h3>
          {#if track.artist}
            <p class="track-artist">{track.artist}</p>
          {/if}
          {#if track.album}
            <p class="track-album">{track.album}{track.year ? ` · ${track.year}` : ''}</p>
          {/if}
          {#if track.genre}
            <p class="track-genre">{track.genre}</p>
          {/if}
        </div>
      </div>

      <!-- Technical specs -->
      <div class="specs-grid">
        {#if track.sample_rate}
          <div class="spec-cell">
            <span class="spec-label">SAMPLE RATE</span>
            <span class="spec-value">{(track.sample_rate / 1000).toFixed(1)} kHz</span>
          </div>
        {/if}
        {#if track.bit_depth}
          <div class="spec-cell">
            <span class="spec-label">BIT DEPTH</span>
            <span class="spec-value">{track.bit_depth} bit</span>
          </div>
        {/if}
        {#if track.bitrate}
          <div class="spec-cell">
            <span class="spec-label">BITRATE</span>
            <span class="spec-value">{track.bitrate} kbps</span>
          </div>
        {/if}
        <div class="spec-cell">
          <span class="spec-label">CHANNELS</span>
          <span class="spec-value">{track.channels === 2 ? 'Stereo' : track.channels === 1 ? 'Mono' : (track.channels ?? '—')}</span>
        </div>
        {#if track.bpm}
          <div class="spec-cell">
            <span class="spec-label">BPM</span>
            <span class="spec-value">{Math.round(track.bpm)}</span>
          </div>
        {/if}
        {#if track.key && track.scale}
          <div class="spec-cell">
            <span class="spec-label">KEY</span>
            <span class="spec-value">{track.key} {track.scale}{track.key_strength != null ? ` · ${(track.key_strength * 100).toFixed(0)}%` : ''}</span>
          </div>
        {/if}
        {#if track.loudness_lufs}
          <div class="spec-cell">
            <span class="spec-label">LOUDNESS</span>
            <span class="spec-value">{track.loudness_lufs} LUFS{track.loudness_range ? ` · ${track.loudness_range} LU` : ''}</span>
          </div>
        {/if}
        <div class="spec-cell">
          <span class="spec-label">DURATION</span>
          <span class="spec-value">{formatDuration(track.duration_seconds)}</span>
        </div>
        <div class="spec-cell">
          <span class="spec-label">SIZE</span>
          <span class="spec-value">{formatSize(track.size_bytes)}</span>
        </div>
        {#if track.track_number}
          <div class="spec-cell">
            <span class="spec-label">TRACK</span>
            <span class="spec-value">{track.track_number}{track.track_total ? ` / ${track.track_total}` : ''}</span>
          </div>
        {/if}
        {#if track.disc_number}
          <div class="spec-cell">
            <span class="spec-label">DISC</span>
            <span class="spec-value">{track.disc_number}{track.disc_total ? ` / ${track.disc_total}` : ''}</span>
          </div>
        {/if}
        {#if track.album_artist}
          <div class="spec-cell spec-cell-full">
            <span class="spec-label">ALBUM ARTIST</span>
            <span class="spec-value">{track.album_artist}</span>
          </div>
        {/if}
        {#if track.composer}
          <div class="spec-cell spec-cell-full">
            <span class="spec-label">COMPOSER</span>
            <span class="spec-value">{track.composer}</span>
          </div>
        {/if}
      </div>

      <!-- AI description (Studio Pink) -->
      {#if hasAi}
        <div class="section">
          <div class="section-header ai-header">
            <svg xmlns="http://www.w3.org/2000/svg" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <path d="M12 2a9 9 0 0 1 9 9c0 3.18-1.65 5.97-4.13 7.6L17 21H7l.13-2.4A9 9 0 0 1 3 11a9 9 0 0 1 9-9z"/>
              <line x1="9" y1="9" x2="9.01" y2="9"/>
              <line x1="15" y1="9" x2="15.01" y2="9"/>
              <path d="M9 13a3 3 0 0 0 6 0"/>
            </svg>
            <span class="section-label ai-label">AI DESCRIPTION</span>
          </div>
          {#if track.description}
            <p class="ai-prose">{track.description}</p>
          {/if}
          {#if track.ai_genre || track.ai_mood || track.ai_instruments}
            <div class="ai-tags">
              {#if track.ai_genre}<span class="ai-tag ai-tag-genre">{track.ai_genre}</span>{/if}
              {#if track.ai_mood}<span class="ai-tag ai-tag-mood">{track.ai_mood}</span>{/if}
              {#if track.ai_instruments}
                {#each track.ai_instruments.split(',').slice(0,3) as inst}
                  <span class="ai-tag ai-tag-instrument">{inst.trim()}</span>
                {/each}
              {/if}
            </div>
          {/if}
        </div>
      {/if}

      <!-- Mood bars (Essentia) -->
      {#if hasMoods}
        <div class="section">
          <span class="section-label">EMOTIVE PROFILE</span>
          <div class="mood-bars">
            {#each moods as mood}
              <div class="mood-row">
                <span class="mood-label">{mood.label.toUpperCase()}</span>
                <div class="mood-track">
                  <div class="mood-fill" style="width: {((mood.value ?? 0) * 100).toFixed(1)}%; background: {mood.color};"></div>
                </div>
                <span class="mood-pct" style="color: {mood.color};">{((mood.value ?? 0) * 100).toFixed(0)}%</span>
              </div>
            {/each}
          </div>
        </div>
      {/if}

      <!-- Essentia classifier -->
      {#if track.detected_genre || track.detected_vocal || track.is_music != null}
        <div class="section">
          <span class="section-label">CLASSIFIER</span>
          <div class="classifier-rows">
            {#if track.is_music != null}
              <div class="classifier-row">
                <span class="classifier-key">TYPE</span>
                <span class="classifier-val">{track.is_music ? 'Music' : 'Non-music'}</span>
              </div>
            {/if}
            {#if track.detected_genre}
              <div class="classifier-row">
                <span class="classifier-key">GENRE</span>
                <span class="classifier-val">{track.detected_genre}</span>
              </div>
            {/if}
            {#if track.detected_vocal}
              <div class="classifier-row">
                <span class="classifier-key">VOCAL</span>
                <span class="classifier-val">
                  {track.detected_vocal}
                  {#if track.detected_vocal_confidence != null}
                    <span class="classifier-conf">({(track.detected_vocal_confidence * 100).toFixed(0)}%)</span>
                  {/if}
                </span>
              </div>
            {/if}
          </div>
        </div>
      {/if}

      <!-- File path -->
      <div class="section filepath-section">
        <span class="section-label">FILE PATH</span>
        <button
          class="filepath"
          onclick={() => player.revealInFinder(track!.path)}
          title="Reveal in Finder"
        >
          <code>{track.path}</code>
        </button>
      </div>

      {#if track.lyrics}
        <div class="section">
          <span class="section-label">LYRICS</span>
          <p class="lyrics-text">{track.lyrics}</p>
        </div>
      {/if}

      {#if track.comment}
        <div class="section">
          <span class="section-label">COMMENTS</span>
          <p class="lyrics-text">{track.comment}</p>
        </div>
      {/if}

    </div>
  {/if}
</aside>

<style>
  .detail-pane {
    width: var(--sg-detail-pane-width, 320px);
    height: 100%;
    flex-shrink: 0;
    background: var(--sg-surface-slate, #161b22);
    border-left: 1px solid rgba(255,255,255,0.08);
    overflow-y: auto;
    overflow-x: hidden;
    scrollbar-width: thin;
    scrollbar-color: rgba(255,255,255,0.1) transparent;
  }

  /* ── Empty state ── */
  .empty-state {
    height: 100%;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 8px;
    opacity: 0.35;
    padding: 2rem;
  }

  .empty-vinyl img {
    width: 64px;
    height: 64px;
    opacity: 0.5;
  }

  .empty-label {
    font-family: "JetBrains Mono", monospace;
    font-size: 12px;
    font-weight: 600;
    color: var(--sg-on-surface, #e3e1e9);
  }

  .empty-sub {
    font-family: "JetBrains Mono", monospace;
    font-size: 10px;
    color: var(--sg-outline, #849495);
  }

  /* ── Pane content ── */
  .pane-inner {
    display: flex;
    flex-direction: column;
    gap: 0;
    padding: 1rem 0.85rem;
  }

  /* ── Track header ── */
  .track-header {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.75rem;
    padding-bottom: 0.85rem;
    border-bottom: 1px solid rgba(255,255,255,0.06);
    margin-bottom: 0.1rem;
  }

  .vinyl-wrap {
    width: 80px;
    height: 80px;
    border-radius: 50%;
    overflow: hidden;
    flex-shrink: 0;
  }

  .vinyl-wrap img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .vinyl-wrap.spinning img {
    animation: spin 4s linear infinite;
  }

  @keyframes spin {
    from { transform: rotate(0deg); }
    to   { transform: rotate(360deg); }
  }

  .track-title-block {
    text-align: center;
    min-width: 0;
  }

  .format-badge {
    display: inline-block;
    font-family: "JetBrains Mono", monospace;
    font-size: 9px;
    font-weight: 700;
    padding: 2px 6px;
    border: 1px solid var(--sg-primary, #00f0ff);
    color: var(--sg-primary, #00f0ff);
    border-radius: 3px;
    letter-spacing: 0.05em;
    margin-bottom: 4px;
  }

  .track-title {
    font-family: Inter, sans-serif;
    font-size: 14px;
    font-weight: 600;
    color: var(--sg-on-surface, #e3e1e9);
    margin: 0;
    word-break: break-word;
  }

  .track-artist {
    font-family: "JetBrains Mono", monospace;
    font-size: 11px;
    color: var(--sg-outline, #849495);
    margin: 3px 0 0;
  }

  .track-album {
    font-family: "JetBrains Mono", monospace;
    font-size: 10px;
    color: var(--sg-outline, #849495);
    opacity: 0.7;
    margin: 2px 0 0;
  }

  /* ── Sections ── */
  .section {
    padding: 0.65rem 0;
    border-top: 1px solid rgba(255,255,255,0.06);
  }

  .section-label {
    display: block;
    font-family: "JetBrains Mono", monospace;
    font-size: 9px;
    font-weight: 700;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--sg-outline, #849495);
    margin-bottom: 0.5rem;
  }

  /* ── Specs grid ── */
  .specs-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 6px;
    padding: 0.65rem;
    background: rgba(13,17,23,0.5);
    border: 1px solid rgba(255,255,255,0.06);
    border-radius: 4px;
    margin: 0.65rem 0;
  }

  .spec-cell {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .spec-label {
    font-family: "JetBrains Mono", monospace;
    font-size: 8px;
    font-weight: 700;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--sg-outline, #849495);
  }

  .spec-value {
    font-family: "JetBrains Mono", monospace;
    font-size: 11px;
    color: var(--sg-on-surface, #e3e1e9);
  }

  /* ── AI section ── */
  .section-header {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-bottom: 0.5rem;
  }

  .ai-header {
    color: var(--sg-secondary, #fe00fe);
  }

  .ai-label {
    margin-bottom: 0;
    color: var(--sg-secondary, #fe00fe);
  }

  .ai-prose {
    font-size: 12px;
    line-height: 1.6;
    color: var(--sg-on-surface-variant, #b9cacb);
    margin: 0 0 0.5rem;
  }

  .ai-tags {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }

  .ai-tag {
    font-family: "JetBrains Mono", monospace;
    font-size: 9px;
    padding: 2px 7px;
    border-radius: 999px;
  }

  /* genre — Studio Pink */
  .ai-tag-genre {
    border: 1px solid rgba(254,0,254,0.35);
    color: var(--sg-secondary, #fe00fe);
    background: rgba(254,0,254,0.08);
  }

  /* mood — amber/warm */
  .ai-tag-mood {
    border: 1px solid rgba(200,120,0,0.45);
    color: #c87800;
    background: rgba(200,120,0,0.1);
  }

  /* instruments — Cyber Cyan */
  .ai-tag-instrument {
    border: 1px solid rgba(0,240,255,0.3);
    color: var(--sg-primary, #00f0ff);
    background: rgba(0,240,255,0.07);
  }

  /* ── Mood bars ── */
  .mood-bars {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .mood-row {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .mood-label {
    font-family: "JetBrains Mono", monospace;
    font-size: 8px;
    color: var(--sg-outline, #849495);
    width: 60px;
    flex-shrink: 0;
    letter-spacing: 0.05em;
  }

  .mood-track {
    flex: 1;
    height: 3px;
    background: rgba(255,255,255,0.06);
    border-radius: 2px;
    overflow: hidden;
  }

  .mood-fill {
    height: 100%;
    border-radius: 2px;
    transition: width 0.4s ease;
  }

  .mood-pct {
    font-family: "JetBrains Mono", monospace;
    font-size: 9px;
    width: 28px;
    text-align: right;
    flex-shrink: 0;
  }

  /* ── File path ── */
  .filepath-section .section-label { margin-bottom: 0.35rem; }

  .filepath {
    background: none;
    border: none;
    cursor: pointer;
    text-align: left;
    padding: 0;
    width: 100%;
  }

  .filepath code {
    font-family: "JetBrains Mono", monospace;
    font-size: 9px;
    color: var(--sg-outline, #849495);
    word-break: break-all;
    line-height: 1.5;
    transition: color 0.15s;
  }

  .filepath:hover code {
    color: var(--sg-primary, #00f0ff);
  }

  .track-genre {
    font-family: "JetBrains Mono", monospace;
    font-size: 10px;
    color: var(--sg-outline, #849495);
    opacity: 0.6;
    margin: 2px 0 0;
  }

  /* full-width spec cell for long text like composer/album artist */
  .spec-cell-full {
    grid-column: 1 / -1;
  }

  /* ── Classifier section ── */
  .classifier-rows {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .classifier-row {
    display: flex;
    gap: 8px;
    align-items: baseline;
  }

  .classifier-key {
    font-family: "JetBrains Mono", monospace;
    font-size: 8px;
    font-weight: 700;
    letter-spacing: 0.1em;
    color: var(--sg-outline, #849495);
    width: 44px;
    flex-shrink: 0;
  }

  .classifier-val {
    font-family: "JetBrains Mono", monospace;
    font-size: 11px;
    color: var(--sg-on-surface, #e3e1e9);
  }

  .classifier-conf {
    font-size: 9px;
    color: var(--sg-outline, #849495);
    margin-left: 3px;
  }

  /* ── Lyrics ── */
  .lyrics-text {
    font-size: 11px;
    line-height: 1.6;
    color: var(--sg-on-surface-variant, #b9cacb);
    white-space: pre-line;
    margin: 0.5rem 0 0;
  }

</style>
