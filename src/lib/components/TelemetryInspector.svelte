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

  interface SystemEventRow {
    id: number;
    event_type: string;
    details: string | null;
    duration_ms: number | null;
    created_at: string;
  }

  interface TelemetrySummary {
    latencies: LatencyStat[];
    recent_failures: PipelineMetricRow[];
  }

  interface RawTelemetryPayload {
    pipeline_metrics: PipelineMetricRow[];
    system_events: SystemEventRow[];
  }

  let activeTab = $state<"latency" | "traces" | "failures">("latency");
  let summary = $state<TelemetrySummary>({ latencies: [], recent_failures: [] });
  let rawPayload = $state<RawTelemetryPayload>({ pipeline_metrics: [], system_events: [] });
  let isLoading = $state(false);
  let errorMessage = $state("");

  // Trace selection state
  let runs = $derived.by(() => {
    const runMap = new Map<string, { start: number; end: number; count: number }>();
    for (const m of rawPayload.pipeline_metrics) {
      const existing = runMap.get(m.run_id);
      if (existing) {
        existing.start = Math.min(existing.start, m.started_at);
        existing.end = Math.max(existing.end, m.ended_at);
        existing.count += 1;
      } else {
        runMap.set(m.run_id, { start: m.started_at, end: m.ended_at, count: 1 });
      }
    }
    return Array.from(runMap.entries())
      .map(([id, info]) => ({
        id,
        date: new Date(info.start).toLocaleString(),
        duration_sec: ((info.end - info.start) / 1000).toFixed(1),
        count: info.count,
        timestamp: info.start,
      }))
      .sort((a, b) => b.timestamp - a.timestamp);
  });

  let selectedRunId = $state<string>("");
  let selectedSpan = $state<PipelineMetricRow | null>(null);

  // Filter metrics for the selected run
  let runMetrics = $derived(
    rawPayload.pipeline_metrics.filter((m) => m.run_id === selectedRunId)
  );

  // SVG dimensions for D3 chart
  let chartContainer = $state<HTMLDivElement | null>(null);

  async function loadData() {
    isLoading = true;
    errorMessage = "";
    try {
      summary = await invoke<TelemetrySummary>("get_telemetry_summary");
      rawPayload = await invoke<RawTelemetryPayload>("get_raw_telemetry_payload");
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

  // Watch tab selection & selected run to render D3 Gantt chart
  $effect(() => {
    if (activeTab === "traces" && runMetrics.length > 0 && chartContainer) {
      renderGanttChart();
    }
  });

  function renderGanttChart() {
    if (!chartContainer) return;
    d3.select(chartContainer).selectAll("*").remove();

    const metrics = [...runMetrics].sort((a, b) => a.started_at - b.started_at);
    const minTime = d3.min(metrics, (d) => d.started_at) || 0;
    const maxTime = d3.max(metrics, (d) => d.ended_at) || 0;
    const totalDuration = maxTime - minTime || 1;

    // Unique track IDs
    const trackIds = Array.from(new Set(metrics.map((d) => d.track_id))).sort((a, b) => a - b);

    // Layout configuration
    const margin = { top: 20, right: 30, bottom: 40, left: 70 };
    const width = chartContainer.clientWidth - margin.left - margin.right;
    const rowHeight = 32;
    const height = Math.max(120, trackIds.length * rowHeight);

    const svg = d3
      .select(chartContainer)
      .append("svg")
      .attr("width", width + margin.left + margin.right)
      .attr("height", height + margin.top + margin.bottom)
      .append("g")
      .attr("transform", `translate(${margin.left},${margin.top})`);

    // Scales
    const xScale = d3
      .scaleLinear()
      .domain([0, totalDuration / 1000]) // In seconds
      .range([0, width]);

    const yScale = d3
      .scaleBand<number>()
      .domain(trackIds)
      .range([0, height])
      .padding(0.25);

    // Color scale for pass names
    const colors: Record<string, string> = {
      audio_analysis: "#00f0ff", // Cyber Cyan
      bpm_correction: "#9b5de5", // Violet
      clap: "#ff007f",           // Studio Pink
      essentia: "#00f5d4",       // Mint Teal
      bpm_refinement: "#fee440", // Yellow
      qwen: "#ff9f1c",           // Orange
      description_embed: "#00bbf9" // Sky Blue
    };

    // Draw X Axis (Time in seconds)
    svg
      .append("g")
      .attr("transform", `translate(0,${height})`)
      .call(d3.axisBottom(xScale).ticks(8).tickFormat((d) => `${d}s`))
      .call((g) => g.select(".domain").attr("stroke", "rgba(255,255,255,0.1)"))
      .call((g) => g.selectAll(".tick line").attr("stroke", "rgba(255,255,255,0.1)"))
      .call((g) => g.selectAll(".tick text").attr("fill", "var(--sg-outline, #849495)").style("font-family", "JetBrains Mono").style("font-size", "9px"));

    // Draw Y Axis (Track IDs)
    svg
      .append("g")
      .call(d3.axisLeft(yScale).tickFormat((d) => `Track ${d}`))
      .call((g) => g.select(".domain").attr("stroke", "rgba(255,255,255,0.1)"))
      .call((g) => g.selectAll(".tick line").attr("stroke", "rgba(255,255,255,0.1)"))
      .call((g) => g.selectAll(".tick text").attr("fill", "var(--sg-outline, #849495)").style("font-family", "JetBrains Mono").style("font-size", "9px"));

    // Draw horizontal grid lines
    svg
      .selectAll(".grid-line")
      .data(trackIds)
      .enter()
      .append("line")
      .attr("x1", 0)
      .attr("x2", width)
      .attr("y1", (d) => (yScale(d) || 0) + yScale.bandwidth() / 2)
      .attr("y2", (d) => (yScale(d) || 0) + yScale.bandwidth() / 2)
      .attr("stroke", "rgba(255,255,255,0.03)")
      .attr("stroke-dasharray", "2,2");

    // Draw Bars for each metric span
    svg
      .selectAll(".bar")
      .data(metrics)
      .enter()
      .append("rect")
      .attr("class", "bar")
      .attr("x", (d) => xScale((d.started_at - minTime) / 1000))
      .attr("y", (d) => yScale(d.track_id) || 0)
      .attr("width", (d) => Math.max(3, xScale((d.ended_at - minTime) / 1000) - xScale((d.started_at - minTime) / 1000)))
      .attr("height", yScale.bandwidth())
      .attr("fill", (d) => colors[d.pass_name] || "#ffffff")
      .attr("rx", 3)
      .style("cursor", "pointer")
      .style("opacity", (d) => (selectedSpan && selectedSpan.id === d.id ? 1.0 : 0.75))
      .style("stroke", (d) => (selectedSpan && selectedSpan.id === d.id ? "#ffffff" : "none"))
      .style("stroke-width", "1.5px")
      .on("mouseover", function () {
        d3.select(this).style("opacity", 1.0);
      })
      .on("mouseout", function (event, d) {
        if (!selectedSpan || selectedSpan.id !== d.id) {
          d3.select(this).style("opacity", 0.75);
        }
      })
      .on("click", (event, d) => {
        selectedSpan = d;
        // Trigger re-render of bars to show selection border
        svg.selectAll(".bar")
          .style("opacity", (b: any) => (b.id === d.id ? 1.0 : 0.75))
          .style("stroke", (b: any) => (b.id === d.id ? "#ffffff" : "none"));
      });
  }
</script>

<div class="telemetry-panel">
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
                    <span class="val">{(stat.avg_duration_ms / 1000).toFixed(2)}s</span>
                    <span class="lbl">AVERAGE</span>
                  </div>
                  <div class="stat-row">
                    <div>
                      <span class="val-sub">{(stat.min_duration_ms / 1000).toFixed(2)}s</span>
                      <span class="lbl-sub">MIN</span>
                    </div>
                    <div>
                      <span class="val-sub">{(stat.max_duration_ms / 1000).toFixed(2)}s</span>
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
                      <span>{r.count} passes</span>
                      <span>·</span>
                      <span>{r.duration_sec}s duration</span>
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
                    <span class="visualizer-title">Run Trace: {selectedRunId}</span>
                    <p class="visualizer-subtitle">Timeline view of concurrent decoders and model inferences</p>
                  </div>
                  <div class="legend">
                    <span class="legend-item"><span class="color-dot audio_analysis"></span>Audio</span>
                    <span class="legend-item"><span class="color-dot clap"></span>CLAP</span>
                    <span class="legend-item"><span class="color-dot essentia"></span>Essentia</span>
                    <span class="legend-item"><span class="color-dot qwen"></span>Qwen</span>
                    <span class="legend-item"><span class="color-dot description_embed"></span>Embed</span>
                  </div>
                </div>

                <div bind:this={chartContainer} class="d3-chart-container"></div>

                <!-- Selected Span Metadata -->
                <div class="span-details-card">
                  {#if !selectedSpan}
                    <div class="span-details-empty">
                      <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10"/><line x1="12" y1="16" x2="12" y2="12"/><line x1="12" y1="8" x2="12.01" y2="8"/></svg>
                      Select any trace span bar in the Gantt chart to inspect execution details and timings.
                    </div>
                  {:else}
                    <div class="span-details-content">
                      <div class="span-field">
                        <span class="label">PASS NAME</span>
                        <span class="value pass-name-val {selectedSpan.pass_name}">{selectedSpan.pass_name}</span>
                      </div>
                      <div class="span-field">
                        <span class="label">ANONYMOUS TRACK ID</span>
                        <span class="value font-mono">#{selectedSpan.track_id}</span>
                      </div>
                      <div class="span-field">
                        <span class="label">STATUS</span>
                        <span class="value status-val {selectedSpan.status}">{selectedSpan.status.toUpperCase()}</span>
                      </div>
                      <div class="span-field">
                        <span class="label">DURATION</span>
                        <span class="value">{(selectedSpan.duration_ms / 1000).toFixed(3)} seconds</span>
                      </div>
                      {#if selectedSpan.audio_duration_sec}
                        <div class="span-field">
                          <span class="label">AUDIO LENGTH</span>
                          <span class="value">{selectedSpan.audio_duration_sec.toFixed(1)} seconds (x{ (selectedSpan.audio_duration_sec / (selectedSpan.duration_ms / 1000)).toFixed(1) } speed)</span>
                        </div>
                      {/if}
                      {#if selectedSpan.error_message}
                        <div class="span-field error-field">
                          <span class="label">ERROR MESSAGE</span>
                          <pre class="error-msg">{selectedSpan.error_message}</pre>
                        </div>
                      {/if}
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
  .telemetry-panel {
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
    font-family: "JetBrains Mono", monospace;
    font-size: 10px;
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
    font-family: "JetBrains Mono", monospace;
    font-size: 12px;
    font-weight: 700;
    letter-spacing: 0.05em;
    text-transform: uppercase;
    margin: 0;
    color: var(--sg-on-surface, #e3e1e9);
  }

  .subtitle {
    font-family: "JetBrains Mono", monospace;
    font-size: 9px;
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
    font-family: "JetBrains Mono", monospace;
    font-size: 9px;
    font-weight: 700;
    padding: 2px 6px;
    border-radius: 3px;
    text-transform: uppercase;
    background: rgba(255, 255, 255, 0.08);
    color: var(--sg-on-surface, #e3e1e9);
    border: 1px solid rgba(255, 255, 255, 0.12);
  }

  .pass-badge.audio_analysis { color: #00f0ff; border-color: rgba(0,240,255,0.3); background: rgba(0,240,255,0.06); }
  .pass-badge.clap { color: #ff007f; border-color: rgba(255,0,127,0.3); background: rgba(255,0,127,0.06); }
  .pass-badge.essentia { color: #00f5d4; border-color: rgba(0,245,212,0.3); background: rgba(0,245,212,0.06); }
  .pass-badge.qwen { color: #ff9f1c; border-color: rgba(255,159,28,0.3); background: rgba(255,159,28,0.06); }
  .pass-badge.description_embed { color: #00bbf9; border-color: rgba(0,187,249,0.3); background: rgba(0,187,249,0.06); }

  .count-badge {
    font-family: "JetBrains Mono", monospace;
    font-size: 8px;
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
    font-family: "JetBrains Mono", monospace;
    font-size: 20px;
    font-weight: 700;
    color: var(--sg-primary, #00f0ff);
  }

  .stat-main .lbl {
    font-family: "JetBrains Mono", monospace;
    font-size: 8px;
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
    font-family: "JetBrains Mono", monospace;
    font-size: 11px;
    font-weight: 700;
    color: var(--sg-on-surface, #e3e1e9);
  }

  .stat-row .lbl-sub {
    font-family: "JetBrains Mono", monospace;
    font-size: 8px;
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
    font-family: "JetBrains Mono", monospace;
    font-size: 9px;
    font-weight: 700;
    letter-spacing: 0.1em;
    color: var(--sg-outline, #849495);
    padding-bottom: 6px;
    border-bottom: 1px solid rgba(255, 255, 255, 0.04);
  }

  .empty-runs {
    font-family: "JetBrains Mono", monospace;
    font-size: 9px;
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
    font-family: "JetBrains Mono", monospace;
    font-size: 10px;
    font-weight: 700;
    color: var(--sg-on-surface, #e3e1e9);
  }

  .run-meta {
    font-family: "JetBrains Mono", monospace;
    font-size: 8px;
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
    font-family: "JetBrains Mono", monospace;
    font-size: 11px;
    font-weight: 700;
    color: var(--sg-on-surface, #e3e1e9);
  }

  .visualizer-subtitle {
    font-family: "JetBrains Mono", monospace;
    font-size: 9px;
    color: var(--sg-outline, #849495);
    margin: 2px 0 0 0;
  }

  .legend {
    display: flex;
    gap: 10px;
    flex-wrap: wrap;
  }

  .legend-item {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-family: "JetBrains Mono", monospace;
    font-size: 8px;
    color: var(--sg-outline, #849495);
  }

  .color-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    display: inline-block;
  }

  .color-dot.audio_analysis { background-color: #00f0ff; }
  .color-dot.clap { background-color: #ff007f; }
  .color-dot.essentia { background-color: #00f5d4; }
  .color-dot.qwen { background-color: #ff9f1c; }
  .color-dot.description_embed { background-color: #00bbf9; }

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
    font-family: "JetBrains Mono", monospace;
    font-size: 9px;
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
    font-family: "JetBrains Mono", monospace;
    font-size: 8px;
    font-weight: 700;
    color: var(--sg-outline, #849495);
    letter-spacing: 0.05em;
  }

  .span-field .value {
    font-family: "JetBrains Mono", monospace;
    font-size: 10px;
    color: var(--sg-on-surface, #e3e1e9);
  }

  .pass-name-val.audio_analysis { color: #00f0ff; }
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
    font-family: "JetBrains Mono", monospace;
    font-size: 9px;
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
    font-family: "JetBrains Mono", monospace;
    font-size: 9px;
    color: var(--sg-outline, #849495);
  }

  .error-box {
    background: rgba(0, 0, 0, 0.2);
    border: 1px solid rgba(255, 255, 255, 0.04);
    border-radius: 3px;
    padding: 8px;
    color: var(--sg-on-surface, #e3e1e9);
    font-family: "JetBrains Mono", monospace;
    font-size: 9px;
    margin: 0;
    white-space: pre-wrap;
    overflow-x: auto;
  }

  .sg-btn {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    font-family: "JetBrains Mono", monospace;
    font-size: 9px;
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
    font-family: "JetBrains Mono", monospace;
    font-size: 10px;
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
    font-family: "JetBrains Mono", monospace;
    font-size: 10px;
    margin-bottom: 12px;
  }
</style>
