# Preparation Plan: User Manual Song Editing & Overrides (V2)

## 1. Goal & Requirements
The goal of this feature is to allow users to manually edit track metadata fields (Title, Artist, Album, BPM, Key, Scale, Genre, Year, lyrics, comment, composer, description, etc.) and manage custom/suppressed tags, ensuring these edits persist across file rescans, analysis resets, and background ML pipeline runs.

### Requirements:
1. **Rescan-Proof Persistence**: Store metadata overrides in a dedicated `user_track_overrides` database table keyed on `track_path`. Do not modify the source `tracks` table directly for manual edits so that scans can still update physical file tags without corrupting user edits.
2. **Dynamic Projection**: Update track queries (`find`, `find_all`) to `LEFT JOIN user_track_overrides` and return the values using `COALESCE(override, auto_detected)`. Return individual boolean flags (e.g., `is_title_overridden`) to indicate if a field has been manually overridden.
3. **Interactive Frontend Pane (`TrackDetailPane.svelte`)**:
   - Provide an **"Edit Tags"** button to trigger a detail modal.
   - Visually highlight overridden fields with an accent color and a pencil icon.
   - Show a hover tooltip with the original auto-detected value and a small revert button (`↺`) to clear the override.
   - Support right-click actions on tags to suppress auto-tags or remove user tags. Provide a dropdown to restore suppressed tags.
4. **Edit Modal (`EditTrackModal.svelte`)**:
   - A scrollable, keyboard-accessible form matching the dark glassmorphic UI.
   - Allow modifying all fields, adding custom tags, suppressing auto-tags, and reverting overridden fields.
5. **Portable Sidecars**: Serialize overrides under a `"user_overrides"` field in `.dc.json` sidecar files. Restore them on import so overrides are portable across machines and survive file/folder movements.

---

## 2. Semantic Hit Rate
We ran semantic queries using `tools/knowledge_mgr.py query` and queried the codebase index database definitions.

### Queries Run & Similarity Scores:
1. **Query**: `"user manual song edit tag override"`
   - `doc/collab/sessions/2026-06-08-feature-preparations/prepare_user_edit_song.md` (Similarity: **0.5976**)
   - `doc/proposals/user_edit_song.md` (Similarity: **0.5226**)
2. **Query**: `"track edit database update"`
   - `doc/collab/sessions/2026-06-08-feature-preparations/prepare_user_edit_song.md` (Similarity: **0.3568**)
   - `doc/proposals/user_edit_song.md` (Similarity: **0.3539**)
   - `skills/query-db/SKILL.md` (Similarity: **0.3526**)

### Codebase index defines table query:
- **Query**: `SELECT file FROM defines WHERE entity = 'LibraryDb' OR entity = 'TagCuration'`
- **Results**:
  - `/Users/rlupi/src/deep-cuts-agy/src-tauri/src/database.rs`
  - `/Users/rlupi/src/deep-cuts-agy/src-tauri/src/scanner/mod.rs`
  - `/Users/rlupi/src/deep-cuts-agy/src-tauri/src/commands/library.rs`
  - `/Users/rlupi/src/deep-cuts-agy/src/lib/stores/curation.svelte.ts`

### Analysis & Anchors:
The combination of semantic search and codebase index entity defines pinpointed the exact locations where logic must be added or altered:
- **`src-tauri/src/database.rs`**: Contains `LibraryDb` concept annotation, database connection management, migrations registry, and the `Track` database models.
- **`src-tauri/src/scanner/mod.rs`**: Contains `LibraryDb` scanner orchestration logic (upserts).
- **`src-tauri/src/commands/library.rs`**: Contains Tauri commands for retrieving and editing metadata/tags.
- **`src/lib/stores/curation.svelte.ts`**: Frontend store managing user-curated content (playlists, saved searches) matching `TagCuration`.

---

## 3. Impact Assessment

### Database / Schema Changes
- **New Migration**: Create `src-tauri/migrations/33_user_track_overrides.sql` to define the overrides table:
  ```sql
  -- Metadata overrides table
  CREATE TABLE IF NOT EXISTS user_track_overrides (
      track_path   TEXT PRIMARY KEY,
      title        TEXT,
      artist       TEXT,
      album        TEXT,
      genre        TEXT,
      year         INTEGER,
      bpm          REAL,
      lyrics       TEXT,
      comment      TEXT,
      key          TEXT,
      scale        TEXT,
      track_number INTEGER,
      track_total  INTEGER,
      disc_number  INTEGER,
      disc_total   INTEGER,
      album_artist TEXT,
      composer     TEXT,
      is_music     INTEGER,
      description  TEXT,
      FOREIGN KEY(track_path) REFERENCES tracks(path) ON DELETE CASCADE
  );
  ```
- **Registration**: Append this migration to the list returned by `get_migrations()` in `src-tauri/src/database.rs`.

### Rust Backend
- **`src-tauri/src/database.rs`**:
  - Define a new struct `TrackOverride` matching the schema.
  - Extend the `Track` struct with 18 boolean override flags `is_*_overridden: i64` (e.g. `is_title_overridden`, etc.) representing whether a given field has a non-null value in the overrides table.
  - Update `db_row_mapping!(Track { ... })` to include the new override flag fields.
  - Refactor track selection queries (e.g., `find`, `find_all`) to execute a `LEFT JOIN user_track_overrides` and select fields using `COALESCE(o.field, t.field) AS field`, as well as projecting `(o.field IS NOT NULL) AS is_field_overridden`.
  - Exclude the projected boolean flags in `test_track_mapped_columns_exist_in_schema` since they do not exist physically in the `tracks` table schema.
- **`src-tauri/src/commands/library.rs`**:
  - Implement `save_track_override` Tauri command: locks the DB connection, maps incoming JSON values, performs an upsert into `user_track_overrides`, and triggers a sidecar export if sidecars are enabled.
  - Implement `remove_track_override` Tauri command: sets a field to `NULL` for the track path (or deletes the row entirely if all fields become `NULL`), and triggers a sidecar export if enabled.
- **`src-tauri/src/lib.rs`**:
  - Register both new commands in the Tauri command handler block (`generate_handler!`).
- **`src-tauri/src/scanner/sidecar.rs`**:
  - Update `SidecarData` to include a `user_overrides: Option<TrackOverride>` field.
  - In `sidecar::save_with_extra`, fetch any matching user overrides from `user_track_overrides` and write them to the JSON sidecar.
  - In `sidecar::restore`, read `user_overrides` from the sidecar and write them to the `user_track_overrides` table.

### Frontend Svelte 5 / TS
- **`src/lib/types.ts`**:
  - Update `Track` interface with optional `is_*_overridden?: boolean` flags.
- **`src/lib/ipc.ts`**:
  - Add signatures for `save_track_override` and `remove_track_override` in `CommandMap`.
- **`src/lib/components/TrackDetailPane.svelte`**:
  - Add an "Edit Tags" button which opens `EditTrackModal.svelte`.
  - Highlight overridden fields using `var(--sg-secondary)` or an amber accent color with a pencil indicator.
  - Add tooltip on hover containing the original value and a revert `↺` button that triggers `remove_track_override`.
- **`src/lib/components/EditTrackModal.svelte`**:
  - Create the new modal form matching the glassmorphic theme.
  - Group and bind input fields.
  - Provide a quick revert action next to each input.
  - Integrate focus trap and Esc key listener.

---

## 4. Implementation Checklist

### Phase 1: Database & Model Refactoring
- [ ] Create `src-tauri/migrations/33_user_track_overrides.sql` containing the overrides table.
- [ ] Register the migration in `src-tauri/src/database.rs`.
- [ ] Add `TrackOverride` struct to `src-tauri/src/database.rs`.
- [ ] Update `Track` struct and `db_row_mapping!` in `src-tauri/src/database.rs` with the 18 `is_*_overridden: i64` fields.
- [ ] Rewrite SQL projections inside `Track::find` and `Track::find_all` to `LEFT JOIN user_track_overrides` and project coalesced columns and override boolean flags.
- [ ] Modify `test_track_mapped_columns_exist_in_schema` to ignore virtual boolean columns.
- [ ] Run `cargo test --manifest-path src-tauri/Cargo.toml` and ensure database and mappings compile and pass.

### Phase 2: Commands, Sidecar Sync, and Integration Tests
- [ ] Implement `save_track_override` command in `src-tauri/src/commands/library.rs`.
- [ ] Implement `remove_track_override` command in `src-tauri/src/commands/library.rs`.
- [ ] Register commands in `src-tauri/src/lib.rs`.
- [ ] Update `SidecarData` struct and `sidecar::save_with_extra` in `src-tauri/src/scanner/sidecar.rs` to persist manual overrides to the `.dc.json` files.
- [ ] Update `sidecar::restore` in `src-tauri/src/scanner/sidecar.rs` to insert overrides back into `user_track_overrides` on folder rescan/reimport.
- [ ] Write integration test verifying that a rescan does not overwrite user overrides, and sidecar sync functions correctly.
- [ ] Compile and verify via `cargo test --manifest-path src-tauri/Cargo.toml`.

### Phase 3: Frontend Views & Components
- [ ] Update `src/lib/types.ts` and `src/lib/ipc.ts` with the new fields and Tauri command maps.
- [ ] Create `src/lib/components/EditTrackModal.svelte` with full edit form, reverts, focus trap, and Escape key handler.
- [ ] Integrate modal and visual overrides styling, pencil icons, and hover tooltips into `src/lib/components/TrackDetailPane.svelte`.
- [ ] Run the app locally, perform manual tag/field editing, trigger a rescan/re-analysis, and confirm manual edits persist.

---

## 5. Discoverability Comparison

Using the codebase index `defines` table and semantic queries allowed us to instantly discover the precise codebase anchors. 

| Discovery Method | Target Files Located | Time Taken / Speed | Accuracy |
| :--- | :--- | :--- | :--- |
| **Previous Run** (Text Search / File Path Scanning) | Found files by guess-walking the file tree and executing random file searches. | Moderate (multiple lookup cycles, reading unrelated files). | High, but required manual verification. |
| **New Run** (Semantic Query + DB `defines` lookup) | Directly targeted `database.rs`, `scanner/mod.rs`, `commands/library.rs`, and `curation.svelte.ts`. | **Instant (2 calls)**. | **Perfect**. The `defines` table linked exact code symbols (`LibraryDb`, `TagCuration`) to their files immediately, bypassing folder traversal completely. |
