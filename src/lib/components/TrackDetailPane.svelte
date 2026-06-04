<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { player, formatDuration, formatSize } from "$lib/stores/player.svelte";
  import { filters } from "$lib/stores/filters.svelte";
  import { ui } from "$lib/stores/ui.svelte";
  import { curation } from "$lib/stores/curation.svelte";
  import PlaylistSelector from "./PlaylistSelector.svelte";
  import MoodRadar, { type MoodValues } from '$lib/components/MoodRadar.svelte';

  const track     = $derived(player.selectedTrack);
  const isPlaying = $derived(player.isPlaying);

  let coverArt = $state<string | null>(null);
  let trackPlaylists = $state<import('$lib/types').Playlist[]>([]);
  let playlistSelectQuery = $state("");
  let selectedPlaylistToAdd = $state<import('$lib/types').Playlist | null>(null);
  let trackTags = $state<string[]>([]);

  async function loadTrackPlaylists() {
    if (track) {
      trackPlaylists = await curation.getPlaylistsForTrack(track.id);
    } else {
      trackPlaylists = [];
    }
  }

  $effect(() => {
    const id = track?.id;
    trackPlaylists = [];
    if (id) {
      loadTrackPlaylists();
    }
  });

  $effect(() => {
    const id = track?.id;
    trackTags = [];
    if (!id) return;
    invoke<Record<number, string[]>>('get_tags_for_tracks', { trackIds: [id] })
      .then(raw => { trackTags = raw[id] ?? []; })
      .catch(() => {});
  });

  $effect(() => {
    const path = track?.path;
    coverArt = null;
    if (!path) return;
    invoke<string | null>('get_cover_art', { path }).then(result => {
      if (track?.path === path) coverArt = result;
    }).catch(() => {});
  });

  const trackMood = $derived<MoodValues | null>(track ? {
    happy:      track.mood_happy      ?? null,
    sad:        track.mood_sad        ?? null,
    aggressive: track.mood_aggressive ?? null,
    relaxed:    track.mood_relaxed    ?? null,
    party:      track.mood_party      ?? null,
    acoustic:   track.mood_acoustic   ?? null,
    electronic: track.mood_electronic ?? null,
  } : null);

  const hasMoods = $derived(trackMood != null && Object.values(trackMood).some(v => v != null));

  // Map a tag's namespace prefix to a { color, bg, border } theme
  function tagTheme(tag: string): { color: string; bg: string; border: string } {
    const prefix = tag.split(':')[0];
    switch (prefix) {
      case 'genre':     return { color: '#fe00fe', bg: 'rgba(254,0,254,0.08)',   border: 'rgba(254,0,254,0.35)' };
      case 'mood':      return { color: '#c87800', bg: 'rgba(200,120,0,0.10)',   border: 'rgba(200,120,0,0.40)' };
      case 'inst':      return { color: '#00f0ff', bg: 'rgba(0,240,255,0.07)',   border: 'rgba(0,240,255,0.30)' };
      case 'vibe':      return { color: '#ff9f1c', bg: 'rgba(255,159,28,0.08)',  border: 'rgba(255,159,28,0.35)' };
      case 'vocal':     return { color: '#9b5de5', bg: 'rgba(155,93,229,0.08)', border: 'rgba(155,93,229,0.35)' };
      case 'context':   return { color: '#00bbf9', bg: 'rgba(0,187,249,0.07)',  border: 'rgba(0,187,249,0.30)' };
      case 'bpm':       return { color: '#fee440', bg: 'rgba(254,228,64,0.07)', border: 'rgba(254,228,64,0.30)' };
      case 'key':       return { color: '#00f5d4', bg: 'rgba(0,245,212,0.07)',  border: 'rgba(0,245,212,0.30)' };
      case 'mastering': return { color: '#849495', bg: 'rgba(132,148,149,0.07)', border: 'rgba(132,148,149,0.25)' };
      case 'len':       return { color: '#849495', bg: 'rgba(132,148,149,0.07)', border: 'rgba(132,148,149,0.25)' };
      default:          return { color: '#fe00fe', bg: 'rgba(254,0,254,0.08)',   border: 'rgba(254,0,254,0.35)' };
    }
  }
  const ext      = $derived(track?.path.split('.').pop()?.toUpperCase() ?? '');

  const PASS_NAMES = ['audio_analysis', 'bpm_correction', 'clap', 'essentia', 'bpm_refinement', 'qwen', 'description_embed'];
  let resetMenuOpen = $state(false);
</script>

<svelte:window onclick={() => { resetMenuOpen = false; }} />

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
        <div class="vinyl-wrap" class:spinning={isPlaying && !coverArt} class:cover={!!coverArt}>
          {#if coverArt}
            <img src={coverArt} alt="Album art" />
          {:else}
            <img src="/deep_cuts_transparent.png" alt="Now playing" />
          {/if}
        </div>
        <div class="track-title-block">
          <span class="format-badge">{ext}</span>
          <h3 class="track-title">{track.title || track.filename}</h3>
          {#if track.artist}
            <button class="track-artist filter-link" onclick={() => { filters.searchQuery = track!.artist!; ui.activeView = 'table'; }}>{track.artist}</button>
          {/if}
          {#if track.album}
            <button class="track-album filter-link" onclick={() => { filters.searchQuery = track!.album!; ui.activeView = 'table'; }}>{track.album}{track.year ? ` · ${track.year}` : ''}</button>
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

      <!-- Tags (all sources, colored by namespace prefix) -->
      {#if track.description}
        <div class="section">
          <span class="section-label ai-label">DESCRIPTION</span>
          <p class="ai-prose">{track.description}</p>
        </div>
      {/if}

      {#if trackTags.length > 0}
        <div class="section">
          <div class="section-header ai-header">
            <svg xmlns="http://www.w3.org/2000/svg" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <path d="M12 2a9 9 0 0 1 9 9c0 3.18-1.65 5.97-4.13 7.6L17 21H7l.13-2.4A9 9 0 0 1 3 11a9 9 0 0 1 9-9z"/>
              <line x1="9" y1="9" x2="15.01" y2="9"/>
              <line x1="15" y1="9" x2="15.01" y2="9"/>
              <path d="M9 13a3 3 0 0 0 6 0"/>
            </svg>
            <span class="section-label ai-label">TAGS</span>
          </div>
          <div class="ai-tags">
            {#each trackTags as tag}
              {@const theme = tagTheme(tag)}
              {@const active = filters.selectedTags.includes(tag)}
              <button
                class="detail-tag-chip"
                class:tag-active={active}
                style="color:{theme.color};background:{active ? theme.border : theme.bg};border-color:{theme.border}"
                title="{active ? 'Remove filter' : 'Filter by'} {tag}"
                onclick={() => { filters.toggleTag(tag); ui.activeView = 'table'; }}
              >{tag.split(':').slice(1).join(':')}<span class="tag-ns">{tag.split(':')[0]}</span></button>
            {/each}
          </div>
        </div>
      {/if}

      <!-- Mood radar (Essentia) -->
      {#if hasMoods && trackMood}
        <div class="section">
          <span class="section-label">EMOTIVE PROFILE</span>
          <div class="mood-radar-wrap">
            <MoodRadar moodA={trackMood} />
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

      <!-- Sounds similar -->
      <div class="section similar-section">
        <button
          class="similar-btn"
          class:similar-active={filters.similarToTrack?.id === track.id}
          class:similar-loading={filters.isSimilarLoading}
          disabled={filters.isSimilarLoading}
          onclick={() => {
            if (filters.similarToTrack?.id === track.id) {
              filters.clearSimilar();
            } else {
              filters.setSimilarTo({ id: track.id, title: track.title ?? track.filename });
            }
          }}
        >
          {#if filters.isSimilarLoading}
            <span class="similar-spinner">⟳</span> Finding similar…
          {:else if filters.similarToTrack?.id === track.id}
            <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
              <circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/>
            </svg>
            Clear similar filter
          {:else}
            <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
              <circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/>
              <line x1="11" y1="8" x2="11" y2="14"/><line x1="8" y1="11" x2="14" y2="11"/>
            </svg>
            Find sounds similar
          {/if}
        </button>
      </div>

      <!-- Playlists Section -->
      <div class="section">
        <span class="section-label">🟣 PLAYLISTS</span>
        
        <!-- List of playlists the track is in -->
        {#if trackPlaylists.length > 0}
          <div class="track-playlists-list" style="display: flex; flex-direction: column; gap: 4px; margin-bottom: 8px;">
            {#each trackPlaylists as pl}
              <div class="track-playlist-item" style="display: flex; justify-content: space-between; align-items: center; background: rgba(255,255,255,0.03); padding: 4px 8px; border-radius: 4px;">
                <span style="font-family: 'JetBrains Mono', monospace; font-size: 11px; color: var(--sg-on-surface, #e3e1e9);">🟣 {pl.name}</span>
                <button 
                  type="button" 
                  style="background: none; border: none; color: var(--sg-outline, #849495); cursor: pointer; font-size: 11px; padding: 2px;"
                  onclick={async () => {
                    await curation.removeTrackFromPlaylistById(pl.id, track.id);
                    await loadTrackPlaylists();
                  }}
                  title="Remove from {pl.name}"
                >
                  🗑️
                </button>
              </div>
            {/each}
          </div>
        {:else}
          <p style="font-family: 'JetBrains Mono', monospace; font-size: 10px; color: var(--sg-outline, #849495); margin-bottom: 8px;">Not in any playlists.</p>
        {/if}

        <!-- Add to playlist selector -->
        <div style="display: flex; flex-direction: column; gap: 6px;">
          <span style="font-family: 'JetBrains Mono', monospace; font-size: 8px; font-weight: 700; color: var(--sg-outline, #849495); letter-spacing: 0.05em;">ADD TO PLAYLIST</span>
          <PlaylistSelector
            placeholder="Search playlist to add..."
            showAllOnFocus={true}
            bind:activePlaylist={selectedPlaylistToAdd}
            bind:value={playlistSelectQuery}
            onselect={async (pl) => {
              if (track) {
                await curation.addTracksToPlaylist(pl.id, [track.id]);
                selectedPlaylistToAdd = null;
                playlistSelectQuery = "";
                await loadTrackPlaylists();
              }
            }}
          />
        </div>
      </div>

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

      <!-- Reset analysis -->
      <div class="section reset-section">
        <button
          class="reset-btn"
          onclick={(e) => { e.stopPropagation(); resetMenuOpen = !resetMenuOpen; }}
        >
          Reset analysis pass ▾
        </button>
        {#if resetMenuOpen}
          <div class="reset-menu">
            {#each PASS_NAMES as pass}
              <button
                class="reset-menu-item"
                onclick={async () => {
                  resetMenuOpen = false;
                  await invoke('reset_pass_for_track', { passName: pass, trackId: track!.id });
                }}
              >{pass}</button>
            {/each}
          </div>
        {/if}
      </div>

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
    width: 250px;
    height: 250px;
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

  .vinyl-wrap.cover {
    border-radius: 4px;
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

  .filter-link {
    background: none;
    border: none;
    padding: 0;
    cursor: pointer;
    text-align: center;
    display: block;
    width: 100%;
    transition: color 0.15s, opacity 0.15s;
  }

  .filter-link:hover {
    color: var(--sg-primary, #00f0ff) !important;
    opacity: 1 !important;
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
    background: rgba(255,255,255,0.04);
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

  .detail-tag-chip {
    font-family: "JetBrains Mono", monospace;
    font-size: 9px;
    font-weight: 600;
    padding: 2px 8px;
    border-radius: 999px;
    border: 1px solid;
    cursor: pointer;
    transition: filter 0.12s;
    letter-spacing: 0.02em;
    display: inline-flex;
    align-items: center;
    gap: 4px;
  }

  .detail-tag-chip:hover {
    filter: brightness(1.25);
  }

  /* Dimmed namespace prefix shown after the label */
  .tag-ns {
    font-size: 8px;
    opacity: 0.5;
    font-weight: 400;
  }

  /* ── Mood radar ── */
  .mood-radar-wrap {
    width: 100%;
    height: 180px;
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

  /* ── Sounds similar ── */
  .similar-section {
    padding-top: 0.5rem;
    padding-bottom: 0.5rem;
  }

  .similar-btn {
    width: 100%;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    font-family: "JetBrains Mono", monospace;
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.05em;
    padding: 7px 12px;
    border-radius: 4px;
    border: 1px solid rgba(254,0,254,0.3);
    background: rgba(254,0,254,0.06);
    color: var(--sg-secondary, #fe00fe);
    cursor: pointer;
    transition: all 0.15s;
  }

  .similar-btn:hover:not(:disabled) {
    background: rgba(254,0,254,0.12);
    border-color: rgba(254,0,254,0.55);
  }

  .similar-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .similar-btn.similar-active {
    background: rgba(254,0,254,0.12);
    border-color: var(--sg-secondary, #fe00fe);
  }

  .similar-spinner {
    display: inline-block;
    animation: spin 1s linear infinite;
  }

  @keyframes spin {
    from { transform: rotate(0deg); }
    to   { transform: rotate(360deg); }
  }

  /* ── Reset button ── */
  .reset-section {
    position: relative;
  }

  .reset-btn {
    width: 100%;
    font-family: "JetBrains Mono", monospace;
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.05em;
    padding: 7px 12px;
    border-radius: 4px;
    border: 1px solid rgba(255,255,255,0.12);
    background: rgba(255,255,255,0.03);
    color: var(--sg-outline, #849495);
    cursor: pointer;
    transition: all 0.15s;
  }

  .reset-btn:hover {
    border-color: rgba(255,80,80,0.45);
    color: #ff6060;
    background: rgba(255,60,60,0.07);
  }

  .reset-menu {
    position: absolute;
    bottom: calc(100% - 0.65rem);
    left: 0;
    right: 0;
    background: var(--sg-surface-slate, #161b22);
    border: 1px solid rgba(255,255,255,0.12);
    border-radius: 4px;
    overflow: hidden;
    z-index: 100;
  }

  .reset-menu-item {
    display: block;
    width: 100%;
    text-align: left;
    font-family: "JetBrains Mono", monospace;
    font-size: 10px;
    padding: 7px 12px;
    background: none;
    border: none;
    color: var(--sg-outline, #849495);
    cursor: pointer;
    transition: all 0.1s;
  }

  .reset-menu-item:hover {
    background: rgba(255,60,60,0.07);
    color: #ff6060;
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
