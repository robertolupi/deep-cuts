<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";
  import * as d3 from "d3";

  interface LatencyStat {
    pass_name: string;
    avg_duration_ms: number;
    min_duration_ms: number;
    max_duration_ms: number;
    count: number;
  }

  interface PipelineMetricRow {
    id: number;
    run_id: string;
    track_id: number;
    pass_name: string;
    status: string;
    duration_ms: number;
    started_at: number;
    ended_at: number;
    audio_duration_sec: number | null;
    error_message: string | null;
  }

  interface MetricsSummary {
    latencies: LatencyStat[];
    recent_failures: PipelineMetricRow[];
  }

  interface AggregatedPassSpan {
    run_id: string;
    pass_name: string;
    started_at: number;
    ended_at: number;
    total: number;
    succeeded: number;
    failed: number;
  }

  let activeTab = $state<"latency" | "traces" | "failures">("latency");
  let summary = $state<MetricsSummary>({ latencies: [], recent_failures: [] });
  let traceSpans = $state<AggregatedPassSpan[]>([]);
  let isLoading = $state(false);
  let errorMessage = $state("");

  // Derive the run list from aggregated spans
  let runs = $derived.by(() => {
    const runMap = new Map<string, { start: number; end: number; passes: number }>();
    for (const s of traceSpans) {
      const existing = runMap.get(s.run_id);
      if (existing) {
        existing.start = Math.min(existing.start, s.started_at);
        existing.end = Math.max(existing.end, s.ended_at);
        existing.passes += 1;
      } else {
        runMap.set(s.run_id, { start: s.started_at, end: s.ended_at, passes: 1 });
      }
    }
    return Array.from(runMap.entries())
      .map(([id, info]) => ({
        id,
        date: new Date(info.start).toLocaleString(),
        duration_sec: formatDuration((info.end - info.start) / 1000),
        passes: info.passes,
        timestamp: info.start,
      }))
      .sort((a, b) => b.timestamp - a.timestamp);
  });

  let selectedRunId = $state<string>("");
  let selectedSpan = $state<AggregatedPassSpan | null>(null);

  // Spans for the selected run, ordered by start time
  let runSpans = $derived(
    traceSpans
      .filter((s) => s.run_id === selectedRunId)
      .sort((a, b) => a.started_at - b.started_at)
  );

  let chartContainer = $state<HTMLDivElement | null>(null);

  async function loadData() {
    isLoading = true;
    errorMessage = "";
    try {
      [summary, traceSpans] = await Promise.all([
        invoke<MetricsSummary>("get_metrics_summary"),
        invoke<AggregatedPassSpan[]>("get_pipeline_run_traces"),
      ]);
      if (runs.length > 0 && !selectedRunId) {
        selectedRunId = runs[0].id;
      }
    } catch (err: any) {
      errorMessage = err.toString();
    } finally {
      isLoading = false;
    }
  }

  onMount(() => {
    loadData();
  });

  $effect(() => {
    if (activeTab === "traces" && runSpans.length > 0 && chartContainer) {
      renderGanttChart();
    }
  });

  // Format a duration in seconds as a compact human-readable string:
  //   < 1s   → "423ms"
  //   < 60s  → "42s"
  //   < 1h   → "3m 07s"
  //   >= 1h  → "1h 23m 07s"
  function formatDuration(seconds: number): string {
    if (seconds < 1) return `${Math.round(seconds * 1000)}ms`;
    const h = Math.floor(seconds / 3600);
    const m = Math.floor((seconds % 3600) / 60);
    const rem = seconds % 60;
    if (h > 0) return `${h}h ${m}m ${String(Math.round(rem)).padStart(2, "0")}s`;
    if (m > 0) return `${m}m ${String(Math.round(rem)).padStart(2, "0")}s`;
    return `${rem.toFixed(1)}s`;
  }

  const PASS_ORDER = [
    "audio_analysis",
    "bpm_correction",
    "sax",
    "clap",
    "essentia",
    "bpm_refinement",
    "qwen",
    "description_embed",
  ];

  const PASS_COLORS: Record<string, string> = {
    audio_analysis:    "#00f0ff",
    bpm_correction:    "#9b5de5",
    sax:               "#5ba3c9",
    clap:              "#ff007f",
    essentia:          "#00f5d4",
    bpm_refinement:    "#fee440",
    qwen:              "#ff9f1c",
    description_embed: "#00bbf9",
  };

  function renderGanttChart() {
    if (!chartContainer) return;
    d3.select(chartContainer).selectAll("*").remove();

    const spans = runSpans;
    const minTime = d3.min(spans, (d) => d.started_at) ?? 0;
    const maxTime = d3.max(spans, (d) => d.ended_at) ?? 0;
    const totalDuration = maxTime - minTime || 1;

    // Y domain: passes present in this run, in pipeline order
    const presentPasses = PASS_ORDER.filter((p) => spans.some((s) => s.pass_name === p));
    // Append any unknown passes not in PASS_ORDER
    spans.forEach((s) => { if (!presentPasses.includes(s.pass_name)) presentPasses.push(s.pass_name); });

    const margin = { top: 16, right: 30, bottom: 36, left: 130 };
    const width = chartContainer.clientWidth - margin.left - margin.right;
    const rowHeight = 36;
    const height = presentPasses.length * rowHeight;

    const svg = d3
      .select(chartContainer)
      .append("svg")
      .attr("width", width + margin.left + margin.right)
      .attr("height", height + margin.top + margin.bottom)
      .append("g")
      .attr("transform", `translate(${margin.left},${margin.top})`);

    const xScale = d3
      .scaleLinear()
      .domain([0, totalDuration / 1000])
      .range([0, width]);

    const yScale = d3
      .scaleBand<string>()
      .domain(presentPasses)
      .range([0, height])
      .padding(0.3);

    // X axis
    svg
      .append("g")
      .attr("transform", `translate(0,${height})`)
      .call(d3.axisBottom(xScale).ticks(6).tickFormat((d) => formatDuration(d as number)))
      .call((g) => g.select(".domain").attr("stroke", "rgba(255,255,255,0.1)"))
      .call((g) => g.selectAll(".tick line").attr("stroke", "rgba(255,255,255,0.1)"))
      .call((g) => g.selectAll(".tick text").attr("fill", "var(--sg-outline, #849495)").style("font-family", "JetBrains Mono").style("font-size", "var(--sg-text-2xs)"));

    // Y axis (pass names)
    svg
      .append("g")
      .call(d3.axisLeft(yScale))
      .call((g) => g.select(".domain").remove())
      .call((g) => g.selectAll(".tick line").remove())
      .call((g) => g.selectAll(".tick text")
        .attr("fill", (d: any) => PASS_COLORS[d] ?? "#e3e1e9")
        .style("font-family", "JetBrains Mono")
        .style("font-size", "var(--sg-text-xs)")
        .style("font-weight", "700"));

    // Horizontal grid lines
    svg.selectAll(".grid-line")
      .data(presentPasses)
      .enter()
      .append("line")
      .attr("x1", 0).attr("x2", width)
      .attr("y1", (d) => (yScale(d) ?? 0) + yScale.bandwidth() / 2)
      .attr("y2", (d) => (yScale(d) ?? 0) + yScale.bandwidth() / 2)
      .attr("stroke", "rgba(255,255,255,0.03)")
      .attr("stroke-dasharray", "2,2");

    // Bars
    svg.selectAll(".bar")
      .data(spans)
      .enter()
      .append("rect")
      .attr("class", "bar")
      .attr("x", (d) => xScale((d.started_at - minTime) / 1000))
      .attr("y", (d) => yScale(d.pass_name) ?? 0)
      .attr("width", (d) => Math.max(4, xScale((d.ended_at - minTime) / 1000) - xScale((d.started_at - minTime) / 1000)))
      .attr("height", yScale.bandwidth())
      .attr("fill", (d) => PASS_COLORS[d.pass_name] ?? "#ffffff")
      .attr("rx", 3)
      .style("cursor", "default")
      .style("opacity", 0.7)
      .style("stroke", "none")
      .style("stroke-width", "1.5px")
      .on("mouseover", (_, d) => {
        selectedSpan = d;
        svg.selectAll(".bar")
          .style("opacity", (b: any) => (b.pass_name === d.pass_name ? 1.0 : 0.4))
          .style("stroke", (b: any) => (b.pass_name === d.pass_name ? "#ffffff" : "none"));
      })
      .on("mouseout", () => {
        selectedSpan = null;
        svg.selectAll(".bar").style("opacity", 0.7).style("stroke", "none");
      });

    // Track count labels inside bars (if wide enough)
    svg.selectAll(".bar-label")
      .data(spans)
      .enter()
      .append("text")
      .attr("class", "bar-label")
      .attr("x", (d) => xScale((d.started_at - minTime) / 1000) + Math.max(4, xScale((d.ended_at - minTime) / 1000) - xScale((d.started_at - minTime) / 1000)) / 2)
      .attr("y", (d) => (yScale(d.pass_name) ?? 0) + yScale.bandwidth() / 2 + 1)
      .attr("text-anchor", "middle")
      .attr("dominant-baseline", "middle")
      .style("font-family", "JetBrains Mono")
      .style("font-size", "var(--sg-text-3xs)")
      .style("font-weight", "700")
      .style("fill", "rgba(0,0,0,0.7)")
      .style("pointer-events", "none")
      .text((d) => {
        const barWidth = Math.max(4, xScale((d.ended_at - minTime) / 1000) - xScale((d.started_at - minTime) / 1000));
        return barWidth >= 28 ? `${d.total}` : "";
      });
  }
</script>

<div class="metrics-panel">
  <!-- Tabs Navigation -->
  <div class="tabs-nav">
    <button class="tab-btn" class:active={activeTab === "latency"} onclick={() => activeTab = "latency"}>Average Latencies</button>
    <button class="tab-btn" class:active={activeTab === "traces"} onclick={() => activeTab = "traces"}>Pipeline Traces</button>
    <button class="tab-btn" class:active={activeTab === "failures"} onclick={() => activeTab = "failures"}>Recent Failures ({summary.recent_failures.length})</button>
  </div>

  <div class="tab-content-container">
    {#if errorMessage}
      <div class="alert-error">
        <span>{errorMessage}</span>
      </div>
    {/if}

    {#if activeTab === "latency"}
      <div class="tab-pane">
        <div class="panel-header-row">
          <div>
            <h4 class="title">Average Execution Latency</h4>
            <p class="subtitle">Processing duration statistics grouped by pipeline analysis pass</p>
          </div>
          <button class="sg-btn" onclick={loadData} disabled={isLoading}>Refresh</button>
        </div>

        {#if summary.latencies.length === 0}
          <div class="empty-state">No pipeline metrics recorded yet. Run a library scan or file analysis.</div>
        {:else}
          <div class="latency-grid">
            {#each summary.latencies as stat}
              <div class="latency-card">
                <div class="latency-header">
                  <span class="pass-badge {stat.pass_name}">{stat.pass_name}</span>
                  <span class="count-badge">{stat.count} runs</span>
                </div>
                <div class="latency-stats">
                  <div class="stat-main">
                    <span class="val">{formatDuration(stat.avg_duration_ms / 1000)}</span>
                    <span class="lbl">AVERAGE</span>
                  </div>
                  <div class="stat-row">
                    <div>
                      <span class="val-sub">{formatDuration(stat.min_duration_ms / 1000)}</span>
                      <span class="lbl-sub">MIN</span>
                    </div>
                    <div>
                      <span class="val-sub">{formatDuration(stat.max_duration_ms / 1000)}</span>
                      <span class="lbl-sub">MAX</span>
                    </div>
                  </div>
                </div>
              </div>
            {/each}
          </div>
        {/if}
      </div>

    {:else if activeTab === "traces"}
      <div class="tab-pane traces-pane">
        <div class="traces-layout">
          <!-- Left side: Run selection list -->
          <div class="runs-sidebar">
            <span class="section-title">ANALYSIS RUNS</span>
            {#if runs.length === 0}
              <p class="empty-runs">No traces recorded.</p>
            {:else}
              <div class="runs-list">
                {#each runs as r}
                  <button
                    class="run-row"
                    class:active={selectedRunId === r.id}
                    onclick={() => {
                      selectedRunId = r.id;
                      selectedSpan = null;
                    }}
                  >
                    <div class="run-date">{r.date}</div>
                    <div class="run-meta">
                      <span>{r.passes} passes</span>
                      <span>·</span>
                      <span>{r.duration_sec}</span>
                    </div>
                  </button>
                {/each}
              </div>
            {/if}
          </div>

          <!-- Right side: Gantt Visualizer + span metadata -->
          <div class="traces-body">
            {#if !selectedRunId}
              <div class="empty-state">Select an analysis run from the sidebar to inspect traces.</div>
            {:else}
              <div class="trace-visualizer-container">
                <div class="visualizer-header">
                  <div>
                    <span class="visualizer-title">Run {new Date(runs.find(r => r.id === selectedRunId)?.timestamp ?? 0).toLocaleString()}</span>
                    <p class="visualizer-subtitle">Aggregated wall-clock span per pipeline phase — click a bar for details</p>
                  </div>
                </div>

                <div bind:this={chartContainer} class="d3-chart-container"></div>

                <!-- Selected Span Metadata -->
                <div class="span-details-card">
                  {#if !selectedSpan}
                    <div class="span-details-empty">
                      <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10"/><line x1="12" y1="16" x2="12" y2="12"/><line x1="12" y1="8" x2="12.01" y2="8"/></svg>
                      Hover over any bar in the Gantt chart to inspect execution details and timings.
                    </div>
                  {:else}
                    <div class="span-details-content">
                      <div class="span-field">
                        <span class="label">PASS</span>
                        <span class="value pass-name-val {selectedSpan.pass_name}">{selectedSpan.pass_name}</span>
                      </div>
                      <div class="span-field">
                        <span class="label">WALL-CLOCK</span>
                        <span class="value">{formatDuration((selectedSpan.ended_at - selectedSpan.started_at) / 1000)}</span>
                      </div>
                      <div class="span-field">
                        <span class="label">TRACKS</span>
                        <span class="value">{selectedSpan.total}</span>
                      </div>
                      <div class="span-field">
                        <span class="label">SUCCEEDED</span>
                        <span class="value status-val success">{selectedSpan.succeeded}</span>
                      </div>
                      {#if selectedSpan.failed > 0}
                        <div class="span-field">
                          <span class="label">FAILED</span>
                          <span class="value status-val failed">{selectedSpan.failed}</span>
                        </div>
                      {/if}
                      <div class="span-field">
                        <span class="label">AVG / TRACK</span>
                        <span class="value">{selectedSpan.total > 0 ? formatDuration(((selectedSpan.ended_at - selectedSpan.started_at) / selectedSpan.total) / 1000) : "—"}</span>
                      </div>
                    </div>
                  {/if}
                </div>
              </div>
            {/if}
          </div>
        </div>
      </div>

    {:else if activeTab === "failures"}
      <div class="tab-pane">
        <div class="panel-header-row">
          <div>
            <h4 class="title">Recent Failures</h4>
            <p class="subtitle">History of failed pipeline analysis operations (anonymized)</p>
          </div>
          <button class="sg-btn" onclick={loadData} disabled={isLoading}>Refresh</button>
        </div>

        {#if summary.recent_failures.length === 0}
          <div class="empty-state">No failed analysis passes found. Your pipeline runs clean!</div>
        {:else}
          <div class="failures-list">
            {#each summary.recent_failures as fail}
              <div class="failure-row">
                <div class="failure-meta">
                  <span class="pass-badge {fail.pass_name}">{fail.pass_name}</span>
                  <span class="track-tag">Track #{fail.track_id}</span>
                  <span class="time-tag">{new Date(fail.started_at).toLocaleString()}</span>
                </div>
                <pre class="error-box">{fail.error_message ?? "Unknown error"}</pre>
              </div>
            {/each}
          </div>
        {/if}
      </div>

    {/if}
  </div>
</div>

<style>
  .metrics-panel {
    display: flex;
    flex-direction: column;
    height: 600px;
    max-height: 80vh;
    background: var(--sg-surface-slate, #161b22);
    border-radius: 6px;
    overflow: hidden;
  }

  /* ── Tabs Navigation ── */
  .tabs-nav {
    display: flex;
    border-bottom: 1px solid rgba(255, 255, 255, 0.08);
    background: rgba(0, 0, 0, 0.15);
  }

  .tab-btn {
    flex: 1;
    padding: 12px;
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-xs);
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--sg-outline, #849495);
    background: transparent;
    border: none;
    border-bottom: 2px solid transparent;
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .tab-btn:hover {
    color: var(--sg-on-surface, #e3e1e9);
    background: rgba(255, 255, 255, 0.02);
  }

  .tab-btn.active {
    color: var(--sg-primary, #00f0ff);
    border-bottom-color: var(--sg-primary, #00f0ff);
    background: rgba(0, 240, 255, 0.03);
  }

  /* ── Content Panes ── */
  .tab-content-container {
    flex: 1;
    overflow-y: auto;
    padding: 1.25rem;
    scrollbar-width: thin;
    scrollbar-color: rgba(255,255,255,0.1) transparent;
  }

  .tab-pane {
    display: flex;
    flex-direction: column;
    gap: 1.25rem;
    height: 100%;
  }

  .panel-header-row {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 1rem;
    border-bottom: 1px solid rgba(255, 255, 255, 0.06);
    padding-bottom: 0.75rem;
  }

  .title {
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-base);
    font-weight: 700;
    letter-spacing: 0.05em;
    text-transform: uppercase;
    margin: 0;
    color: var(--sg-on-surface, #e3e1e9);
  }

  .subtitle {
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-2xs);
    color: var(--sg-outline, #849495);
    margin: 4px 0 0 0;
  }

  /* ── Latency Tab ── */
  .latency-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
    gap: 1rem;
  }

  .latency-card {
    background: rgba(0, 0, 0, 0.18);
    border: 1px solid rgba(255, 255, 255, 0.06);
    border-radius: 5px;
    padding: 12px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .latency-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .pass-badge {
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-2xs);
    font-weight: 700;
    padding: 2px 6px;
    border-radius: 3px;
    text-transform: uppercase;
    background: rgba(255, 255, 255, 0.08);
    color: var(--sg-on-surface, #e3e1e9);
    border: 1px solid rgba(255, 255, 255, 0.12);
  }

  .pass-badge.audio_analysis { color: #00f0ff; border-color: rgba(0,240,255,0.3); background: rgba(0,240,255,0.06); }
  .pass-badge.sax { color: #5ba3c9; border-color: rgba(91,163,201,0.3); background: rgba(91,163,201,0.06); }
  .pass-badge.clap { color: #ff007f; border-color: rgba(255,0,127,0.3); background: rgba(255,0,127,0.06); }
  .pass-badge.essentia { color: #00f5d4; border-color: rgba(0,245,212,0.3); background: rgba(0,245,212,0.06); }
  .pass-badge.qwen { color: #ff9f1c; border-color: rgba(255,159,28,0.3); background: rgba(255,159,28,0.06); }
  .pass-badge.description_embed { color: #00bbf9; border-color: rgba(0,187,249,0.3); background: rgba(0,187,249,0.06); }

  .count-badge {
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-3xs);
    color: var(--sg-outline, #849495);
  }

  .latency-stats {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .stat-main {
    display: flex;
    flex-direction: column;
  }

  .stat-main .val {
    font-family: var(--sg-font-mono);
    font-size: 20px;
    font-weight: 700;
    color: var(--sg-primary, #00f0ff);
  }

  .stat-main .lbl {
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-3xs);
    color: var(--sg-outline, #849495);
    letter-spacing: 0.05em;
  }

  .stat-row {
    display: flex;
    gap: 16px;
    border-top: 1px solid rgba(255, 255, 255, 0.04);
    padding-top: 8px;
  }

  .stat-row .val-sub {
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-sm);
    font-weight: 700;
    color: var(--sg-on-surface, #e3e1e9);
  }

  .stat-row .lbl-sub {
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-3xs);
    color: var(--sg-outline, #849495);
    display: block;
  }

  /* ── Traces Tab (Gantt) ── */
  .traces-pane {
    gap: 0;
  }

  .traces-layout {
    display: grid;
    grid-template-columns: 240px 1fr;
    height: 100%;
    border: 1px solid rgba(255, 255, 255, 0.06);
    border-radius: 5px;
    overflow: hidden;
  }

  .runs-sidebar {
    background: rgba(0, 0, 0, 0.12);
    border-right: 1px solid rgba(255, 255, 255, 0.06);
    padding: 10px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    overflow-y: auto;
  }

  .section-title {
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-2xs);
    font-weight: 700;
    letter-spacing: 0.1em;
    color: var(--sg-outline, #849495);
    padding-bottom: 6px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.04);
  }

  .empty-runs {
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-2xs);
    color: var(--sg-outline, #849495);
    text-align: center;
    padding: 20px 0;
  }

  .runs-list {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .run-row {
    background: transparent;
    border: 1px solid transparent;
    border-radius: 4px;
    padding: 8px;
    text-align: left;
    cursor: pointer;
    transition: all 0.12s;
  }

  .run-row:hover {
    background: rgba(255, 255, 255, 0.03);
    border-color: rgba(255, 255, 255, 0.08);
  }

  .run-row.active {
    background: rgba(0, 240, 255, 0.04);
    border-color: rgba(0, 240, 255, 0.2);
  }

  .run-date {
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-xs);
    font-weight: 700;
    color: var(--sg-on-surface, #e3e1e9);
  }

  .run-meta {
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-3xs);
    color: var(--sg-outline, #849495);
    margin-top: 3px;
    display: flex;
    gap: 4px;
  }

  .traces-body {
    padding: 12px;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    background: rgba(0, 0, 0, 0.22);
  }

  .trace-visualizer-container {
    display: flex;
    flex-direction: column;
    gap: 16px;
    height: 100%;
  }

  .visualizer-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 1rem;
    border-bottom: 1px solid rgba(255,255,255,0.04);
    padding-bottom: 10px;
  }

  .visualizer-title {
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-sm);
    font-weight: 700;
    color: var(--sg-on-surface, #e3e1e9);
  }

  .visualizer-subtitle {
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-2xs);
    color: var(--sg-outline, #849495);
    margin: 2px 0 0 0;
  }

  .d3-chart-container {
    width: 100%;
    overflow-x: auto;
    background: rgba(0, 0, 0, 0.15);
    border: 1px solid rgba(255, 255, 255, 0.04);
    border-radius: 4px;
  }

  .span-details-card {
    background: rgba(0, 0, 0, 0.18);
    border: 1px solid rgba(255,255,255,0.06);
    border-radius: 4px;
    padding: 10px 12px;
    min-height: 50px;
  }

  .span-details-empty {
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-2xs);
    color: var(--sg-outline, #849495);
    display: flex;
    align-items: center;
    gap: 6px;
    justify-content: center;
    padding: 12px 0;
  }

  .span-details-content {
    display: flex;
    flex-wrap: wrap;
    gap: 16px 24px;
  }

  .span-field {
    display: flex;
    flex-direction: column;
    gap: 3px;
  }

  .span-field .label {
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-3xs);
    font-weight: 700;
    color: var(--sg-outline, #849495);
    letter-spacing: 0.05em;
  }

  .span-field .value {
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-xs);
    color: var(--sg-on-surface, #e3e1e9);
  }

  .pass-name-val.audio_analysis { color: #00f0ff; }
  .pass-name-val.sax { color: #5ba3c9; }
  .pass-name-val.clap { color: #ff007f; }
  .pass-name-val.essentia { color: #00f5d4; }
  .pass-name-val.qwen { color: #ff9f1c; }
  .pass-name-val.description_embed { color: #00bbf9; }

  .status-val.success { color: var(--sg-success, #00f5d4); }
  .status-val.failed { color: var(--sg-error, #ff4b4b); }

  .error-field {
    width: 100%;
    border-top: 1px solid rgba(255, 75, 75, 0.1);
    padding-top: 8px;
    margin-top: 4px;
  }

  .error-msg {
    background: rgba(255, 75, 75, 0.05);
    border: 1px solid rgba(255, 75, 75, 0.15);
    border-radius: 3px;
    color: var(--sg-error, #ff4b4b);
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-2xs);
    padding: 6px;
    margin: 4px 0 0 0;
    white-space: pre-wrap;
    max-height: 120px;
    overflow-y: auto;
  }

  /* ── Failures Tab ── */
  .failures-list {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .failure-row {
    background: rgba(255, 75, 75, 0.02);
    border: 1px solid rgba(255, 75, 75, 0.1);
    border-radius: 4px;
    padding: 10px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .failure-meta {
    display: flex;
    align-items: center;
    gap: 12px;
    flex-wrap: wrap;
  }

  .track-tag, .time-tag {
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-2xs);
    color: var(--sg-outline, #849495);
  }

  .error-box {
    background: rgba(0, 0, 0, 0.2);
    border: 1px solid rgba(255, 255, 255, 0.04);
    border-radius: 3px;
    padding: 8px;
    color: var(--sg-on-surface, #e3e1e9);
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-2xs);
    margin: 0;
    white-space: pre-wrap;
    overflow-x: auto;
  }

  .sg-btn {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-2xs);
    font-weight: 700;
    padding: 5px 10px;
    border: 1px solid rgba(255,255,255,0.12);
    border-radius: 4px;
    background: rgba(255,255,255,0.04);
    color: var(--sg-outline, #849495);
    cursor: pointer;
    transition: all 0.12s;
  }

  .sg-btn:hover:not(:disabled) {
    border-color: rgba(255,255,255,0.25);
    color: var(--sg-on-surface, #e3e1e9);
  }

  .empty-state {
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-xs);
    color: var(--sg-outline, #849495);
    text-align: center;
    padding: 40px 0;
  }

  .alert-error {
    background: rgba(255, 75, 75, 0.08);
    border: 1px solid rgba(255, 75, 75, 0.2);
    color: var(--sg-error, #ff4b4b);
    border-radius: 4px;
    padding: 8px 12px;
    font-family: var(--sg-font-mono);
    font-size: var(--sg-text-xs);
    margin-bottom: 12px;
  }
</style>
