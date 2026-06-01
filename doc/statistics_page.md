# Statistics Page

## Motivation

A dedicated statistics page lets you understand the structure, character, and coverage of your library — and compare slices of it against each other. The original music-index prototype (Streamlit/Plotly) established the core ideas; this design generalises them to all the data Deep Cuts now produces and integrates comparison across arbitrary saved searches and playlists.

---

## Comparison Model

The page is built around **track sets**. A track set is any of:

- The full library
- A saved search (dynamic — re-evaluated at view time)
- A playlist (static — fixed list of track IDs)
- The current filter state ("what's showing in the library right now")

Up to **3 track sets** can be loaded side by side. Each set gets a colour. All charts render all sets overlaid or grouped, so differences are immediately visible.

Example comparisons:
- "My own songs" vs "References folder" vs "Full library"
- "Workout playlist" vs "Late night saved search"
- "Analysed tracks" vs "Pending analysis" (coverage audit)

---

## Sections

### 1. Summary KPIs

A row of metric cards at the top, one column per track set. Metrics shown:

| Metric | Source |
|---|---|
| Track count | Count |
| Total duration | `duration_seconds` sum |
| Average BPM | `bpm` mean |
| BPM std deviation | `bpm` stddev |
| Most common key | `key` + `scale` mode |
| Key variety | Distinct key+scale count / 24 |
| % vocals | `detected_vocal` |
| % analysed | Rows with non-null CLAP embedding |
| Average loudness | `loudness_lufs` mean |
| Mood centroid | Radar thumbnail (inline) |

Deltas between sets are shown as ↑↓ indicators relative to the first set.

---

### 2. BPM Distribution

Overlaid histograms (one per set, semi-transparent fill). 40 bins. X-axis: BPM. Y-axis: % of tracks in set (normalised, so sets of different sizes are comparable).

Vertical lines for each set's mean BPM.

Optional: BPM range bands labelled (Slow / Mid / Fast / V.Fast) matching the sidebar filter presets.

---

### 3. Key & Scale Distribution

Two charts:

**Key frequency (chromatic)** — grouped bar chart, 12 chromatic pitches (C, C#, D, Eb, E, F, F#, G, Ab, A, Bb, B), one bar group per set. Shows whether a set is tonally biassed toward certain root notes.

**Major vs Minor** — stacked or grouped bar, one bar per set. Quick read on tonal character.

**Key + Scale top-N ranking** — horizontal bar chart, top 16 key+scale combinations by frequency across all sets combined, showing each set's count side by side.

**Camelot wheel** — a circular plot (or heatmap of the 24 Camelot positions) showing density per set. Highlights harmonic "hot spots."

---

### 4. Mood Profile

**Radar chart** — one polygon per set, overlaid. Axes: happy, sad, aggressive, relaxed, party, acoustic, electronic. Values: mean score across tracks in the set.

**Mood distributions** — one histogram per mood dimension (7 histograms), each showing all sets overlaid. Reveals whether a set is bimodal (tracks are either happy or not) vs uniformly distributed.

**Mood × BPM heatmap** — rows: BPM ranges, columns: dominant mood (argmax of mood vector). Shows whether high-BPM tracks in a set tend toward a particular mood. Generalises the BPM × Key matrix from the prototype.

---

### 5. Genre Breakdown

**Treemap** — one treemap per set (or a combined treemap with set as the root level). Hierarchy: top-level genre → sub-genre. Drawn from `detected_genre` (Essentia) and `ai_genre` (Qwen2) with a toggle to switch source.

**Top genres bar chart** — top 12 genres, grouped bars, one group per genre. Direct cross-set comparison.

---

### 6. Instruments & Vocal Character

Sourced from Qwen2-Audio free-text descriptions (parsed into tags once the tag system exists — see `tagging_ideas.md`) and `detected_vocal`.

**Instrument frequency** — horizontal bar chart, top 15 instruments mentioned. One bar group per set.

**Vocal breakdown** — stacked bar: vocals / instrumental / unknown. One bar per set.

---

### 7. Duration & File Quality

**Duration distribution** — histogram, track length in minutes. Useful for spotting sets that skew toward short loops vs full songs.

**Bitrate / sample rate distribution** — histogram or pie. Reveals quality profile of a set (lossy vs lossless, 44.1 vs 48 kHz, etc.)

**Loudness distribution** — histogram of `loudness_lufs`. One per set overlaid. Useful for checking whether a reference set is mastered louder than your own tracks.

---

### 8. Analysis Coverage

A progress-style section showing what percentage of each set has been through each analysis pass:

| Pass | Metric |
|---|---|
| Essentia | Non-null `key` |
| Mood classifiers | Non-null `mood_happy` |
| Qwen2-Audio | Non-null `description` |
| CLAP embeddings | Non-null embedding in `embeddings` table |
| UMAP coordinates | Non-null coords in `map_coordinates` |
| AcoustID enrichment | `acoustid_status` breakdown |

Shown as a table with colour-coded coverage bars. Stale tracks (per `is_stale`) broken out separately.

---

### 9. Similarity & Overlap

How much do two sets overlap or resemble each other?

**Track overlap** — simple: count of tracks that appear in both sets (for static sets / playlists). Shown as a Venn-style count.

**Embedding centroid distance** — compute the mean CLAP embedding for each set; report cosine distance between centroids. A measure of how acoustically similar the sets are in aggregate.

**Nearest-neighbour cross-set similarity** — for each track in set A, find its nearest neighbour in set B by CLAP distance; report the distribution of those distances as a histogram. A tight distribution near 0 means the sets are acoustically close; a broad distribution means they cover different territory.

---

### 10. Listening History (future)

Once play events are logged, a time-series section can show:
- Tracks played per day / week
- Most-played tracks and artists
- Mood profile of what you've actually listened to vs what's in the library

---

## UI Layout

```
┌─────────────────────────────────────────────────────────┐
│  Set A: [Full Library ▾]  Set B: [References ▾]  Set C: [+ Add] │
├─────────────────────────────────────────────────────────┤
│  [Summary KPIs — metric cards]                          │
├────────────────────┬────────────────────────────────────┤
│  BPM Distribution  │  Key & Scale                       │
├────────────────────┴────────────────────────────────────┤
│  Mood Profile (radar + distributions)                   │
├────────────────────┬────────────────────────────────────┤
│  Genre Treemap     │  Instruments & Vocals              │
├────────────────────┴────────────────────────────────────┤
│  Duration · Loudness · File Quality                     │
├─────────────────────────────────────────────────────────┤
│  Analysis Coverage                                      │
├─────────────────────────────────────────────────────────┤
│  Similarity & Overlap (if 2+ sets)                      │
└─────────────────────────────────────────────────────────┘
```

Charts are rendered with D3 on canvas/SVG, consistent with the map view. No third-party charting library needed — D3 already covers histograms, bar charts, radar charts, and heatmaps.

---

## Implementation Notes

- All statistics are computed in Rust (SQL aggregations + application-level stats) via IPC commands. The frontend receives pre-aggregated data, not raw track lists, keeping payloads small.
- Heavy computations (centroid distances, nearest-neighbour distributions) run in a background task and stream results to the frontend as they complete.
- Charts share a consistent colour scheme per set (set A = accent colour 1, set B = accent colour 2, set C = accent colour 3) carried through all sections.

---

## Open Questions

1. **Export** — should statistics be exportable (CSV of aggregates, PNG of charts)? Natural ask for a "production habits" workflow.

2. **Percentile mode** — the prototype normalised mood scores to percentile rank within the collection, making skewed distributions more readable. Worth offering as a toggle on mood charts.

3. **Which charts need interactivity** — clicking a bar in the genre chart to filter to those tracks, for example, would be powerful but complex. Scope for v1: all charts are read-only; interactivity in a follow-up.

4. **Saved statistics snapshots** — rather than a separate snapshot mechanism, the `analysis_results` versioning table (`private/analysis-data-retention.md`) already stores timestamped pass outputs. Querying it at a historical `ran_at` timestamp gives a past view of any track set's statistics for free — "here's how the mood profile of your References folder looked 3 months ago vs today."

---

## Cross-References

- **Playlists & saved searches** (`playlists_and_saved_searches.md`) — these are the primary track set inputs. The nearest-neighbour overlap section can directly suggest tracks to add to a playlist, turning a statistical observation into a curation action.
- **Tag system** (`tagging_ideas.md`) — the analysis coverage section can auto-apply a `needs-analysis` tag to tracks missing a given pass, making coverage gaps immediately actionable as a saved search.
- **Mood filtering / radar** (`mood_filtering_ideas.md`) — the mood centroid radar thumbnail in the summary KPIs reuses the radar component, making any track set's mood profile comparable at a glance without opening the filter sidebar.
- **Analysis data retention** (`private/analysis-data-retention.md`) — versioned pass results enable historical comparison without a separate snapshot mechanism; past statistics are derivable from `analysis_results` timestamps.
