# Preparation Plan: Playlist & Saved Search View Enhancements

## 1. Goal & Requirements

This feature enhances the library curation experience in Deep Cuts by implementing:
1. **Interactive Playlist Curation & Reordering**:
   - High-fidelity drag-and-drop reordering inside Svelte 5 `TrackList.svelte` when a playlist is active.
   - Keyboard navigation via Arrow keys, plus `Alt + ArrowUp/Down` shortcut for keyboard-driven track reordering.
2. **Transition Visual Feedback**:
   - Camelot harmonic key compatibility badges (green for compatible transitions, red for key change warnings) between consecutive track rows.
   - Tempo slope indicators showing BPM changes step-by-step (e.g. `↗️ +4.0 BPM`, `↘️ -2.5 BPM`).
3. **Continuous Energy Sparkline (Waveform Arc)**:
   - A single, continuous energy SVG sparkline stitched from the 128-bin waveform envelopes of all tracks in the playlist, rendered below the track table to visualize the set's narrative arc.
4. **Auto-Naming for Saved Searches**:
   - Extends the `generateSmartName` naming utility to auto-detect and append active mood parameters alongside genre and BPM ranges.
5. **AI Suggested Additions Panel**:
   - A recommendation panel in the sidebar recommending tracks from the library based on a computed centroid of CLAP embeddings + average mood + average BPM of the current playlist or saved search.
6. **Transition Pathfinder (TSP Optimizer)**:
   - An "Optimize Transitions" action reordering playlist tracks using the Traveling Salesperson Problem (TSP) algorithm over structural CLAP embedding distances to minimize transition friction.

---

## 2. Semantic Hit Rate

Based on semantic search queries and file inspections, the implementation anchors around the following key modules:

### A. Auto-Naming & Filtering
* **Files**:
  - `src/lib/utils/naming.ts` (100% Match): Houses `generateSmartName` and `FilterState`. Needs modification to support mood filters.
  - `src/lib/utils/naming.test.ts` (100% Match): Houses Vitest unit tests for name generation. Needs additional test cases for mood combinations.
  - `src/lib/stores/filters.svelte.ts` (100% Match): Manages the reactive filters state, deriving the `autoName` value.

### B. Playlist Database & Command Logic
* **Files**:
  - `src-tauri/src/commands/playlists.rs` (100% Match): Contains database operations for playlist track retrieval, insertions, deletions, and positioning.
  - `src-tauri/migrations/18_playlists.sql` (100% Match): Defines the database schema for `playlists` and `playlist_tracks` (using a primary key of `(playlist_id, position)`).
  - `src-tauri/src/commands/map.rs` (80% Match): Defines mathematical helpers (`l2_normalize`, `l2_distance_sq`) and similarity search logic.

### C. Playlist UI & Sparkline
* **Files**:
  - `src/lib/components/TrackList.svelte` (100% Match): Renders the main track list table. Needs HTML5 drag-and-drop handlers, keyboard navigation listeners, transition compatibility badges, and the Waveform Arc SVG sparkline.
  - `src/lib/utils/mapMath.ts` (90% Match): Houses the `camelotMap` mapping keys to Camelot codes and colors.

### D. AI Suggested Additions
* **Files**:
  - `src/lib/components/FilterSidebar.svelte` (100% Match): Sidebar containing filters and curations. Needs a "Suggested Additions" panel that reactively updates based on the current playlist or saved search.

---

## 3. Impact Assessment

### Database / Schema Changes
No database migrations are required. The existing SQLite schema fully supports all required operations:
- `playlist_tracks` tracks the order with the `position` column.
- `tracks` stores mood columns (`mood_happy`, `mood_sad`, etc.), `bpm`, `key`, `scale`, and the 128-bin `waveform_data` JSON string.
- `audio_embeddings` stores the normalized CLAP vectors for semantic/structural distance.

### Rust Backend
New commands will be added to the backend to support recommendations and TSP reordering:
1. **TSP Optimization Command**:
   - `optimize_playlist_transitions(playlist_id: i64, track_ids: Vec<i64>) -> Result<(), AppError>`
     - Fetches CLAP embeddings for the specified tracks.
     - Computes a symmetric $K \times K$ distance matrix based on Euclidean distance of L2-normalized CLAP vectors.
     - Solves open TSP using Held-Karp (for $K \le 16$) or Nearest Neighbor + 2-opt local search (for $K > 16$).
     - Permutes database positions of the selected tracks in a single transaction, shifting to negative buffer positions to avoid UNIQUE primary key violations.
2. **AI Recommendation Command**:
   - `suggest_playlist_tracks(playlist_id: Option<i64>, track_ids: Option<Vec<i64>>, limit: Option<usize>) -> Result<Vec<Track>, AppError>`
     - Computes the average CLAP vector (centroid) of the active playlist/set.
     - Queries `audio_embeddings` using sqlite-vec MATCH to retrieve the top 100 closest tracks from the library.
     - Excludes tracks already in the playlist.
     - Reranks in Rust using a weighted blend of CLAP distance, mood vector distance, and BPM proximity.

### Frontend Svelte 5 / TS
1. **`naming.ts` & `naming.test.ts`**:
   - Extend `FilterState` to include `moodHappyMin`, `moodSadMin`, etc.
   - Update `generateSmartName` to detect non-zero minimum mood thresholds and append active moods (e.g. `(Happy)`).
2. **`TrackList.svelte`**:
   - Add HTML5 drag-and-drop attributes (`draggable={!!curation.activePlaylist}`) to track rows.
   - Implement `Alt+ArrowUp/Down` keyboard shortcuts to call `curation.reorderPlaylistTrack` directly.
   - Add an inline helper checking Camelot key compatibility using `camelotMap` from `mapMath.ts`.
   - Render a custom `<tr>` containing the compatibility badge and BPM slope between track rows.
   - Stitch `waveform_data` values into an SVG path to render the neon Waveform Arc below the table.
   - Render an "Optimize Transitions" button next to the track count toolbar when a playlist is active.
3. **`FilterSidebar.svelte`**:
   - Add a collapsible "Suggested Additions" panel under the Filters tab.
   - Use Svelte's `$effect` to load recommended tracks from `suggest_playlist_tracks` whenever active tracks change.

---

## 4. Implementation Checklist

### Step 1: Naming & Unit Tests
- [ ] **Extend `naming.ts`**:
  - Add mood properties (`moodHappyMin`, `moodSadMin`, etc.) to the `FilterState` interface.
  - Modify `generateSmartName` to detect when a mood filter's min value is $> 0.0$.
  - Format active moods into a comma-separated list and append it inside parentheses (e.g. `(Happy, Relaxed)`).
- [ ] **Add Unit Tests in `naming.test.ts`**:
  - Add tests validating combinations of genre, BPM, and mood (e.g. `Pop (100–120 BPM) (Happy)`).
  - Run tests with `npm run test` or via Vitest command to verify.

### Step 2: Rust Backend Tauri Commands
- [ ] **Implement TSP Solver (`playlists.rs`)**:
  - Implement exact Held-Karp solver for path size $K \le 16$.
  - Implement heuristic Nearest Neighbor + 2-opt solver for path size $K > 16$.
  - Add `optimize_playlist_transitions` Tauri command.
  - Implement database transaction to swap track positions using negative values to avoid UNIQUE constraint errors.
- [ ] **Implement AI Suggestions (`playlists.rs`)**:
  - Add `suggest_playlist_tracks` command.
  - Fetch CLAP embeddings for the input playlist tracks, compute the L2-normalized centroid.
  - Match nearest 100 library tracks, filter out existing playlist tracks, and rerank by combined (CLAP + Mood + BPM) scores.
- [ ] **Expose commands in `lib.rs`**:
  - Register `optimize_playlist_transitions` and `suggest_playlist_tracks` commands in `tauri::Builder::default().invoke_handler(...)`.
- [ ] **Write Backend Tests**:
  - Add integration tests verifying that Held-Karp computes optimal TSP path on a small coordinate set.
  - Add unit tests verifying centroid computation on mock embedding vectors.

### Step 3: Frontend Playlist View Enhancements
- [ ] **Add Drag-to-Reorder in `TrackList.svelte`**:
  - Attach `draggable` to table rows.
  - Track drag source index in Svelte local state.
  - On drop, invoke `curation.reorderPlaylistTrack(playlistId, fromIndex, toIndex)`.
- [ ] **Add Keyboard Navigation & Reordering**:
  - Attach keydown handler to the window in `TrackList.svelte`.
  - Capture ArrowUp / ArrowDown to move selected track.
  - Capture Alt + ArrowUp / ArrowDown to call `reorderPlaylistTrack` on the backend.
- [ ] **Stitch Continuous Waveform Arc**:
  - Parse `waveform_data` (128 floats) for all filtered tracks.
  - Downsample stitched points to 500 bins for optimal performance.
  - Generate an SVG path and render a styled sparkline with a neon glow below the table.
- [ ] **Integrate Key & BPM Transition Indicators**:
  - Add helper function `areKeysCompatible` utilizing `camelotMap` codes.
  - Insert transition rows (`<tr class="transition-row">`) between table tracks.
  - Render harmonic compatibility labels (green/red) and tempo slope indicators (e.g. `↘️ -3.0 BPM`).
- [ ] **Add "Optimize Transitions" Button**:
  - Render an "Optimize Transitions" button in the track toolbar.
  - Clicking this calls `optimize_playlist_transitions` on the active playlist tracks, then refreshes the active list.

### Step 4: AI Sidebar Recommendations
- [ ] **Add "Suggested Additions" panel in `FilterSidebar.svelte`**:
  - Render a collapsible panel at the bottom of the Filter tab.
  - Bind reactive effect to call `suggest_playlist_tracks` with current track IDs.
  - Render suggestions list with a quick-add `+` button that appends the track to the current playlist.

---

## 5. Verification & Safety Guards
- Always use single transactions for playlist position updates to keep database state consistent.
- Ensure the keyboard navigation handler bypasses inputs, textareas, and editable tags.
- Run `cargo test` in `src-tauri` and Vitest in `src` to ensure no regressions occur.
