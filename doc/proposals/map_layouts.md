---
status: active
owner: Roberto
last_verified: 2026-06-07
implemented_by:
superseded_by:
related_code:
related_skills:
---

# Map Layouts

## Motivation

The current map has a single layout derived from CLAP acoustic embeddings via UMAP. This is useful for perceptual similarity but collapses all other musical dimensions into one view. Different analytical questions call for different spatial arrangements:

- "What tracks share a mood?" → mood layout
- "What tracks are in a compatible key?" → tonal layout
- "What tracks have a similar vibe/theme?" → semantic layout
- "What tracks mix well at this BPM?" → rhythmic layout

Multiple named layouts, each computed from a different feature space, let the user switch perspectives on the same library.

---

## Current State

The app now has several switchable projection modes, but not the full persisted layout-management model described later in this proposal.

| Area | Status | Evidence / Notes |
| :--- | :--- | :--- |
| Switchable map projection modes | Implemented | `MusicMap.svelte` and backend map commands support modes such as `sonic`, `description`, `hybrid`, `essentia`, `harmonic`, and `genre_wheel`. |
| Animated transition between projections | Implemented | Position interpolation is part of the current map interaction model. |
| Dynamic on-demand projection computation | Implemented | Projections are computed on demand rather than stored as named layout definitions. |
| Persisted layout catalog | Need human review | The proposed `map_layouts` table, parameter serialization, and staleness logic are not implemented. Revisit only if dynamic projection recomputation becomes a real workflow problem. |
| Idle-time precomputation and coordinate stabilizers | Need human review | These are plausible scaling ideas but were not found in the production path. |

---

## Layouts

### Acoustic Similarity (current)
- **Input**: CLAP 512-dim embeddings
- **What clusters**: perceptual sound texture — timbre, instrumentation, production style
- **Strength**: best general-purpose layout; captures "sounds like" relationships
- **Weakness**: mood and key are secondary signals, not guaranteed to cluster

### Semantic / Vibe
- **Input**: sentence embeddings of Qwen2-Audio free-text descriptions
- **What clusters**: lyrical atmosphere, narrative mood, thematic content
- **Strength**: captures things CLAP misses — "melancholic rainy day", "driving industrial"
- **Weakness**: depends on Qwen2 description quality; short or generic descriptions produce weak clusters
- **Note**: requires embedding the Qwen2 text with a sentence model (e.g. `all-MiniLM-L6-v2` via ONNX — small and fast)

### Mood Profile
- **Input**: Essentia 7-dim mood vector (`mood_happy`, `mood_sad`, `mood_aggressive`, `mood_relaxed`, `mood_party`, `mood_acoustic`, `mood_electronic`)
- **What clusters**: mood regions — this layout will cluster most cleanly of all, since the input dimensions are already semantically labelled
- **Strength**: pairs perfectly with mood contour overlays and the radar filter (see `mood_filtering_ideas.md`)
- **Weakness**: only 7 dimensions — less nuanced than CLAP; many tracks may pile up in similar mood regions

### Rhythmic
- **Input**: BPM + Essentia rhythm descriptors (danceability, beat strength, onset rate)
- **What clusters**: tempo families, groove types
- **Strength**: useful for DJ-style track selection and transition planning
- **Weakness**: BPM alone produces horizontal bands rather than interesting clusters; needs rhythm descriptors to add structure

### Tonal / Harmonic
- **Input**: key (encoded as circle-of-fifths angle), scale (major/minor), key strength, Camelot wheel position
- **What clusters**: harmonic compatibility — tracks that mix well in key
- **Strength**: directly useful for harmonic mixing; Camelot neighbours will be spatially close
- **Weakness**: very low-dimensional input; many tracks share the same key so clusters may be coarse

### Hybrid (user-weighted)
- **Input**: weighted combination of any of the above feature spaces, concatenated and re-projected.
- **Dimensional Normalization & PCA Blending**: High-dimensional features like the 512-dimensional CLAP embeddings dominate low-dimensional spaces (e.g., 7-dim mood vectors) when concatenated directly. To solve this, the CLAP space is compressed using Principal Component Analysis (PCA) to a lower dimension (e.g., 16 or 32 components) and Z-score normalized before being combined with other normalized feature vectors. This ensures balanced feature influence based on user sliders.
- **What clusters**: whatever the user emphasises
- **UI**: a set of weight sliders (Acoustic / Semantic / Mood / Rhythmic / Tonal), each 0–100%. Layout recomputes when weights are committed.
- **Strength**: most flexible; lets the user ask "acoustic similarity but with mood as a tiebreaker"
- **Weakness**: UMAP on concatenated spaces can be slow; weights are hard to reason about intuitively

---

## Status

* **Implemented**: The map projection modes (`sonic`, `description`, `hybrid`, `essentia`, `harmonic`, `genre_wheel`) are fully implemented and switchable in the Svelte frontend (`MusicMap.svelte`) and backend `map.rs`. Position interpolation / transitions between layouts are also functional.
* **Not Implemented**: The `map_layouts` and coordinate layout tracking database tables, param serialization, and layout staleness logic in SQLite are not implemented. Projections are computed dynamically on-demand.

---

## Computation Strategy

**On-demand with caching**: a layout is computed the first time the user selects it, then cached in `map_coordinates`. Subsequent switches are instant. Recomputation is only triggered manually ("Recompute Map" button) or when the layout is detectably stale.

This avoids computing all layouts upfront (expensive on first launch) while keeping switches fast after the first load.

### Idle-Time Precomputation / Background Worker Queue
To prevent blocking user interactions when navigating to a new layout, the application schedules layout coordinate calculations asynchronously. A low-priority background worker queue monitors CPU idle states. When the system detects user inactivity, the worker processes outstanding stale layouts or computes coordinates for uninitialized layouts (e.g., semantic or tonal layouts) slice by slice.

### Coordinate Instability & Regressor Projections
Standard UMAP projections are highly stochastic; adding even a few new tracks and re-running UMAP can completely shift, rotate, or mirror the global coordinate space, disorienting the user. To preserve visual consistency:
- **Local Regressors**: A K-Nearest Neighbors (KNN) regressor or Parametric UMAP network is fitted on the original, stable layout coordinates.
- **Out-of-Sample Extension**: When new tracks are added to the library, their high-dimensional embeddings are projected onto the stable 2D canvas using the pre-trained local regressor rather than re-computing the entire UMAP graph.
- **Global Re-alignment**: Global UMAP recomputation is deferred until a major library update threshold is met (e.g., >20% new tracks added), at which point a Procrustes alignment transformation is applied to map the new layout coordinates onto the old coordinates to minimize spatial drift.

UMAP runs in the existing Rust/Python analysis pipeline. Each layout is a separate UMAP projection with appropriate input features and potentially different hyperparameters (e.g. the tonal layout benefits from lower `n_neighbors` to preserve fine harmonic structure).

---

## UI

### Layout switcher

Replace the current `PROJECTION` toggle (PCA / UMAP) in the map toolbar with a **Layout** dropdown:

```
LAYOUT  [ Acoustic ▾ ]   COLOR  [ Genre ▾ ]
```

Selecting a layout that hasn't been computed yet shows a "Computing…" spinner and triggers the UMAP job. Switching between already-computed layouts animates the dots smoothly from their old to new positions (interpolating x, y per track over ~800ms).

### Animated transition

When switching layouts, tracks that exist in both layouts animate to their new positions. This is visually striking and helps the user build a mental model of how the two spaces relate — a track that barely moves between acoustic and mood layouts is one where sound and mood are well-aligned.

Implementation: interpolate (x, y) per track using a D3 transition, driven by a `$state` layout ID change in the Svelte store.

### Hybrid weight panel

For the hybrid layout, a secondary panel expands below the toolbar showing weight sliders for each feature space. A "Recompute" button applies the new weights. Presets (e.g. "50% acoustic + 50% mood") can be saved.

---

## Overlay Compatibility

All layouts support the same colour modes and overlays (genre, BPM, Camelot, mood contours, user tags). Some combinations are especially informative:

| Layout | Best overlay |
|---|---|
| Acoustic | Genre — validates that acoustic clusters align with genre labels |
| Mood | Mood contours — contours will be tight and readable here |
| Tonal | Camelot — harmonic structure becomes explicit |
| Rhythmic | BPM gradient — confirms tempo bands |
| Semantic | User tags — see where your curated categories fall |

---

## Open Questions

1. **Animated transition feasibility** — smooth per-track position interpolation across 2000+ dots at 60fps is achievable on canvas, but needs benchmarking. May need to reduce dot count or skip animation for very large libraries.

2. **Semantic layout sentence model** — which model to use for embedding Qwen2 descriptions? `all-MiniLM-L6-v2` is 80 MB and fast; a larger model gives better embeddings but adds to the bundle size. Could reuse the CLAP text encoder if it produces compatible embeddings.

3. **Hybrid weight UX** — sliders are flexible but hard to reason about. An alternative: predefined blend presets with descriptive names ("Sound + Mood", "DJ Mix", "Harmonic") that set weights under the hood.

4. **PCA option** — currently PCA is offered alongside UMAP as a faster alternative. Should PCA be available as a projection option per layout, or dropped in favour of UMAP-only?

---

## Cross-References

- **Mood filtering / radar** (`mood_filtering_ideas.md`) — the mood layout is where contour overlays will look best, since input dimensions are clean 7-dim Essentia scores rather than opaque 512-dim embeddings. The radar target profile and the mood map overlay should update in sync.
- **Playlists** (`playlist_view_enhancements.md`) — rendering a playlist's tracks highlighted on the map reveals whether it is acoustically coherent (tight cluster) or diverse (scattered). Spatially adjacent unlabelled tracks become natural candidates to add to the playlist.
- **Compact embedding** (`private/acousticbrainz-exploration.md`) — a bottleneck network trained on CLAP + Essentia + Qwen2 data would be a superior input for the hybrid layout, learning the optimal feature weighting rather than relying on user-tuned sliders over concatenated raw spaces.
