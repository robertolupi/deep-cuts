<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { onMount } from 'svelte';
  import * as d3 from 'd3';
  import { filters } from '$lib/stores/filters.svelte';
  import MoodRadar, { type MoodValues } from '$lib/components/MoodRadar.svelte';

  // ── Types ──────────────────────────────────────────────────────────────────

  interface LabelCount { label: string; count: number; }

  interface TrackSetStats {
    track_count: number;
    total_duration_seconds: number;
    avg_bpm: number | null;
    bpm_stddev: number | null;
    most_common_key: string | null;
    key_variety: number;
    pct_vocals: number;
    pct_analysed: number;
    avg_loudness_lufs: number | null;
    avg_mood_happy: number | null;
    avg_mood_sad: number | null;
    avg_mood_aggressive: number | null;
    avg_mood_relaxed: number | null;
    avg_mood_party: number | null;
    avg_mood_acoustic: number | null;
    avg_mood_electronic: number | null;
    major_count: number;
    minor_count: number;
    vocal_count: number;
    instrumental_count: number;
    unknown_vocal_count: number;
    coverage_essentia: number;
    coverage_mood: number;
    coverage_qwen: number;
    coverage_qwen_description: number;
    coverage_qwen_instruments: number;
    coverage_qwen_mood: number;
    coverage_qwen_genre: number;
    coverage_clap: number;
    coverage_umap: number;
    coverage_acoustid: number;
    bpm_values: number[];
    duration_values: number[];
    loudness_values: number[];
    key_distribution: LabelCount[];
    genre_distribution: LabelCount[];
    instrument_distribution: LabelCount[];
  }

  interface MoodRow { label: string; valA: number | null; valB: number | null | undefined; }
  interface CoverageRow { label: string; pctA: number; pctB: number | undefined; }
  interface VocalRow { label: string; cntA: number; cntB: number | undefined; }
  interface WatchedDirectory { id: number; name: string; path: string; }
  interface Playlist { id: number; name: string; }

  type SetSource =
    | { kind: 'library' }
    | { kind: 'filter' }
    | { kind: 'folder'; dir: WatchedDirectory }
    | { kind: 'playlist'; playlist: Playlist };

  // ── State ──────────────────────────────────────────────────────────────────

  let statsA = $state<TrackSetStats | null>(null);
  let statsB = $state<TrackSetStats | null>(null);
  let loadingA = $state(false);
  let loadingB = $state(false);
  let error = $state('');

  let watchedDirs = $state<WatchedDirectory[]>([]);
  let playlists = $state<Playlist[]>([]);
  let sourceA = $state<SetSource>({ kind: 'library' });
  let sourceB = $state<SetSource>({ kind: 'filter' });
  let menuOpen = $state<'A' | 'B' | null>(null);

  // ── Set colours ────────────────────────────────────────────────────────────

  const COLOR_A = '#00f0ff';
  const COLOR_B = '#ff7c5c';
  const CHROMATIC_ORDER = ['C','C#','D','Eb','E','F','F#','G','Ab','A','Bb','B'];

  // ── Helpers ──────────────────────────────────────────────────────────────

  function sourceLabel(s: SetSource): string {
    if (s.kind === 'library') return 'Full Library';
    if (s.kind === 'filter')  return 'Current Filter';
    if (s.kind === 'playlist') return `Playlist: ${s.playlist.name}`;
    return s.dir.name || s.dir.path.split('/').pop() || s.dir.path;
  }

  async function idsForSource(s: SetSource): Promise<number[] | null> {
    if (s.kind === 'library') return null;
    if (s.kind === 'filter')  return filters.filteredTracks.map(t => t.id);
    if (s.kind === 'playlist') {
      const pTracks = await invoke<{ track_id: number | null }[]>('get_playlist_tracks', { playlistId: s.playlist.id });
      return pTracks.map(pt => pt.track_id).filter((id): id is number => id !== null);
    }
    const all = await invoke<{ id: number; watched_directory_id: number }[]>('get_tracks');
    return all.filter(t => t.watched_directory_id === s.dir.id).map(t => t.id);
  }

  // ── Data loading ───────────────────────────────────────────────────────────

  async function loadSet(which: 'A' | 'B') {
    const source = which === 'A' ? sourceA : sourceB;
    if (which === 'A') { loadingA = true; error = ''; }
    else               { loadingB = true; }
    try {
      const ids = await idsForSource(source);
      const stats = await invoke<TrackSetStats>('get_track_stats', { trackIds: ids });
      if (which === 'A') statsA = stats;
      else               statsB = stats;
    } catch (e: any) { error = String(e); }
    finally {
      if (which === 'A') loadingA = false;
      else               loadingB = false;
    }
  }

  // Reload Set A whenever its source changes
  $effect(() => { void sourceA; loadSet('A'); });

  // Reload Set B whenever its source changes, or when filteredTracks changes while B is on filter
  $effect(() => {
    const src = sourceB;
    if (src.kind === 'filter') void filters.filteredTracks;
    loadSet('B');
  });

  // ── Derived display data ───────────────────────────────────────────────────

  const toMoodValues = (s: TrackSetStats): MoodValues => ({
    happy:      s.avg_mood_happy,
    sad:        s.avg_mood_sad,
    aggressive: s.avg_mood_aggressive,
    relaxed:    s.avg_mood_relaxed,
    party:      s.avg_mood_party,
    acoustic:   s.avg_mood_acoustic,
    electronic: s.avg_mood_electronic,
  });

  const moodA = $derived(statsA ? toMoodValues(statsA) : null);
  const moodB = $derived(statsB ? toMoodValues(statsB) : undefined);

  const moodRows = $derived<MoodRow[]>(statsA ? [
    { label: 'Happy',      valA: statsA.avg_mood_happy,      valB: statsB?.avg_mood_happy },
    { label: 'Sad',        valA: statsA.avg_mood_sad,        valB: statsB?.avg_mood_sad },
    { label: 'Aggressive', valA: statsA.avg_mood_aggressive, valB: statsB?.avg_mood_aggressive },
    { label: 'Relaxed',    valA: statsA.avg_mood_relaxed,    valB: statsB?.avg_mood_relaxed },
    { label: 'Party',      valA: statsA.avg_mood_party,      valB: statsB?.avg_mood_party },
    { label: 'Acoustic',   valA: statsA.avg_mood_acoustic,   valB: statsB?.avg_mood_acoustic },
    { label: 'Electronic', valA: statsA.avg_mood_electronic, valB: statsB?.avg_mood_electronic },
  ] : []);

  const coverageRows = $derived<CoverageRow[]>(statsA ? [
    { label: 'Essentia (key/BPM)', pctA: statsA.coverage_essentia, pctB: statsB?.coverage_essentia },
    { label: 'Mood Classifiers',   pctA: statsA.coverage_mood,     pctB: statsB?.coverage_mood },
    { label: 'Qwen2-Audio (Overall)', pctA: statsA.coverage_qwen,     pctB: statsB?.coverage_qwen },
    { label: '  └ Description',    pctA: statsA.coverage_qwen_description, pctB: statsB?.coverage_qwen_description },
    { label: '  └ AI Instruments', pctA: statsA.coverage_qwen_instruments, pctB: statsB?.coverage_qwen_instruments },
    { label: '  └ AI Mood',        pctA: statsA.coverage_qwen_mood, pctB: statsB?.coverage_qwen_mood },
    { label: '  └ AI Genre',       pctA: statsA.coverage_qwen_genre, pctB: statsB?.coverage_qwen_genre },
    { label: 'CLAP Embeddings',    pctA: statsA.coverage_clap,     pctB: statsB?.coverage_clap },
    { label: 'UMAP Coordinates',   pctA: statsA.coverage_umap,     pctB: statsB?.coverage_umap },
    { label: 'AcoustID Enrichment',pctA: statsA.coverage_acoustid, pctB: statsB?.coverage_acoustid },
  ] : []);

  const vocalRows = $derived<VocalRow[]>(statsA ? [
    { label: 'Voice',        cntA: statsA.vocal_count,        cntB: statsB?.vocal_count },
    { label: 'Instrumental', cntA: statsA.instrumental_count, cntB: statsB?.instrumental_count },
    { label: 'Unknown',      cntA: statsA.unknown_vocal_count,cntB: statsB?.unknown_vocal_count },
  ] : []);

  // ── Helpers ────────────────────────────────────────────────────────────────

  function fmtDuration(secs: number): string {
    const h = Math.floor(secs / 3600);
    const m = Math.floor((secs % 3600) / 60);
    return h > 0 ? `${h}h ${m}m` : `${m}m`;
  }

  function fmt1(n: number | null | undefined): string {
    return n == null ? '—' : n.toFixed(1);
  }

  function fmt0(n: number | null | undefined): string {
    return n == null ? '—' : Math.round(n).toString();
  }

  // ── D3 rendering helpers ───────────────────────────────────────────────────

  function renderSharedHistogram(
    svgEl: SVGSVGElement,
    valsA: number[], totalA: number,
    valsB: number[] | null, totalB: number,
    numBins = 40
  ) {
    d3.select(svgEl).selectAll('*').remove();
    if (!valsA.length) return;

    // Shared domain from the union of both value sets
    const allVals = valsB ? [...valsA, ...valsB] : valsA;
    const domainMin = d3.min(allVals)!;
    const domainMax = d3.max(allVals)!;

    const W = svgEl.clientWidth || 320;
    const H = svgEl.clientHeight || 120;
    const m = { top: 8, right: 8, bottom: 26, left: 32 };
    const w = W - m.left - m.right;
    const h = H - m.top - m.bottom;

    const x = d3.scaleLinear().domain([domainMin, domainMax]).range([0, w]);

    // Shared thresholds so both sets are binned identically
    const thresholds = d3.range(numBins).map(i => domainMin + (i / numBins) * (domainMax - domainMin));
    const binner = d3.bin().domain([domainMin, domainMax]).thresholds(thresholds);

    const toBins = (vals: number[], total: number) =>
      binner(vals).map(b => ({ x0: b.x0!, x1: b.x1!, pct: total > 0 ? b.length / total * 100 : 0 }));

    const binsA = toBins(valsA, totalA);
    const binsB = valsB ? toBins(valsB, totalB) : null;

    const maxPct = d3.max([...binsA, ...(binsB ?? [])], b => b.pct) ?? 1;
    const y = d3.scaleLinear().domain([0, maxPct * 1.1]).range([h, 0]);

    const g = d3.select(svgEl).append('g').attr('transform', `translate(${m.left},${m.top})`);

    g.append('g').attr('transform', `translate(0,${h})`)
      .call(d3.axisBottom(x).ticks(6).tickSize(3))
      .call(ax => { ax.selectAll('text').style('fill','#849495').style('font-size','var(--sg-text-2xs)'); ax.select('.domain').style('stroke','rgba(255,255,255,0.1)'); ax.selectAll('.tick line').style('stroke','rgba(255,255,255,0.1)'); });
    g.append('g')
      .call(d3.axisLeft(y).ticks(4).tickFormat(d => `${d}%`).tickSize(3))
      .call(ax => { ax.selectAll('text').style('fill','#849495').style('font-size','var(--sg-text-2xs)'); ax.select('.domain').style('stroke','rgba(255,255,255,0.1)'); ax.selectAll('.tick line').style('stroke','rgba(255,255,255,0.1)'); });

    const drawBars = (bins: typeof binsA, color: string, offset: number, widthFrac: number) => {
      g.selectAll(null).data(bins).join('rect')
        .attr('x', d => x(d.x0) + offset)
        .attr('y', d => y(d.pct))
        .attr('width', d => Math.max(0, (x(d.x1) - x(d.x0)) * widthFrac - 1))
        .attr('height', d => Math.max(0, h - y(d.pct)))
        .attr('fill', color).attr('opacity', 0.7).attr('rx', 1);
    };

    if (binsB) {
      drawBars(binsA, COLOR_A, 0, 0.5);
      drawBars(binsB, COLOR_B, (x(binsB[0]?.x1 ?? 1) - x(binsB[0]?.x0 ?? 0)) * 0.5, 0.5);
    } else {
      drawBars(binsA, COLOR_A, 0, 1);
    }
  }

  function renderKeyBars(svgEl: SVGSVGElement, distA: LabelCount[], totalA: number, distB: LabelCount[] | null, totalB: number) {
    d3.select(svgEl).selectAll('*').remove();
    const keys = CHROMATIC_ORDER;
    const pct = (dist: LabelCount[], k: string, total: number) =>
      total > 0 ? (dist.find(d => d.label === k)?.count ?? 0) / total * 100 : 0;

    const W = svgEl.clientWidth || 360;
    const H = svgEl.clientHeight || 100;
    const m = { top: 8, right: 8, bottom: 22, left: 26 };
    const w = W - m.left - m.right, h = H - m.top - m.bottom;
    const g = d3.select(svgEl).append('g').attr('transform', `translate(${m.left},${m.top})`);

    const maxPct = Math.max(
      ...keys.map(k => pct(distA, k, totalA)),
      ...(distB ? keys.map(k => pct(distB, k, totalB)) : [0])
    );
    const x = d3.scaleBand().domain(keys).range([0, w]).padding(0.15);
    const y = d3.scaleLinear().domain([0, maxPct * 1.1 || 1]).range([h, 0]);

    g.append('g').attr('transform', `translate(0,${h})`)
      .call(d3.axisBottom(x).tickSize(0))
      .call(ax => { ax.selectAll('text').style('fill','#849495').style('font-size','var(--sg-text-3xs)'); ax.select('.domain').style('stroke','rgba(255,255,255,0.1)'); });
    g.append('g').call(d3.axisLeft(y).ticks(4).tickFormat(d => `${d}%`).tickSize(3))
      .call(ax => { ax.selectAll('text').style('fill','#849495').style('font-size','var(--sg-text-3xs)'); ax.select('.domain').style('stroke','rgba(255,255,255,0.1)'); ax.selectAll('.tick line').style('stroke','rgba(255,255,255,0.1)'); });

    const bw = x.bandwidth() / (distB ? 2 : 1);
    keys.forEach(k => {
      const xPos = x(k) ?? 0;
      const pA = pct(distA, k, totalA);
      g.append('rect').attr('x', xPos).attr('y', y(pA)).attr('width', bw).attr('height', h - y(pA))
        .attr('fill', COLOR_A).attr('opacity', 0.7).attr('rx', 1);
      if (distB) {
        const pB = pct(distB, k, totalB);
        g.append('rect').attr('x', xPos + bw).attr('y', y(pB)).attr('width', bw).attr('height', h - y(pB))
          .attr('fill', COLOR_B).attr('opacity', 0.7).attr('rx', 1);
      }
    });
  }

  function renderHorizBars(
    svgEl: SVGSVGElement,
    itemsA: LabelCount[], totalA: number,
    itemsB: LabelCount[] | null, totalB: number,
    maxItems = 12
  ) {
    d3.select(svgEl).selectAll('*').remove();
    // Sort labels by Set A percentage descending, then include any B-only labels
    const labelsA = itemsA.slice(0, maxItems).map(i => i.label);
    const labelsB = (itemsB ?? []).slice(0, maxItems).map(i => i.label).filter(l => !labelsA.includes(l));
    const labels = [...labelsA, ...labelsB].slice(0, maxItems);
    if (!labels.length) return;

    const pct = (items: LabelCount[], lbl: string, total: number) =>
      total > 0 ? (items.find(i => i.label === lbl)?.count ?? 0) / total * 100 : 0;
    const cnt = (items: LabelCount[], lbl: string) => items.find(i => i.label === lbl)?.count ?? 0;

    const maxPct = Math.max(
      ...labels.map(l => pct(itemsA, l, totalA)),
      ...(itemsB ? labels.map(l => pct(itemsB, l, totalB)) : [0])
    );

    const rowH = 18;
    const m = { top: 4, right: 52, bottom: 4, left: 90 };
    const W = svgEl.clientWidth || 320;
    const H = labels.length * rowH + m.top + m.bottom;
    d3.select(svgEl).attr('height', H);
    const w = W - m.left - m.right;
    const g = d3.select(svgEl).append('g').attr('transform', `translate(${m.left},${m.top})`);
    const x = d3.scaleLinear().domain([0, maxPct || 1]).range([0, w]);
    const barH = itemsB ? rowH * 0.36 : rowH * 0.52;

    labels.forEach((lbl, i) => {
      const yPos = i * rowH;
      g.append('text').attr('x', -4).attr('y', yPos + rowH / 2)
        .attr('text-anchor', 'end').attr('dominant-baseline', 'middle')
        .style('font-family', 'var(--sg-font-mono)').style('font-size', 'var(--sg-text-3xs)').style('fill', '#849495')
        .text(lbl.length > 16 ? lbl.slice(0, 15) + '…' : lbl);

      const pA = pct(itemsA, lbl, totalA);
      const cA = cnt(itemsA, lbl);
      g.append('rect').attr('x', 0).attr('y', yPos + (itemsB ? 1 : (rowH - barH) / 2))
        .attr('width', x(pA)).attr('height', barH).attr('fill', COLOR_A).attr('opacity', 0.7).attr('rx', 1);
      if (pA > 0) g.append('text')
        .attr('x', x(pA) + 3).attr('y', yPos + (itemsB ? barH / 2 + 1 : rowH / 2))
        .attr('dominant-baseline', 'middle').style('font-family','var(--sg-font-mono)').style('font-size','7px').style('fill', COLOR_A).style('opacity','0.85')
        .text(`${pA.toFixed(1)}% (${cA})`);

      if (itemsB) {
        const pB = pct(itemsB, lbl, totalB);
        const cB = cnt(itemsB, lbl);
        g.append('rect').attr('x', 0).attr('y', yPos + barH + 2)
          .attr('width', x(pB)).attr('height', barH).attr('fill', COLOR_B).attr('opacity', 0.7).attr('rx', 1);
        if (pB > 0) g.append('text')
          .attr('x', x(pB) + 3).attr('y', yPos + barH * 1.5 + 2)
          .attr('dominant-baseline', 'middle').style('font-family','var(--sg-font-mono)').style('font-size','7px').style('fill', COLOR_B).style('opacity','0.85')
          .text(`${pB.toFixed(1)}% (${cB})`);
      }
    });
  }

  // ── SVG refs ───────────────────────────────────────────────────────────────

  let svgBpm: SVGSVGElement = $state(undefined as unknown as SVGSVGElement);
  let svgDuration: SVGSVGElement = $state(undefined as unknown as SVGSVGElement);
  let svgLoudness: SVGSVGElement = $state(undefined as unknown as SVGSVGElement);
  let svgKey: SVGSVGElement = $state(undefined as unknown as SVGSVGElement);
  let svgGenre: SVGSVGElement = $state(undefined as unknown as SVGSVGElement);
  let svgInstruments: SVGSVGElement = $state(undefined as unknown as SVGSVGElement);

  function scheduleRender(fn: () => void) {
    requestAnimationFrame(() => requestAnimationFrame(fn));
  }

  $effect(() => {
    const vA = statsA?.bpm_values ?? [], tA = statsA?.track_count ?? 1;
    const vB = statsB?.bpm_values ?? null,  tB = statsB?.track_count ?? 1;
    if (svgBpm) scheduleRender(() => renderSharedHistogram(svgBpm, vA, tA, vB, tB, 40));
  });
  $effect(() => {
    const vA = statsA?.duration_values ?? [], tA = statsA?.track_count ?? 1;
    const vB = statsB?.duration_values ?? null,  tB = statsB?.track_count ?? 1;
    if (svgDuration) scheduleRender(() => renderSharedHistogram(svgDuration, vA, tA, vB, tB, 30));
  });
  $effect(() => {
    const vA = statsA?.loudness_values ?? [], tA = statsA?.track_count ?? 1;
    const vB = statsB?.loudness_values ?? null,  tB = statsB?.track_count ?? 1;
    if (svgLoudness) scheduleRender(() => renderSharedHistogram(svgLoudness, vA, tA, vB, tB, 30));
  });
  $effect(() => {
    const distA = statsA?.key_distribution, tA = statsA?.track_count ?? 1;
    const distB = statsB?.key_distribution ?? null, tB = statsB?.track_count ?? 1;
    if (svgKey && distA) scheduleRender(() => renderKeyBars(svgKey, distA, tA, distB, tB));
  });
  $effect(() => {
    const distA = statsA?.genre_distribution, tA = statsA?.track_count ?? 1;
    const distB = statsB?.genre_distribution ?? null, tB = statsB?.track_count ?? 1;
    if (svgGenre && distA) scheduleRender(() => renderHorizBars(svgGenre, distA, tA, distB, tB));
  });
  $effect(() => {
    const distA = statsA?.instrument_distribution, tA = statsA?.track_count ?? 1;
    const distB = statsB?.instrument_distribution ?? null, tB = statsB?.track_count ?? 1;
    if (svgInstruments && distA) scheduleRender(() => renderHorizBars(svgInstruments, distA, tA, distB, tB));
  });

  onMount(async () => {
    const [dirs, lists] = await Promise.all([
      invoke<WatchedDirectory[]>('get_watched_directories'),
      invoke<Playlist[]>('get_playlists'),
    ]);
    watchedDirs = dirs;
    playlists = lists;
  });
</script>

<svelte:window onclick={() => { menuOpen = null; }} />

<div class="stats-panel">

  <!-- ── Status bar ───────────────────────────────────────────────────────── -->
  <div class="set-bar">
    {#snippet setSlot(which: 'A' | 'B', color: string, source: SetSource, stats: TrackSetStats | null, loading: boolean)}
      <div class="set-slot">
        <span class="set-dot" style="background:{color}"></span>
        <div class="set-picker-wrap">
          <button
            class="set-picker-btn"
            onclick={(e) => { e.stopPropagation(); menuOpen = menuOpen === which ? null : which; }}
          >
            <span class="set-name">{sourceLabel(source)}</span>
            <span class="set-chevron">▾</span>
          </button>
          {#if menuOpen === which}
            <div class="set-menu">
              <button class="set-menu-item" onclick={(e) => { e.stopPropagation(); if(which==='A') sourceA={kind:'library'}; else sourceB={kind:'library'}; menuOpen=null; }}>Full Library</button>
              <button class="set-menu-item" onclick={(e) => { e.stopPropagation(); if(which==='A') sourceA={kind:'filter'}; else sourceB={kind:'filter'}; menuOpen=null; }}>Current Filter</button>
              {#if watchedDirs.length}
                <div class="set-menu-sep"></div>
                {#each watchedDirs as dir}
                  <button class="set-menu-item" onclick={(e) => { e.stopPropagation(); if(which==='A') sourceA={kind:'folder',dir}; else sourceB={kind:'folder',dir}; menuOpen=null; }}>
                    📁 {dir.name || dir.path.split('/').pop()}
                  </button>
                {/each}
              {/if}
              {#if playlists.length}
                <div class="set-menu-sep"></div>
                {#each playlists as pl}
                  <button class="set-menu-item" onclick={(e) => { e.stopPropagation(); if(which==='A') sourceA={kind:'playlist',playlist:pl}; else sourceB={kind:'playlist',playlist:pl}; menuOpen=null; }}>
                    🎵 {pl.name}
                  </button>
                {/each}
              {/if}
            </div>
          {/if}
        </div>
        {#if stats}<span class="set-count">{stats.track_count} tracks</span>{/if}
        {#if loading}<span class="loading-badge">Computing…</span>{/if}
      </div>
    {/snippet}
    {@render setSlot('A', COLOR_A, sourceA, statsA, loadingA)}
    <div class="set-divider">vs</div>
    {@render setSlot('B', COLOR_B, sourceB, statsB, loadingB)}
  </div>

  {#if error}
    <div class="error-row">{error}</div>
  {/if}

  {#if !statsA && !loadingA}
    <div class="empty-state">Loading…</div>
  {:else}

  <div class="stats-body">

    <!-- ── 1. Summary KPIs ───────────────────────────────────────────────── -->
    <section class="section">
      <h2 class="section-title">Summary</h2>
      <div class="kpi-grid">
        {#snippet kpi(label: string, valA: string, valB: string | null)}
          <div class="kpi-card">
            <div class="kpi-label">{label}</div>
            <div class="kpi-val-a">{valA}</div>
            {#if valB != null}<div class="kpi-val-b">{valB}</div>{/if}
          </div>
        {/snippet}
        {@render kpi('Tracks', fmt0(statsA?.track_count), statsB ? fmt0(statsB.track_count) : null)}
        {@render kpi('Duration', statsA ? fmtDuration(statsA.total_duration_seconds) : '—', statsB ? fmtDuration(statsB.total_duration_seconds) : null)}
        {@render kpi('Avg BPM', fmt1(statsA?.avg_bpm), statsB ? fmt1(statsB.avg_bpm) : null)}
        {@render kpi('Top Key', statsA?.most_common_key ?? '—', statsB?.most_common_key ?? null)}
        {@render kpi('Key Variety', statsA ? `${(statsA.key_variety * 100).toFixed(0)}%` : '—', statsB ? `${(statsB.key_variety * 100).toFixed(0)}%` : null)}
        {@render kpi('% Vocals', statsA ? `${statsA.pct_vocals.toFixed(0)}%` : '—', statsB ? `${statsB.pct_vocals.toFixed(0)}%` : null)}
        {@render kpi('% Analysed', statsA ? `${statsA.pct_analysed.toFixed(0)}%` : '—', statsB ? `${statsB.pct_analysed.toFixed(0)}%` : null)}
        {@render kpi('Avg LUFS', fmt1(statsA?.avg_loudness_lufs), statsB ? fmt1(statsB.avg_loudness_lufs) : null)}
      </div>
    </section>

    <!-- ── 2 & 3. BPM + Key ──────────────────────────────────────────────── -->
    <div class="two-col">
      <section class="section">
        <h2 class="section-title">BPM Distribution</h2>
        <svg bind:this={svgBpm} class="chart-svg chart-hist"></svg>
      </section>

      <section class="section">
        <h2 class="section-title">Key Distribution (Chromatic)</h2>
        <svg bind:this={svgKey} class="chart-svg chart-key"></svg>
        {#if statsA}
          {@const totalA = statsA.major_count + statsA.minor_count || 1}
          {@const majA = statsA.major_count / totalA * 100}
          {@const minA = statsA.minor_count / totalA * 100}
          {@const totalB = statsB ? (statsB.major_count + statsB.minor_count || 1) : 1}
          <div class="scale-rows">
            <div class="scale-row">
              <span class="scale-label">Major</span>
              <div class="scale-track">
                <div class="scale-fill-a" style="width:{majA}%"></div>
                {#if statsB}<div class="scale-fill-b" style="width:{statsB.major_count / totalB * 100}%"></div>{/if}
              </div>
              <span class="scale-pct">{majA.toFixed(0)}%</span>
            </div>
            <div class="scale-row">
              <span class="scale-label">Minor</span>
              <div class="scale-track">
                <div class="scale-fill-a" style="width:{minA}%"></div>
                {#if statsB}<div class="scale-fill-b" style="width:{statsB.minor_count / totalB * 100}%"></div>{/if}
              </div>
              <span class="scale-pct">{minA.toFixed(0)}%</span>
            </div>
          </div>
        {/if}
      </section>
    </div>

    <!-- ── 4. Mood Profile ───────────────────────────────────────────────── -->
    <section class="section">
      <h2 class="section-title">Mood Profile</h2>
      <div class="mood-layout">
        <div class="chart-radar">
          {#if moodA}
            <MoodRadar moodA={moodA} moodB={moodB} colorA={COLOR_A} colorB={COLOR_B} />
          {/if}
        </div>
        <div class="mood-bars">
          {#each moodRows as row}
            <div class="mood-row">
              <span class="mood-label">{row.label}</span>
              <div class="mood-track">
                {#if row.valA != null}
                  <div class="mood-fill mood-fill-a" style="width:{(row.valA) * 100}%"></div>
                {/if}
                {#if row.valB != null}
                  <div class="mood-fill mood-fill-b" style="width:{(row.valB) * 100}%"></div>
                {/if}
              </div>
              <span class="mood-val">{fmt1(row.valA)}</span>
            </div>
          {/each}
        </div>
      </div>
    </section>

    <!-- ── 5 & 6. Genre + Vocals/Instruments ─────────────────────────────── -->
    <div class="two-col">
      <section class="section">
        <h2 class="section-title">Top Genres</h2>
        <div class="horiz-wrap">
          <svg bind:this={svgGenre} class="chart-svg chart-horiz"></svg>
        </div>
      </section>

      <section class="section">
        <h2 class="section-title">Vocal Character</h2>
        <div class="vocal-bars">
          {#each vocalRows as row}
            {@const totalA = statsA?.track_count || 1}
            {@const totalB = statsB?.track_count || 1}
            <div class="vocal-row">
              <span class="vocal-label">{row.label}</span>
              <div class="vocal-track">
                <div class="vocal-fill vocal-fill-a" style="width:{row.cntA / totalA * 100}%"></div>
                {#if statsB && row.cntB != null}
                  <div class="vocal-fill vocal-fill-b" style="width:{row.cntB / totalB * 100}%"></div>
                {/if}
              </div>
              <span class="vocal-count">{row.cntA}</span>
            </div>
          {/each}
        </div>

        <h2 class="section-title" style="margin-top:1rem">Instruments</h2>
        <div class="horiz-wrap">
          <svg bind:this={svgInstruments} class="chart-svg chart-horiz"></svg>
        </div>
      </section>
    </div>

    <!-- ── 7. Duration & Loudness ─────────────────────────────────────────── -->
    <div class="two-col">
      <section class="section">
        <h2 class="section-title">Duration (min)</h2>
        <svg bind:this={svgDuration} class="chart-svg chart-hist"></svg>
      </section>
      <section class="section">
        <h2 class="section-title">Loudness (LUFS)</h2>
        <svg bind:this={svgLoudness} class="chart-svg chart-hist"></svg>
      </section>
    </div>

    <!-- ── 8. Analysis Coverage ──────────────────────────────────────────── -->
    <section class="section">
      <h2 class="section-title">Analysis Coverage</h2>
      <table class="cov-table">
        <thead>
          <tr>
            <th>Pass</th>
            <th>Set A</th>
            {#if statsB}<th>Set B</th>{/if}
          </tr>
        </thead>
        <tbody>
          {#each coverageRows as row}
            <tr>
              <td class="pass-name">{row.label}</td>
              <td>
                <div class="cov-wrap">
                  <div class="cov-track">
                    <div class="cov-bar" style="width:{row.pctA}%;background:{COLOR_A}"></div>
                  </div>
                  <span class="cov-pct">{row.pctA.toFixed(0)}%</span>
                </div>
              </td>
              {#if statsB}
                <td>
                  <div class="cov-wrap">
                    <div class="cov-track">
                      <div class="cov-bar" style="width:{row.pctB ?? 0}%;background:{COLOR_B}"></div>
                    </div>
                    <span class="cov-pct">{(row.pctB ?? 0).toFixed(0)}%</span>
                  </div>
                </td>
              {/if}
            </tr>
          {/each}
        </tbody>
      </table>
    </section>

  </div>
  {/if}
</div>

<style>
  .stats-panel {
    display: flex;
    flex-direction: column;
    height: 100%;
    overflow: hidden;
    background: var(--sg-surface, #0d1117);
    font-family: var(--sg-font-mono);
  }

  /* ── Set bar ── */
  .set-bar {
    display: flex;
    align-items: center;
    gap: 1.5rem;
    padding: 0.55rem 1.2rem;
    background: var(--sg-surface-slate, #161b22);
    border-bottom: 1px solid rgba(255,255,255,0.07);
    flex-shrink: 0;
  }

  .set-slot { display: flex; align-items: center; gap: 6px; }

  .set-dot { width: 8px; height: 8px; border-radius: 50%; flex-shrink: 0; }

  .set-picker-wrap { position: relative; }

  .set-picker-btn {
    display: flex; align-items: center; gap: 4px;
    background: none; border: 1px solid rgba(255,255,255,0.1);
    border-radius: 4px; padding: 3px 8px; cursor: pointer;
    transition: border-color 0.15s;
  }
  .set-picker-btn:hover { border-color: rgba(255,255,255,0.25); }

  .set-name {
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-xs); font-weight: 700; letter-spacing: 0.06em;
    color: var(--sg-on-surface, #e3e1e9);
  }

  .set-chevron { font-size: var(--sg-text-3xs); color: var(--sg-outline, #849495); }

  .set-menu {
    position: absolute; top: calc(100% + 4px); left: 0;
    background: var(--sg-surface-slate, #161b22);
    border: 1px solid rgba(255,255,255,0.12);
    border-radius: 4px; overflow: hidden; z-index: 200;
    min-width: 160px;
  }

  .set-menu-item {
    display: block; width: 100%; text-align: left;
    font-family: var(--sg-font-mono); font-size: var(--sg-text-xs);
    padding: 7px 12px; background: none; border: none;
    color: var(--sg-on-surface, #e3e1e9); cursor: pointer;
    transition: background 0.1s;
  }
  .set-menu-item:hover { background: rgba(255,255,255,0.06); }

  .set-menu-sep { height: 1px; background: rgba(255,255,255,0.08); margin: 2px 0; }

  .set-count {
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-2xs); color: var(--sg-outline, #849495);
  }

  .set-divider {
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-2xs); color: var(--sg-outline, #849495);
    letter-spacing: 0.06em; opacity: 0.6;
  }

  .loading-badge { font-size: var(--sg-text-2xs); color: var(--sg-primary, #00f0ff); opacity: 0.7; letter-spacing: 0.05em; }

  .error-row {
    padding: 0.5rem 1.2rem; font-size: var(--sg-text-xs); color: #ff6b6b;
    background: rgba(255,80,80,0.05); border-bottom: 1px solid rgba(255,80,80,0.15);
  }

  .empty-state {
    display: flex; align-items: center; justify-content: center; flex: 1;
    font-size: var(--sg-text-sm); color: var(--sg-outline, #849495);
  }

  /* ── Scrollable body ── */
  .stats-body {
    flex: 1; overflow-y: auto; padding: 1rem 1.2rem;
    display: flex; flex-direction: column; gap: 1rem;
  }

  /* ── Section card ── */
  .section {
    background: var(--sg-surface-slate, #161b22);
    border: 1px solid rgba(255,255,255,0.07);
    border-radius: 6px; padding: 0.8rem 1rem;
  }

  .section-title {
    font-size: var(--sg-text-2xs); font-weight: 700; letter-spacing: 0.12em;
    color: var(--sg-outline, #849495); text-transform: uppercase;
    margin: 0 0 0.6rem 0;
  }

  /* ── KPIs ── */
  .kpi-grid { display: flex; flex-wrap: wrap; gap: 0.5rem; }

  .kpi-card {
    background: rgba(255,255,255,0.03); border: 1px solid rgba(255,255,255,0.07);
    border-radius: 5px; padding: 0.45rem 0.7rem; min-width: 80px; flex: 1;
  }
  .kpi-label { font-size: var(--sg-text-3xs); color: var(--sg-outline,#849495); letter-spacing: 0.06em; text-transform: uppercase; margin-bottom: 3px; }
  .kpi-val-a { font-size: var(--sg-text-md); font-weight: 700; color: var(--sg-primary,#00f0ff); }
  .kpi-val-b { font-size: var(--sg-text-sm); font-weight: 600; color: #ff7c5c; margin-top: 1px; }

  /* ── Charts ── */
  .chart-svg { display: block; width: 100%; }
  .chart-hist { height: 120px; }
  .chart-key  { height: 100px; }
  .chart-radar { height: 200px; }
  .chart-horiz { height: auto; min-height: 40px; }

  /* ── Two-column layout ── */
  .two-col { display: grid; grid-template-columns: 1fr 1fr; gap: 1rem; }

  /* ── Scale (major/minor) rows ── */
  .scale-rows { margin-top: 0.6rem; display: flex; flex-direction: column; gap: 4px; }

  .scale-row { display: flex; align-items: center; gap: 6px; }

  .scale-label { font-size: var(--sg-text-3xs); color: var(--sg-outline,#849495); width: 36px; flex-shrink: 0; }

  .scale-track {
    flex: 1; height: 14px; background: rgba(255,255,255,0.04); border-radius: 3px;
    overflow: hidden; display: flex; flex-direction: column; gap: 1px;
  }

  .scale-fill-a { height: 6px; background: var(--sg-primary,#00f0ff); opacity: 0.75; border-radius: 2px; }
  .scale-fill-b { height: 6px; background: #ff7c5c; opacity: 0.75; border-radius: 2px; }

  .scale-pct { font-size: var(--sg-text-3xs); color: var(--sg-outline,#849495); width: 28px; text-align: right; }

  /* ── Mood ── */
  .mood-layout { display: flex; gap: 1rem; align-items: flex-start; }
  .mood-layout .chart-radar { width: 220px; flex-shrink: 0; }

  .mood-bars { flex: 1; display: flex; flex-direction: column; gap: 7px; padding-top: 4px; }

  .mood-row { display: flex; align-items: center; gap: 6px; }
  .mood-label { font-size: var(--sg-text-3xs); color: var(--sg-outline,#849495); width: 64px; flex-shrink: 0; }

  .mood-track {
    flex: 1; height: 14px; background: rgba(255,255,255,0.04); border-radius: 3px;
    overflow: hidden; display: flex; flex-direction: column; gap: 1px;
  }
  .mood-fill { height: 6px; border-radius: 2px; }
  .mood-fill-a { background: var(--sg-primary,#00f0ff); opacity: 0.75; }
  .mood-fill-b { background: #ff7c5c; opacity: 0.75; }

  .mood-val { font-size: var(--sg-text-3xs); color: var(--sg-outline,#849495); width: 28px; text-align: right; }

  /* ── Vocal ── */
  .vocal-bars { display: flex; flex-direction: column; gap: 5px; }
  .vocal-row { display: flex; align-items: center; gap: 6px; }
  .vocal-label { font-size: var(--sg-text-3xs); color: var(--sg-outline,#849495); width: 72px; flex-shrink: 0; }

  .vocal-track {
    flex: 1; height: 14px; background: rgba(255,255,255,0.04); border-radius: 3px;
    overflow: hidden; display: flex; flex-direction: column; gap: 1px;
  }
  .vocal-fill { height: 6px; border-radius: 2px; }
  .vocal-fill-a { background: var(--sg-primary,#00f0ff); opacity: 0.75; }
  .vocal-fill-b { background: #ff7c5c; opacity: 0.75; }
  .vocal-count { font-size: var(--sg-text-3xs); color: var(--sg-outline,#849495); width: 32px; text-align: right; }

  /* ── Horizontal bar chart ── */
  .horiz-wrap { overflow: hidden; }

  /* ── Analysis coverage table ── */
  .cov-table { width: 100%; border-collapse: collapse; font-size: var(--sg-text-2xs); }

  .cov-table th {
    font-size: var(--sg-text-3xs); color: var(--sg-outline,#849495); text-align: left;
    padding: 3px 8px 5px 0; border-bottom: 1px solid rgba(255,255,255,0.07);
    font-weight: 700; letter-spacing: 0.06em;
  }

  .cov-table td { padding: 4px 8px 4px 0; border-bottom: 1px solid rgba(255,255,255,0.04); vertical-align: middle; }

  .pass-name { color: var(--sg-on-surface,#e3e1e9); font-size: var(--sg-text-2xs); white-space: nowrap; }

  .cov-wrap { display: flex; align-items: center; gap: 8px; }

  .cov-track { width: 80px; height: 6px; background: rgba(255,255,255,0.04); border-radius: 3px; overflow: hidden; flex-shrink: 0; }

  .cov-bar { height: 100%; border-radius: 3px; }

  .cov-pct { font-size: var(--sg-text-3xs); color: var(--sg-outline,#849495); min-width: 28px; }

  /* ── Responsive ── */
  @media (max-width: 700px) {
    .two-col { grid-template-columns: 1fr; }
    .mood-layout { flex-direction: column; }
    .mood-layout .chart-radar { width: 100%; }
  }
</style>
