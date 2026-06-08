# Preparation Plan: Music Map Improvements

This document outlines the preparation and step-by-step implementation plan for the **Music Map Improvements** feature (DBSCAN + TF-IDF topological labeling, Pinned HUD Mini-map inset, soft-boundary outlier compression, and visual micro-jittering).

---

## 1. Goal & Requirements

The music map visualizes the audio collection in a 2D space. To elevate its usability, this feature implements:
1. **Soft Boundary Outlier Compression & Visual Micro-Jittering**: Replaces hard clamping of outlier coordinates with asymptotic squeezing (using `tanh` or `arctan` compression) and deterministic pseudo-random micro-jittering based on the track ID hash to prevent dot collisions.
2. **Coordinate-Based Spatial Region Discovery (DBSCAN)**: Runs a 2D density-based clustering algorithm on the projected coordinate space to discover natural topological regions.
3. **Regional Tag Summarization (TF-IDF)**: Extracts the most representative metadata tags for each spatial region by calculating term frequency-inverse document frequency (TF-IDF) scores.
4. **Exemplar (Medoid) Selection**: Computes the geometric centroid of each spatial cluster and identifies the closest track to serve as the exemplar/representative track.
5. **Collision Avoidance and Zoom-Dependent Level of Detail (LoD)**: Employs a lightweight D3 force-directed collision simulation to prevent label overlaps. Implements high-level coarse labels at low zoom levels ($k < 2.0$) and detailed sub-cluster labels at high zoom levels ($k \ge 2.0$).
6. **Outlier Satellite & Pinned HUD Mini-Map Inset**: Identifies global outliers in the embedding space ($k=5$ nearest neighbors mean L2 distance $> 95\text{th}$ percentile), projects them separately, and renders them in a fixed-corner mini-map overlay HUD.

---

## 2. Semantic Hit Rate

We ran semantic queries using the knowledge manager CLI `knowledge_mgr.py query` to locate relevant files and anchor points.

### Queries and Results
1. **Query**: `"DBSCAN"`
   - **Similarity**: 0.38
   - **Match**: `src-tauri/src/analysis/structure_cluster.rs` (Section: DBSCAN structure clustering)
   - **Insight**: This contains an existing string-similarity based DBSCAN implementation. We can reuse or adapt the clustering pattern for 2D spatial points.

2. **Query**: `"projection coordinates"`
   - **Similarity**: 0.34
   - **Match**: `doc/proposals/map_layouts.md` (Section: Computation Strategy)
   - **Insight**: Details on coordinate systems and background caching. Anchors `get_projection_coordinates` and `recompute_projection` commands.

3. **Query**: `"recompute_projection"`
   - **Similarity**: 0.34
   - **Match**: `src-tauri/src/commands/map.rs` (Section: coordinate projection commands)
   - **Insight**: Confirms that coordinate projection calculations are stored in the database's `track_coords` table on demand and read dynamically.

### Affected Files and Anchors
- **Backend Schema**: `src-tauri/src/database.rs` and a new migration file under `src-tauri/migrations/`.
- **Backend Map Logic**: `src-tauri/src/commands/map.rs` where projection, KNN computation, coordinates standardization, and coordinate queries are defined.
- **Frontend Map Layout**: `src/lib/components/MusicMap.svelte` for the main canvas rendering and toolbar settings.
- **Frontend Types**: `src/lib/utils/mapMath.ts` and `src/lib/types.ts`.

---

## 3. Impact Assessment

### Database / Schema Changes
- **Migration File**: Create `src-tauri/migrations/33_music_map_improvements.sql` to add:
  - `is_map_outlier INTEGER NOT NULL DEFAULT 0` to the `tracks` table.
  - `is_non_music INTEGER NOT NULL DEFAULT 0` to the `tracks` table.
- **Struct Mapping**: Update `src-tauri/src/database.rs`:
  - Add fields `is_map_outlier: i64` and `is_non_music: i64` to the `Track` struct.
  - Add mapping logic in the `db_row_mapping!` macro.

### Rust Backend
- **Outlier Analysis**:
  - In `src-tauri/src/commands/map.rs`, write a k-NN distance calculator. Before running UMAP/PCA, compute the average L2 distance of each track to its $k=5$ nearest neighbors.
  - Flag tracks above the 95th percentile with `is_map_outlier = 1`.
  - Pass 1: Project core tracks (`is_map_outlier = 0`) into the standard region, then standardize to `[10, 90] × [10, 90]`.
  - Pass 2: Project outlier tracks (`is_map_outlier = 1`) separately, then map to satellite coordinates `[0, 8] × [0, 8]`.
- **Soft Standardization & Micro-Jittering**:
  - Modify `standardize_to_100` in `commands/map.rs` to take `track_ids: &[i64]`.
  - Apply `tanh` or `arctan` to values falling outside the `p1` and `p99` percentiles to map them into `[0, 5]` and `[95, 100]` ranges.
  - Apply a deterministic micro-jitter (up to 0.25 coordinate units) to squashed tracks using a fast hash function of the `track_id`.
- **Topological Labels Command**:
  - Add a new Tauri command:
    ```rust
    #[tauri::command]
    pub fn get_map_labels(
        music_only: bool,
        conn_state: tauri::State<'_, Mutex<Connection>>,
    ) -> Result<Vec<MapLabel>, String>
    ```
  - Inside `get_map_labels`:
    1. Retrieve active `(x, y)` coordinates and track metadata from `track_coords` and `tracks`.
    2. Run 2D DBSCAN on coordinates at two parameter settings: Coarse (`eps = 12.0`, `min_samples = 15`) and Fine (`eps = 6.0`, `min_samples = 8`).
    3. For each cluster, fetch all associated tags from the `track_tags` and `tags` tables.
    4. Compute TF-IDF for each tag in the cluster:
       - $TF = \text{tracks\_in\_cluster\_with\_tag} / \text{total\_tracks\_in\_cluster}$
       - $IDF = \ln(\text{total\_library\_tracks} / \text{total\_tracks\_with\_tag})$
       - Select top 2-3 tags to form a human-readable phrase.
    5. Find the centroid of each cluster, then select the medoid track as the exemplar.
    6. Return a list of `MapLabel` structures.

### Frontend Svelte 5 / TS
- **Type Definitions**:
  - Update `MappedTrackPoint` in `mapMath.ts` to include `is_map_outlier?: number` and `is_non_music?: number`.
  - Add a `MapLabel` interface:
    ```typescript
    export interface MapLabel {
      id: number;
      x: number;
      y: number;
      text: string;
      exemplar_id: number;
      exemplar_name: string;
      level: 'coarse' | 'fine';
    }
    ```
- **Label Rendering & Force Collision**:
  - In `MusicMap.svelte`, load map labels from the backend.
  - Implement a lightweight D3 force simulation (`d3.forceSimulation`) using `d3.forceCollide` on the labels after scaling them into screen coordinates.
  - Render labels directly on the canvas screen context (so text size remains constant and readable) or as absolute-positioned overlay elements.
  - Apply Zoom-dependent LoD: Coarse labels visible when `transform.k < 2.0`, Fine labels visible when `transform.k >= 2.0`.
- **Mini-Map HUD Inset**:
  - Add a small overlay HUD box in the bottom-right corner of the canvas.
  - Filter `visibleTracks` to select those with `is_map_outlier === 1`.
  - Draw outlier track dots in the HUD mini-map box.
  - Add expand/collapse capabilities to the HUD mini-map panel.

---

## 4. Implementation Checklist

### Phase 1: Database Migration & Schema Extensions
- [ ] Create database migration `src-tauri/migrations/33_music_map_improvements.sql` adding `is_map_outlier` and `is_non_music` columns.
- [ ] Register the migration in `get_migrations()` inside `src-tauri/src/database.rs`.
- [ ] Add `is_map_outlier` and `is_non_music` to the `Track` struct and row mapping macros.
- [ ] Compile and run `cargo test` to verify migration compatibility.

### Phase 2: Outlier Split & Coordinate Compression
- [ ] Write the $k=5$ L2 distance neighbor calculation in `commands/map.rs`.
- [ ] Separate tracks into core and outlier groups in `recompute_projection`.
- [ ] Run PCA/UMAP separately for the core and outlier subsets.
- [ ] Implement `tanh`/`arctan` based soft outlier squeezing inside `standardize_to_100`.
- [ ] Implement deterministic track ID hash micro-jittering.
- [ ] Write unit tests verifying that outliers map to `[0, 5]` and `[95, 100]` with micro-jittering and no overlap.

### Phase 3: Spatial Labels & TF-IDF Generation
- [ ] Implement the `dbscan_2d` algorithm in `commands/map.rs`.
- [ ] Implement the TF-IDF tag ranker and select characteristic names.
- [ ] Implement geometric medoid/exemplar selection.
- [ ] Create the `get_map_labels` Tauri IPC command and register it in `src-tauri/src/lib.rs`.
- [ ] Write backend unit tests for 2D DBSCAN, TF-IDF calculation, and medoid math.

### Phase 4: Svelte Canvas & Layout Upgrades
- [ ] Update `mapMath.ts` and `types.ts` TS models.
- [ ] Write Svelte logic to invoke `get_map_labels` and apply a D3 force-directed collision simulation.
- [ ] Render regional labels on the map screen space.
- [ ] Implement zoom threshold logic to toggle Coarse vs. Fine labels.
- [ ] Draw the outlier satellite HUD overlay in the bottom-right corner of `MusicMap.svelte`.
- [ ] Implement expanding and collapsing behavior for the HUD.
