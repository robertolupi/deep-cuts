<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { onMount } from 'svelte';
  import * as d3 from 'd3';
  import { filters } from '$lib/stores/filters.svelte';

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

  // ── State ──────────────────────────────────────────────────────────────────

  let statsA = $state<TrackSetStats | null>(null);  // full library
  let statsB = $state<TrackSetStats | null>(null);  // current filter
  let loadingA = $state(false);
  let loadingB = $state(false);
  let error = $state('');

  // ── Set colours ────────────────────────────────────────────────────────────

  const COLOR_A = '#00f0ff';
  const COLOR_B = '#ff7c5c';
  const CHROMATIC_ORDER = ['C','C#','D','Eb','E','F','F#','G','Ab','A','Bb','B'];

  // ── Data loading ───────────────────────────────────────────────────────────

  async function loadLibrary() {
    loadingA = true; error = '';
    try {
      statsA = await invoke<TrackSetStats>('get_track_stats', { trackIds: null });
    } catch (e: any) { error = String(e); }
    finally { loadingA = false; }
  }

  async function loadFilter(ids: number[]) {
    loadingB = true;
    try {
      statsB = await invoke<TrackSetStats>('get_track_stats', { trackIds: ids });
    } catch (e: any) { error = String(e); }
    finally { loadingB = false; }
  }

  // Reload filter stats whenever filteredTracks changes
  $effect(() => {
    const ids = filters.filteredTracks.map((t) => t.id);
    loadFilter(ids);
  });

  // ── Derived display data ───────────────────────────────────────────────────

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
    { label: 'Qwen2-Audio',        pctA: statsA.coverage_qwen,     pctB: statsB?.coverage_qwen },
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
      .call(ax => { ax.selectAll('text').style('fill','#849495').style('font-size','9px'); ax.select('.domain').style('stroke','rgba(255,255,255,0.1)'); ax.selectAll('.tick line').style('stroke','rgba(255,255,255,0.1)'); });
    g.append('g')
      .call(d3.axisLeft(y).ticks(4).tickFormat(d => `${d}%`).tickSize(3))
      .call(ax => { ax.selectAll('text').style('fill','#849495').style('font-size','9px'); ax.select('.domain').style('stroke','rgba(255,255,255,0.1)'); ax.selectAll('.tick line').style('stroke','rgba(255,255,255,0.1)'); });

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

  function renderMoodRadar(svgEl: SVGSVGElement, sA: TrackSetStats | null, sB: TrackSetStats | null) {
    d3.select(svgEl).selectAll('*').remove();
    if (!sA) return;

    const axes: { key: keyof TrackSetStats; label: string }[] = [
      { key: 'avg_mood_happy',      label: 'Happy'      },
      { key: 'avg_mood_party',       label: 'Party'      },
      { key: 'avg_mood_electronic',  label: 'Electronic' },
      { key: 'avg_mood_aggressive',  label: 'Aggressive' },
      { key: 'avg_mood_sad',         label: 'Sad'        },
      { key: 'avg_mood_relaxed',     label: 'Relaxed'    },
      { key: 'avg_mood_acoustic',    label: 'Acoustic'   },
    ];
    const N = axes.length;
    const W = svgEl.clientWidth || 220;
    const H = svgEl.clientHeight || 200;
    const cx = W / 2, cy = H / 2;
    const R = Math.min(cx, cy) - 28;
    const g = d3.select(svgEl).append('g');

    [0.25, 0.5, 0.75, 1.0].forEach(r => {
      g.append('circle').attr('cx', cx).attr('cy', cy).attr('r', R * r)
        .attr('fill', 'none').attr('stroke', 'rgba(255,255,255,0.08)').attr('stroke-width', 1);
    });
    axes.forEach((ax, i) => {
      const angle = (i / N) * 2 * Math.PI - Math.PI / 2;
      g.append('line').attr('x1', cx).attr('y1', cy)
        .attr('x2', cx + R * Math.cos(angle)).attr('y2', cy + R * Math.sin(angle))
        .attr('stroke', 'rgba(255,255,255,0.12)').attr('stroke-width', 1);
      g.append('text')
        .attr('x', cx + (R + 14) * Math.cos(angle)).attr('y', cy + (R + 14) * Math.sin(angle))
        .attr('text-anchor', 'middle').attr('dominant-baseline', 'middle')
        .style('font-family', 'JetBrains Mono, monospace').style('font-size', '8px').style('fill', '#849495')
        .text(ax.label);
    });
    const drawPolygon = (stats: TrackSetStats, color: string) => {
      const pts = axes.map((ax, i) => {
        const val = (stats[ax.key] as number | null) ?? 0;
        const angle = (i / N) * 2 * Math.PI - Math.PI / 2;
        return `${cx + R * val * Math.cos(angle)},${cy + R * val * Math.sin(angle)}`;
      });
      g.append('polygon').attr('points', pts.join(' '))
        .attr('fill', color).attr('fill-opacity', 0.15).attr('stroke', color).attr('stroke-width', 1.5);
    };
    drawPolygon(sA, COLOR_A);
    if (sB) drawPolygon(sB, COLOR_B);
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
      .call(ax => { ax.selectAll('text').style('fill','#849495').style('font-size','8px'); ax.select('.domain').style('stroke','rgba(255,255,255,0.1)'); });
    g.append('g').call(d3.axisLeft(y).ticks(4).tickFormat(d => `${d}%`).tickSize(3))
      .call(ax => { ax.selectAll('text').style('fill','#849495').style('font-size','8px'); ax.select('.domain').style('stroke','rgba(255,255,255,0.1)'); ax.selectAll('.tick line').style('stroke','rgba(255,255,255,0.1)'); });

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
        .style('font-family', 'JetBrains Mono, monospace').style('font-size', '8px').style('fill', '#849495')
        .text(lbl.length > 16 ? lbl.slice(0, 15) + '…' : lbl);

      const pA = pct(itemsA, lbl, totalA);
      const cA = cnt(itemsA, lbl);
      g.append('rect').attr('x', 0).attr('y', yPos + (itemsB ? 1 : (rowH - barH) / 2))
        .attr('width', x(pA)).attr('height', barH).attr('fill', COLOR_A).attr('opacity', 0.7).attr('rx', 1);
      if (pA > 0) g.append('text')
        .attr('x', x(pA) + 3).attr('y', yPos + (itemsB ? barH / 2 + 1 : rowH / 2))
        .attr('dominant-baseline', 'middle').style('font-family','JetBrains Mono,monospace').style('font-size','7px').style('fill', COLOR_A).style('opacity','0.85')
        .text(`${pA.toFixed(1)}% (${cA})`);

      if (itemsB) {
        const pB = pct(itemsB, lbl, totalB);
        const cB = cnt(itemsB, lbl);
        g.append('rect').attr('x', 0).attr('y', yPos + barH + 2)
          .attr('width', x(pB)).attr('height', barH).attr('fill', COLOR_B).attr('opacity', 0.7).attr('rx', 1);
        if (pB > 0) g.append('text')
          .attr('x', x(pB) + 3).attr('y', yPos + barH * 1.5 + 2)
          .attr('dominant-baseline', 'middle').style('font-family','JetBrains Mono,monospace').style('font-size','7px').style('fill', COLOR_B).style('opacity','0.85')
          .text(`${pB.toFixed(1)}% (${cB})`);
      }
    });
  }

  // ── SVG refs ───────────────────────────────────────────────────────────────

  let svgBpm: SVGSVGElement = $state(undefined as unknown as SVGSVGElement);
  let svgDuration: SVGSVGElement = $state(undefined as unknown as SVGSVGElement);
  let svgLoudness: SVGSVGElement = $state(undefined as unknown as SVGSVGElement);
  let svgRadar: SVGSVGElement = $state(undefined as unknown as SVGSVGElement);
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
    const sA = statsA, sB = statsB;
    if (svgRadar) scheduleRender(() => renderMoodRadar(svgRadar, sA, sB));
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

  onMount(() => { loadLibrary(); });
</script>

<div class="stats-panel">

  <!-- ── Status bar ───────────────────────────────────────────────────────── -->
  <div class="set-bar">
    <div class="set-slot">
      <span class="set-dot" style="background:{COLOR_A}"></span>
      <span class="set-name">Full Library</span>
      {#if statsA}<span class="set-count">{statsA.track_count} tracks</span>{/if}
      {#if loadingA}<span class="loading-badge">Computing…</span>{/if}
    </div>
    <div class="set-divider">vs</div>
    <div class="set-slot">
      <span class="set-dot" style="background:{COLOR_B}"></span>
      <span class="set-name">Current Filter</span>
      {#if statsB}<span class="set-count">{statsB.track_count} tracks</span>{/if}
      {#if loadingB}<span class="loading-badge">Computing…</span>{/if}
    </div>
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
        <svg bind:this={svgRadar} class="chart-svg chart-radar"></svg>
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
                  <div class="cov-bar" style="width:{row.pctA}%;background:{COLOR_A}"></div>
                  <span class="cov-pct">{row.pctA.toFixed(0)}%</span>
                </div>
              </td>
              {#if statsB}
                <td>
                  <div class="cov-wrap">
                    <div class="cov-bar" style="width:{row.pctB ?? 0}%;background:{COLOR_B}"></div>
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
    font-family: "JetBrains Mono", monospace;
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

  .set-name {
    font-size: 10px; font-weight: 700; letter-spacing: 0.06em;
    color: var(--sg-on-surface, #e3e1e9);
  }

  .set-count {
    font-size: 9px; color: var(--sg-outline, #849495);
  }

  .set-divider {
    font-size: 9px; color: var(--sg-outline, #849495);
    letter-spacing: 0.06em; opacity: 0.6;
  }

  .loading-badge { font-size: 9px; color: var(--sg-primary, #00f0ff); opacity: 0.7; letter-spacing: 0.05em; }

  .error-row {
    padding: 0.5rem 1.2rem; font-size: 10px; color: #ff6b6b;
    background: rgba(255,80,80,0.05); border-bottom: 1px solid rgba(255,80,80,0.15);
  }

  .empty-state {
    display: flex; align-items: center; justify-content: center; flex: 1;
    font-size: 11px; color: var(--sg-outline, #849495);
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
    font-size: 9px; font-weight: 700; letter-spacing: 0.12em;
    color: var(--sg-outline, #849495); text-transform: uppercase;
    margin: 0 0 0.6rem 0;
  }

  /* ── KPIs ── */
  .kpi-grid { display: flex; flex-wrap: wrap; gap: 0.5rem; }

  .kpi-card {
    background: rgba(255,255,255,0.03); border: 1px solid rgba(255,255,255,0.07);
    border-radius: 5px; padding: 0.45rem 0.7rem; min-width: 80px; flex: 1;
  }
  .kpi-label { font-size: 8px; color: var(--sg-outline,#849495); letter-spacing: 0.06em; text-transform: uppercase; margin-bottom: 3px; }
  .kpi-val-a { font-size: 14px; font-weight: 700; color: var(--sg-primary,#00f0ff); }
  .kpi-val-b { font-size: 11px; font-weight: 600; color: #ff7c5c; margin-top: 1px; }

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

  .scale-label { font-size: 8px; color: var(--sg-outline,#849495); width: 36px; flex-shrink: 0; }

  .scale-track {
    flex: 1; height: 14px; background: rgba(255,255,255,0.04); border-radius: 3px;
    overflow: hidden; display: flex; flex-direction: column; gap: 1px;
  }

  .scale-fill-a { height: 6px; background: var(--sg-primary,#00f0ff); opacity: 0.75; border-radius: 2px; }
  .scale-fill-b { height: 6px; background: #ff7c5c; opacity: 0.75; border-radius: 2px; }

  .scale-pct { font-size: 8px; color: var(--sg-outline,#849495); width: 28px; text-align: right; }

  /* ── Mood ── */
  .mood-layout { display: flex; gap: 1rem; align-items: flex-start; }
  .mood-layout .chart-radar { width: 220px; flex-shrink: 0; }

  .mood-bars { flex: 1; display: flex; flex-direction: column; gap: 7px; padding-top: 4px; }

  .mood-row { display: flex; align-items: center; gap: 6px; }
  .mood-label { font-size: 8px; color: var(--sg-outline,#849495); width: 64px; flex-shrink: 0; }

  .mood-track {
    flex: 1; height: 14px; background: rgba(255,255,255,0.04); border-radius: 3px;
    overflow: hidden; display: flex; flex-direction: column; gap: 1px;
  }
  .mood-fill { height: 6px; border-radius: 2px; }
  .mood-fill-a { background: var(--sg-primary,#00f0ff); opacity: 0.75; }
  .mood-fill-b { background: #ff7c5c; opacity: 0.75; }

  .mood-val { font-size: 8px; color: var(--sg-outline,#849495); width: 28px; text-align: right; }

  /* ── Vocal ── */
  .vocal-bars { display: flex; flex-direction: column; gap: 5px; }
  .vocal-row { display: flex; align-items: center; gap: 6px; }
  .vocal-label { font-size: 8px; color: var(--sg-outline,#849495); width: 72px; flex-shrink: 0; }

  .vocal-track {
    flex: 1; height: 14px; background: rgba(255,255,255,0.04); border-radius: 3px;
    overflow: hidden; display: flex; flex-direction: column; gap: 1px;
  }
  .vocal-fill { height: 6px; border-radius: 2px; }
  .vocal-fill-a { background: var(--sg-primary,#00f0ff); opacity: 0.75; }
  .vocal-fill-b { background: #ff7c5c; opacity: 0.75; }
  .vocal-count { font-size: 8px; color: var(--sg-outline,#849495); width: 32px; text-align: right; }

  /* ── Horizontal bar chart ── */
  .horiz-wrap { overflow: hidden; }

  /* ── Analysis coverage table ── */
  .cov-table { width: 100%; border-collapse: collapse; font-size: 9px; }

  .cov-table th {
    font-size: 8px; color: var(--sg-outline,#849495); text-align: left;
    padding: 3px 8px 5px 0; border-bottom: 1px solid rgba(255,255,255,0.07);
    font-weight: 700; letter-spacing: 0.06em;
  }

  .cov-table td { padding: 4px 8px 4px 0; border-bottom: 1px solid rgba(255,255,255,0.04); vertical-align: middle; }

  .pass-name { color: var(--sg-on-surface,#e3e1e9); font-size: 9px; white-space: nowrap; }

  .cov-wrap { display: flex; align-items: center; gap: 6px; min-width: 120px; }

  .cov-bar { height: 6px; border-radius: 3px; flex-shrink: 0; min-width: 0; max-width: 80px; }

  .cov-pct { font-size: 8px; color: var(--sg-outline,#849495); min-width: 28px; }

  /* ── Responsive ── */
  @media (max-width: 700px) {
    .two-col { grid-template-columns: 1fr; }
    .mood-layout { flex-direction: column; }
    .mood-layout .chart-radar { width: 100%; }
  }
</style>
