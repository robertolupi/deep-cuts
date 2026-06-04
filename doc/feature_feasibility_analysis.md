# Technical Feasibility & Performance Analysis

This document provides a comparative analysis of the 7 proposed features we brainstormed. Each analysis details the implementation complexity, database changes required, and user interface responsiveness guidelines.

---

## 📊 Summary Comparison Matrix

| Feature Area | Complexity | DB Changes? | UI Responsiveness (60fps?) | Sizing (Est.) |
| :--- | :---: | :---: | :--- | :---: |
| **1. Two-Stage Structural Search** | **Medium** | Yes (migration) | High (Stage 1 SQLite <1ms, Stage 2 DTW ~1ms) | 3.5 Days |
| **2. Spectral Map Projections** | **High** | Yes (migration) | High (Offloaded to Rust thread pool; Svelte animations 60fps) | 7.0 Days |
| **3. Tracks with Long Silences Detector** | **Low** | No (schema ready) | High (Reactive filtering on client-side store) | 1.0 Day |
| **4. Mastering Style Auto-Tagging** | **Medium** | Yes (migration) | High (Background calculations; Svelte displays statically) | 3.0 Days |
| **5. Playlist Drag-to-Reorder & Visualizer UI** | **Medium-High** | No (schema ready) | High (Optimistic UI state; Svelte `animate:flip`) | 4.0 Days |
| **6. Saved Search Smart Auto-Naming** | **Low** | No (schema ready) | High (Derived Svelte 5 state is instant <0.1ms) | 1.5 Days |
| **7. Vibe-Based Playlist Recommendations** | **Medium** | No (schema ready) | High (Rust background thread vectors distance check <2ms) | 2.5 Days |

---

## 🔍 Detailed Feature Breakdowns

### 1. Two-Stage Structural Similarity Search (sqlite-vec + DTW)
* **Complexity**: **Medium**. The 128-point envelope is already extracted in the `audio_analysis` pass, so implementing this requires L2-normalization of these envelopes, populating a new `vec0` table, and writing the DTW alignment logic in Rust (which is lightweight and straightforward, $O(N \cdot M)$).
* **Database Requirements**: **Migration Required**. Create a new virtual table `track_waveform_embeddings` `using vec0(embedding float[128])` to index envelopes. Add triggers or cascade statements to handle track deletion.
* **UI Interactivity & Responsiveness**: **High**. Tauri commands run asynchronously, and the calculations (Stage 1 SQLite query < 1ms, Stage 2 Rust DTW ~1-2ms) run in under 5ms combined. The Svelte 5 main thread remains unblocked and can render at 60fps.

### 2. Spectral Map Projections (Laplacian Eigenmaps)
* **Complexity**: **High**. Extracting eigenvectors of the graph Laplacian $L = D - S$ has an $O(N^3)$ computational bottleneck. Dense eigensolvers in `faer` scale poorly beyond $N \approx 2000$. A sparse iterative solver (e.g., Lanczos) combined with $k$-NN sparse graphs is required for scalability.
* **Database Requirements**: **Migration Required**. The current `track_coords` table only supports a single cached layout because `track_id` is the primary key. We need a new table or an alteration to support a composite primary key `(track_id, layout_name)` to cache multiple coordinate layouts simultaneously.
* **UI Interactivity & Responsiveness**: **High / Offloaded**. Position interpolation/animation runs at 60 FPS on the Svelte main thread (driven by canvas and D3). The eigensolver calculation is CPU-bound and must be executed asynchronously on a background thread pool (via `tokio::task::spawn_blocking` or `rayon`), saving results to the DB and notifying the frontend upon completion.

### 3. Tracks with Long Silences Detector
* **Complexity**: **Low**. The Rust backend already implements silence detection during the audio-analysis pass and populates the necessary columns. We only need to expose Svelte 5 reactive filtering and UI chips.
* **Database Requirements**: **None**. The database schema is already updated via Migration 12 (`silence_regions` and `has_long_silence` fields are already present).
* **UI Interactivity & Responsiveness**: **High**. Since the analysis runs asynchronously in background threads, UI updates are simple client-side reactive array filters that run in sub-milliseconds.

### 4. Dynamic Range & Mastering Style Auto-Tagging
* **Complexity**: **Medium**. The DSP components (`loudness_lufs`, `loudness_range`, and 128-point envelope `waveform_data`) are already calculated in the existing `audio_analysis` pass in Rust. The remaining work involves creating a lightweight analysis pass (`mastering_analysis`) to run a rule-based classification algorithm.
* **Database Requirements**: **Migration Required**. Adds a `mastering_style` (TEXT) column to the `tracks` table, updating the Rust database mapper/structs.
* **UI Interactivity & Responsiveness**: **High**. The classification is computed asynchronously in the background. The Svelte 5 frontend displays it instantly, maintaining a 60fps frame rate. On-the-fly threshold updates also execute in `<0.05ms` on the frontend main thread.

### 5. Playlist Drag-to-Reorder & Visualizer UI
* **Complexity**: **Medium-High**. Drag-to-reorder requires integrating Svelte 5 runes with native drag/drop APIs across paginated lists. The visualizer requires calculating relative BPM changes and Camelot wheel steps, rendered as connecting SVG/HTML widgets between playlist tracks.
* **Database Requirements**: **None**. The database schema is already updated via Migration 18 (`playlists` and `playlist_tracks` tables are present with `position` columns).
* **UI Interactivity & Responsiveness**: **High**. Svelte `animate:flip` handles the coordinate morphs smoothly. The drag-and-drop system should implement optimistic UI updates so the list reorders instantly, before sending the async SQLite reordering transaction (which takes 2–10ms) to Tauri.

### 6. Saved Search Smart Auto-Naming
* **Complexity**: **Low**. The name generation is a deterministic, pure function of the active Svelte filter store (`src/lib/stores/filters.svelte.ts`). It compiles queries, keys, genres, BPM, and mood parameters into a human-readable string.
* **Database Requirements**: **None**. The existing `saved_searches` table (defined in migration 18) already has a `name` column. If we need to track if a name was custom-edited or auto-generated, we can store an `isAutoNamed` boolean flag inside the existing `query_json` field to avoid schema modifications.
* **UI Interactivity & Responsiveness**: **High**. Computing the auto-name is trivial (<0.1ms) and can be bound to a Svelte 5 `$derived` state, providing real-time name previews as sliders and text inputs are adjusted. No Rust offloading or Web Workers are required.

### 7. Vibe-Based Playlist Recommendations
* **Complexity**: **Medium**. The underlying 512-d CLAP audio embeddings, 384-d MiniLM text embeddings, and Essentia classifier columns are already stored in the DB, meaning the backend vector similarity foundation is ready. No new models are required.
* **Database Requirements**: **None**. The schema already supports playlists, playlist tracks, and high-dimensional embeddings with foreign keys and indexes. 
* **UI Interactivity & Responsiveness**: **High**. By offloading the centroid matching calculations to a Tauri Rust command (which executes on a background thread pool), the UI will easily maintain 60fps on the main thread. A distance check against 10,000 tracks completes in under 2ms in Rust.
