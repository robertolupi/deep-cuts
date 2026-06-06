<script lang="ts">
  import { onMount, onDestroy, untrack } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import * as d3 from 'd3';
  import { library } from '$lib/stores/library.svelte';
  import { filters } from '$lib/stores/filters.svelte';
  import { player } from '$lib/stores/player.svelte';
  import { theme } from '$lib/stores/theme.svelte';
  import { ui } from '$lib/stores/ui.svelte';
  import { curation } from '$lib/stores/curation.svelte';

  import { camelotMap, resolveTrackColor, STRUCTURE_CLUSTER_COLORS, STRUCTURE_CLUSTER_LABELS, STRUCTURE_CLUSTER_REGEX } from '$lib/utils/mapMath';
  import type { MappedTrackPoint } from '$lib/utils/mapMath';

  // Optional prop: when set, the map will pan to and select this track
  let { focusTrackId = $bindable(null) }: { focusTrackId?: number | null } = $props();

  let projectedTracks = $state<MappedTrackPoint[]>([]);
  let isRecomputing   = $state(false);
  let isLoading       = $state(false);
  let algorithm       = $state<'pca' | 'umap'>('pca');

  let colorCoding = $state<'genre' | 'camelot' | 'bpm' | 'mood' | 'structure'>('genre');

  // Legend state
  let legendOpen        = $state(true);
  let legendInteracting = $state(false);
  let legendHideTimer: ReturnType<typeof setTimeout> | null = null;

  // Map Sonic vibe states
  let searchQuery = $state("");
  let similarityScores = $state<Map<number, number>>(new Map());
  let isSearchingSimilarity = $state(false);

  // Map Mode and Blend Weight Settings
  let mapMode = $state<'sonic' | 'description' | 'hybrid' | 'essentia' | 'harmonic' | 'genre_wheel'>('hybrid');
  let blendWeight = $state(0.5); // 0.0 (semantic) to 1.0 (sonic)

  // Default to sonic similarity if Qwen analysis has not been run for all tracks
  let hasCheckedQwen = $state(false);
  $effect(() => {
    if (!hasCheckedQwen && library.tracks.length > 0) {
      const allQwenAnalyzed = library.tracks.every(
        t => t.description !== null && t.description !== undefined && t.description.trim() !== ""
      );
      if (!allQwenAnalyzed) {
        mapMode = 'sonic';
      }
      hasCheckedQwen = true;
    }
  });

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
    for (const t of projectedTracks) {
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

  // Derived D3 contour density heatmap from similarity scores and track coordinates
  const contours = $derived.by(() => {
    if (similarityScores.size === 0 || visibleTracks.length === 0) return [];
    
    const weightedPoints = visibleTracks
      .map(t => {
        const score = similarityScores.get(t.id) ?? 0;
        return {
          x: xScale(t.x),
          y: yScale(t.y),
          weight: score
        };
      })
      .filter(p => p.weight > 0);

    if (weightedPoints.length === 0) return [];

    try {
      const bw = Math.max(15, Math.min(width, height) / 18);
      const densityGenerator = d3.contourDensity<any>()
        .x(d => d.x)
        .y(d => d.y)
        .weight(d => d.weight)
        .size([width, height])
        .bandwidth(bw)
        .thresholds(12);

      return densityGenerator(weightedPoints);
    } catch (e) {
      console.error("Failed to generate density contours:", e);
      return [];
    }
  });

  // Query similarity for map sonic search
  async function runSimilarityQuery() {
    const q = searchQuery.trim();
    if (!q) {
      similarityScores = new Map();
      return;
    }
    isSearchingSimilarity = true;
    try {
      const weight = mapMode === 'sonic' ? 1.0 : mapMode === 'description' ? 0.0 : blendWeight;
      const results = await invoke<{ id: number; score: number }[]>("search_hybrid_vibe", {
        query: q,
        clapWeight: weight,
        limit: library.tracks.length || 5000,
      });
      const newScores = new Map<number, number>();
      for (const r of results) {
        newScores.set(r.id, r.score);
      }
      similarityScores = newScores;
    } catch (err: any) {
      ui.showToast(`Vibe query failed: ${err.toString()}`, "error");
    } finally {
      isSearchingSimilarity = false;
    }
  }



  function getTrackColor(track: MappedTrackPoint): string {
    return resolveTrackColor(track, colorCoding, dynamicGenreColors, themeColors);
  }

  async function loadCoordinates() {
    isLoading = true;
    try {
      projectedTracks = await invoke<MappedTrackPoint[]>('get_projection_coordinates', {
        musicOnly: filters.musicOnly,
      });
      if (projectedTracks.length > 0) {
        const storedAlgo = projectedTracks[0].algorithm;
        if (storedAlgo === 'umap' || storedAlgo === 'pca') {
          algorithm = storedAlgo;
        }
      } else if (!isRecomputing) {
        // Automatically compute map coordinates (defaulting to fast PCA) if none exist
        runProjectionRecompute('pca');
      }
    } catch (err: any) {
      ui.showToast(err.toString(), 'error');
    } finally {
      isLoading = false;
    }
  }

  async function runProjectionRecompute(algoOverride?: 'pca' | 'umap') {
    if (algoOverride) {
      algorithm = algoOverride;
    }
    isRecomputing = true;
    try {
      if (mapMode === 'harmonic') {
        ui.showToast('Calculating Harmonic Circle layout…', 'success');
      } else if (mapMode === 'essentia') {
        ui.showToast('Calculating Emotive Mood Circle layout…', 'success');
      } else if (mapMode === 'genre_wheel') {
        ui.showToast('Calculating Genre Circle layout…', 'success');
      } else if (algorithm === 'umap') {
        ui.showToast('Running UMAP projection… this may take a few seconds', 'success');
      } else {
        ui.showToast('Running PCA projection…', 'success');
      }
      const weight = mapMode === 'sonic' ? 1.0 : mapMode === 'description' ? 0.0 : blendWeight;
      const count = await invoke<number>('recompute_projection', {
        musicOnly: filters.musicOnly,
        clapWeight: weight,
        algorithm,
        nNeighbors: 20,
        minDist: 0.1,
        perplexity: 30,
        projectionMode: mapMode,
      });
      if (mapMode === 'harmonic') {
        ui.showToast(`Mapped ${count} tracks using Harmonic Circle Layout`, 'success');
      } else if (mapMode === 'essentia') {
        ui.showToast(`Mapped ${count} tracks using Emotive Mood Circle Layout`, 'success');
      } else if (mapMode === 'genre_wheel') {
        ui.showToast(`Mapped ${count} tracks using Genre Circle Layout`, 'success');
      } else {
        ui.showToast(`Projected ${count} tracks into 2D space using ${algorithm.toUpperCase()}`, 'success');
      }
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

    // Draw D3 contours/density heatmap as glowing background overlays
    if (contours && contours.length > 0) {
      ctx.save();
      ctx.globalAlpha = 0.16; // soft opacity for glowing contour layout
      
      const maxVal = Math.max(...contours.map(c => c.value), 1e-6);
      const minVal = Math.max(1e-5, Math.min(...contours.map(c => c.value)));
      const logMax = Math.log(maxVal);
      const logMin = Math.log(minVal);
      const geoPath = d3.geoPath().context(ctx);
      
      for (const contour of contours) {
        ctx.beginPath();
        geoPath(contour);
        
        const val = Math.max(minVal, contour.value);
        const normVal = logMax === logMin ? 0.5 : (Math.log(val) - logMin) / (logMax - logMin);
        let color = "";
        
        if (currentThemeStr === 'accessible') {
          // Yellow-to-Purple high contrast gradient
          color = d3.interpolatePlasma(normVal * 0.95);
        } else if (currentThemeStr === 'light') {
          // Smooth light-mode Harmonious Cool colors (Blues/Teals)
          color = d3.interpolateCool(normVal * 0.85);
        } else {
          // Dark/Neon themed glowing palette (Magma/Plasma spectrum)
          color = d3.interpolateMagma(normVal * 0.85 + 0.1);
        }
        
        ctx.fillStyle = color;
        ctx.fill();
      }
      ctx.restore();
    }

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
      .on('start', () => {
        legendInteracting = true;
        if (legendHideTimer !== null) { clearTimeout(legendHideTimer); legendHideTimer = null; }
      })
      .on('zoom', (event) => { transform = event.transform; })
      .on('end', () => {
        legendHideTimer = setTimeout(() => { legendInteracting = false; legendHideTimer = null; }, 800);
      });
    d3.select(canvas).call(zoomBehavior);

    // Restore current transform so zoom doesn't jump.
    // Use untrack to avoid this effect re-running on every zoom event (transform is $state).
    zoomBehavior.transform(d3.select(canvas), untrack(() => transform));

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

  /**
   * Converts a client-space point to canvas-internal pixel coordinates.
   * Necessary because the canvas HTML `width`/`height` attributes are set from
   * the mapContainer (which includes the toolbar), while the canvas CSS-renders
   * at a smaller height. Using raw `clientX - rect.left` coordinates without
   * this scaling causes the hit target to be shifted upward.
   */
  function getCanvasCoords(clientX: number, clientY: number): [number, number] {
    const rect = canvas!.getBoundingClientRect();
    return [
      (clientX - rect.left) * (canvas!.width  / rect.width),
      (clientY - rect.top)  * (canvas!.height / rect.height),
    ];
  }

  function handleCanvasClick(event: MouseEvent) {
    if (!canvas) return;
    const [cx, cy] = getCanvasCoords(event.clientX, event.clientY);
    const nearest = hitTest(cx, cy, 5.0);
    if (!nearest) return;
    const fullTrack = library.tracks.find(t => t.id === nearest.id);
    if (fullTrack) player.playTrack(fullTrack);
  }

  function handleCanvasMouseMove(event: MouseEvent) {
    if (!canvas) return;
    const [mx, my] = getCanvasCoords(event.clientX, event.clientY);
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
        d3.zoomIdentity.translate(width / 2 - tx * 6, height / 2 - ty * 6).scale(6)
      );
  }

  // Focus from "Similar" button in PlayerBar
  $effect(() => {
    if (focusTrackId == null || projectedTracks.length === 0) return;
    panToTrack(focusTrackId);
    focusTrackId = null;
  });

  // Also handle ui.mapFocusTrackId (e.g. "Locate on Map" from PlayerBar)
  $effect(() => {
    if (ui.mapFocusTrackId == null || projectedTracks.length === 0) return;
    const id = ui.mapFocusTrackId;
    ui.mapFocusTrackId = null;
    if (zoomBehavior) {
      panToTrack(id);
    } else {
      // Component just mounted — zoomBehavior not ready yet, retry after layout
      setTimeout(() => panToTrack(id), 150);
    }
  });

  // Re-fetch stored coordinates whenever musicOnly scope changes.
  // This is a cheap DB read (no UMAP re-run). The user must press
  // "Recompute Map" to get a truly music-only UMAP layout.
  $effect(() => {
    const _scope = filters.musicOnly;
    loadCoordinates();
  });

  $effect(() => { drawCanvas(); });

  $effect(() => {
    if (canvas) {
      initD3Zoom();
    }
  });

  let unlistenProj: any;
  let resizeObserver: ResizeObserver;

  onMount(async () => {
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
    <!-- Track count and active algorithm -->
    <div class="toolbar-badge" style="display: flex; align-items: center; gap: 6px;">
      {#if isRecomputing}
        <span class="spin-icon" style="color: var(--sg-primary, #00f0ff);">
          <svg xmlns="http://www.w3.org/2000/svg" width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round">
            <path d="M21 12a9 9 0 1 1-6.219-8.56"/>
          </svg>
        </span>
      {/if}
      <code>
        {visibleTracks.length} / {projectedTracks.length} tracks
        {#if projectedTracks.length > 0}
          · {mapMode === 'harmonic' ? 'HARMONIC' : mapMode === 'essentia' ? 'EMOTIVE' : mapMode === 'genre_wheel' ? 'GENRE' : algorithm.toUpperCase()}
        {/if}
      </code>
    </div>

    <!-- Color coding -->
    <div class="toolbar-group">
      <span class="toolbar-label">COLOR</span>
      <div class="toolbar-toggle">
        {#each [['genre','Genre'],['camelot','Camelot'],['bpm','BPM'],['mood','Mood'],['structure','Structure']] as [val, label]}
          <button
            class="ttog-btn"
            class:ttog-active={colorCoding === val}
            onclick={() => colorCoding = val as any}
          >{label}</button>
        {/each}
      </div>
    </div>

    <!-- Map Mode Selection -->
    <div class="toolbar-group">
      <span class="toolbar-label">MODE</span>
      <div class="toolbar-toggle">
        {#each [['sonic','Sonic'],['description','Description'],['hybrid','Hybrid'],['essentia','Mood'],['harmonic','Harmonic'],['genre_wheel','Genre']] as [val, label]}
          <button
            class="ttog-btn"
            class:ttog-active={mapMode === val}
            onclick={() => {
              mapMode = val as any;
              if (mapMode === 'harmonic') {
                colorCoding = 'camelot';
              } else if (mapMode === 'essentia') {
                colorCoding = 'mood';
              } else if (mapMode === 'genre_wheel') {
                colorCoding = 'genre';
              }
              runProjectionRecompute();
              if (searchQuery) runSimilarityQuery();
            }}
          >{label}</button>
        {/each}
      </div>
    </div>

    <!-- Blend Weight Slider (visible only in Hybrid mode) -->
    {#if mapMode === 'hybrid'}
      <div class="toolbar-group blend-slider-group">
        <span class="toolbar-label">BLEND</span>
        <div style="display: flex; align-items: center; gap: 8px;">
          <span class="slider-side-label">Qwen</span>
          <input
            type="range"
            min="0"
            max="1"
            step="0.05"
            bind:value={blendWeight}
            onchange={() => {
              runProjectionRecompute();
              if (searchQuery) runSimilarityQuery();
            }}
            class="blend-slider"
          />
          <span class="slider-side-label">CLAP</span>
          <span class="blend-percent-badge">{Math.round(blendWeight * 100)}%</span>
        </div>
      </div>
    {/if}

    {#if mapMode !== 'harmonic' && mapMode !== 'essentia' && mapMode !== 'genre_wheel'}
      <!-- Algorithm toggle -->
      <div class="toolbar-group">
        <span class="toolbar-label">PROJECTION</span>
        <div class="toolbar-toggle">
          <button
            class="ttog-btn"
            class:ttog-active={algorithm === 'pca'}
            onclick={() => {
              if (algorithm !== 'pca') {
                runProjectionRecompute('pca');
              }
            }}
            disabled={isRecomputing || isLoading}
          >
            PCA
          </button>
          <button
            class="ttog-btn"
            class:ttog-active={algorithm === 'umap'}
            onclick={() => {
              if (algorithm !== 'umap') {
                runProjectionRecompute('umap');
              }
            }}
            disabled={isRecomputing || isLoading}
          >
            UMAP
          </button>
        </div>
      </div>
    {/if}

    <!-- Sonic Vibe Search -->
    <div class="toolbar-group sonic-search-group">
      <span class="toolbar-label">SONIC VIBE</span>
      <div class="search-input-wrapper">
        <input
          type="text"
          bind:value={searchQuery}
          placeholder="Search vibe (e.g. ambient)..."
          onkeydown={(e) => {
            if (e.key === 'Enter') {
              runSimilarityQuery();
            }
          }}
          class="sonic-search-input"
        />
        {#if searchQuery}
          <button class="search-clear-btn" onclick={() => { searchQuery = ""; runSimilarityQuery(); }}>×</button>
        {/if}
      </div>
      <button 
        class="ttog-btn action-btn-accent" 
        onclick={runSimilarityQuery}
        disabled={isSearchingSimilarity}
      >
        {#if isSearchingSimilarity}
          Searching...
        {:else}
          Query
        {/if}
      </button>

    </div>

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
      <p>No audio features analysed yet. Go to the Library and click "Start Analysis" to generate embeddings.</p>
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

  <!-- Map legend -->
  <div
    class="map-legend"
    class:legend-hidden={legendInteracting}
    class:legend-collapsed={!legendOpen}
  >
    <button class="legend-toggle" onclick={() => legendOpen = !legendOpen} title={legendOpen ? 'Collapse legend' : 'Expand legend'}>
      <span class="legend-toggle-icon">{legendOpen ? '▾' : '▴'}</span>
      <span class="legend-title">
        {#if colorCoding === 'genre'}GENRES
        {:else if colorCoding === 'camelot'}CAMELOT
        {:else if colorCoding === 'bpm'}BPM
        {:else if colorCoding === 'structure'}STRUCTURE
        {:else}MOOD
        {/if}
      </span>
    </button>

    {#if legendOpen}
      <div class="legend-body">
        {#if colorCoding === 'genre'}
          {#each [...topGenres, 'Other', 'Unknown'] as genre}
            {@const color = dynamicGenreColors[genre] ?? '#999'}
            <div class="legend-row">
              <span class="legend-swatch" style="background:{color};"></span>
              <span class="legend-label">{genre}</span>
            </div>
          {/each}

        {:else if colorCoding === 'mood'}
          {#each [
            ['Happy',      '#ffeb3b'],
            ['Sad',        '#2979ff'],
            ['Aggressive', '#ff1744'],
            ['Relaxed',    '#00e676'],
            ['Party',      '#d500f9'],
            ['Acoustic',   '#ff9100'],
            ['Electronic', '#00e5ff'],
          ] as [label, color]}
            <div class="legend-row">
              <span class="legend-swatch" style="background:{color};"></span>
              <span class="legend-label">{label}</span>
            </div>
          {/each}

        {:else if colorCoding === 'camelot'}
          <div class="legend-camelot">
            <div class="legend-camelot-col">
              <span class="legend-col-header">MINOR</span>
                {#each [
                ['Abm','#00E5FF'],['Ebm','#00B0FF'],['Bbm','#2979FF'],
                ['Fm', '#651FFF'],['Cm', '#AA00FF'],['Gm', '#D500F9'],
                ['Dm', '#F50057'],['Am', '#FF1744'],['Em', '#FF9100'],
                ['Bm', '#FFEA00'],['F#m','#76FF03'],['C#m','#00E676'],
              ] as [key, color]}
                <div class="legend-row">
                  <span class="legend-swatch" style="background:{color};"></span>
                  <span class="legend-label">{key}</span>
                </div>
              {/each}
            </div>
            <div class="legend-camelot-col">
              <span class="legend-col-header">MAJOR</span>
              {#each [
                ['B', '#80DEEA'],['F#','#82B1FF'],['C#','#8C9EFF'],
                ['Ab','#B388FF'],['Eb','#EA80FC'],['Bb','#FF80AB'],
                ['F', '#FF8A80'],['C', '#FFE082'],['G', '#FFF59D'],
                ['D', '#C6FF00'],['A', '#A7FFEB'],['E', '#A5D6A7'],
              ] as [key, color]}
                <div class="legend-row">
                  <span class="legend-swatch" style="background:{color};"></span>
                  <span class="legend-label">{key}</span>
                </div>
              {/each}
            </div>
          </div>

        {:else if colorCoding === 'structure'}
          {#each Object.entries(STRUCTURE_CLUSTER_LABELS) as [id, label]}
            {@const color = STRUCTURE_CLUSTER_COLORS[Number(id) % STRUCTURE_CLUSTER_COLORS.length]}
            {@const rx = STRUCTURE_CLUSTER_REGEX[Number(id)]}
            <div
              class="legend-row legend-row-clickable"
              role="button"
              tabindex="0"
              title={rx}
              onclick={() => { filters.structureFilter = rx; }}
              onkeydown={(e) => e.key === 'Enter' && (filters.structureFilter = rx)}
            >
              <span class="legend-swatch" style="background:{color};"></span>
              <span class="legend-label">{label}</span>
            </div>
          {/each}
          <div class="legend-row">
            <span class="legend-swatch" style="background:#333340;"></span>
            <span class="legend-label">Unclassified</span>
          </div>

        {:else}
          <!-- BPM gradient bar -->
          <div class="legend-bpm">
            <div
              class="legend-bpm-bar"
              style="background: linear-gradient(to right, {themeColors.bpmCool}, {themeColors.bpmHot});"
            ></div>
            <div class="legend-bpm-labels">
              <span>70</span>
              <span>180 BPM</span>
            </div>
          </div>
        {/if}
      </div>
    {/if}
  </div>

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
        {#if filters.semanticQuery.trim() && filters.semanticTrackScores.has(hoveredTrack.id)}
          {@const score = filters.semanticTrackScores.get(hoveredTrack.id)}
          {#if score !== undefined}
            <span class="ht-badge ht-score">{Math.round(score)}%</span>
          {/if}
        {/if}
        {#if filters.clapQuery.trim() && filters.clapTrackScores.has(hoveredTrack.id)}
          {@const score = filters.clapTrackScores.get(hoveredTrack.id)}
          {#if score !== undefined}
            <span class="ht-badge ht-score-clap">{Math.round(score)}%</span>
          {/if}
        {/if}
        {#if colorCoding === 'structure' && hoveredTrack.structure_cluster_id != null}
          <span class="ht-badge ht-structure">{STRUCTURE_CLUSTER_LABELS[hoveredTrack.structure_cluster_id] ?? `Cluster ${hoveredTrack.structure_cluster_id}`}</span>
        {/if}
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
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-xs);
    color: var(--sg-outline, #849495);
  }

  .toolbar-group {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .toolbar-label {
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-3xs);
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
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-xs);
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

  .toolbar-hint {
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-2xs);
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
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-base);
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
    font-family: var(--sg-font-ui);
    font-size: var(--sg-text-base);
    font-weight: 600;
    color: var(--sg-on-surface, #e3e1e9);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .ht-artist {
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-xs);
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
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-3xs);
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

  .ht-score {
    border-color: rgba(0, 240, 255, 0.45);
    color: var(--sg-primary, #00f0ff);
    background: rgba(0, 240, 255, 0.12);
    font-weight: 700;
  }

  .ht-score-clap {
    border-color: rgba(254, 0, 254, 0.45);
    color: var(--sg-secondary, #fe00fe);
    background: rgba(254, 0, 254, 0.12);
    font-weight: 700;
  }

  .ht-structure {
    border-color: rgba(255, 200, 80, 0.3);
    color: #ffc850;
    background: rgba(255, 200, 80, 0.08);
    font-family: monospace;
  }

  /* ── Sonic Vibe Search Styles ── */
  .sonic-search-group {
    margin-left: 0.5rem;
  }

  .search-input-wrapper {
    position: relative;
    display: flex;
    align-items: center;
  }

  .sonic-search-input {
    background: rgba(0, 0, 0, 0.25);
    border: 1px solid rgba(255, 255, 255, 0.12);
    border-radius: 4px;
    color: var(--sg-on-surface, #e3e1e9);
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-xs);
    padding: 4px 20px 4px 8px;
    width: 175px;
    outline: none;
    transition: all 0.15s ease-in-out;
  }

  .sonic-search-input:focus {
    border-color: var(--sg-primary, #00f0ff);
    box-shadow: 0 0 8px rgba(0, 240, 255, 0.2);
    background: rgba(0, 0, 0, 0.35);
  }

  .search-clear-btn {
    position: absolute;
    right: 6px;
    background: none;
    border: none;
    color: var(--sg-outline, #849495);
    cursor: pointer;
    font-size: var(--sg-text-base);
    line-height: 1;
    padding: 0;
    transition: color 0.12s;
  }

  .search-clear-btn:hover {
    color: var(--sg-on-surface, #e3e1e9);
  }

  .action-btn-accent {
    border-color: rgba(0, 240, 255, 0.3) !important;
    color: var(--sg-primary, #00f0ff) !important;
  }

  .action-btn-accent:hover {
    background: rgba(0, 240, 255, 0.1) !important;
    border-color: var(--sg-primary, #00f0ff) !important;
  }

  .action-btn-save-vibe {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    border-color: rgba(254, 0, 254, 0.3) !important;
    color: var(--sg-secondary, #fe00fe) !important;
  }

  .action-btn-save-vibe:hover {
    background: rgba(254, 0, 254, 0.1) !important;
    border-color: var(--sg-secondary, #fe00fe) !important;
  }

  /* ── Blend Slider Styles ── */
  .blend-slider-group {
    margin-left: 0.25rem;
  }

  .slider-side-label {
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-3xs);
    color: var(--sg-outline, #849495);
    opacity: 0.85;
  }

  .blend-slider {
    -webkit-appearance: none;
    appearance: none;
    width: 80px;
    height: 4px;
    border-radius: 2px;
    background: rgba(255, 255, 255, 0.1);
    outline: none;
    cursor: pointer;
  }

  .blend-slider::-webkit-slider-thumb {
    -webkit-appearance: none;
    appearance: none;
    width: 10px;
    height: 10px;
    border-radius: 50%;
    background: var(--sg-primary, #00f0ff);
    cursor: pointer;
    box-shadow: 0 0 6px var(--sg-primary, #00f0ff);
    transition: transform 0.1s ease;
  }

  .blend-slider::-webkit-slider-thumb:hover {
    transform: scale(1.2);
  }

  .blend-percent-badge {
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-2xs);
    font-weight: 700;
    color: var(--sg-primary, #00f0ff);
    background: rgba(0, 240, 255, 0.08);
    padding: 1px 4px;
    border-radius: 3px;
    border: 1px solid rgba(0, 240, 255, 0.15);
    min-width: 26px;
    text-align: center;
  }

  /* ── Map Legend ── */
  .map-legend {
    position: absolute;
    bottom: 16px;
    right: 16px;
    z-index: 100;
    background: var(--sg-surface-slate, #161b22);
    border: 1px solid rgba(255,255,255,0.09);
    border-radius: 6px;
    min-width: 120px;
    max-width: 260px;
    backdrop-filter: blur(10px);
    opacity: 1;
    transition: opacity 0.25s ease;
    pointer-events: auto;
  }

  .map-legend.legend-hidden {
    opacity: 0;
    pointer-events: none;
  }

  .legend-toggle {
    display: flex;
    align-items: center;
    gap: 5px;
    width: 100%;
    background: none;
    border: none;
    cursor: pointer;
    padding: 5px 8px;
    border-radius: 6px;
  }

  .legend-toggle:hover { background: rgba(255,255,255,0.04); }

  .legend-toggle-icon {
    font-size: var(--sg-text-2xs);
    color: var(--sg-outline, #849495);
    line-height: 1;
  }

  .legend-title {
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-3xs);
    font-weight: 700;
    letter-spacing: 0.1em;
    color: var(--sg-outline, #849495);
    text-transform: uppercase;
  }

  .legend-body {
    padding: 2px 8px 8px;
    display: flex;
    flex-direction: column;
    gap: 3px;
  }

  .legend-row {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .legend-row-clickable {
    cursor: pointer;
    border-radius: 3px;
    padding: 1px 3px;
    margin: 0 -3px;
    transition: background 0.1s;
  }
  .legend-row-clickable:hover {
    background: rgba(255,255,255,0.07);
  }

  .legend-swatch {
    width: 9px;
    height: 9px;
    border-radius: 50%;
    flex-shrink: 0;
  }

  .legend-label {
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-2xs);
    color: var(--sg-on-surface, #e3e1e9);
    white-space: nowrap;
  }

  /* Camelot two-column layout */
  .legend-camelot {
    display: flex;
    gap: 10px;
  }

  .legend-camelot-col {
    display: flex;
    flex-direction: column;
    gap: 3px;
  }

  .legend-col-header {
    font-family: var(--sg-font-mono);
    font-size: 7px;
    font-weight: 700;
    letter-spacing: 0.08em;
    color: var(--sg-outline, #849495);
    margin-bottom: 2px;
  }

  /* BPM gradient */
  .legend-bpm {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding-top: 2px;
  }

  .legend-bpm-bar {
    height: 8px;
    border-radius: 4px;
    width: 110px;
  }

  .legend-bpm-labels {
    display: flex;
    justify-content: space-between;
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-3xs);
    color: var(--sg-outline, #849495);
  }
</style>
