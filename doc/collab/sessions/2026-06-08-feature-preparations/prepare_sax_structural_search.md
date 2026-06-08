# Preparation Plan: SAX-Based Structural Similarity Search

## 1. Goal & Requirements
The goal of this feature is to replace the client-side JavaScript-based structural similarity logic (`levDistance` in `filters.svelte.ts`) with a high-performance Rust backend command (`search_by_structure`). This transition enables faster query execution, lower memory overhead, and sets up the substrate for a visual Block Composer UI for song architecture queries.

### Core Requirements
1. **Rust-side Similarity Matching**: Re-implement Levenshtein distance on the Rust backend to compute alignment similarity between structural alphabet sequences (e.g. `"IIVVVVPCCCCO"` representation).
2. **IPC Command Integration**: Add a Tauri command `search_by_structure` that queries the database, computes structural distances against a target alphabet sequence, and returns matching tracks sorted by distance.
3. **Database Schema Expansion**: Add schema migrations for storing repetition vectors and classification labels to fully integrate with retrained neural sequence models.
4. **Svelte Store Integration**: Update the frontend `filters.svelte.ts` store to query the new IPC command instead of computing edit distances client-side.
5. **Visual Block Composer**: Add Svelte component widgets to allow constructing structural query arrays (e.g. `[Intro, Verse, Chorus, Outro]`), translating them to regex/alphabet queries, and displaying match scores.

---

## 2. Semantic Hit Rate
The following semantic queries were run to find relevant components and modules:

1. **Query**: `"search_similar_tracks_audio blended_embedding_distance"` (Similarity: 0.5210)
   - *Match*: `src-tauri/src/commands/map.rs:search_similar_tracks_audio`
   - *Insight*: Shows how the backend currently handles similarity lookups, blending acoustic distance with SAX minimum distance. The new command can follow a similar structure.
2. **Query**: `"search_by_structure"` (Similarity: 0.5698)
   - *Match*: `doc/research/sax_structural_search.md:Validation Plan`
   - *Insight*: Outlines the expected input signature and testing requirements for the `search_by_structure` command.
3. **Query**: `"levDistance filters.svelte.ts"` (Similarity: 0.4629)
   - *Match*: `doc/operations/codex-feedback/item-F2-split-svelte-components.md`
   - *Insight*: Confirms `filters.svelte.ts` holds the client-side calculation to be replaced.

---

## 3. Impact Assessment

### Database / Schema Changes
- **New Migration**: Create `src-tauri/migrations/33_waveform_repetition.sql` to add columns:
  - `waveform_repetition TEXT` (JSON array of 16 floats representing SSM-based repetition scores)
  - `waveform_labels TEXT` (JSON array of 16 label strings)
- **Database Model Mapping**:
  - Update `Track` struct in `src-tauri/src/database.rs` to include `waveform_repetition` and `waveform_labels`.
  - Update the `db_row_mapping!` macro to include the new fields at indices 55 and 56.

### Rust Backend
- **Core Algorithm**:
  - Implement a fast edit-distance / Levenshtein distance function in Rust (e.g. in `src-tauri/src/analysis/sax_alignment.rs` or as a utility module).
- **Tauri IPC Command**:
  - Implement `search_by_structure` in `src-tauri/src/commands/map.rs`:
    ```rust
    #[tauri::command]
    pub fn search_by_structure(
        seed_alignment: String,
        max_distance: Option<usize>,
        conn_state: tauri::State<'_, Mutex<Connection>>,
    ) -> Result<Vec<AudioSimilarityResult>, String>
    ```
  - Fetch all track IDs and `sax_alignment` strings from the database, compute Levenshtein distances against `seed_alignment`, filter by `max_distance` (default = 4), and sort ascending.
- **Tauri Registration**:
  - Register the new command in `src-tauri/src/lib.rs`.

### Frontend Svelte 5 / TS
- **`src/lib/stores/filters.svelte.ts`**:
  - Replace the client-side JS `levDistance` function.
  - Modify `setStructureSimilarTo(track)` to call `invoke("search_by_structure", { seedAlignment: track.alignment, maxDistance: 4 })` asynchronously.
  - Store results in `structureSimilarIds` and `structureSimilarScores`.
  - Bind reactive state variables for the Block Composer (`structureQueryBlocks`, `structureSearchTolerance`).
- **`src/lib/components/BlockComposer.svelte`** (New Component):
  - A draggable lane where users can place blocks (`Intro`, `Verse`, `Pre-Chorus`, `Chorus`, `Bridge`, `Outro`, `Wildcard`).
  - Automatically compiles block selections into a target alphabet string or a regex query.

---

## 4. Implementation Checklist

### Step 1: Database Migration & Schema Mapping
- [ ] Create SQL migration `src-tauri/migrations/33_waveform_repetition.sql` containing:
  ```sql
  ALTER TABLE tracks ADD COLUMN waveform_repetition TEXT;
  ALTER TABLE tracks ADD COLUMN waveform_labels TEXT;
  ```
- [ ] Add the fields to the `Track` struct in `src-tauri/src/database.rs`:
  ```rust
  pub waveform_repetition: Option<String>,
  pub waveform_labels: Option<String>,
  ```
- [ ] Update the `db_row_mapping!` macro in `database.rs` to include `waveform_repetition` and `waveform_labels`.

### Step 2: Implement Rust Levenshtein Matcher
- [ ] Implement `lev_distance(a: &str, b: &str) -> usize` in `src-tauri/src/analysis/sax_alignment.rs` (or a helper module):
  - Standard dynamic programming table with two-row optimization for $O(\min(M, N))$ space.
- [ ] Add unit tests in Rust to verify accuracy against standard edge cases.

### Step 3: Implement Tauri Command
- [ ] Add `search_by_structure` command to `src-tauri/src/commands/map.rs`:
  - Query SQLite for all non-null `sax_alignment` strings.
  - Compute distances and map to `AudioSimilarityResult` structs.
  - Filter by `max_distance` (default = 4) and sort by distance ascending.
- [ ] Register `search_by_structure` command in `src-tauri/src/lib.rs`.
- [ ] Add integration test in Rust checking that query against a seed returns sorted results.

### Step 4: Refactor Frontend Store
- [ ] Open `src/lib/stores/filters.svelte.ts`.
- [ ] Update `setStructureSimilarTo(track)` to invoke the Tauri command:
  ```typescript
  const results = await invoke("search_by_structure", {
    seedAlignment: track.alignment,
    maxDistance: 4,
  });
  ```
- [ ] Set `structureSimilarIds` and `structureSimilarScores` from the backend output.
- [ ] Remove the unused `levDistance` function from `filters.svelte.ts`.

### Step 5: Implement UI Block Composer
- [ ] Create `src/lib/components/BlockComposer.svelte`.
- [ ] Add a palette of named blocks with colors mapped to CSS structure variables.
- [ ] Map active composer sequence to structure query parameters.
- [ ] Add match percentage badge display to track list rows when matching active patterns.
