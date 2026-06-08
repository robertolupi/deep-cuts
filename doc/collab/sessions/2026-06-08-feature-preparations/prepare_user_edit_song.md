# Preparation Plan: User Manual Song Editing & Overrides

## 1. Goal & Requirements
The goal of this feature is to allow users to manually edit track metadata fields (Title, Artist, Album, BPM, Key, Scale, Genre, Year, lyrics, comment, composer, description, etc.) and manage custom/suppressed tags, ensuring these edits persist across file rescans, analysis resets, and background ML pipeline runs.

### Requirements:
1. **Rescan-Proof Persistence**: Store metadata overrides in a dedicated `user_track_overrides` database table keyed on `track_path`. Do not modify the source `tracks` table directly for manual edits so that scans can still update physical file tags without corrupting user edits.
2. **Dynamic Projection**: Update track queries (`find`, `find_all`) to `LEFT JOIN user_track_overrides` and return the values using `COALESCE(override, auto_detected)`. Return individual boolean flags (e.g. `is_title_overridden`) to indicate if a field has been manually overridden.
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
We ran semantic queries using `tools/knowledge_mgr.py query` to locate existing tag management, metadata storage, and sidecar sync details.

### Queries Run & Matches:
*   **Query**: `"user metadata overrides"`
*   **Matches**:
    1.  `doc/proposals/user_edit_song.md` (Section: `Current State`) — **Similarity: 0.5155**
        - *Key takeaway*: Confirms that user tags and suppressions are already partially implemented and fully functional (backed by `user_suppressed_tags` and `track_tags` tables), but metadata field overrides (BPM, key, title, description) are completely missing.
    2.  `doc/proposals/user_edit_song.md` (Section: `Acceptance Criteria`) — **Similarity: 0.4082**
        - *Key takeaway*: Details UI specs, modal layout, accessibility, sidecar schema, and integration tests.
    3.  `doc/proposals/user_edit_song.md` (Section: `1. The Challenge`) — **Similarity: 0.3866**
        - *Key takeaway*: Identifies the three points of conflict (Scanner upserts, analysis pass resets, and sidecar sync) and explains why a separate table + COALESCE approach is the optimal design.
    4.  `doc/proposals/user_edit_song.md` (Section: `5. Portability & Sidecar Integration`) — **Similarity: 0.3524**
        - *Key takeaway*: Outlines the exact JSON format for overrides in the sidecar and the save/restore steps.

### Analysis & Anchors:
The codebase search confirms that `user_suppressed_tags` and tag suppressions are already fully functional. We only need to implement the database schema, Rust query projections, Tauri endpoints, sidecar sync, and frontend modal for **core metadata overrides** (`user_track_overrides`). 
- **DB Anchor**: `src-tauri/src/database.rs` (schema migrations and `Track` mapping).
- **Backend Anchor**: `src-tauri/src/commands/library.rs` (Tauri commands and tag handling).
- **Sidecar Anchor**: `src-tauri/src/scanner/sidecar.rs` (serialization/deserialization).
- **Frontend Anchor**: `src/lib/components/TrackDetailPane.svelte` (edit trigger and field display indicators).

---

## 3. Impact Assessment

### Database / Schema Changes
- **New Migration**: Create `src-tauri/migrations/33_user_track_overrides.sql` to define the overrides table:
  ```sql
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
- **Registration**: Register the new file at the end of the vector returned by `get_migrations()` in `src-tauri/src/database.rs`.

### Rust Backend
- **`src-tauri/src/database.rs`**:
  - Add `TrackOverride` struct representing the manual field overrides.
  - Add `is_*_overridden: Option<bool>` boolean fields to the `Track` struct.
  - Add the corresponding `is_*_overridden` fields to the `db_row_mapping!(Track { ... })` macro.
  - Implement `Track::select_sql() -> &'static str` to return the complete column selection query (coalescing override values and projecting override flags) with `LEFT JOIN user_track_overrides`.
  - Rewrite `Track::find_all` and `Track::find` to query via `Track::select_sql()`.
  - Update `test_in_memory_migrations_boot_successfully` to use `Track::select_sql()`.
  - Exclude the virtual boolean override fields (`is_*_overridden`) in `test_track_mapped_columns_exist_in_schema` since they do not exist physically in the `tracks` table.
- **`src-tauri/src/commands/library.rs`**:
  - Implement `save_track_override` Tauri command (calls `INSERT OR REPLACE` into `user_track_overrides` and triggers sidecar export).
  - Implement `remove_track_override` Tauri command (sets specific field to `NULL` for the path; deletes row if all fields are `NULL`; triggers sidecar export).
- **`src-tauri/src/lib.rs`**:
  - Register `save_track_override` and `remove_track_override` commands inside `generate_handler![]`.
- **`src-tauri/src/scanner/sidecar.rs`**:
  - Add `SidecarUserOverrides` struct representing the fields on the JSON file.
  - Add `user_overrides: Option<SidecarUserOverrides>` to `SidecarData`.
  - In `sidecar::save_with_extra`, query `user_track_overrides` for the track path and populate `user_overrides` in the output file.
  - In `sidecar::restore`, read `user_overrides` and execute an upsert statement into `user_track_overrides`.

### Frontend Svelte 5 / TS
- **`src/lib/types.ts`**:
  - Update the `Track` interface with the new `is_title_overridden?: boolean`, etc., properties.
- **`src/lib/ipc.ts`**:
  - Update `CommandMap` with types for `save_track_override` and `remove_track_override`.
- **`src/lib/components/TrackDetailPane.svelte`**:
  - Add an `[ Edit Tags ]` button inside the top bar.
  - Highlight overridden fields (e.g. if `track.is_bpm_overridden` is true) using a soft accent color (e.g. `var(--sg-secondary)`) and render a tiny edit pencil icon (✏️).
  - Add a hover tooltip displaying `Auto-detected: <value>` and a revert `↺` button next to overridden fields.
  - Bind clicking the revert button to `remove_track_override` and refetch track data.
  - Right-click context actions for automatic tags (suppress) and custom tags (delete). Clicking empty background displays the dropdown of suppressed tags for restoration.
- **`src/lib/components/EditTrackModal.svelte`**:
  - Create this new component matching the glassmorphic Sonic Glitch UI.
  - Populate inputs from `player.selectedTrack`, allowing user editing of all metadata.
  - Allow reverting individual overridden fields directly inside the form.
  - Allow adding new tags via autocomplete against `library.allTags`.
  - Implement a focus trap and Esc key listener for keyboard accessibility.
  - On save, execute `save_track_override` command, refresh the library, and close the modal.

---

## 4. Implementation Checklist

### Phase 1: Database & Struct Setup
- [ ] Create database migration `/Users/rlupi/src/deep-cuts-agy/src-tauri/migrations/33_user_track_overrides.sql` containing the `user_track_overrides` table.
- [ ] Register the migration in `src-tauri/src/database.rs` under `get_migrations()`.
- [ ] Add `TrackOverride` struct to `src-tauri/src/database.rs`.
- [ ] Add the 18 `is_*_overridden: Option<bool>` fields to `Track` struct and `db_row_mapping!` macro in `src-tauri/src/database.rs`.
- [ ] Implement `Track::select_sql() -> &'static str` returning the query with `LEFT JOIN user_track_overrides` and `COALESCE` selections.
- [ ] Refactor `Track::find_all`, `Track::find`, and `test_in_memory_migrations_boot_successfully` in `src-tauri/src/database.rs` to build queries from `Track::select_sql()`.
- [ ] Update `test_track_mapped_columns_exist_in_schema` in `src-tauri/src/database.rs` to exclude `is_*_overridden` columns from checking physical table schema.
- [ ] Verify database compilation and runs: `cargo test --manifest-path src-tauri/Cargo.toml`.

### Phase 2: Backend Commands & Sidecar Sync
- [ ] Add `save_track_override` command inside `src-tauri/src/commands/library.rs`.
- [ ] Add `remove_track_override` command inside `src-tauri/src/commands/library.rs` (validating field against whitelist of columns).
- [ ] Register both new commands in `src-tauri/src/lib.rs` inside `generate_handler![]`.
- [ ] Define `SidecarUserOverrides` struct in `src-tauri/src/scanner/sidecar.rs`.
- [ ] Add `user_overrides` to `SidecarData` in `src-tauri/src/scanner/sidecar.rs`.
- [ ] Update `sidecar::save_with_extra` to fetch overrides and include them in the written sidecar file.
- [ ] Update `sidecar::restore` to read overrides and insert them into `user_track_overrides`.
- [ ] Add integration test verifying sidecar export/restore and rescan-proof nature of metadata overrides.
- [ ] Run `cargo test --manifest-path src-tauri/Cargo.toml` to ensure backend logic compiles and tests pass.

### Phase 3: Frontend Types & Components
- [ ] Add `is_*_overridden?: boolean` properties to `Track` in `src/lib/types.ts`.
- [ ] Register `save_track_override` and `remove_track_override` in `CommandMap` inside `src/lib/ipc.ts`.
- [ ] Create `src/lib/components/EditTrackModal.svelte` with full edit form, inline field reverts, tag manager (with autocomplete), accessibility keyboard navigation (focus trap, Escape key), and Tauri command saving.
- [ ] Modify `src/lib/components/TrackDetailPane.svelte` to include the "Edit Tags" button.
- [ ] Implement highlight styling, pencil icons, hover tooltips, and revert button actions in `TrackDetailPane.svelte` for each overridable field.
- [ ] Add right-click tag suppression / custom tag removal, and dropdown suppressed tag restoration.
- [ ] Run `npm run tauri dev` and manually test editing track metadata, verifying overrides survive folder rescans/re-analyses, and ensuring sidecar export preserves the user-defined overrides.
