# Technical Feasibility & Performance Analysis

This document provides a comparative analysis of the 5 proposed features we brainstormed. Each analysis details the implementation complexity, database changes required, and user interface responsiveness guidelines.

---

## Current Implementation Snapshot

This document is a feasibility brainstorm, not a commitment list. Several items were implemented differently from the original sketch, so use this table before treating an older section as current design.

| Feature Area | Current Status | Evidence / Notes |
| :--- | :--- | :--- |
| Two-Stage Structural Search | Partially implemented | SAX extraction, alignment, alignment segments, structure clusters, and structure filtering exist. The specific sqlite-vec waveform table and full block-query UI are still research/proposal material. |
| Spectral Map Projections | Partially implemented | Multiple dynamic projection modes are available from the frontend and backend. The proposed persisted multi-layout schema, staleness tracking, and parameter serialization are not implemented. |
| Mastering Style Auto-Tagging | Implemented | Implemented in the audio analysis pass and stored as `mastering:*` tags rather than a dedicated column. |
| Playlist Drag-to-Reorder & Visualizer UI | Partially implemented | Playlist schema, commands, position storage, and saved-search naming exist. Dedicated drag/drop polish and transition visualizer ideas still need review. |
| Vibe-Based Playlist Recommendations | Need human review | Embedding sources exist, but no dedicated recommendations workflow was found. Re-evaluate product fit before implementation. |

---

## 📊 Summary Comparison Matrix

| Feature Area | Complexity | DB Changes? | UI Responsiveness (60fps?) | Sizing (Est.) | Status |
| :--- | :---: | :---: | :--- | :---: | :---: |
| **1. Two-Stage Structural Search** | **Medium** | Yes (migration) | High (Stage 1 SQLite <1ms, Stage 2 DTW ~1ms) | 3.5 Days | Not started |
| **2. Spectral Map Projections** | **High** | Yes (migration) | High (Offloaded to Rust thread pool; Svelte animations 60fps) | 7.0 Days | Not started |
| **3. Mastering Style Auto-Tagging** | **Medium** | Yes (migration) | High (Background calculations; Svelte displays statically) | 3.0 Days | ✅ Done |
| **4. Playlist Drag-to-Reorder & Visualizer UI** | **Medium-High** | No (schema ready) | High (Optimistic UI state; Svelte `animate:flip`) | 4.0 Days | Not started |
| **5. Vibe-Based Playlist Recommendations** | **Medium** | No (schema ready) | High (Rust background thread vectors distance check <2ms) | 2.5 Days | Not started |

---

## 🔍 Detailed Feature Breakdowns

### 1. Two-Stage Structural Similarity Search (sqlite-vec + DTW)
* **Complexity**: **Medium**. The 128-point envelope is already extracted in the `audio_analysis` pass, so implementing this requires L2-normalization of these envelopes, populating a new `vec0` table, and writing the DTW alignment logic in Rust (which is lightweight and straightforward, $O(N \cdot M)$).
* **Database Requirements**: **Migration Required**. Create a new virtual table `track_waveform_embeddings` `using vec0(embedding float[128])` to index envelopes. Add triggers or cascade statements to handle track deletion.
* **UI Interactivity & Responsiveness**: **High**. Tauri commands run asynchronously, and the calculations (Stage 1 SQLite query < 1ms, Stage 2 Rust DTW ~1-2ms) run in under 5ms combined. The Svelte 5 main thread remains unblocked and can render at 60fps.

### 2. Spectral Map Projections (Laplacian Eigenmaps)
* **Complexity**: **High**. Extracting eigenvectors of the graph Laplacian $L = D - S$ has an $O(N^3)$ computational bottleneck. Dense eigensolvers in `faer` scale poorly beyond $N \approx 2000$. A sparse iterative solver (e.g., Lanczos) combined with $k$-NN sparse graphs is required for scalability.
* **Database Requirements**: **Migration Required**. The current `track_coords` table still uses `track_id` as primary key, supporting only a single cached layout. A new schema with a composite primary key `(track_id, layout_name)` is required to cache multiple coordinate layouts simultaneously.
* **UI Interactivity & Responsiveness**: **High / Offloaded**. Position interpolation/animation runs at 60 FPS on the Svelte main thread (driven by canvas and D3). The eigensolver calculation is CPU-bound and must be executed asynchronously on a background thread pool (via `tokio::task::spawn_blocking` or `rayon`), saving results to the DB and notifying the frontend upon completion.

### 3. Dynamic Range & Mastering Style Auto-Tagging ✅
* **Status**: Implemented in `src-tauri/src/analysis/audio.rs` as part of the `audio_analysis` pass.
* **Implementation**: Simpler than originally scoped — no separate pass or `mastering_style` column was needed. The rule-based classification runs inline inside `audio_analysis` immediately after loudness values are computed, writing to the `mastering` tag namespace via `upsert_track_tag`. Two tags are currently defined:
  - `mastering:brickwalled` — LUFS > −7.0 and loudness range < 4.0 LU
  - `mastering:dynamic` — loudness range > 8.0 LU
* **Database Requirements**: No separate migration for a column. The `track_tags` `score` and `discard` columns (migrations 21–22) are the only schema additions from this era.
* **UI Interactivity & Responsiveness**: High, as predicted. Tags appear in `TrackDetailPane` alongside all other tags with no special handling.

### 4. Playlist Drag-to-Reorder & Visualizer UI
* **Complexity**: **Medium-High**. Drag-to-reorder requires integrating Svelte 5 runes with native drag/drop APIs across paginated lists. The visualizer requires calculating relative BPM changes and Camelot wheel steps, rendered as connecting SVG/HTML widgets between playlist tracks.
* **Database Requirements**: **None**. The database schema is already updated via Migration 18 (`playlists` and `playlist_tracks` tables are present with `position` columns).
* **UI Interactivity & Responsiveness**: **High**. Svelte `animate:flip` handles the coordinate morphs smoothly. The drag-and-drop system should implement optimistic UI updates so the list reorders instantly, before sending the async SQLite reordering transaction (which takes 2–10ms) to Tauri.

### 5. Vibe-Based Playlist Recommendations
* **Complexity**: **Medium**. The underlying 512-d CLAP audio embeddings and Essentia classifier columns are already stored in the DB, so no new models are required for the core similarity search.
* **Database Requirements**: **None**. The schema already supports playlists, playlist tracks, and high-dimensional embeddings with foreign keys and indexes.
* **Embedding sources**: Two embedding spaces are available:
  - `audio_embeddings` — 512-d CLAP audio embeddings, one per track
  - `description_embeddings` — 384-d MiniLM (all-MiniLM-L6-v2) text embeddings of the Qwen-generated description
* **UI Interactivity & Responsiveness**: **High**. By offloading the centroid matching calculations to a Tauri Rust command (which executes on a background thread pool), the UI will easily maintain 60fps on the main thread. A distance check against 10,000 tracks completes in under 2ms in Rust.
