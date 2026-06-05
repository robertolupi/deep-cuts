<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { player, formatDuration, formatSize } from "$lib/stores/player.svelte";
  import { filters } from "$lib/stores/filters.svelte";
  import { curation } from "$lib/stores/curation.svelte";
  import { library } from "$lib/stores/library.svelte";
  import Autocomplete from "./Autocomplete.svelte";
  import TagsAutocomplete from "./TagsAutocomplete.svelte";
  import MoodRadar, { type MoodValues } from '$lib/components/MoodRadar.svelte';
  import CollapsiblePane from '$lib/components/CollapsiblePane.svelte';

  const track     = $derived(player.selectedTrack);
  const isPlaying = $derived(player.isPlaying);

  let coverArt = $state<string | null>(null);
  let trackPlaylists = $state<import('$lib/types').Playlist[]>([]);
  let playlistSelectQuery = $state("");
  let selectedPlaylistToAdd = $state<import('$lib/types').Playlist | null>(null);

  const playlistSuggestions = $derived.by(() => {
    const q = playlistSelectQuery.trim().toLowerCase();
    if (!q) {
      return curation.playlists;
    }
    return curation.playlists.filter(pl => pl.name.toLowerCase().includes(q)).slice(0, 12);
  });

  $effect(() => {
    if (!selectedPlaylistToAdd) {
      playlistSelectQuery = "";
    } else if (selectedPlaylistToAdd.name !== playlistSelectQuery) {
      playlistSelectQuery = selectedPlaylistToAdd.name;
    }
  });
  type TagMeta = { name: string; source: string; score: number | null; discard: boolean };
  let trackTags = $state<TagMeta[]>([]);

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
    invoke<Record<number, TagMeta[]>>('get_tags_with_meta_for_tracks', { trackIds: [id] })
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

  let newTagInput = $state("");
  let showRestoreMenu = $state(false);
  const suppressedTags = $derived(trackTags.filter(t => t.discard));

  async function handleAddUserTag() {
    if (!track || !newTagInput.trim()) return;
    try {
      await invoke('add_user_tag', { trackPath: track.path, tagName: newTagInput.trim() });
      newTagInput = "";
      const raw = await invoke<Record<number, TagMeta[]>>('get_tags_with_meta_for_tracks', { trackIds: [track.id] });
      trackTags = raw[track.id] ?? [];
      await library.fetchTags();
    } catch (e) {
      console.error("Failed to add user tag:", e);
    }
  }

  async function handleRemoveUserTag(tagName: string) {
    if (!track) return;
    try {
      await invoke('remove_user_tag', { trackPath: track.path, tagName });
      const raw = await invoke<Record<number, TagMeta[]>>('get_tags_with_meta_for_tracks', { trackIds: [track.id] });
      trackTags = raw[track.id] ?? [];
      await library.fetchTags();
    } catch (e) {
      console.error("Failed to remove user tag:", e);
    }
  }

  async function handleSuppressTag(tagName: string) {
    if (!track) return;
    try {
      await invoke('suppress_tag', { trackPath: track.path, tagName });
      const raw = await invoke<Record<number, TagMeta[]>>('get_tags_with_meta_for_tracks', { trackIds: [track.id] });
      trackTags = raw[track.id] ?? [];
      await library.fetchTags();
    } catch (e) {
      console.error("Failed to suppress tag:", e);
    }
  }

  async function handleUnsuppressTag(tagName: string) {
    if (!track) return;
    try {
      await invoke('unsuppress_tag', { trackPath: track.path, tagName });
      const raw = await invoke<Record<number, TagMeta[]>>('get_tags_with_meta_for_tracks', { trackIds: [track.id] });
      trackTags = raw[track.id] ?? [];
      await library.fetchTags();
    } catch (e) {
      console.error("Failed to unsuppress tag:", e);
    }
  }
</script>

<svelte:window onclick={() => { resetMenuOpen = false; }} />

<CollapsiblePane side="right" width="var(--sg-detail-pane-width, 320px)" hasIndicator={!!track}>
  {#snippet children({ collapse })}
  <div class="pane-scroll">
  {#if !track}
    <!-- Empty state -->
    <div class="pane-topbar">
      <button class="collapse-btn" onclick={collapse} title="Collapse detail pane">
        <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
          <polyline points="9 18 15 12 9 6"/>
        </svg>
      </button>
    </div>
    <div class="empty-state">
      <div class="empty-vinyl">
        <img src="/deep_cuts_transparent.png" alt="No track" />
      </div>
      <p class="empty-label">Select a track</p>
      <p class="empty-sub">Details appear here</p>
    </div>
  {:else}
    <div class="pane-inner">

      <!-- Collapse button -->
      <div class="pane-topbar">
        <button class="collapse-btn" onclick={collapse} title="Collapse detail pane">
          <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
            <polyline points="9 18 15 12 9 6"/>
          </svg>
        </button>
      </div>

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
            <button class="track-artist filter-link" onclick={() => { filters.searchQuery = track!.artist!; }}>{track.artist}</button>
          {/if}
          {#if track.album}
            <button class="track-album filter-link" onclick={() => { filters.searchQuery = track!.album!; }}>{track.album}{track.year ? ` · ${track.year}` : ''}</button>
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
        
        <div 
          class="ai-tags" 
          onclick={(e) => {
            if (e.target === e.currentTarget && suppressedTags.length > 0) {
              showRestoreMenu = !showRestoreMenu;
            }
          }}
          style="min-height: 20px; cursor: {suppressedTags.length > 0 ? 'pointer' : 'default'};"
          title={suppressedTags.length > 0 ? "Click background to restore suppressed tags" : ""}
        >
          {#if trackTags.length === 0}
            <span style="font-family: var(--sg-font-mono); font-size: var(--sg-text-xs); color: var(--sg-outline, #849495);">No tags.</span>
          {/if}
          {#each trackTags as tag}
            {@const theme = tagTheme(tag.name)}
            {@const active = filters.selectedTags.includes(tag.name)}
            {@const scoreStr = tag.score != null ? ` · score ${tag.score.toFixed(3)}` : ''}
            <button
              class="detail-tag-chip"
              class:tag-active={active}
              class:tag-discarded={tag.discard}
              class:tag-user={tag.source === 'user'}
              style="color:{tag.source === 'user' ? '#000' : theme.color};background:{active ? theme.border : (tag.source === 'user' ? theme.color : theme.bg)};border-color:{theme.border}; font-weight:{tag.source === 'user' ? 'bold' : '600'}; --user-glow:{theme.border}"
              title={tag.discard ? 'Click to restore suppressed tag' : `Source: ${tag.source}${scoreStr} · Right-click to ${tag.source === 'user' ? 'delete' : 'suppress'}`}
              onclick={() => {
                if (tag.discard) {
                  handleUnsuppressTag(tag.name);
                } else {
                  filters.toggleTag(tag.name);
                }
              }}
              oncontextmenu={(e) => {
                e.preventDefault();
                if (tag.discard) return;
                if (tag.source === 'user') {
                  handleRemoveUserTag(tag.name);
                } else {
                  handleSuppressTag(tag.name);
                }
              }}
            >{tag.name.split(':').slice(1).join(':')}<span class="tag-ns" style="{tag.source === 'user' ? 'color: #000; opacity: 0.6; font-weight: bold;' : ''}">{tag.name.split(':')[0]}</span></button>
          {/each}
        </div>

        {#if showRestoreMenu && suppressedTags.length > 0}
          <div class="restore-menu" style="background: var(--sg-surface-slate, #161b22); border: 1px solid rgba(255,255,255,0.12); border-radius: 4px; padding: 4px; margin-top: 4px; display: flex; flex-direction: column; gap: 2px;">
            <span style="font-family: var(--sg-font-mono); font-size: var(--sg-text-3xs); color: var(--sg-outline, #849495); padding: 2px 4px; font-weight: bold;">RESTORE SUPPRESSED TAG:</span>
            {#each suppressedTags as tag}
              <button 
                style="background: none; border: none; text-align: left; font-family: var(--sg-font-mono); font-size: var(--sg-text-xs); color: var(--sg-on-surface, #e3e1e9); padding: 4px; cursor: pointer; border-radius: 2px;"
                onclick={() => {
                  handleUnsuppressTag(tag.name);
                  showRestoreMenu = false;
                }}
                onmouseenter={(e) => { e.currentTarget.style.background = 'rgba(255,255,255,0.05)'; }}
                onmouseleave={(e) => { e.currentTarget.style.background = 'none'; }}
              >
                ↺ {tag.name}
              </button>
            {/each}
          </div>
        {/if}



        {#snippet addTagButton()}
          <button
            onclick={handleAddUserTag}
            style="background: var(--sg-surface-slate, #161b22); border: 1px solid rgba(255,255,255,0.12); border-radius: 4px; padding: 4px 8px; font-family: var(--sg-font-mono); font-size: var(--sg-text-xs); color: var(--sg-outline, #849495); cursor: pointer;"
          >+</button>
        {/snippet}

        <div class="add-tag-box" style="margin-top: 8px;">
          <TagsAutocomplete
            bind:value={newTagInput}
            excludeTags={trackTags.map(t => t.name)}
            placeholder="Add tag (e.g. genre:synthwave)..."
            onselect={(tag) => {
              newTagInput = tag;
              handleAddUserTag();
            }}
            onkeydown={(e) => {
              if (e.key === 'Enter') handleAddUserTag();
              if (e.key === 'Escape') newTagInput = '';
            }}
            buttonSnippet={addTagButton}
          />
        </div>
      </div>

      <!-- Mood radar (Essentia) -->
      {#if hasMoods && trackMood}
        <div class="section">
          <span class="section-label">EMOTIVE PROFILE</span>
          <div
            class="mood-radar-wrap"
            title="Click to filter by this track's mood profile"
            onclick={() => {
              const PAD = filters.moodTolerance;
              const clamp = (v: number) => Math.max(0, Math.min(1, v));
              const set = (val: number | null, setMin: (v: number) => void, setMax: (v: number) => void) => {
                if (val == null) return;
                setMin(clamp(val - PAD));
                setMax(clamp(val + PAD));
              };
              set(trackMood!.happy,      v => filters.moodHappyMin = v,      v => filters.moodHappyMax = v);
              set(trackMood!.sad,        v => filters.moodSadMin = v,        v => filters.moodSadMax = v);
              set(trackMood!.aggressive, v => filters.moodAggressiveMin = v, v => filters.moodAggressiveMax = v);
              set(trackMood!.relaxed,    v => filters.moodRelaxedMin = v,    v => filters.moodRelaxedMax = v);
              set(trackMood!.party,      v => filters.moodPartyMin = v,      v => filters.moodPartyMax = v);
              set(trackMood!.acoustic,   v => filters.moodAcousticMin = v,   v => filters.moodAcousticMax = v);
              set(trackMood!.electronic, v => filters.moodElectronicMin = v, v => filters.moodElectronicMax = v);
            }}
          >
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
                <button class="classifier-val filter-link classifier-link" onclick={() => { filters.genreFilter = track!.detected_genre!; }}>{track.detected_genre}</button>
              </div>
            {/if}
            {#if track.detected_vocal}
              <div class="classifier-row">
                <span class="classifier-key">VOCAL</span>
                <span class="classifier-val">
                  <button class="filter-link classifier-link" onclick={() => { filters.vocalFilter = track!.detected_vocal as "voice" | "instrumental"; }}>{track.detected_vocal}</button>
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
                <span style="font-family: var(--sg-font-mono); font-size: var(--sg-text-sm); color: var(--sg-on-surface, #e3e1e9);">🟣 {pl.name}</span>
                <button 
                  type="button" 
                  style="background: none; border: none; color: var(--sg-outline, #849495); cursor: pointer; font-size: var(--sg-text-sm); padding: 2px;"
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
          <p style="font-family: var(--sg-font-mono); font-size: var(--sg-text-xs); color: var(--sg-outline, #849495); margin-bottom: 8px;">Not in any playlists.</p>
        {/if}

        <!-- Add to playlist selector -->
        <div style="display: flex; flex-direction: column; gap: 6px;">
          <span style="font-family: var(--sg-font-mono); font-size: var(--sg-text-3xs); font-weight: 700; color: var(--sg-outline, #849495); letter-spacing: 0.05em;">ADD TO PLAYLIST</span>
          
          {#snippet playlistItemSnippet(pl: import('$lib/types').Playlist)}
            <span style="font-family: var(--sg-font-mono); font-size: var(--sg-text-xs);">{pl.name}</span>
          {/snippet}

          {#snippet playlistClearButtonSnippet()}
            {#if selectedPlaylistToAdd || playlistSelectQuery}
              <button type="button" class="clear-x" onclick={() => {
                playlistSelectQuery = "";
                selectedPlaylistToAdd = null;
              }}>×</button>
            {/if}
          {/snippet}

          <div class="playlist-wrap">
            <svg xmlns="http://www.w3.org/2000/svg" width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" class="playlist-search-icon">
              <path d="M12 2A10 10 0 0 0 2 12a10 10 0 0 0 10 10 10 10 0 0 0 10-10A10 10 0 0 0 12 2zm0 15a5 5 0 1 1 0-10 5 5 0 0 1 0 10z"/>
              <circle cx="12" cy="12" r="2"/>
            </svg>
            <Autocomplete
              bind:value={playlistSelectQuery}
              options={playlistSuggestions}
              placeholder="Search playlist to add..."
              onselect={async (pl) => {
                if (track) {
                  await curation.addTracksToPlaylist(pl.id, [track.id]);
                  selectedPlaylistToAdd = null;
                  playlistSelectQuery = "";
                  await loadTrackPlaylists();
                }
              }}
              itemSnippet={playlistItemSnippet}
              buttonSnippet={playlistClearButtonSnippet}
              borderless={true}
            />
          </div>
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
  </div>
  {/snippet}
</CollapsiblePane>

<style>
  .pane-scroll {
    height: 100%;
    overflow-y: auto;
    overflow-x: hidden;
    scrollbar-width: thin;
    scrollbar-color: rgba(255,255,255,0.1) transparent;
  }

  .pane-topbar {
    display: flex;
    justify-content: flex-end;
    padding: 6px 6px 0;
  }

  .collapse-btn {
    background: none;
    border: none;
    cursor: pointer;
    color: var(--sg-outline, #849495);
    padding: 2px;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 3px;
    flex-shrink: 0;
  }

  .collapse-btn:hover {
    color: var(--sg-on-surface, #e3e1e9);
    background: rgba(255,255,255,0.05);
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
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-base);
    font-weight: 600;
    color: var(--sg-on-surface, #e3e1e9);
  }

  .empty-sub {
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-xs);
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
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-2xs);
    font-weight: 700;
    padding: 2px 6px;
    border: 1px solid var(--sg-primary, #00f0ff);
    color: var(--sg-primary, #00f0ff);
    border-radius: 3px;
    letter-spacing: 0.05em;
    margin-bottom: 4px;
  }

  .track-title {
    font-family: var(--sg-font-ui);
    font-size: var(--sg-text-md);
    font-weight: 600;
    color: var(--sg-on-surface, #e3e1e9);
    margin: 0;
    word-break: break-word;
  }

  .track-artist {
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-sm);
    color: var(--sg-outline, #849495);
    margin: 3px 0 0;
  }

  .track-album {
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-xs);
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
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-2xs);
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
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-3xs);
    font-weight: 700;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--sg-outline, #849495);
  }

  .spec-value {
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-sm);
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
    font-size: var(--sg-text-base);
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
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-2xs);
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

  .detail-tag-chip:hover:not(.tag-discarded) {
    filter: brightness(1.25);
  }

  .tag-discarded {
    opacity: 0.35;
    cursor: default;
    text-decoration: line-through;
  }

  .tag-user {
    box-shadow: 0 0 6px var(--user-glow);
  }

  /* Dimmed namespace prefix shown after the label */
  .tag-ns {
    font-size: var(--sg-text-3xs);
    opacity: 0.5;
    font-weight: 400;
  }

  /* ── Mood radar ── */
  .mood-radar-wrap {
    width: 100%;
    height: 180px;
    cursor: pointer;
    border-radius: 4px;
    transition: box-shadow 0.15s;
  }

  .mood-radar-wrap:hover {
    box-shadow: 0 0 0 1px rgba(254,228,64,0.35);
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
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-2xs);
    color: var(--sg-outline, #849495);
    word-break: break-all;
    line-height: 1.5;
    transition: color 0.15s;
  }

  .filepath:hover code {
    color: var(--sg-primary, #00f0ff);
  }

  .track-genre {
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-xs);
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
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-3xs);
    font-weight: 700;
    letter-spacing: 0.1em;
    color: var(--sg-outline, #849495);
    width: 44px;
    flex-shrink: 0;
  }

  .classifier-val {
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-sm);
    color: var(--sg-on-surface, #e3e1e9);
  }

  .classifier-conf {
    font-size: var(--sg-text-2xs);
    color: var(--sg-outline, #849495);
    margin-left: 3px;
  }

  .classifier-link {
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-sm);
    color: var(--sg-on-surface, #e3e1e9);
    text-align: left;
    display: inline;
    width: auto;
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
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-xs);
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
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-xs);
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
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-xs);
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
    font-size: var(--sg-text-sm);
    line-height: 1.6;
    color: var(--sg-on-surface-variant, #b9cacb);
    white-space: pre-line;
    margin: 0.5rem 0 0;
  }

  .playlist-wrap {
    display: flex;
    align-items: center;
    gap: 4px;
    position: relative;
    width: 100%;
    background: var(--sg-surface-container, #1e1f25);
    border: 1px solid rgba(255,255,255,0.08);
    border-radius: 4px;
    padding: 0.4rem 0.5rem;
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-sm);
    transition: border-color 0.15s;
    box-sizing: border-box;
  }

  .playlist-wrap:focus-within {
    border-color: var(--sg-primary, #00f0ff);
  }

  .playlist-wrap .playlist-search-icon {
    flex-shrink: 0;
    margin-left: 8px;
    color: var(--sg-outline, #849495);
    pointer-events: none;
  }

  .playlist-wrap .clear-x {
    position: absolute;
    right: 6px;
    background: none;
    border: none;
    cursor: pointer;
    color: var(--sg-outline, #849495);
    font-size: var(--sg-text-md);
    line-height: 1;
    padding: 0 2px;
  }

</style>
