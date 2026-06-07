---
status: proposed
owner: Roberto
last_verified: 2026-06-07
implemented_by:
superseded_by:
related_code:
related_skills:
---

# Music Map Improvements

## Current State

The core map quality improvements are shipped. Percentile-clipped normalization, silence detection, energy-based CLAP window selection, non-music filtering via Essentia `is_music`, and multiple projection modes are all implemented. What remains unimplemented: the outlier satellite / pinned HUD mini-map inset, algorithm parameter configuration UI, and topological labeling (DBSCAN + TF-IDF cluster labels).

| Area | Status | Evidence / Notes |
| :--- | :--- | :--- |
| Percentile-clipped normalization (§1) | Implemented | `standardize_to_100` in `commands/map.rs` uses p1–p99 clipping. |
| Soft-boundary outlier compression + micro-jitter | Need human review | The mechanism described in §1 may or may not be fully applied; verify against current `map.rs`. |
| Multiple projection algorithms with tunable parameters (§2) | Partially implemented | Multiple modes (`sonic`, `description`, `hybrid`, `essentia`, `harmonic`, `genre_wheel`) exist; algorithm parameter UI is not implemented. |
| Outlier satellite / Pinned HUD Mini-Map Inset (§3) | Not implemented | Deferred. |
| Non-music detection and map filtering (§4) | Implemented | Essentia `is_music` check filters non-music tracks from projection. |
| Silence detection pass (§5) | Implemented | `analysis/audio.rs` populates `silence_regions` and `has_long_silence`. |
| Energy-based CLAP window selection (§6) | Implemented | `select_clap_window_pcts` in `embeddings.rs` selects three loudest spaced bins. |
| Topological map labeling (§7) | Not implemented | DBSCAN + TF-IDF cluster labels not implemented. |

---

## Accepted Constraints

- Outlier tracks are not hidden — they remain accessible in all views and are only visually separated or clamped, never removed.
- Non-music tracks (Essentia `is_music = false`) are excluded from map projection but remain visible in table view and accessible via "Show non-music content" toggle.
- CLAP window selection must remain fully deterministic: same audio in → same windows out.
- Any algorithm change that alters embeddings requires bumping `pass_version::CLAP` and regenerating `track_coords`.

---

## Rejected Alternatives

- **Min/max normalization in `standardize_to_100`:** A handful of acoustically extreme tracks define the bounding box and compress 96%+ of the library into a small canvas region. Replaced by p1–p99 percentile clipping.
- **Fixed 25%/50%/75% CLAP window positions:** Blind to track content; places windows in silent intros/outros. Replaced by energy-based selection. (Full history in `doc/research/clap_window_selection.md`.)
- **LLM text-only non-music detection:** Slow and unreliable for filtering out non-music audio; Essentia `is_music` classifier is faster and more accurate.

---

## Implementation Plan

Remaining work:

1. **Verify soft-boundary squashing** — check whether the sigmoid/arctan compression described in §1 is actually applied in `map.rs` or if it is only hard-clamping. Implement if missing.
2. **Topological map labeling (§7)** — DBSCAN on 2D output coordinates, TF-IDF tag summarization per cluster, exemplar (medoid) selection, D3 force-directed label placement. This is the highest-impact remaining item for map usability.
3. **Outlier satellite region (§3)** — deferred until topological labeling is working; re-evaluate product priority at that time.
4. **Algorithm parameter UI (§2)** — settings panel for algorithm choice and hyperparameters; deferred until a concrete user need is identified.

---

## Validation Plan

- After verifying §1: confirm in SQLite that `is_map_outlier` flag is set for extreme tracks; main cluster fills canvas; outlier dot is still hoverable.
- After §7: cluster labels appear on map at appropriate zoom levels; force simulation prevents label collisions.

---

## Acceptance Criteria

- **User-visible:** Map no longer compresses the main library cluster; outlying tracks are soft-clamped to canvas edges rather than dominating the bounding box. Hovering over edge-stacked dots reveals distinct tracks via micro-jitter offsets.
- **User-visible (deferred):** A settings panel exposes algorithm choice (PCA/t-SNE/Diffusion/UMAP) and per-algorithm tunable parameters; outlier tracks appear in a pinned HUD mini-map inset that can be expanded or dismissed.
- **User-visible:** A "Show non-music content" sidebar toggle (off by default) hides audiobooks, podcasts, and jingles from the map; those tracks remain accessible in the table view.
- **User-visible:** CLAP embeddings are computed from the highest-energy non-silent windows, so similarity clusters reflect characteristic musical moments rather than silent intros/outros.
- **Data model:** `tracks` table gains `is_map_outlier BOOLEAN DEFAULT 0`, `is_non_music BOOLEAN DEFAULT 0`, `silence_regions TEXT`, and `has_long_silence BOOLEAN DEFAULT 0` columns via new migrations.
- **Data model (deferred):** Settings schema entries `map_algorithm`, `map_tsne_perplexity`, `map_tsne_epochs`, `map_tsne_theta`, `map_diffusion_steps`, `map_diffusion_epsilon`, and `map_normalization_percentile` are persisted in app config/DB.
- **IPC / frontend boundary:** `recompute_projection` command honours algorithm and parameter arguments that were previously silently ignored; `track_coords` or `tracks` rows include `is_map_outlier` and `is_non_music` flags consumed by the Svelte map component.
- **Analysis pipeline:** Silence-detection pre-pass runs before CLAP; energy-based window selection uses its output; re-analysis of existing tracks is triggered when pass versions change.
- **Tests:** Rust unit tests for `standardize_to_100` with p1/p99 clipping, soft-boundary squashing, and micro-jitter determinism; rule-based non-music classifier tested against known-good and known-bad fixtures; silence detection tested against synthetic waveforms with known silent segments.
- **Local verification:** After `recompute_projection`, confirm in SQLite that `is_map_outlier` is set for extreme tracks; visually verify main cluster fills canvas and outlier dot is still hoverable.
- **Theme / accessibility:** Satellite inset and outlier dots must be distinguishable without relying on color alone (dashed border or label); map dot tooltips remain keyboard-accessible.

---

## Historical / Research Notes

### 1. Fix Map Cramping: Percentile-Clipped Normalization

**Problem:** `standardize_to_100` in `commands/map.rs` maps the absolute min/max of UMAP output coordinates to `[0, 100]`. A handful of acoustically extreme tracks (early ragtime, heavy metal) sit far from the main cluster and define the bounding box, compressing 96%+ of the library into a small region of the canvas.

**Fix:** Replace min/max normalization with **p1–p99 percentile clipping**. Tracks outside the percentile range get clamped to 0 or 100 (they land at the canvas edge rather than disappearing). At p1, only ~70 tracks (3.7% of a 1,886-track library) are clamped — a negligible loss of precision for those extreme outliers.

### Soft Boundary Outlier Compression & Visual Micro-Jittering
Instead of hard clamping outlying coordinates (which forces multiple tracks to pile up directly on top of each other along the border of the canvas), the application uses **Soft Boundary Outlier Compression**:
- **Mathematical Squeezing**: A soft squashing function (e.g., Sigmoid or ArcTan) is applied to coordinates past the 99th and below the 1st percentiles. This compresses coordinates asymptotically near the boundaries (e.g. `[99, 100]`), preserving relative ordering and distance without allowing them to stretch the core canvas area.
- **Visual Micro-Jittering**: To prevent perfect visual overlap (dot collisions) for highly similar compressed outliers, a deterministic pseudo-random micro-jitter (using the track ID hash) is added to the rendering coordinates on the canvas. This guarantees that stacked tracks are offset by a tiny amount, allowing users to hover over and select individual overlapping dots.

```rust
// Current (problematic):
let x_min = coords.iter().map(|p| p.0).fold(f64::MAX, f64::min);
let x_max = coords.iter().map(|p| p.0).fold(f64::MIN, f64::max);

// Proposed: sort and index into the 1st / 99th percentile
let mut xs: Vec<f64> = coords.iter().map(|p| p.0).collect();
xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
let x_lo = xs[(n as f64 * 0.01) as usize];
let x_hi = xs[(n as f64 * 0.99) as usize];
// then clamp output to [0.0, 100.0]
```

---

### 2. Multiple Projection Algorithms with Tunable Parameters

**Problem:** `rag-umap 0.0.0` has no public configuration surface — all parameters (n_neighbors, min_dist, epochs) are hardcoded. The `_algorithm`, `_n_neighbors`, `_min_dist`, and `_perplexity` arguments accepted by `recompute_projection` are currently ignored.

**Proposed algorithm menu**, each available from a settings UI:

### A. PCA (`linfa-reduction`)
- Fast, deterministic, no warmup cost.
- Good global structure — proven by `tools/projection_comparison.png`.
- Parameters: none meaningful beyond the CLAP/description blend weight already exposed.
- Recommended as the **default algorithm**.

### B. t-SNE (`bhtsne` crate)
- Best for discovering tight local clusters.
- Barnes-Hut approximation makes it tractable for 2,000–10,000 tracks.
- Tunable parameters:
  - `perplexity` (default 30, range 5–100): controls neighborhood size. Higher = broader clusters.
  - `epochs` (default 1000, range 200–3000): more epochs = better convergence, slower.
  - `theta` (default 0.5, range 0.0–1.0): Barnes-Hut accuracy vs. speed tradeoff. 0 = exact.

### C. Diffusion Map (`linfa-reduction`)
- Good for smooth continuous manifolds (gradual BPM/key progressions).
- Tunable parameters:
  - `steps` (default 1, range 1–5): diffusion time. Higher = more global structure.
  - `epsilon` (default auto): kernel bandwidth. Controls neighborhood scale.

### D. UMAP (`rag-umap`, kept as legacy option)
- Current algorithm. Parameters not tunable due to crate limitations.
- Retained because it works adequately once normalization is fixed.
- Label it "UMAP (default params)" in the UI to set expectations.

**Settings schema additions** (stored in the app database or config):
```
map_algorithm: "pca" | "tsne" | "diffusion" | "umap"
map_tsne_perplexity: f64       // default 30
map_tsne_epochs: u32           // default 1000
map_tsne_theta: f64            // default 0.5
map_diffusion_steps: u32       // default 1
map_diffusion_epsilon: f64     // default 0 (auto)
map_clap_blend_weight: f64     // default 0.5 (already exists)
map_normalization_percentile: f64 // default 1.0 (p1/p99 clipping)
```

---

### 3. Outlier Handling: Satellite Regions

**Problem:** Acoustically extreme tracks (1920s ragtime, heavy metal) are correctly placed far from the main cluster by the projection algorithm. They are not wrong — they are genuinely distant in acoustic space. But their existence stretches the canvas and marginalizes the main library mass.

**Observation from data:** The outliers are not random — they form coherent sub-clusters among themselves (all the ragtime tracks cluster together; the metal tracks cluster together). A secondary projection of just the outliers is meaningful.

**Proposed approach: two-pass projection with satellite placement**

1. **Outlier detection:** For each track, compute its mean L2 distance to its k=5 nearest neighbors in the full embedding space. Tracks above the 95th percentile of this distribution are flagged as outliers. This threshold auto-scales to any library size and genre mix.

2. **Pass 1 (core tracks):** Run the selected projection algorithm on non-outlier tracks only. Normalize to `[10, 90] × [10, 90]`, leaving a 10-unit margin on all sides.

3. **Pass 2 (outlier tracks):** Run a separate projection on just the outlier tracks among themselves. Normalize to a small reserved region (e.g. `[0, 8] × [0, 8]`). This preserves the intra-outlier structure (ragtime tracks cluster together, metal tracks cluster separately).

4. **Visual treatment:** The satellite region gets a subtle visual separator — a faint dashed border or a slightly different background tint — and a label ("Acoustic Outliers" or similar). Dots within it are fully interactive: selectable, playable, included in pathfinding and similarity search.

### Pinned HUD Mini-Map Inset
To improve layout coherence, the outlier satellite region is designed as a **Pinned HUD Mini-Map Inset**:
- Rather than rendering inside the main infinite-canvas zoom/pan area, the satellite container is rendered in a fixed corner of the UI (e.g., bottom-right) as an overlay HUD element.
- This inset panel remains at 100% scale regardless of the user's primary map zoom or pan.
- A toggle allows the user to expand this inset into a full split-screen panel, or dismiss it. Selecting a track inside the mini-map draws a radial connector line linking it back to the core map viewport if relevant.

**Database change:** Add `is_map_outlier BOOLEAN DEFAULT 0` to the `tracks` table (or `track_coords`), populated during `recompute_projection`. The frontend uses this flag to apply the satellite visual treatment.

---

### 4. Non-Music Detection and Map Filtering

**Problem:** Some libraries contain non-music content (audiobooks, podcasts, field recordings, sound effects, short jingles) that pollutes the map projection. These tracks have no acoustic relationship to the rest of the library and distort the projection for everyone.

### Heuristic VAD & Music Classifier Filtering
Relying entirely on LLM text descriptions or tag regex patterns is slow and unreliable for filtering out non-music audio. Instead, the application leverages local, high-speed audio-based feature extraction:
- **Heuristic Vocal Activity Detection (VAD)**: A lightweight local VAD pass identifies long stretches of spoken speech (low pitch variance, regular conversational silence intervals).
- **Local Essentia Classifiers**: Local ONNX-based Essentia models analyze the spectral shape, zero-crossing rate, and onset density. If these profiles match known non-music signatures (e.g., `voice`, `speech`, or `noise`), the track is immediately flagged.
- This offloads speech/non-music identification to deterministic audio analysis, avoiding the need to run costly LLM text generation just to detect silent segments or narrated intros.

**Detection:** A simple rule-based classifier using existing signals — no new model needed:

| Signal | Non-music indicator |
|---|---|
| `silence_regions` | > 80% of track duration is detected silence |
| `detected_genre` (Essentia) | value contains `Speech` or `Non-music` |
| `bpm` | NULL and `duration_seconds` > 60 (long ambient / spoken) |
| Qwen description | contains "spoken word", "narration", "field recording", "sound effect" |
| `duration_seconds` | < 20 with no BPM (jingles, skips, interludes) |

A track scoring 2 or more of these signals is flagged `is_non_music = true`. These tracks are:
- Excluded from the map projection entirely.
- Still visible and playable in the table view.
- Filterable in the sidebar ("Show non-music content" toggle, off by default).

**Database change:** Add `is_non_music BOOLEAN DEFAULT 0` to the `tracks` table, computed during the analysis pipeline (after silence detection and Essentia/Qwen passes are available).

---

### 5. Silence Detection Pass

**Problem:** The current CLAP embedding pipeline samples windows at fixed 25%, 50%, 75% positions. If a track has a long silent intro or outro, one or more of these windows captures silence rather than music content, producing a contaminated embedding.

**New analysis pass: silence detection**

Runs as a lightweight pre-pass during track analysis (before CLAP), using only the decoded waveform:

1. Compute RMS energy over non-overlapping ~10ms chunks.
2. Flag any contiguous run below −60 dBFS lasting > 2 seconds as a silence region.
3. Store results in two new columns:
   - `silence_regions TEXT` — JSON array of `[start_sec, end_sec]` pairs, e.g. `[[0.0, 92.3], [245.1, 247.8]]`.
   - `has_long_silence BOOLEAN` — quick filter flag, true if any silence region > 10 seconds.

**Uses beyond CLAP:**
- The waveform renderer in the player bar can shade detected silence regions.
- The filter sidebar can expose a "Has long silence" toggle (useful for DJs skipping tracks with dead intros).
- Non-music detection (§4) uses `silence_regions` as one of its signals.

**Database change:** Migration adds `silence_regions TEXT` and `has_long_silence BOOLEAN DEFAULT 0` to the `tracks` table.

---

### 6. Energy-Based CLAP Window Selection

**Problem (continued from §5):** Fixed 25/50/75% window positions are naive. They are blind to the actual content of the track.

**Proposed replacement:** Select CLAP windows from the highest-energy non-silent segments of the waveform.

**Key insight:** The highest-energy segments of a track are structurally its most significant moments — the drop, the chorus, the peak build. By selecting windows there, we are not just avoiding silence: we are embedding the musically defining moments of the track. Two tracks that share a similar drop or chorus character will end up closer together on the map. This makes similarity search and map clustering reflect what listeners actually respond to, rather than whatever happens to fall at an arbitrary timestamp.

**Algorithm (fully deterministic — same audio in, same windows out):**

1. Use the silence regions from §5 to build a silence mask over the full waveform.
2. Compute RMS energy in 1-second blocks.
3. Zero out blocks that fall within a silence region.
4. Score each possible 10-second window by summing the RMS blocks it covers.
5. Greedily pick the 3 highest-scoring non-overlapping windows (minimum 10s gap between selected windows to ensure they cover different structural sections of the track).
6. Run CLAP inference on these 3 windows and mean-pool as today.

**Fallback:** If fewer than 3 non-silent 10-second windows exist (very short track or mostly silence), use as many as available and loop-pad the remainder as the current code does.

**Impact:** Tracks with long silent intros or outros will produce significantly better embeddings. Similarity search will match tracks by their characteristic sound at peak energy — their drop or chorus — rather than by the texture of their intro or outro. This change requires re-running the CLAP pass for all tracks in the library.

---

### 7. Topological Map Labeling & Exemplars (DBSCAN + TF-IDF)

**Problem:** Currently, the map relies purely on color codes and a side legend to communicate region characteristics. This requires constant cognitive context-switching for the user to understand what "acoustic zone" they are looking at.

**Proposed Solution:** Automatically compute and display regional text labels and representative "exemplar" tracks directly on the 2D music map canvas.

### A. Coordinate-Based Spatial Region Discovery
To identify physical regions directly as they are currently projected on the screen:
- Run a 2D spatial density clustering algorithm (such as **DBSCAN** or **HDBSCAN**) on the output coordinates $(x,y)$.
- This groups adjacent tracks into spatial clusters and isolates outliers. Because it clusters the *coordinates*, the regions automatically adapt when switching layouts (e.g. grouping by harmonics in Tonal mode vs. mood states in Mood mode).

### B. Regional Tag Summarization (TF-IDF)
For each spatial cluster $C$, we mathematically determine its characteristic descriptors:
- Run a **TF-IDF (Term Frequency-Inverse Document Frequency)** extraction over all metadata categories (genres, instruments, Qwen descriptors) belonging to the tracks in Cluster $C$.
- Term Frequency values are normalized against the entire library's Inverse Document Frequency. The top 2–3 highest scoring tags define the region (e.g. `"Synthesizer / Gritty / Bass"` or `"Acoustic / Melancholy / Guitar"`).

### C. Exemplar (Medoid) Selection
- **The Calculation**: Compute the geometric centroid of each spatial cluster $C$. The track in that cluster whose coordinate $(x,y)$ is closest to the centroid is selected as the **Exemplar** for the region.
- **The UI**: Display the Exemplar track's title/artist inline below the region label (e.g. *Exemplar: Massive Attack - Teardrop*). Hovering over a regional label highlights the exemplar track dot.

### D. Dynamic Placement and Level of Detail (LoD)
- **Collision Avoidance**: Run a lightweight force-directed simulation (`d3.forceCollide`) on the labels in Svelte to prevent overlapping text boxes from cluttering the canvas.
- **Zoom-Dependent LoD**:
  - *Zoomed Out*: Show a few coarse, high-level cluster labels (e.g., `"Electronic Vibe"`, `"Rock / Metal"`).
  - *Zoomed In*: High-level clusters split, fading in sub-cluster labels (e.g., `"Synthwave"`, `"Industrial"`, `"Hard Rock"`).

---

*(Implementation status is tracked in the Current State table at the top of this document.)*
