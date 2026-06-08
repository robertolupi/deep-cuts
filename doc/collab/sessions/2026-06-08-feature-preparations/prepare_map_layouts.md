# Preparation Plan: Map Layouts & Spectral Map Projections

This document outlines the preparation and step-by-step implementation plan for the **Map Layouts & Spectral Map Projections** feature (persistent layout catalog, caching, idle-time background precomputation, and coordinate stability mechanisms).

---

## 1. Goal & Requirements

The music map visualizes an audio collection in 2D space. The current implementation computes layouts on demand and overwrites a single table (`track_coords`) on every change, which is slow (especially for UMAP) and disorienting due to stochastic coordinate shifting. 

To improve layout switching speeds and coordinate consistency, this feature implements:
1. **Multiple Persisted Named Layouts**: A persistent layout catalog (`map_layouts` and `map_coordinates` tables) caching coordinates for six distinct perspectives:
   - **Sonic Similarity**: 512-dim CLAP audio embeddings projected via UMAP/PCA.
   - **Semantic / Vibe**: 384-dim Sentence embeddings (MiniLM) of Qwen2 description text.
   - **Mood Profile**: 7-dim Essentia mood vector.
   - **Rhythmic**: BPM + Essentia rhythm descriptors (danceability, beat strength, onset rate).
   - **Tonal / Harmonic**: Key (angle on the Circle of Fifths) + Scale (Major/Minor) + key strength.
   - **Hybrid**: User-weighted combinations of the above spaces (e.g. 50% Acoustic + 50% Mood).
2. **On-Demand Caching & Staleness Logic**: Layouts are queried from the cache instantly (under 5ms). If a layout is stale or missing, Svelte displays a loading spinner while the backend computes and caches it. Layouts are invalidated (marked `is_stale = 1`) when new tracks are added or their embeddings are updated.
3. **Idle-Time Precomputation Queue**: A low-priority background queue checks for stale/uncomputed layouts and calculates their coordinates asynchronously when the CPU is idle (i.e. no active imports or user analysis passes running), preventing blocking.
4. **Coordinate Stability (KNN Regressor & Procrustes)**:
   - **Incremental Additions (Out-of-Sample projection)**: When new tracks are added, instead of recalculating the entire layout, a distance-weighted $K$-Nearest Neighbors (KNN) regressor projects them onto the existing stable 2D canvas based on high-dimensional distances.
   - **Global Realignment (Procrustes Alignment)**: When a full recompute is manually triggered or the library grows past a threshold (e.g. >20% new unaligned tracks), a full UMAP is run. Then, a 2D Orthogonal Procrustes transformation is computed using SVD to rotate, scale, and translate the new coordinates to align with the old coordinates, preventing disorienting flips or rotations.
5. **UI & Transitions**:
   - Replace the PCA/UMAP toggle with a **Layout** dropdown.
   - Implement smooth coordinate transitions (x, y interpolation using D3 transitions) over 800ms.
   - Add a hybrid weight slider/preset panel under the toolbar.

---

## 2. Semantic Hit Rate

We ran semantic queries using the knowledge manager CLI (`knowledge_mgr.py query`) to locate relevant files and anchor points.

### Queries and Results
1. **Query**: `"Map Layouts"`
   - **Similarity**: 0.6255
   - **Match**: `doc/proposals/map_layouts.md` (Motivation, Current State, Computation Strategy)
   - **Insight**: Anchors the requirements for this plan. Confirms switchable projection modes are implemented in the UI but the persistence database schema, cache catalog, and stability logic are missing.
2. **Query**: `"projection modes"`
   - **Similarity**: 0.4149
   - **Match**: `doc/proposals/map_layouts.md` (Current State, Status, UI dropdown mockups)
   - **Insight**: Points directly to Svelte frontend (`MusicMap.svelte`) and Rust backend command handlers (`src-tauri/src/commands/map.rs`).
3. **Query**: `"run_sentence_embed"`
   - **Similarity**: 0.2974
   - **Match**: `src-tauri/src/embeddings.rs`
   - **Insight**: Confirms the existence of `all-MiniLM-L6-v2` ONNX sentence embedding inference, which is ready to convert Qwen2-Audio textual descriptions into vectors for the Semantic/Vibe layout.

### Affected Files and Anchors
- **Database Migrations**: `src-tauri/migrations/` (new migration file `33_map_layouts.sql`).
- **Database Schema**: `src-tauri/src/database.rs` to register the new tables and run schema verification tests.
- **Analysis Hooks**: `src-tauri/src/analysis/clap.rs` (ClapPass) and description embeddings to mark cached layouts stale when embeddings update.
- **Backend Map Logic**: `src-tauri/src/commands/map.rs` containing `get_projection_coordinates`, `recompute_projection`, and layout calculations.
- **Frontend Svelte View**: `src/lib/components/MusicMap.svelte` for rendering, layout switching, and interpolation.

---

## 3. Impact Assessment

### Database / Schema Changes
- **New Migration**: Create `src-tauri/migrations/33_map_layouts.sql`:
  ```sql
  CREATE TABLE map_layouts (
      slug TEXT PRIMARY KEY,
      name TEXT NOT NULL,
      config_json TEXT NOT NULL,
      is_stale INTEGER NOT NULL DEFAULT 1,
      updated_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
  );

  CREATE TABLE map_coordinates (
      layout_slug TEXT NOT NULL,
      track_id INTEGER NOT NULL,
      x REAL NOT NULL,
      y REAL NOT NULL,
      music_only INTEGER NOT NULL DEFAULT 0,
      PRIMARY KEY(layout_slug, track_id, music_only),
      FOREIGN KEY(track_id) REFERENCES tracks(id) ON DELETE CASCADE,
      FOREIGN KEY(layout_slug) REFERENCES map_layouts(slug) ON DELETE CASCADE
  );

  -- Seed default layout entries
  INSERT INTO map_layouts (slug, name, config_json, is_stale) VALUES
  ('sonic', 'Sonic Similarity', '{"clap_weight":1.0,"algorithm":"pca"}', 1),
  ('description', 'Semantic Vibe', '{"clap_weight":0.0,"algorithm":"pca"}', 1),
  ('hybrid', 'Hybrid (Sonic + Semantic)', '{"clap_weight":0.5,"algorithm":"pca"}', 1),
  ('essentia', 'Mood Map', '{"algorithm":"spring"}', 1),
  ('harmonic', 'Harmonic Circle', '{"algorithm":"spring"}', 1),
  ('genre_wheel', 'Genre Wheel', '{"algorithm":"spring"}', 1);
  ```
- **Drop Table**: In the same migration, run `DROP TABLE IF EXISTS track_coords;` to cleanly remove the legacy flat coordinates table.
- **Tests Updating**: In `src-tauri/src/database.rs`, remove `"track_coords"` from the expected tables check, and add `"map_layouts"` and `"map_coordinates"`.

### Rust Backend
- **Tauri Commands Modification**:
  - `get_projection_coordinates(layout_slug: String, music_only: bool)`: Queries `map_coordinates` joined with `tracks` for the specified `layout_slug`. If coordinates do not exist, returns an empty list (prompting Svelte to trigger recomputation) or returns coordinates with an `is_stale` flag.
  - `recompute_projection(...)`: Accepts the active `layout_slug` and configuration overrides. It will:
    1. Fetch any cached coordinates for this layout to act as "anchor" tracks.
    2. Determine whether to do a **KNN Regressor Out-of-Sample projection** (for newly added tracks when anchors exist) or a **Global Recompute** (when manual recompute is pressed or new tracks exceed 20% of anchors).
    3. If global recompute runs and anchors exist, apply the **Procrustes Alignment** function using SVD to align the new coordinate matrix to the anchor coordinate matrix.
    4. Save the coordinates into `map_coordinates` and set `is_stale = 0` in `map_layouts`.
- **Coordinate Stability Algorithms**:
  - **Procrustes Alignment**:
    - Center both datasets: $X_c = X - \mu_X$, $Y_c = Y - \mu_Y$.
    - Scale normalize to unit variance: $X_{\text{norm}} = X_c / s_X$, $Y_{\text{norm}} = Y_c / s_Y$.
    - Compute covariance: $A = X_{\text{norm}}^T Y_{\text{norm}}$.
    - SVD of covariance: $A = U \Sigma V^T$.
    - Rotation: $R = V U^T$. If $\det(R) < 0$, adjust $V$'s last column and recompute.
    - Transform new points: $Y_{\text{aligned}} = s_X (Y_{\text{norm}} R) + \mu_X$.
  - **Distance-Weighted KNN Regressor**:
    - Find the $K=5$ nearest anchor tracks by embedding distance.
    - Interpolate coordinates using weights $w_i = 1 / (\text{distance}_i + \epsilon)$.
- **Hooks & Invalidation**:
  - When the CLAP or Qwen2 sentence embedding pass writes results to the DB, run `UPDATE map_layouts SET is_stale = 1 WHERE slug IN (...)` for dependent layouts.
- **Background Precomputation Task**:
  - In `lib.rs`, spawn a low-priority tokio thread that runs every 30 seconds to check for layouts where `is_stale = 1`. If no analysis or import job is in progress, run the coordinate recomputation in the background, update the database, and emit the `"projection-updated"` event when completed.

### Frontend Svelte 5 / TS
- **Layout Switcher UI**:
  - Replace the PCA/UMAP toggle button group in `MusicMap.svelte` with a Svelte Dropdown component containing options: `Sonic Similarity`, `Semantic Vibe`, `Mood Map`, `Harmonic Circle`, `Genre Wheel`, and `Hybrid`.
  - When a layout is selected, check if it has cached coordinates. If missing or stale, show the Svelte load spinner and invoke `recompute_projection` or allow the background thread to finish.
- **Smooth Transitions**:
  - In `MusicMap.svelte`, track coordinates using a Svelte `mapCoordinates` store or Rune.
  - When coordinates update, use a D3 transition on the canvas context over 800ms. Animate the points by interpolating each track's `(x, y)` coordinate between the old and new coordinates (e.g. using `d3.interpolateNumber`).
- **Hybrid Weight Panel**:
  - Render an expandable panel under the toolbar when `Hybrid` is active, with sliders for Acoustic, Semantic, and Mood parameters, and preset selectors.

---

## 4. Implementation Checklist

### Phase 1: Database Migration & Schema Extensions
- [ ] Create database migration `src-tauri/migrations/33_map_layouts.sql` defining `map_layouts` and `map_coordinates` tables, dropping `track_coords`, and seeding default layouts.
- [ ] Register the migration in `get_migrations()` inside `src-tauri/src/database.rs`.
- [ ] Remove `"track_coords"` and add `"map_layouts"` and `"map_coordinates"` to expected tables validation checks in `database.rs` tests.
- [ ] Compile and run `cargo test` to verify database boot and migrations.

### Phase 2: Coordinate Stability Algorithms in Rust
- [ ] Implement the `orthogonal_procrustes` function in `commands/map.rs` using the `thin_svd()` method from `faer`.
- [ ] Implement `knn_regressor_project` to interpolate 2D coordinates for new tracks based on the closest 5 anchor tracks.
- [ ] Write Rust unit tests in `map.rs` verifying:
  - Procrustes rotation, translation, and scaling alignment on a simulated 2D coordinate dataset.
  - KNN regressor distance-weighted coordinate calculation accuracy.
- [ ] Run `cargo test --lib` to ensure correctness.

### Phase 3: Layout Caching, Invalidation, and Background Tasks
- [ ] Rewrite `get_projection_coordinates` command to accept `layout_slug` and load from `map_coordinates`.
- [ ] Rewrite `recompute_projection` to support checking coordinates cache, out-of-sample KNN projections for minor track updates, and Procrustes-aligned global recomputation.
- [ ] Add staleness update triggers in CLAP and Description analysis passes (`save_result` hooks).
- [ ] Implement the idle background worker thread in `lib.rs` checking database staleness, and emitting `"projection-updated"` on completion.
- [ ] Run tests to verify coordinate persistence and cache hits.

### Phase 4: Svelte Canvas UI & Smooth Transitions
- [ ] Replace PCA/UMAP buttons with Svelte Layout selection dropdown.
- [ ] Implement smooth `(x, y)` transition logic in Svelte using `d3.timer` or coordinate interpolation.
- [ ] Build the Hybrid weight preset panel expander inside the toolbar.
- [ ] Manually test switching between layouts and observe dot transitions.
