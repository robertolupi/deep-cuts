# Brainstorming: User Manual Song Editing & Overrides

This document outlines the design for allowing users to manually edit track metadata (Title, Artist, Album, BPM, Genre, etc.) and manage tags (adding custom tags, suppressing automatic tags) such that these changes persist across automatic scans, background ML analysis, and file updates.

---

## Acceptance Criteria

- **User-visible:** An "Edit Tags" button in `TrackDetailPane` opens `EditTrackModal.svelte`; all core metadata fields (title, artist, album, BPM, key, scale, genre, year, description, lyrics, composer, etc.) are editable in the modal with a scrollable form matching the dark glassmorphic theme.
- **User-visible:** Overridden fields are visually distinguished with an accent highlight and a pencil indicator in `TrackDetailPane`; hovering shows the original auto-detected value; a revert button (↺) restores the field to its auto-detected value.
- **User-visible:** Automatic tags shown in the detail pane can be suppressed via right-click context menu; suppressed tags render with strikethrough; a restore action re-enables them. Custom user tags can be added and removed.
- **User-visible:** All overrides survive a full library rescan and analysis re-run without being overwritten.
- **Data model:** Migration `23_user_track_overrides.sql` creates `user_track_overrides` (keyed on `track_path`, one nullable column per overridable field) and `user_suppressed_tags` (composite PK on `track_path` + `tag_name`) tables. `Track::find_all` / `Track::find` LEFT JOIN `user_track_overrides` using COALESCE and return per-field `is_*_overridden` boolean flags.
- **IPC / frontend boundary:** New Tauri commands `save_track_override`, `remove_track_override`, `add_user_tag`, `remove_user_tag`, `suppress_tag`, and `unsuppress_tag` are registered and typed on the TypeScript side; `Track` interface gains `is_*_overridden` boolean fields.
- **Sidecar round-trip:** `SidecarData` gains `user_overrides`, `user_tags`, and `suppressed_tags` fields; sidecar save and restore logic reads/writes these tables so overrides survive folder moves and are portable across machines.
- **Analysis pipeline behavior:** Overrides stored in `user_track_overrides` are not cleared by the scanner upsert or by analysis pass resets; suppression rows in `user_suppressed_tags` persist across all pipeline runs.
- **Tests:** Rust integration test confirms that a rescan does not overwrite an existing `user_track_overrides` row; unit tests for `save_track_override` and `suppress_tag` commands; test that sidecar round-trip preserves overrides and suppressions.
- **Local verification:** Edit a track's BPM, trigger a rescan, confirm the custom value is still shown; suppress a tag, re-run analysis, confirm the tag remains suppressed; export and reimport sidecar, confirm overrides are restored.
- **Theme / accessibility:** Edit modal must be keyboard-navigable (focus trap, Esc closes, Tab cycles fields); pencil and revert icons carry `aria-label` attributes.

---

## Current State

This proposal is partially implemented, but the shipped shape is narrower than the full override design below.

| Area | Status | Evidence / Notes |
| :--- | :--- | :--- |
| User tag additions | Implemented | The app supports user-managed tags in the library command/UI path. |
| Automatic tag suppression/discard | Implemented | Tag metadata includes discard state, and the UI can hide or suppress unwanted automatic tags. |
| Metadata override tables | Partially implemented | Later migrations added user-tag and suppression support, but the broad `user_track_overrides`/`COALESCE` field override design is not fully shipped. |
| Manual editing of core fields | Need human review | Title, artist, BPM, key, description, and other field-level overrides need review against the current scanner and sidecar model before implementation. |
| Sidecar round-trip for overrides | Need human review | Sidecar support exists for analysis metadata, but override-specific sync should be revalidated before relying on this design. |

---

## 1. The Challenge

1. **Scanner Upserts**: The file scanner reads tags from files and writes/updates the `tracks` table directly via `upsert_tracks_transactional` (which runs `ON CONFLICT(path) DO UPDATE SET...`). Any full rescan or automatic file update will overwrite manual changes made to the `tracks` table.
2. **Analysis Passes**: Background tasks (Essentia, Qwen, audio analysis) write ML-derived metadata directly into the `tracks` table. During resets/re-analysis, they clear existing tags of their specific source (`DELETE FROM track_tags WHERE track_id = ?1 AND source = ?2`) and re-write them. This deletes any local modifications to automatic tags (like marking them as discarded in `track_tags`).
3. **Sidecar Sync**: The application can export/restore ML metadata to/from `.dc.json` sidecar files.

To support manual user overrides and tag modifications that survive rescans and analysis passes, we store the overrides and suppressions in dedicated tables using the stable file path as the key, and sync them with the sidecar files.

---

## 2. Database Schema Design

We will use two dedicated tables for tracking manual changes: `user_track_overrides` for metadata and `user_suppressed_tags` for suppressing automatic tags.

### Migration SQL
Create `src-tauri/migrations/23_user_track_overrides.sql`:
```sql
-- Metadata overrides
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

-- Tag suppressions (rescan-proof)
CREATE TABLE IF NOT EXISTS user_suppressed_tags (
    track_path TEXT NOT NULL,
    tag_name   TEXT NOT NULL,
    PRIMARY KEY (track_path, tag_name),
    FOREIGN KEY (track_path) REFERENCES tracks(path) ON DELETE CASCADE
);
CREATE INDEX idx_user_suppressed_tags_path ON user_suppressed_tags(track_path);
```

### Retrieving Track Data (joins and COALESCE)
We will update `Track::find_all` and `Track::find` in `src-tauri/src/database.rs` to left join `user_track_overrides` and project overridden fields. We will also return boolean indicators for each field to show if it is overridden:

```sql
SELECT 
    t.id,
    t.watched_directory_id,
    t.path,
    t.filename,
    t.size_bytes,
    t.last_modified,
    t.duration_seconds,
    t.sample_rate,
    t.bitrate,
    t.channels,
    t.bit_depth,
    COALESCE(o.title, t.title) AS title,
    COALESCE(o.artist, t.artist) AS artist,
    COALESCE(o.album, t.album) AS album,
    COALESCE(o.genre, t.genre) AS genre,
    COALESCE(o.year, t.year) AS year,
    COALESCE(o.track_number, t.track_number) AS track_number,
    COALESCE(o.track_total, t.track_total) AS track_total,
    COALESCE(o.disc_number, t.disc_number) AS disc_number,
    COALESCE(o.disc_total, t.disc_total) AS disc_total,
    COALESCE(o.album_artist, t.album_artist) AS album_artist,
    COALESCE(o.composer, t.composer) AS composer,
    COALESCE(o.comment, t.comment) AS comment,
    COALESCE(o.bpm, t.bpm) AS bpm,
    COALESCE(o.lyrics, t.lyrics) AS lyrics,
    t.waveform_data,
    COALESCE(o.key, t.key) AS key,
    COALESCE(o.scale, t.scale) AS scale,
    t.key_strength,
    t.loudness_lufs,
    t.loudness_range,
    t.silence_regions,
    t.has_long_silence,
    t.detected_genre,
    t.detected_vocal,
    t.detected_vocal_confidence,
    t.mood_happy,
    t.mood_sad,
    t.mood_aggressive,
    t.mood_relaxed,
    t.mood_party,
    t.mood_acoustic,
    t.mood_electronic,
    COALESCE(o.is_music, t.is_music) AS is_music,
    t.ai_genre,
    t.ai_mood,
    t.ai_instruments,
    COALESCE(o.description, t.description) AS description,
    t.is_stale,
    -- Override flags
    (o.title IS NOT NULL) AS is_title_overridden,
    (o.artist IS NOT NULL) AS is_artist_overridden,
    (o.album IS NOT NULL) AS is_album_overridden,
    (o.genre IS NOT NULL) AS is_genre_overridden,
    (o.year IS NOT NULL) AS is_year_overridden,
    (o.bpm IS NOT NULL) AS is_bpm_overridden,
    (o.lyrics IS NOT NULL) AS is_lyrics_overridden,
    (o.comment IS NOT NULL) AS is_comment_overridden,
    (o.key IS NOT NULL) AS is_key_overridden,
    (o.scale IS NOT NULL) AS is_scale_overridden,
    (o.track_number IS NOT NULL) AS is_track_number_overridden,
    (o.track_total IS NOT NULL) AS is_track_total_overridden,
    (o.disc_number IS NOT NULL) AS is_disc_number_overridden,
    (o.disc_total IS NOT NULL) AS is_disc_total_overridden,
    (o.album_artist IS NOT NULL) AS is_album_artist_overridden,
    (o.composer IS NOT NULL) AS is_composer_overridden,
    (o.is_music IS NOT NULL) AS is_is_music_overridden,
    (o.description IS NOT NULL) AS is_description_overridden
FROM tracks t
LEFT JOIN user_track_overrides o ON t.path = o.track_path;
```

---

## 3. Rust Backend & IPC Commands

We will introduce a model struct representing the overrides and expose Tauri commands to save or clear them.

### Data Structures
Add the `TrackOverride` struct to `src-tauri/src/database.rs`:
```rust
#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct TrackOverride {
    pub track_path: String,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub genre: Option<String>,
    pub year: Option<i64>,
    pub bpm: Option<f64>,
    pub lyrics: Option<String>,
    pub comment: Option<String>,
    pub key: Option<String>,
    pub scale: Option<String>,
    pub track_number: Option<i64>,
    pub track_total: Option<i64>,
    pub disc_number: Option<i64>,
    pub disc_total: Option<i64>,
    pub album_artist: Option<String>,
    pub composer: Option<String>,
    pub is_music: Option<i64>,
    pub description: Option<String>,
}
```

### Tag Retrieval Queries With Suppression Support
To handle tag suppression seamlessly, we will update the tag queries in `src-tauri/src/commands/library.rs`:

1. **`get_tags_with_meta_for_tracks`**:
   We will left join `user_suppressed_tags` using the track path and tag name, and set `discard = 1` if a suppression matches:
   ```sql
   SELECT tt.track_id, t.name, tt.source, tt.score,
          (CASE WHEN ust.tag_name IS NOT NULL THEN 1 ELSE tt.discard END) AS discard
   FROM track_tags tt
   JOIN tags t ON t.id = tt.tag_id
   JOIN tracks tr ON tr.id = tt.track_id
   LEFT JOIN user_suppressed_tags ust ON ust.track_path = tr.path AND ust.tag_name = t.name
   WHERE tt.track_id IN ({})
   ```

2. **Active Tags Queries** (`get_tags_for_tracks`, `get_all_track_tags`, `get_all_tags`):
   Filter out suppressed tags:
   ```sql
   SELECT tt.track_id, t.name
   FROM track_tags tt
   JOIN tags t ON t.id = tt.tag_id
   JOIN tracks tr ON tr.id = tt.track_id
   LEFT JOIN user_suppressed_tags ust ON ust.track_path = tr.path AND ust.tag_name = t.name
   WHERE tt.discard = 0 AND ust.tag_name IS NULL
   ```

### Tag Modification IPC Endpoints
Expose commands in `src-tauri/src/lib.rs` for user-added tags and suppressions:

```rust
/// Adds a custom user-defined tag to a track (source: "user")
#[tauri::command]
fn add_user_tag(
    conn_state: tauri::State<'_, std::sync::Mutex<rusqlite::Connection>>,
    track_path: String,
    tag_name: String
) -> Result<(), String> {
    let conn = conn_state.lock().map_err(|e| e.to_string())?;
    // 1. Get track ID from path
    // 2. Ensure tag exists in tags table, get tag ID
    // 3. Insert into track_tags with source = "user" and score = 1.0
    Ok(())
}

/// Removes a custom user-defined tag
#[tauri::command]
fn remove_user_tag(
    conn_state: tauri::State<'_, std::sync::Mutex<rusqlite::Connection>>,
    track_path: String,
    tag_name: String
) -> Result<(), String> {
    let conn = conn_state.lock().map_err(|e| e.to_string())?;
    // Delete from track_tags where track_path matches and source = "user"
    Ok(())
}

/// Suppresses an automatic tag (from analysis or file metadata)
#[tauri::command]
fn suppress_tag(
    conn_state: tauri::State<'_, std::sync::Mutex<rusqlite::Connection>>,
    track_path: String,
    tag_name: String
) -> Result<(), String> {
    let conn = conn_state.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT OR IGNORE INTO user_suppressed_tags (track_path, tag_name) VALUES (?1, ?2)",
        rusqlite::params![track_path, tag_name]
    ).map_err(|e| e.to_string())?;
    Ok(())
}

/// Unsuppresses a tag (restores its auto-detected behavior)
#[tauri::command]
fn unsuppress_tag(
    conn_state: tauri::State<'_, std::sync::Mutex<rusqlite::Connection>>,
    track_path: String,
    tag_name: String
) -> Result<(), String> {
    let conn = conn_state.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "DELETE FROM user_suppressed_tags WHERE track_path = ?1 AND tag_name = ?2",
        rusqlite::params![track_path, tag_name]
    ).map_err(|e| e.to_string())?;
    Ok(())
}
```

---

## 4. Frontend Integration: Dedicated Edit Modal & Tag Controls

Instead of modifying `TrackDetailPane.svelte` to contain complex inline input fields and toggles, we keep it clean and read-only. We delegate editing logic to a separate component: `EditTrackModal.svelte`.

### A. Modifications & Visual Cues in `TrackDetailPane.svelte`
- **Edit Trigger**: Add a clean, styled button: `[ Edit Tags ]` at the top of the detail pane.
- **State Management**:
  - Define a reactive modal trigger: `let showEditModal = $state(false);`
  - Render the modal conditionally:
    ```svelte
    {#if showEditModal}
      <EditTrackModal 
        track={track} 
        onclose={() => showEditModal = false} 
      />
    {/if}
    ```
- **Subtle Visual Cues for Overridden Fields**:
  To ensure the user is aware of manual modifications without breaking the clean aesthetic:
  1. **Accent Highlight**: Fields that have been overridden (e.g. if `track.is_bpm_overridden` is true) will have their value rendered in a subtle accent color (e.g. `--sg-secondary` or a soft amber highlight like `rgba(255, 165, 0, 0.85)`).
  2. **Pencil Indicator**: Display a tiny pencil icon (✏️ or a SVG pen) next to the overridden tag label/value.
  3. **Hover Tooltips**: When hovering over the overridden value, display a tooltip showing the original auto-detected value (e.g. `Auto-detected: 124 BPM`).
  4. **Quick Revert ↺**: Render a tiny, low-opacity revert button (↺) next to the value on hover. Clicking this button immediately triggers the `remove_track_override` IPC command for that field and refreshes the state.

- **Interaction with Tags in the Detail Pane**:
  - Automatically-added tags that have been suppressed (`tag.discard` evaluates to `true`) will render with a strikethrough styling and lower opacity (already supported via `.tag-discarded`).
  - Clicking on a tag in the detail pane:
    - If it's a normal active tag: Toggles the search filter for that tag.
    - We can add a right-click context menu (or a small context action button on hover) to **"Suppress tag"** or **"Remove custom tag"**, which calls the corresponding IPC command directly.
    - To **restore a suppressed tag**, clicking on the empty tag container area (outside any individual active tag chip) will show a dropdown menu listing the currently suppressed tags for this track, allowing the user to select one and restore it (unsuppress it).

### B. Designing `EditTrackModal.svelte` (New Component)
Create a clean modal component `src/lib/components/EditTrackModal.svelte`:

1. **Local State**:
   - Bind inputs to a reactive draft object `editFields` populated from the current track:
     ```ts
     let { track, onclose } = $props();
     let editFields = $state({
       title: track.title ?? "",
       artist: track.artist ?? "",
       album: track.album ?? "",
       genre: track.genre ?? "",
       year: track.year ?? null,
       bpm: track.bpm ?? null,
       lyrics: track.lyrics ?? "",
       comment: track.comment ?? "",
       key: track.key ?? "",
       scale: track.scale ?? "",
       track_number: track.track_number ?? null,
       track_total: track.track_total ?? null,
       disc_number: track.disc_number ?? null,
       disc_total: track.disc_total ?? null,
       album_artist: track.album_artist ?? "",
       composer: track.composer ?? "",
       is_music: track.is_music ?? null,
       description: track.description ?? ""
     });
     ```
   - Manage user-added custom tags and suppressed tags in a list inside the modal:
     - Allow users to type new tags (with autocomplete against `library.allTags`) and click "Add Tag".
     - Display all active tags for the track, with a small `[x]` next to them. Clicking `[x]` on an automatic tag suppresses it, while clicking `[x]` on a user-defined tag removes it.

2. **Form Layout**:
   - Organized fields in a scrollable container inside a modal overlay (matching the dark glassmorphic design theme).
   - Use standard controls for text/number inputs, selects, and textareas.

3. **Reversion Control inside Modal**:
   - Next to each input, if the track has a field marked as overridden (e.g., `track.is_title_overridden`), show a `[Revert]` or `↺` button.

4. **Saving changes**:
   - Clicking "Save" triggers:
     - Invoking Tauri IPC command `save_track_override` with the draft `editFields`.
     - Refreshing the libraries via `await library.fetchTracks()`.
     - Updating `player.selectedTrack` so the `TrackDetailPane` automatically reflects updates.
     - Calling `onclose()`.

---

## 5. Portability & Sidecar Integration

We want user overrides, custom tags, and tag suppressions to survive folder movements and be shareable across machines.

### Sidecar Format (`.dc.json`)
Add a dedicated `user_overrides` field inside `SidecarData` (`src-tauri/src/scanner/sidecar.rs`):
```json
{
  "version": 1,
  "pass_versions": { ... },
  "pass_run_times": { ... },
  "ml_metadata": { ... },
  "user_overrides": {
    "title": "My custom title",
    "artist": "My custom artist",
    "bpm": 124.0,
    "track_number": 3,
    "track_total": 12,
    "disc_number": 1,
    "disc_total": 1,
    "album_artist": "Various Artists",
    "composer": "Composer Name",
    "is_music": 1,
    "description": "Custom track description"
  },
  "user_tags": ["genre:my-custom-synthwave", "vibe:warm-glow"],
  "suppressed_tags": ["mood:aggressive", "genre:rock"]
}
```

### Scanner & Restore Logic
1. **Sidecar Save**:
   - In `sidecar::save()`, fetch any rows in `user_track_overrides` for that track path and write them to the JSON file under `"user_overrides"`.
   - Fetch any custom tags (`source = 'user'`) linked to this track and write them under `"user_tags"`.
   - Fetch any suppressed tags in `user_suppressed_tags` and write them under `"suppressed_tags"`.
2. **Sidecar Restore**:
   - In `sidecar::restore()`, if the `"user_overrides"` field is present, parse it and transactionally insert/update it into the `user_track_overrides` table.
   - If `"user_tags"` is present, create/link them in `track_tags` with `source = 'user'`.
   - If `"suppressed_tags"` is present, parse and insert them into the `user_suppressed_tags` table.
