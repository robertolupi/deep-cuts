<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { invoke, convertFileSrc } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import * as d3 from 'd3';
  import type { WatchedDirectory, Track } from '$lib/types';
  import { library } from '$lib/stores/library.svelte';

  import { camelotMap, resolveTrackColor } from '$lib/utils/mapMath';
  import type { MappedTrackPoint } from '$lib/utils/mapMath';

  interface AudioSimilarityResult {
    id: number;
    distance: number;
    title: string | null;
    artist: string | null;
    bpm: number | null;
    key: string | null;
    scale: string | null;
  }

  // Reactive state using Svelte 5 runes
  let tracks = $state<MappedTrackPoint[]>([]);
  let selectedTrack = $state<MappedTrackPoint | null>(null);
  let similarTracks = $state<AudioSimilarityResult[]>([]);
  let isRecomputing = $state(false);
  let isLoading = $state(false);

  // Derived states from the global library store
  const allScannedTracks = $derived(library.tracks);
  const watchedDirs = $derived(library.directories);

  // Collection filter state
  let mapFilterDirId = $state<number | null>(null);
  let simFilterMode = $state<string>('inherit');

  // Sidebar Controls
  let algorithm = $state<'umap' | 'tsne'>('umap');
  let nNeighbors = $state(20);
  let minDist = $state(0.1);
  let perplexity = $state(30.0);
  let colorCoding = $state<'genre' | 'camelot' | 'bpm'>('genre');

  // Canvas size state
  let canvas = $state<HTMLCanvasElement | null>(null);
  let mapContainer = $state<HTMLElement | null>(null);
  let width = $state(760);
  let height = $state(480);
  const padding = 30;

  // D3 Selection and Hover state
  let transform = $state(d3.zoomIdentity);
  let hoveredTrack = $state<MappedTrackPoint | null>(null);

  // Toast status feedback
  let errorMessage = $state('');
  let successMessage = $state('');
  let toastTimeout: any;

  // Theme detection state
  let currentThemeStr = $state("dark");

  $effect(() => {
    const htmlEl = document.documentElement;
    currentThemeStr = htmlEl.getAttribute("data-theme") || "dark";

    const observer = new MutationObserver(() => {
      currentThemeStr = htmlEl.getAttribute("data-theme") || "dark";
    });

    observer.observe(htmlEl, {
      attributes: true,
      attributeFilter: ["data-theme"]
    });

    return () => observer.disconnect();
  });

  // Dynamically compute the top genres and their colors from the database metadata
  const topGenres = $derived.by(() => {
    const counts: Record<string, number> = {};
    for (const t of allScannedTracks) {
      const g = t.genre;
      if (g && g.trim()) {
        const normalized = g.split(/[---,;/]/)[0].trim();
        if (normalized) {
          counts[normalized] = (counts[normalized] || 0) + 1;
        }
      }
    }

    return Object.entries(counts)
      .sort((a, b) => b[1] - a[1])
      .map(entry => entry[0])
      .slice(0, 10);
  });

  const genrePalette = $derived.by(() => {
    if (currentThemeStr === 'accessible') {
      return [
        "#00ffff", // Cyan
        "#ff00ff", // Magenta
        "#ffff00", // Yellow
        "#00ff00", // Green
        "#ff0000", // Red
        "#0080ff", // Blue
        "#ff8000", // Orange
        "#ffffff", // White
        "#00ff80", 
        "#8000ff"
      ];
    } else if (currentThemeStr === 'light') {
      return [
        "#4f46e5", // Indigo
        "#0284c7", // Sky Blue
        "#dc2626", // Red
        "#db2777", // Pink
        "#16a34a", // Green
        "#ea580c", // Orange
        "#9333ea", // Purple
        "#2563eb", // Royal Blue
        "#0d9488", // Teal
        "#b45309"  // Amber/Brown
      ];
    } else {
      // Dark Mode (default)
      return [
        "#00e5ff", // Cyber Cyan
        "#ff007f", // Vibrant Pink/Magenta
        "#8a2be2", // Indigo/Purple
        "#76ff03", // Lime Green
        "#ffeb3b", // Gold Yellow
        "#ff9100", // Orange
        "#00e676", // Emerald Green
        "#2979ff", // Electric Blue
        "#d500f9", // Neon Violet
        "#a1887f"  // Soft Brown
      ];
    }
  });

  const dynamicGenreColors = $derived.by(() => {
    const map: Record<string, string> = {};
    const genres = topGenres;
    const palette = genrePalette;

    genres.forEach((g, i) => {
      map[g] = palette[i % palette.length];
    });

    // Add fallback colors for Other and Unknown
    if (currentThemeStr === 'accessible') {
      map["Other"] = "#a0a0a0";
      map["Unknown"] = "#808080";
    } else if (currentThemeStr === 'light') {
      map["Other"] = "#64748b";
      map["Unknown"] = "#94a3b8";
    } else {
      map["Other"] = "#9e9e9e";
      map["Unknown"] = "#757575";
    }

    return map;
  });

  const themeColors = $derived.by(() => {
    if (currentThemeStr === 'accessible') {
      return {
        selectedHalo: '#ffff00', // Pure yellow
        selectedHaloOuter: 'rgba(255, 255, 0, 0.3)',
        hoveredHalo: '#ffffff',
        dotBorder: '#ffffff',
        dotBorderWidth: 0.8,
        canvasBg: '#000000',
        bpmCool: '#00ffff',
        bpmHot: '#ff00ff'
      };
    } else if (currentThemeStr === 'light') {
      return {
        selectedHalo: '#6366f1', // Primary Cobalt Indigo
        selectedHaloOuter: 'rgba(99, 102, 241, 0.25)',
        hoveredHalo: '#0f172a',
        dotBorder: '#ffffff', // White border for popping on light background
        dotBorderWidth: 0.6,
        canvasBg: '#f8fafc',
        bpmCool: '#0284c7', // Sky Blue
        bpmHot: '#db2777'  // Pink
      };
    } else {
      // Dark Theme (default)
      return {
        selectedHalo: '#00F2FE', // Cyber Cyan
        selectedHaloOuter: 'rgba(0, 242, 254, 0.25)',
        hoveredHalo: '#ffffff',
        dotBorder: 'rgba(10, 11, 16, 0.4)', // Dark border for contrast on dark bg
        dotBorderWidth: 0.5,
        canvasBg: '#0a0b10',
        bpmCool: '#00B0FF',
        bpmHot: '#ff007f'
      };
    }
  });


  // Derived state
  const visibleTracks = $derived.by(() => {
    return mapFilterDirId === null
      ? tracks
      : tracks.filter(t => t.watched_directory_id === mapFilterDirId);
  });

  const effectiveSimDirId = $derived.by(() => {
    return simFilterMode === 'inherit'
      ? mapFilterDirId
      : simFilterMode === 'all'
        ? null
        : parseInt(simFilterMode);
  });

  // Helper Scales
  const xScale = $derived(d3.scaleLinear().domain([0, 100]).range([padding, width - padding]));
  const yScale = $derived(d3.scaleLinear().domain([0, 100]).range([height - padding, padding]));

  function showToast(msg: string, type: 'success' | 'error') {
    clearTimeout(toastTimeout);
    if (type === 'error') {
      errorMessage = msg;
      successMessage = '';
    } else {
      successMessage = msg;
      errorMessage = '';
    }
    toastTimeout = setTimeout(() => {
      errorMessage = '';
      successMessage = '';
    }, 4500);
  }

  function getTrackColor(track: MappedTrackPoint): string {
    return resolveTrackColor(track, colorCoding, dynamicGenreColors, themeColors);
  }

  async function loadCoordinates() {
    isLoading = true;
    try {
      tracks = await invoke<MappedTrackPoint[]>('get_projection_coordinates');
    } catch (err: any) {
      showToast(err.toString(), 'error');
    } finally {
      isLoading = false;
    }
  }


  async function runProjectionRecompute() {
    isRecomputing = true;
    try {
      showToast("Running UMAP projection... (~3s)", 'success');
      const count = await invoke<number>('recompute_projection', {
        algorithm,
        nNeighbors,
        minDist,
        perplexity
      });
      showToast(`Successfully projected ${count} music tracks into 2D space!`, 'success');
      await loadCoordinates();
    } catch (err: any) {
      showToast(err.toString(), 'error');
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

    // Draw all points
    for (const track of visibleTracks) {
      const color = getTrackColor(track);
      ctx.beginPath();
      ctx.arc(xScale(track.x), yScale(track.y), dotR, 0, 2 * Math.PI);
      ctx.fillStyle = color;
      ctx.fill();

      // Draw subtle contrast border (skip in accessible/high-contrast if requested)
      if (currentThemeStr !== 'accessible') {
        ctx.beginPath();
        ctx.arc(xScale(track.x), yScale(track.y), dotR, 0, 2 * Math.PI);
        ctx.strokeStyle = themeColors.dotBorder;
        ctx.lineWidth = Math.max(0.1, themeColors.dotBorderWidth / transform.k);
        ctx.stroke();
      }
    }

    // Highlight hovered track with bright halo
    if (hoveredTrack) {
      ctx.beginPath();
      ctx.arc(xScale(hoveredTrack.x), yScale(hoveredTrack.y), Math.max(1.5, 7 / transform.k), 0, 2 * Math.PI);
      ctx.strokeStyle = themeColors.hoveredHalo;
      ctx.lineWidth = Math.max(0.2, 1.5 / transform.k);
      ctx.stroke();
    }

    // Highlight selected track with thick neon halo
    if (selectedTrack) {
      ctx.beginPath();
      ctx.arc(xScale(selectedTrack.x), yScale(selectedTrack.y), Math.max(2.0, 9 / transform.k), 0, 2 * Math.PI);
      ctx.strokeStyle = themeColors.selectedHalo;
      ctx.lineWidth = Math.max(0.3, 2.2 / transform.k);
      ctx.stroke();

      if (currentThemeStr !== 'accessible') {
        ctx.beginPath();
        ctx.arc(xScale(selectedTrack.x), yScale(selectedTrack.y), Math.max(2.5, 13 / transform.k), 0, 2 * Math.PI);
        ctx.strokeStyle = themeColors.selectedHaloOuter;
        ctx.lineWidth = Math.max(0.1, 1.0 / transform.k);
        ctx.stroke();
      }
    }

    ctx.restore();
  }

  let zoomBehavior: any;

  function initD3Zoom() {
    if (!canvas) return;
    zoomBehavior = d3.zoom<HTMLCanvasElement, unknown>()
      .scaleExtent([0.5, 12])
      .on('zoom', (event) => {
        transform = event.transform;
      });

    d3.select(canvas).call(zoomBehavior);
  }

  function resetZoom() {
    if (!canvas || !zoomBehavior) return;
    d3.select(canvas)
      .transition()
      .duration(750)
      .call(zoomBehavior.transform, d3.zoomIdentity);
  }

  function handleCanvasClick(event: MouseEvent) {
    if (!canvas) return;
    const rect = canvas.getBoundingClientRect();
    const clickX = event.clientX - rect.left;
    const clickY = event.clientY - rect.top;

    const dataX = xScale.invert((clickX - transform.x) / transform.k);
    const dataY = yScale.invert((clickY - transform.y) / transform.k);

    let nearest: MappedTrackPoint | null = null;
    let minDistance = 5.0; // max radius search bound

    for (const t of visibleTracks) {
      const dist = Math.hypot(t.x - dataX, t.y - dataY);
      if (dist < minDistance) {
        minDistance = dist;
        nearest = t;
      }
    }

    if (nearest) {
      selectTrackPoint(nearest);
    }
  }

  function handleCanvasMouseMove(event: MouseEvent) {
    if (!canvas) return;
    const rect = canvas.getBoundingClientRect();
    const mouseX = event.clientX - rect.left;
    const mouseY = event.clientY - rect.top;

    const dataX = xScale.invert((mouseX - transform.x) / transform.k);
    const dataY = yScale.invert((mouseY - transform.y) / transform.k);

    let nearest: MappedTrackPoint | null = null;
    let minDistance = 3.0;

    for (const t of visibleTracks) {
      const dist = Math.hypot(t.x - dataX, t.y - dataY);
      if (dist < minDistance) {
        minDistance = dist;
        nearest = t;
      }
    }

    if (nearest !== hoveredTrack) {
      hoveredTrack = nearest;
    }
  }

  async function selectTrackPoint(track: MappedTrackPoint) {
    selectedTrack = track;
    similarTracks = [];
    await fetchSimilarTracks(track);
  }

  async function fetchSimilarTracks(track: MappedTrackPoint) {
    try {
      const args: any = { trackId: track.id };
      if (effectiveSimDirId !== null) args.directoryId = effectiveSimDirId;
      const simResults = await invoke<AudioSimilarityResult[]>('search_similar_tracks_audio', args);
      similarTracks = simResults;
    } catch (err: any) {
      console.error(err);
    }
  }

  async function refreshSimilarTracks() {
    if (!selectedTrack) return;
    await fetchSimilarTracks(selectedTrack);
  }

  function panToNode(nodeId: number) {
    let node = visibleTracks.find(t => t.id === nodeId);
    if (!node) {
      mapFilterDirId = null;
      node = tracks.find(t => t.id === nodeId);
    }
    if (!node || !canvas || !zoomBehavior) return;

    const targetX = xScale(node.x);
    const targetY = yScale(node.y);

    d3.select(canvas)
      .transition()
      .duration(850)
      .call(
        zoomBehavior.transform as any,
        d3.zoomIdentity.translate(width / 2 - targetX * 2, height / 2 - targetY * 2).scale(2.2)
      )
      .on('end', () => {
        if (node) {
          selectTrackPoint(node);
        }
      });
  }

  // Audio playback state
  let audioSrc = $state("");
  let audioPaused = $state(true);
  let audioCurrentTime = $state(0);
  let audioDuration = $state(0);

  const selectedTrackPath = $derived.by(() => {
    if (!selectedTrack) return "";
    const selectedId = selectedTrack.id;
    const t = allScannedTracks.find(t => t.id === selectedId);
    return t ? convertFileSrc(t.path) : "";
  });

  const isSelectedTrackPlaying = $derived(
    selectedTrackPath && audioSrc === selectedTrackPath && !audioPaused
  );

  function playTrack(trackId: number) {
    const fullTrack = allScannedTracks.find(t => t.id === trackId);
    if (!fullTrack) return;

    const trackSrc = convertFileSrc(fullTrack.path);
    if (audioSrc === trackSrc) {
      audioPaused = !audioPaused;
    } else {
      audioSrc = trackSrc;
      audioPaused = false;
      audioCurrentTime = 0;
    }
  }

  function handlePlayMainClick() {
    if (!selectedTrack) return;
    playTrack(selectedTrack.id);
  }

  function handleAudioEnded() {
    audioPaused = true;
    audioCurrentTime = 0;
  }

  function formatTime(seconds: number): string {
    if (isNaN(seconds) || !isFinite(seconds)) return "0:00";
    const mins = Math.floor(seconds / 60);
    const secs = Math.floor(seconds % 60);
    return `${mins}:${secs < 10 ? '0' : ''}${secs}`;
  }

  // Svelte 5 reactive drawing trigger
  $effect(() => {
    drawCanvas();
  });

  let unlistenProj: any;
  let resizeObserver: ResizeObserver;

  onMount(async () => {
    await loadCoordinates();
    initD3Zoom();

    unlistenProj = await listen('projection-updated', () => {
      loadCoordinates();
    });

    resizeObserver = new ResizeObserver((entries) => {
      const { width: w } = entries[0].contentRect;
      width  = Math.max(300, Math.floor(w));
      height = Math.max(200, Math.floor(w * 0.58));
    });
    if (mapContainer) {
      resizeObserver.observe(mapContainer);
    }
  });

  onDestroy(() => {
    if (unlistenProj) {
      unlistenProj();
    }
    if (resizeObserver) {
      resizeObserver.disconnect();
    }
    clearTimeout(toastTimeout);
  });
</script>

<div class="music-map-layout">
  <!-- Left Side: Controls & Legend -->
  <div class="sidebar glass-panel">
    <div class="header-section">
      <h4>Music Map Controls</h4>
      <p class="desc font-xs">Project acoustic sound signatures down to 2D using advanced dimension reduction math.</p>
    </div>

    <!-- Toast Notifications -->
    {#if errorMessage}
      <div class="toast error-toast font-xs">{errorMessage}</div>
    {/if}
    {#if successMessage}
      <div class="toast success-toast font-xs">{successMessage}</div>
    {/if}

    <div class="divider"></div>

    <!-- Parameter Config Fields -->
    <div class="form-group select-group">
      <label for="algorithm">Dimensionality Reduction</label>
      <select id="algorithm" bind:value={algorithm}>
        <option value="umap">UMAP (Highly Structured Clusters)</option>
        <option value="tsne">t-SNE (Neighborhood Density)</option>
      </select>
    </div>

    {#if algorithm === 'umap'}
      <div class="form-group range-group">
        <label for="n-neighbors">UMAP Neighbors: {nNeighbors}</label>
        <input id="n-neighbors" type="range" min="5" max="50" step="5" bind:value={nNeighbors} />
      </div>
      <div class="form-group range-group">
        <label for="min-dist">UMAP Min Distance: {minDist}</label>
        <input id="min-dist" type="range" min="0.0" max="0.5" step="0.05" bind:value={minDist} />
      </div>
    {:else}
      <div class="form-group range-group">
        <label for="perplexity">t-SNE Perplexity: {perplexity}</label>
        <input id="perplexity" type="range" min="5" max="100" step="5" bind:value={perplexity} />
      </div>
    {/if}

    <button 
      class="btn-primary recompute-btn {isRecomputing ? 'loading' : ''}" 
      onclick={runProjectionRecompute}
      disabled={isRecomputing || (tracks.length === 0 && isLoading)}
    >
      {#if isRecomputing}
        <span class="btn-spinner"></span> Projecting...
      {:else}
        <svg xmlns="http://www.w3.org/2000/svg" width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
          <path d="M21.5 2v6h-6M21.34 15.57a10 10 0 1 1-.57-8.38l5.67-5.67"/>
        </svg>
        Recompute Music Map
      {/if}
    </button>

    <div class="divider"></div>

    <!-- Collection filter for the map canvas -->
    {#if watchedDirs.length > 1}
      <div class="form-group select-group">
        <label for="map-filter">Show on Map</label>
        <select id="map-filter" bind:value={mapFilterDirId}>
          <option value={null}>All Collections</option>
          {#each watchedDirs as dir}
            <option value={dir.id}>{dir.name}</option>
          {/each}
        </select>
      </div>
    {/if}

    <!-- Color Coding legend panel -->
    <div class="form-group select-group">
      <label for="color-coding">Color Coordinates By</label>
      <select id="color-coding" bind:value={colorCoding}>
        <option value="genre">Musical Genre</option>
        <option value="camelot">Camelot Key Scale</option>
        <option value="bpm">BPM (Beats Per Minute)</option>
      </select>
    </div>

    <!-- Legend Display -->
    <div class="legend-panel">
      <span class="legend-hdr font-xxs">Legend Keys</span>
      {#if colorCoding === 'genre'}
        <div class="legend-grid">
          {#each Object.entries(dynamicGenreColors) as [g, col]}
            <div class="legend-item">
              <span class="bullet" style="background-color: {col}"></span>
              <span class="label font-xs">{g}</span>
            </div>
          {/each}
        </div>
      {:else if colorCoding === 'camelot'}
        <div class="camelot-colors-row">
          <div class="c-bullet inner-maj" title="Camelot Major keys (Chromatic Pastel Keys)">Major</div>
          <div class="c-bullet outer-min" title="Camelot Minor keys (Chromatic Vibrant Keys)">Minor</div>
        </div>
        <span class="range-caption font-xxs mt-2">Outer circle minor scales styled with neon chromatic shifts (1A–12A). Inner circle major scales styled with soft pastel scales (1B–12B).</span>
      {:else}
        <div class="gradient-strip">
          <span class="label font-xs">70 BPM (Ambient Blue)</span>
          <div class="gradient-bar"></div>
          <span class="label font-xs">180 BPM (Pink Energetic)</span>
        </div>
      {/if}
    </div>
  </div>

  <!-- Right Side Layout: Top Canvas, Bottom Details -->
  <div class="main-map-column">
    <!-- Map Plot Canvas Glassmorphic wrap -->
    <div class="map-container glass-panel" bind:this={mapContainer}>
      <div class="zoom-instructions">
        <span>🖱️ Scroll to zoom · Double-click to reset view · Click dot to select song</span>
      </div>
      <canvas
        bind:this={canvas}
        width={width}
        height={height}
        onclick={handleCanvasClick}
        ondblclick={resetZoom}
        onmousemove={handleCanvasMouseMove}
        onmouseleave={() => { hoveredTrack = null; }}
      ></canvas>

      <!-- Full-width Hover Detail Pane under Canvas -->
      <div class="hover-detail-pane" class:active={hoveredTrack}>
        {#if hoveredTrack}
          <div class="hover-left-col">
            <span class="hover-title font-sm font-semibold">{hoveredTrack.title || hoveredTrack.filename}</span>
            <span class="hover-artist font-xs text-muted">{hoveredTrack.artist || 'Unknown Artist'}</span>
          </div>
          
          <div class="hover-right-col">
            <span class="badge badge-cyan font-xxs">{hoveredTrack.genre || 'Unknown'}</span>
            <span class="badge badge-magenta font-xxs">
              {Math.round(hoveredTrack.bpm || 120)} BPM
            </span>
            {#if hoveredTrack.key}
              <span class="badge badge-camelot font-xxs" style="text-transform: none">
                {camelotMap[hoveredTrack.scale === 'minor' ? hoveredTrack.key + 'm' : hoveredTrack.key]?.code || '?'} ({hoveredTrack.key} {hoveredTrack.scale || ''})
              </span>
            {/if}
          </div>
        {:else}
          <div class="hover-placeholder-content">
            <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="placeholder-icon">
              <circle cx="12" cy="12" r="10"/>
              <line x1="12" y1="16" x2="12" y2="12"/>
              <line x1="12" y1="8" x2="12.01" y2="8"/>
            </svg>
            <span class="font-xs">Hover over any coordinate point on the music map to inspect quick features instantly...</span>
          </div>
        {/if}
      </div>
    </div>

    <!-- Active Details Panel -->
    {#if selectedTrack}
      <div class="details-panel glass-panel">
        <div class="active-track-header">
          <div class="meta-wrap">
            <h5>{selectedTrack.title || selectedTrack.filename}</h5>
            <span class="desc font-xs">{selectedTrack.artist || 'Unknown Artist'}</span>
            
            <button 
              class="btn-primary btn-play-main font-xxs" 
              onclick={handlePlayMainClick}
            >
              {#if isSelectedTrackPlaying}
                <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="currentColor">
                  <rect x="6" y="4" width="4" height="16"/>
                  <rect x="14" y="4" width="4" height="16"/>
                </svg>
                Pause Song
              {:else}
                <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="currentColor">
                  <polygon points="5 3 19 12 5 21 5 3"/>
                </svg>
                Play Song
              {/if}
            </button>
          </div>

          <div class="meta-badges">
            <span class="badge badge-cyan font-xs">{selectedTrack.genre || 'Unknown Genre'}</span>
            <span class="badge badge-magenta font-xs">{Math.round(selectedTrack.bpm || 120)} BPM</span>
            {#if selectedTrack.key}
              <span class="badge badge-camelot font-xs">
                Key: {camelotMap[selectedTrack.scale === 'minor' ? selectedTrack.key + 'm' : selectedTrack.key]?.code || '?'} ({selectedTrack.key} {selectedTrack.scale || ''})
              </span>
            {/if}
          </div>
        </div>

        <!-- Inline Custom Audio Player -->
        <div class="inline-audio-player">
          <audio 
            src={audioSrc} 
            bind:paused={audioPaused} 
            bind:currentTime={audioCurrentTime} 
            bind:duration={audioDuration}
            onended={handleAudioEnded}
          ></audio>
          
          <button class="btn-play-pause" onclick={handlePlayMainClick} disabled={!audioSrc}>
            {#if !audioPaused}
              <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="currentColor">
                <rect x="6" y="4" width="4" height="16"/>
                <rect x="14" y="4" width="4" height="16"/>
              </svg>
            {:else}
              <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="currentColor">
                <polygon points="5 3 19 12 5 21 5 3"/>
              </svg>
            {/if}
          </button>
          
          <div class="audio-timeline-wrap">
            <input
              type="range"
              min="0"
              max={audioDuration || 0}
              step="0.1"
              bind:value={audioCurrentTime}
              class="audio-scrubber"
              disabled={!audioSrc}
            />
            <div class="audio-time-row">
              <span>{formatTime(audioCurrentTime)}</span>
              <span>{formatTime(audioDuration)}</span>
            </div>
          </div>
        </div>

        <div class="divider"></div>

        <!-- Recommendations Segment -->
        <div class="recommendations-section">
          <div class="rec-section-header">
            <h6>Nearest Neighbors (Acoustic Similarity)</h6>
            {#if watchedDirs.length > 1}
              <div class="sim-filter-wrap">
                <label for="sim-filter" class="font-xxs text-muted">Search in</label>
                <select
                  id="sim-filter"
                  class="sim-filter-select font-xxs"
                  bind:value={simFilterMode}
                  onchange={refreshSimilarTracks}
                >
                  <option value="inherit">{mapFilterDirId === null ? 'All collections' : (watchedDirs.find(d => d.id === mapFilterDirId)?.name ?? 'Same collection')}</option>
                  {#if mapFilterDirId !== null}
                    <option value="all">All collections</option>
                  {/if}
                  {#each watchedDirs as dir}
                    {#if dir.id !== mapFilterDirId}
                      <option value={dir.id.toString()}>{dir.name}</option>
                    {/if}
                  {/each}
                </select>
              </div>
            {/if}
          </div>

          {#if similarTracks.length > 0}
            <div class="rec-grid">
              {#each similarTracks as track, i (track.id)}
                <div class="rec-card">
                  <span class="rec-rank font-xs font-bold">#{i + 1}</span>
                  <div class="rec-meta">
                    <span class="rec-title font-sm font-semibold">{track.title || allScannedTracks.find(t => t.id === track.id)?.filename || 'Unknown Title'}</span>
                    <span class="rec-artist font-xs text-muted">{track.artist || 'Unknown Artist'}</span>
                  </div>
                  
                  <div class="rec-badges">
                    <span class="badge badge-similarity">
                      {Math.round((1 - (track.distance || 0.0) / 2) * 100)}% Match
                    </span>
                    <span class="badge badge-bpm font-xxs">
                      {Math.round(track.bpm || 120)} BPM
                    </span>
                    {#if track.key}
                      <span class="badge badge-camelot font-xxs" style="text-transform: none">
                        {camelotMap[track.scale === 'minor' ? track.key + 'm' : track.key]?.code || '?'} ({track.key} {track.scale || ''})
                      </span>
                    {/if}
                  </div>

                  <div class="rec-actions">
                    <button 
                      class="btn-primary play-btn-small font-xxs" 
                      onclick={() => playTrack(track.id)}
                      title={audioSrc === convertFileSrc(allScannedTracks.find(t => t.id === track.id)?.path ?? '') && !audioPaused ? "Pause Song" : "Play Song"}
                    >
                      {#if audioSrc === convertFileSrc(allScannedTracks.find(t => t.id === track.id)?.path ?? '') && !audioPaused}
                        Pause
                      {:else}
                        Play
                      {/if}
                    </button>
                    <button 
                      class="btn-secondary pan-btn font-xxs" 
                      onclick={() => panToNode(track.id)}
                    >
                      Locate on Map
                    </button>
                  </div>
                </div>
              {/each}
            </div>
          {:else}
            <span class="range-caption">No similar tracks found. Verify that all tracks have been analyzed first.</span>
          {/if}
        </div>
      </div>
    {:else}
      <div class="details-panel empty-selection glass-panel">
        <h5>No Track Selected</h5>
        <p class="font-sm">Click on any music track dot on the UMAP projection map above to preview, load wavesurfer widgets, and fetch similar tracks instantly.</p>
      </div>
    {/if}
  </div>
</div>

<style>
  .music-map-layout {
    display: grid;
    grid-template-columns: 310px 1fr;
    gap: 1.5rem;
    width: 100%;
    height: 100%;
    min-height: 0;
    overflow: hidden;
  }

  .sidebar {
    padding: 1.5rem;
    display: flex;
    flex-direction: column;
    height: 100%;
    overflow-y: auto;
    box-sizing: border-box;
    background: rgba(255, 255, 255, 0.03);
    border: 1px solid var(--border-color);
  }

  .main-map-column {
    display: flex;
    flex-direction: column;
    gap: 1.5rem;
    height: 100%;
    min-height: 0;
    overflow-y: auto;
    padding-bottom: 2rem;
  }

  .map-container {
    width: 100%;
    position: relative;
    padding: 1.25rem;
    box-sizing: border-box;
    display: flex;
    flex-direction: column;
    gap: 1rem;
    align-items: stretch;
  }

  canvas {
    display: block;
    background: var(--bg-main);
    border: 1px solid var(--border-color);
    border-radius: var(--radius-md);
    cursor: grab;
  }

  canvas:active {
    cursor: grabbing;
  }

  .zoom-instructions {
    position: absolute;
    top: 1.5rem;
    left: 2rem;
    background: var(--bg-main);
    border: 1px solid var(--border-color);
    padding: 0.4rem 0.8rem;
    border-radius: var(--radius-sm);
    color: var(--text-muted);
    font-size: 0.68rem;
    font-weight: 600;
  }

  h4 {
    font-size: 1.15rem;
    font-weight: 700;
    margin-bottom: 0.25rem;
    border-left: 3px solid var(--color-primary);
    padding-left: 10px;
  }

  h5 {
    font-size: 1.1rem;
    font-weight: 700;
    color: var(--text-primary);
    margin-bottom: 0.2rem;
  }

  h6 {
    font-size: 0.85rem;
    font-weight: 700;
    color: var(--color-accent-cyan);
    margin-bottom: 1rem;
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  .desc {
    color: var(--text-muted);
    line-height: 1.4;
  }

  .divider {
    height: 1px;
    background: var(--border-color);
    margin: 1.5rem 0;
    width: 100%;
  }

  .form-group {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
    margin-bottom: 1.25rem;
    width: 100%;
  }

  .form-group label {
    font-size: 0.72rem;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--text-secondary);
  }

  select {
    background: var(--bg-card);
    border: 2px solid var(--border-color);
    padding: 0.6rem;
    border-radius: var(--radius-md);
    color: var(--text-primary);
    font-size: 0.82rem;
    outline: none;
    transition: var(--transition-fast);
  }
  select:focus {
    border-color: var(--color-primary);
  }

  input[type="range"] {
    -webkit-appearance: none;
    appearance: none;
    width: 100%;
    height: 4px;
    border-radius: 2px;
    background: var(--border-color);
    outline: none;
  }

  input[type="range"]::-webkit-slider-thumb {
    -webkit-appearance: none;
    appearance: none;
    width: 12px;
    height: 12px;
    border-radius: 50%;
    background: var(--color-primary);
    cursor: pointer;
    box-shadow: var(--cyan-glow);
  }

  .range-caption {
    font-size: 0.65rem;
    color: var(--text-muted);
    line-height: 1.3;
  }

  .recompute-btn {
    width: 100%;
    padding: 0.7rem;
    justify-content: center;
    font-size: 0.85rem;
    display: flex;
    align-items: center;
    gap: 0.4rem;
  }

  .legend-panel {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    width: 100%;
  }

  .legend-hdr {
    font-size: 0.72rem;
    font-weight: 700;
    text-transform: uppercase;
    color: var(--text-muted);
    margin-bottom: 0.25rem;
  }

  .legend-grid {
    display: grid;
    grid-template-columns: repeat(2, 1fr);
    gap: 0.5rem;
  }

  .legend-item {
    display: flex;
    align-items: center;
    gap: 0.4rem;
  }

  .legend-item .bullet {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    display: inline-block;
    flex-shrink: 0;
  }

  .legend-item .label {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    color: var(--text-secondary);
  }

  .camelot-colors-row {
    display: flex;
    gap: 1rem;
  }

  .c-bullet {
    padding: 0.25rem 0.6rem;
    border-radius: var(--radius-sm);
    font-size: 0.68rem;
    font-weight: bold;
    text-transform: uppercase;
  }

  .inner-maj {
    background: rgba(165, 214, 167, 0.15);
    border: 1px solid #a5d6a7;
    color: #a5d6a7;
  }

  .outer-min {
    background: rgba(0, 242, 254, 0.15);
    border: 1px solid #00F2FE;
    color: #00F2FE;
  }

  .gradient-strip {
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
  }

  .gradient-bar {
    height: 8px;
    border-radius: var(--radius-sm);
    background: linear-gradient(to right, #00B0FF, #76FF03, #FFEA00, #F50057);
    width: 100%;
  }

  .hover-detail-pane {
    width: 100%;
    height: 58px;
    background: var(--bg-card);
    border: 2px solid var(--border-color);
    border-radius: var(--radius-md);
    padding: 0.75rem 1.25rem;
    box-sizing: border-box;
    display: flex;
    align-items: center;
    justify-content: space-between;
    transition: all 0.2s ease-in-out;
  }

  .hover-detail-pane.active {
    background: rgba(255, 0, 127, 0.03);
    border-color: rgba(255, 0, 127, 0.25);
    box-shadow: 0 0 15px rgba(255, 0, 127, 0.05);
  }

  .hover-left-col {
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
    text-align: left;
    max-width: 60%;
  }

  .hover-title {
    color: var(--text-primary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .hover-artist {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .hover-right-col {
    display: flex;
    gap: 0.5rem;
    align-items: center;
  }

  .hover-placeholder-content {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    color: var(--text-muted);
    font-weight: 500;
  }

  .placeholder-icon {
    color: var(--color-primary);
  }

  .details-panel {
    padding: 1.5rem;
    box-sizing: border-box;
    display: flex;
    flex-direction: column;
    background: var(--bg-card);
    border: 2px solid var(--border-color);
    border-radius: var(--radius-lg);
  }

  .empty-selection {
    align-items: center;
    justify-content: center;
    text-align: center;
    color: var(--text-secondary);
    min-height: 200px;
  }

  .active-track-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1.5rem;
    width: 100%;
  }

  .active-track-header .meta-wrap {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    max-width: 350px;
  }

  .meta-badges {
    display: flex;
    gap: 0.5rem;
    align-items: center;
    flex-wrap: wrap;
  }

  .btn-play-main {
    display: inline-flex;
    align-items: center;
    gap: 0.4rem;
    padding: 0.4rem 0.8rem;
    align-self: start;
    cursor: pointer;
  }

  .recommendations-section {
    width: 100%;
  }

  .rec-section-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    flex-wrap: wrap;
    gap: 0.5rem;
    margin-bottom: 0.75rem;
  }

  .rec-section-header h6 {
    margin: 0;
  }

  .sim-filter-wrap {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    flex-shrink: 0;
  }

  .sim-filter-select {
    font-size: 0.7rem;
    padding: 0.2rem 0.4rem;
    background: var(--bg-main);
    border: 2px solid var(--border-color);
    border-radius: var(--radius-sm);
    color: var(--text-secondary);
    cursor: pointer;
  }

  .sim-filter-select:focus {
    outline: none;
    border-color: var(--color-primary);
  }

  .rec-grid {
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
    max-height: 350px;
    overflow-y: auto;
    padding-right: 0.5rem;
  }

  .rec-card {
    display: flex;
    align-items: center;
    gap: 1.25rem;
    background: var(--bg-card);
    border: 2px solid var(--border-color);
    padding: 0.6rem 1rem;
    border-radius: var(--radius-md);
    transition: var(--transition-fast);
  }

  .rec-card:hover {
    background: var(--bg-card-hover);
    border-color: var(--color-primary);
  }

  .rec-rank {
    color: var(--text-muted);
  }

  .rec-meta {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-width: 0;
  }

  .rec-title {
    color: var(--text-primary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .rec-artist {
    color: var(--text-muted);
    font-weight: 400;
    font-size: 0.72rem;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .rec-badges {
    display: flex;
    gap: 0.4rem;
    flex-shrink: 0;
  }

  .badge-similarity {
    background: rgba(0, 242, 254, 0.08);
    border: 1px solid rgba(0, 242, 254, 0.2);
    color: var(--color-accent-cyan);
    font-weight: 700;
  }

  .rec-actions {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    flex-shrink: 0;
  }

  .play-btn-small {
    padding: 0.3rem 0.6rem;
    cursor: pointer;
  }

  .pan-btn {
    white-space: nowrap;
    font-size: 0.72rem;
    padding: 0.3rem 0.6rem;
  }

  .toast {
    padding: 0.6rem 0.8rem;
    border-radius: var(--radius-sm);
    margin-top: 0.5rem;
    animation: fadeIn 0.2s ease-out;
  }
  
  .error-toast {
    background: rgba(255, 23, 68, 0.15);
    border: 1px solid rgba(255, 23, 68, 0.4);
    color: #ff1744;
  }

  .success-toast {
    background: rgba(0, 229, 255, 0.15);
    border: 1px solid rgba(0, 229, 255, 0.4);
    color: #00e5ff;
  }

  @keyframes fadeIn {
    from { opacity: 0; transform: translateY(-5px); }
    to { opacity: 1; transform: translateY(0); }
  }

  .inline-audio-player {
    display: flex;
    align-items: center;
    gap: 1rem;
    width: 100%;
    margin-top: 1rem;
    padding: 0.75rem 1.25rem;
    background: var(--bg-main);
    border: 2px solid var(--border-color);
    border-radius: var(--radius-md);
  }

  .btn-play-pause {
    width: 40px;
    height: 40px;
    border-radius: 50%;
    background: linear-gradient(135deg, var(--color-primary), #6366f1);
    border: none;
    color: #fff;
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    box-shadow: 0 4px 10px rgba(99, 102, 241, 0.2);
    transition: var(--transition-fast);
    flex-shrink: 0;
  }

  :global(html[data-theme="accessible"]) .btn-play-pause {
    background: #000;
    color: #fff;
    border: 3px solid #fff;
    box-shadow: none;
    border-radius: 0;
  }

  .btn-play-pause:hover {
    transform: scale(1.06);
    box-shadow: 0 6px 15px rgba(99, 102, 241, 0.4);
  }

  :global(html[data-theme="accessible"]) .btn-play-pause:hover {
    background: #fff;
    color: #000;
    transform: none;
  }

  .btn-play-pause:disabled {
    opacity: 0.5;
    cursor: not-allowed;
    transform: none;
    box-shadow: none;
  }

  .audio-timeline-wrap {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    flex: 1;
    min-width: 0;
  }

  .audio-scrubber {
    -webkit-appearance: none;
    appearance: none;
    width: 100%;
    height: 6px;
    border-radius: 3px;
    background: var(--border-color);
    outline: none;
    cursor: pointer;
  }

  .audio-scrubber::-webkit-slider-thumb {
    -webkit-appearance: none;
    appearance: none;
    width: 12px;
    height: 12px;
    border-radius: 50%;
    background: var(--color-accent-cyan);
    cursor: pointer;
    box-shadow: var(--cyan-glow);
    transition: transform 0.1s ease;
  }

  .audio-scrubber::-webkit-slider-thumb:hover {
    transform: scale(1.2);
  }

  .audio-time-row {
    display: flex;
    justify-content: space-between;
    font-size: 0.72rem;
    color: var(--text-muted);
    font-weight: 500;
  }
</style>
