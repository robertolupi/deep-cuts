<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import * as d3 from 'd3';
  import { library } from '$lib/stores/library.svelte';
  import { filters } from '$lib/stores/filters.svelte';
  import { player } from '$lib/stores/player.svelte';
  import { theme } from '$lib/stores/theme.svelte';
  import { ui } from '$lib/stores/ui.svelte';

  import { camelotMap, resolveTrackColor } from '$lib/utils/mapMath';
  import type { MappedTrackPoint } from '$lib/utils/mapMath';

  // Optional prop: when set, the map will pan to and select this track
  let { focusTrackId = $bindable(null) }: { focusTrackId?: number | null } = $props();

  let projectedTracks = $state<MappedTrackPoint[]>([]);
  let isRecomputing   = $state(false);
  let isLoading       = $state(false);

  let colorCoding = $state<'genre' | 'camelot' | 'bpm'>('genre');

  // Canvas
  let canvas        = $state<HTMLCanvasElement | null>(null);
  let mapContainer  = $state<HTMLElement | null>(null);
  let width         = $state(800);
  let height        = $state(600);
  const padding     = 30;

  // Interaction
  let transform    = $state(d3.zoomIdentity);
  let hoveredTrack = $state<MappedTrackPoint | null>(null);
  let hoverX       = $state(0);
  let hoverY       = $state(0);

  const currentThemeStr = $derived(theme.resolvedTheme);

  // Build a Set of filtered track IDs for O(1) lookup
  const filteredIds = $derived.by(() => {
    const s = new Set<number>();
    for (const t of filters.filteredTracks) s.add(t.id);
    return s;
  });

  // Only show tracks that have projection coords AND pass current filters
  const visibleTracks = $derived(projectedTracks.filter(t => filteredIds.has(t.id)));

  // Currently selected track on the map (mirrors player store)
  const selectedTrack = $derived(player.selectedTrack);

  const topGenres = $derived.by(() => {
    const counts: Record<string, number> = {};
    for (const t of library.tracks) {
      const g = t.genre;
      if (g?.trim()) {
        const normalized = g.split(/[---,;/]/)[0].trim();
        if (normalized) counts[normalized] = (counts[normalized] || 0) + 1;
      }
    }
    return Object.entries(counts).sort((a, b) => b[1] - a[1]).map(e => e[0]).slice(0, 10);
  });

  const genrePalette = $derived(
    currentThemeStr === 'accessible'
      ? ["#00ffff","#ff00ff","#ffff00","#00ff00","#ff0000","#0080ff","#ff8000","#ffffff","#00ff80","#8000ff"]
      : currentThemeStr === 'light'
        ? ["#4f46e5","#0284c7","#dc2626","#db2777","#16a34a","#ea580c","#9333ea","#2563eb","#0d9488","#b45309"]
        : ["#00e5ff","#ff007f","#8a2be2","#76ff03","#ffeb3b","#ff9100","#00e676","#2979ff","#d500f9","#a1887f"]
  );

  const dynamicGenreColors = $derived.by(() => {
    const map: Record<string, string> = {};
    topGenres.forEach((g, i) => { map[g] = genrePalette[i % genrePalette.length]; });
    map["Other"]   = currentThemeStr === 'light' ? "#64748b" : "#9e9e9e";
    map["Unknown"] = currentThemeStr === 'light' ? "#94a3b8" : "#757575";
    return map;
  });

  const themeColors = $derived.by(() => {
    if (currentThemeStr === 'accessible') return {
      selectedHalo: '#ffff00', selectedHaloOuter: 'rgba(255,255,0,0.3)',
      hoveredHalo: '#ffffff', dotBorder: '#ffffff', dotBorderWidth: 0.8,
      canvasBg: '#000000', bpmCool: '#00ffff', bpmHot: '#ff00ff',
    };
    if (currentThemeStr === 'light') return {
      selectedHalo: '#6366f1', selectedHaloOuter: 'rgba(99,102,241,0.25)',
      hoveredHalo: '#0f172a', dotBorder: '#ffffff', dotBorderWidth: 0.6,
      canvasBg: '#f8fafc', bpmCool: '#0284c7', bpmHot: '#db2777',
    };
    return {
      selectedHalo: '#00F2FE', selectedHaloOuter: 'rgba(0,242,254,0.25)',
      hoveredHalo: '#ffffff', dotBorder: 'rgba(10,11,16,0.4)', dotBorderWidth: 0.5,
      canvasBg: '#0a0b10', bpmCool: '#00B0FF', bpmHot: '#ff007f',
    };
  });

  const xScale = $derived(d3.scaleLinear().domain([0, 100]).range([padding, width - padding]));
  const yScale = $derived(d3.scaleLinear().domain([0, 100]).range([height - padding, padding]));

  function getTrackColor(track: MappedTrackPoint): string {
    return resolveTrackColor(track, colorCoding, dynamicGenreColors, themeColors);
  }

  async function loadCoordinates() {
    isLoading = true;
    try {
      projectedTracks = await invoke<MappedTrackPoint[]>('get_projection_coordinates', {
        musicOnly: filters.musicOnly,
      });
    } catch (err: any) {
      ui.showToast(err.toString(), 'error');
    } finally {
      isLoading = false;
    }
  }

  async function runProjectionRecompute() {
    isRecomputing = true;
    try {
      ui.showToast('Running projection… this may take a few seconds', 'success');
      const count = await invoke<number>('recompute_projection', {
        musicOnly: filters.musicOnly,
        algorithm: 'umap',
        nNeighbors: 20,
        minDist: 0.1,
        perplexity: 30,
      });
      ui.showToast(`Projected ${count} tracks into 2D space`, 'success');
      await loadCoordinates();
    } catch (err: any) {
      ui.showToast(err.toString(), 'error');
    } finally {
      isRecomputing = false;
    }
  }

  function drawCanvas() {
    if (!canvas) return;
    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    ctx.clearRect(0, 0, width, height);
    ctx.save();
    ctx.translate(transform.x, transform.y);
    ctx.scale(transform.k, transform.k);

    const dotR    = Math.max(1.0, 4.5  / transform.k);
    const strokeW = Math.max(0.1, 0.5  / transform.k);

    for (const track of visibleTracks) {
      const color = getTrackColor(track);
      ctx.beginPath();
      ctx.arc(xScale(track.x), yScale(track.y), dotR, 0, 2 * Math.PI);
      ctx.fillStyle = color;
      ctx.fill();

      if (currentThemeStr !== 'accessible') {
        ctx.beginPath();
        ctx.arc(xScale(track.x), yScale(track.y), dotR, 0, 2 * Math.PI);
        ctx.strokeStyle = themeColors.dotBorder;
        ctx.lineWidth = Math.max(0.1, themeColors.dotBorderWidth / transform.k);
        ctx.stroke();
      }
    }

    if (hoveredTrack) {
      ctx.beginPath();
      ctx.arc(xScale(hoveredTrack.x), yScale(hoveredTrack.y), Math.max(1.5, 7 / transform.k), 0, 2 * Math.PI);
      ctx.strokeStyle = themeColors.hoveredHalo;
      ctx.lineWidth = Math.max(0.2, 1.5 / transform.k);
      ctx.stroke();
    }

    if (selectedTrack) {
      const pt = projectedTracks.find(t => t.id === selectedTrack.id);
      if (pt) {
        ctx.beginPath();
        ctx.arc(xScale(pt.x), yScale(pt.y), Math.max(2.0, 9 / transform.k), 0, 2 * Math.PI);
        ctx.strokeStyle = themeColors.selectedHalo;
        ctx.lineWidth = Math.max(0.3, 2.2 / transform.k);
        ctx.stroke();

        if (currentThemeStr !== 'accessible') {
          ctx.beginPath();
          ctx.arc(xScale(pt.x), yScale(pt.y), Math.max(2.5, 13 / transform.k), 0, 2 * Math.PI);
          ctx.strokeStyle = themeColors.selectedHaloOuter;
          ctx.lineWidth = Math.max(0.1, 1.0 / transform.k);
          ctx.stroke();
        }
      }
    }

    ctx.restore();
  }

  let zoomBehavior: any;

  function initD3Zoom() {
    if (!canvas) return;
    zoomBehavior = d3.zoom<HTMLCanvasElement, unknown>()
      .scaleExtent([0.5, 12])
      .on('zoom', (event) => { transform = event.transform; });
    d3.select(canvas).call(zoomBehavior);
  }

  function resetZoom() {
    if (!canvas || !zoomBehavior) return;
    d3.select(canvas).transition().duration(750).call(zoomBehavior.transform, d3.zoomIdentity);
  }

  function hitTest(canvasX: number, canvasY: number, radius: number): MappedTrackPoint | null {
    const dataX = xScale.invert((canvasX - transform.x) / transform.k);
    const dataY = yScale.invert((canvasY - transform.y) / transform.k);
    let nearest: MappedTrackPoint | null = null;
    let minDist = radius;
    for (const t of visibleTracks) {
      const d = Math.hypot(t.x - dataX, t.y - dataY);
      if (d < minDist) { minDist = d; nearest = t; }
    }
    return nearest;
  }

  function handleCanvasClick(event: MouseEvent) {
    if (!canvas) return;
    const rect = canvas.getBoundingClientRect();
    const nearest = hitTest(event.clientX - rect.left, event.clientY - rect.top, 5.0);
    if (!nearest) return;
    const fullTrack = library.tracks.find(t => t.id === nearest.id);
    if (fullTrack) player.playTrack(fullTrack);
  }

  function handleCanvasMouseMove(event: MouseEvent) {
    if (!canvas) return;
    const rect = canvas.getBoundingClientRect();
    const mx = event.clientX - rect.left;
    const my = event.clientY - rect.top;
    const nearest = hitTest(mx, my, 3.0);
    if (nearest !== hoveredTrack) hoveredTrack = nearest;
    hoverX = event.clientX;
    hoverY = event.clientY;
  }

  function panToTrack(trackId: number) {
    const node = projectedTracks.find(t => t.id === trackId);
    if (!node || !canvas || !zoomBehavior) return;
    const tx = xScale(node.x);
    const ty = yScale(node.y);
    d3.select(canvas)
      .transition().duration(850)
      .call(
        zoomBehavior.transform as any,
        d3.zoomIdentity.translate(width / 2 - tx * 2, height / 2 - ty * 2).scale(2.2)
      );
  }

  // Focus from "Similar" button in PlayerBar
  $effect(() => {
    if (focusTrackId == null || projectedTracks.length === 0) return;
    panToTrack(focusTrackId);
    focusTrackId = null;
  });

  // Also handle ui.mapFocusTrackId
  $effect(() => {
    if (ui.mapFocusTrackId == null || projectedTracks.length === 0) return;
    panToTrack(ui.mapFocusTrackId);
    ui.mapFocusTrackId = null;
  });

  // Re-fetch stored coordinates whenever musicOnly scope changes.
  // This is a cheap DB read (no UMAP re-run). The user must press
  // "Recompute Map" to get a truly music-only UMAP layout.
  $effect(() => {
    const _scope = filters.musicOnly;
    loadCoordinates();
  });

  $effect(() => { drawCanvas(); });

  let unlistenProj: any;
  let resizeObserver: ResizeObserver;

  onMount(async () => {
    initD3Zoom();

    unlistenProj = await listen('projection-updated', () => loadCoordinates());

    resizeObserver = new ResizeObserver((entries) => {
      const { width: w, height: h } = entries[0].contentRect;
      width  = Math.max(300, Math.floor(w));
      height = Math.max(200, Math.floor(h));
    });
    if (mapContainer) resizeObserver.observe(mapContainer);
  });

  onDestroy(() => {
    unlistenProj?.();
    resizeObserver?.disconnect();
  });
</script>

<div class="map-view" bind:this={mapContainer}>
  <!-- Floating toolbar -->
  <div class="map-toolbar">
    <!-- Track count -->
    <div class="toolbar-badge">
      <code>{visibleTracks.length} / {projectedTracks.length} tracks</code>
    </div>

    <!-- Color coding -->
    <div class="toolbar-group">
      <span class="toolbar-label">COLOR</span>
      <div class="toolbar-toggle">
        {#each [['genre','Genre'],['camelot','Camelot'],['bpm','BPM']] as [val, label]}
          <button
            class="ttog-btn"
            class:ttog-active={colorCoding === val}
            onclick={() => colorCoding = val as 'genre' | 'camelot' | 'bpm'}
          >{label}</button>
        {/each}
      </div>
    </div>

    <!-- Recompute -->
    <button
      class="toolbar-btn toolbar-btn-primary"
      onclick={runProjectionRecompute}
      disabled={isRecomputing || isLoading}
    >
      {#if isRecomputing}
        <span class="spin-icon">
          <svg xmlns="http://www.w3.org/2000/svg" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
            <path d="M21 12a9 9 0 1 1-6.219-8.56"/>
          </svg>
        </span>
        Computing…
      {:else}
        <svg xmlns="http://www.w3.org/2000/svg" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
          <polyline points="23 4 23 10 17 10"/><path d="M20.49 15a9 9 0 1 1-.18-4.51"/>
        </svg>
        Recompute Map
      {/if}
    </button>

    <!-- Hint -->
    <span class="toolbar-hint">Scroll to zoom · Drag to pan · Click dot to play</span>
  </div>

  <!-- Canvas -->
  {#if isLoading}
    <div class="map-loading">
      <span class="spin-icon large">
        <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <path d="M21 12a9 9 0 1 1-6.219-8.56"/>
        </svg>
      </span>
      <span>Loading map…</span>
    </div>
  {:else if projectedTracks.length === 0}
    <div class="map-empty">
      <p>No projection data yet.</p>
      <button class="toolbar-btn toolbar-btn-primary" onclick={runProjectionRecompute} disabled={isRecomputing}>
        Compute Map Now
      </button>
    </div>
  {:else}
    <canvas
      bind:this={canvas}
      {width}
      {height}
      onclick={handleCanvasClick}
      ondblclick={resetZoom}
      onmousemove={handleCanvasMouseMove}
      onmouseleave={() => { hoveredTrack = null; }}
    ></canvas>
  {/if}

  <!-- Hover tooltip (follows cursor) -->
  {#if hoveredTrack}
    <div
      class="hover-tooltip"
      style="left: {hoverX + 14}px; top: {hoverY - 10}px;"
    >
      <span class="ht-title">{hoveredTrack.title || hoveredTrack.filename}</span>
      {#if hoveredTrack.artist}
        <span class="ht-artist">{hoveredTrack.artist}</span>
      {/if}
      <div class="ht-badges">
        {#if hoveredTrack.genre}<span class="ht-badge ht-genre">{hoveredTrack.genre}</span>{/if}
        {#if hoveredTrack.bpm}<span class="ht-badge">{Math.round(hoveredTrack.bpm)} BPM</span>{/if}
        {#if hoveredTrack.key}
          <span class="ht-badge">{hoveredTrack.key} {hoveredTrack.scale ?? ''}</span>
        {/if}
      </div>
    </div>
  {/if}
</div>

<style>
  .map-view {
    position: relative;
    width: 100%;
    height: 100%;
    overflow: hidden;
    background: var(--sg-waveform-bg, #0d1117);
    display: flex;
    flex-direction: column;
  }

  canvas {
    display: block;
    flex: 1;
    min-height: 0;
    width: 100%;
    height: 100%;
    cursor: grab;
  }

  canvas:active { cursor: grabbing; }

  /* ── Toolbar ── */
  .map-toolbar {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: 0.75rem;
    padding: 0.5rem 0.85rem;
    background: var(--sg-surface-slate, #161b22);
    border-bottom: 1px solid var(--sg-surface-high, rgba(255,255,255,0.07));
    flex-wrap: wrap;
  }

  .toolbar-badge code {
    font-family: "JetBrains Mono", monospace;
    font-size: 10px;
    color: var(--sg-outline, #849495);
  }

  .toolbar-group {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .toolbar-label {
    font-family: "JetBrains Mono", monospace;
    font-size: 8px;
    font-weight: 700;
    letter-spacing: 0.1em;
    color: var(--sg-outline, #849495);
  }

  .toolbar-toggle {
    display: flex;
    border: 1px solid rgba(255,255,255,0.1);
    border-radius: 4px;
    overflow: hidden;
  }

  .ttog-btn {
    font-family: "JetBrains Mono", monospace;
    font-size: 10px;
    padding: 4px 10px;
    border: none;
    background: rgba(255,255,255,0.02);
    color: var(--sg-outline, #849495);
    cursor: pointer;
    border-right: 1px solid rgba(255,255,255,0.08);
    transition: all 0.12s;
  }

  .ttog-btn:last-child { border-right: none; }

  .ttog-btn:hover {
    background: rgba(255,255,255,0.06);
    color: var(--sg-on-surface, #e3e1e9);
  }

  .ttog-active {
    background: rgba(0,240,255,0.12) !important;
    color: var(--sg-primary, #00f0ff) !important;
  }

  .toolbar-btn {
    display: flex;
    align-items: center;
    gap: 5px;
    font-family: "JetBrains Mono", monospace;
    font-size: 10px;
    padding: 4px 10px;
    border: 1px solid rgba(255,255,255,0.1);
    border-radius: 4px;
    background: rgba(255,255,255,0.02);
    color: var(--sg-outline, #849495);
    cursor: pointer;
    transition: all 0.12s;
    white-space: nowrap;
  }

  .toolbar-btn:hover {
    color: var(--sg-on-surface, #e3e1e9);
    border-color: rgba(255,255,255,0.2);
    background: rgba(255,255,255,0.05);
  }

  .toolbar-btn-primary {
    border-color: rgba(0,240,255,0.35);
    color: var(--sg-primary, #00f0ff);
    background: rgba(0,240,255,0.08);
  }

  .toolbar-btn-primary:hover:not(:disabled) {
    background: rgba(0,240,255,0.15);
    border-color: var(--sg-primary, #00f0ff);
  }

  .toolbar-btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .toolbar-hint {
    font-family: "JetBrains Mono", monospace;
    font-size: 9px;
    color: var(--sg-outline, #849495);
    opacity: 0.5;
    margin-left: auto;
  }

  /* ── Loading / empty ── */
  .map-loading, .map-empty {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 1rem;
    color: var(--sg-outline, #849495);
    font-family: "JetBrains Mono", monospace;
    font-size: 12px;
  }

  /* ── Spinner ── */
  .spin-icon {
    display: inline-flex;
    animation: spin 1s linear infinite;
  }

  .spin-icon.large { transform-origin: center; }

  @keyframes spin {
    from { transform: rotate(0deg); }
    to   { transform: rotate(360deg); }
  }

  /* ── Hover tooltip ── */
  .hover-tooltip {
    position: fixed;
    z-index: 200;
    pointer-events: none;
    background: var(--sg-surface-slate, #161b22);
    border: 1px solid rgba(0,240,255,0.25);
    border-radius: 5px;
    padding: 6px 10px;
    display: flex;
    flex-direction: column;
    gap: 2px;
    max-width: 240px;
    backdrop-filter: blur(8px);
  }

  .ht-title {
    font-family: Inter, sans-serif;
    font-size: 12px;
    font-weight: 600;
    color: var(--sg-on-surface, #e3e1e9);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .ht-artist {
    font-family: "JetBrains Mono", monospace;
    font-size: 10px;
    color: var(--sg-outline, #849495);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .ht-badges {
    display: flex;
    gap: 4px;
    margin-top: 2px;
    flex-wrap: wrap;
  }

  .ht-badge {
    font-family: "JetBrains Mono", monospace;
    font-size: 8px;
    padding: 2px 6px;
    border-radius: 999px;
    border: 1px solid rgba(255,255,255,0.1);
    color: var(--sg-outline, #849495);
    background: rgba(255,255,255,0.04);
  }

  .ht-genre {
    border-color: rgba(0,240,255,0.25);
    color: var(--sg-primary, #00f0ff);
    background: rgba(0,240,255,0.07);
  }
</style>
