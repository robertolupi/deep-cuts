# AcoustID Metadata Enrichment

## Goal

When a track is loaded (selected/played), automatically fingerprint it and query AcoustID + MusicBrainz to fill in missing metadata (title, artist, album, year, genre, cover art). The lookup is on-demand and lazy — it runs once per track and never repeats, even if it found nothing.

A global setting controls whether the feature is active, so users who want strictly local operation can opt out.

---

## Services Used

- **Chromaprint / `fpcalc`** — generates an acoustic fingerprint from the audio file. LGPL v2.1; bundled as a sidecar binary.
- **AcoustID API** — matches the fingerprint to a MusicBrainz recording ID. Free with a registered API key; no commercial restrictions. Rate limit: 3 req/sec.
- **MusicBrainz API** — resolves the recording ID to full metadata (title, artist, album, year, genre tags). CC0 data; free, no commercial restrictions. Rate limit: 1 req/sec.
- **Cover Art Archive** — MusicBrainz-affiliated service for album artwork. CC0. Queried by MusicBrainz release ID.

---

## Database Changes

### New column: `acoustid_status` on `tracks`

Tracks the state of the lookup for each track:

| Value | Meaning |
|---|---|
| `NULL` | Never attempted |
| `'pending'` | In-flight (crash recovery: treat as NULL on startup) |
| `'found'` | Lookup succeeded and data was written |
| `'not_found'` | Lookup ran but AcoustID returned no match |
| `'error'` | Network or API error — eligible for retry |

This column prevents redundant lookups and ensures own/obscure songs are not re-queried indefinitely.

### New column: `musicbrainz_id` on `tracks`

Stores the resolved MusicBrainz recording MBID (UUID string), for future use (e.g. linking out to MusicBrainz, fetching additional data later).

### New column: `enriched_metadata` on `tracks`

Stores the fetched metadata from MusicBrainz as a serialized JSON string. This stores the raw, rich fetched metadata (title, artist, album, year, genre, cover_art_url) to enable the comparison/diff UI, conflict resolution, and rollback capabilities without cluttering the local file tags.

### New column: `cover_art` on `tracks`

Stores the downloaded album artwork JPEG image as binary data (`BLOB`). A `NULL` value indicates that no cover art has been fetched or is available.

### New setting: `acoustid_enrichment_enabled`

Stored in `app_settings` (key/value table already used for `theme` and `model_path`). Default: `'true'`. When `'false'`, no fingerprinting or network requests are made.

### Migration

A new migration file: `migrations/NN_acoustid.sql`

```sql
ALTER TABLE tracks ADD COLUMN acoustid_status TEXT;
ALTER TABLE tracks ADD COLUMN musicbrainz_id TEXT;
ALTER TABLE tracks ADD COLUMN enriched_metadata TEXT;
ALTER TABLE tracks ADD COLUMN cover_art BLOB;
INSERT OR IGNORE INTO app_settings (key, value) VALUES ('acoustid_enrichment_enabled', 'true');
```

On startup, any rows with `acoustid_status = 'pending'` are reset to `NULL` (crash recovery).

---

## Backend: Rust

### Sidecar: `fpcalc`

`fpcalc` is Chromaprint's standalone binary. It is registered as a Tauri sidecar and bundled for macOS (arm64 + x86_64), with stubs for Windows/Linux for future use.

```toml
# tauri.conf.json
"bundle": {
  "externalBin": ["binaries/fpcalc"]
}
```

`fpcalc -json <path>` outputs `{ "duration": ..., "fingerprint": "..." }`.

### New module: `src-tauri/src/acoustid.rs`

Responsible for the full enrichment pipeline for a single track:

```
fpcalc → AcoustID API → MusicBrainz API → (Cover Art Archive) → DB write
```

Key function:

```rust
pub async fn enrich_track(track_id: i64, path: &str, force: bool, conn: &Mutex<Connection>) -> Result<()>
```

Steps:
1. Check `acoustid_enrichment_enabled` setting — return early if disabled (even if forced).
2. Check `acoustid_status` for this track — return early if not NULL or 'error', unless `force` is `true`.
3. Set `acoustid_status = 'pending'`.
4. Run `fpcalc -json <path>` via `std::process::Command`.
5. POST to `https://api.acoustid.org/v2/lookup` with fingerprint + duration + `meta=recordings+releasegroups+compress`.
6. If no results: set `acoustid_status = 'not_found'`, clear `enriched_metadata`, and return.
7. Pick the highest-confidence result. Extract recording MBID.
8. GET `https://musicbrainz.org/ws/2/recording/<mbid>?inc=artists+releases+tags&fmt=json`.
9. Serialize the response fields (title, artist, album, year, genre, cover_art_url) as JSON and store it in the `enriched_metadata` column.
10. Check for conflicts:
    - **No conflicts**: If all non-NULL fields in the fetched metadata match the existing track values, or if the existing track values are NULL, automatically write them to the track's columns and set `acoustid_status = 'found'`.
    - **Conflicts exist**: If any populated field in the fetched metadata differs from a non-NULL field in the existing database row, set `acoustid_status = 'conflict'`.
11. Handle cover art from Cover Art Archive:
    - **Current behavior**: If remote cover art is found, it **overrides** any existing cover art in the database (local embedded artwork is replaced by the high-quality fetched artwork).
    - **Future behavior**: Remote cover art will be treated as a conflict if different, prompting the user via the metadata diff dialog to choose between the local or fetched artwork.
12. Write `musicbrainz_id` to the database.
13. Emit a `track-enriched` Tauri event so the frontend can refresh.

### New IPC command: `enrich_track_metadata`

```rust
#[tauri::command]
async fn enrich_track_metadata(track_id: i64, force: Option<bool>, state: State<AppState>) -> Result<(), String>
```

Called by the frontend when a track is selected (normally `force: false`) or when the user manually clicks "Refresh/Identify Track" (with `force: true`). Runs `enrich_track` in a spawned async task so it doesn't block the UI. Silently no-ops if the setting is off or if the track was already processed and `force` is false.

### New Local Analysis Pass: `cover_art_extraction`

To prevent the initial folder/directory scanner from slowing down due to reading large, binary embedded image blobs, extracting embedded cover art is offloaded to a dedicated local background analysis pass:

* **Pass Name**: `'cover_art_extraction'`
* **Priority**: `25` (runs early in the background pipeline after basic audio analysis)
* **Execution Logic**:
  1. For each track, check if `cover_art` is already populated. If it is, skip.
  2. Parse the local audio file using our existing metadata parser (e.g. `lofty`).
  3. Extract any embedded cover/front artwork (JPEG/PNG).
  4. Write the binary image data directly to the track's `cover_art` column.
  5. If no embedded artwork is found, leave the column `NULL`. This signals to the online enrichment pipeline (`enrich_track_metadata`) that it is eligible for remote retrieval.

### Rate limiting

MusicBrainz enforces 1 req/sec; AcoustID allows 3/sec but MusicBrainz is the bottleneck. A `tokio::sync::Semaphore` with 1 permit plus a 1-second inter-request delay enforces this globally across all enrichment tasks.

This rate limit makes bulk/batch enrichment impractical — at 1 req/sec, a 2000-track library would take ~33 minutes minimum (two API calls per track). The on-demand-per-track approach is therefore the primary and intended mode. There is no "enrich whole library" batch action. Users naturally enrich tracks as they listen to them.

---

## Frontend

### Trigger: on track select (Automatic)

In `player.svelte.ts`, after `playTrack()` sets the selected track, invoke standard lazy-enrichment:

```ts
invoke('enrich_track_metadata', { trackId: track.id, force: false });
```

No await — fire and forget.

### Trigger: "Identify Track" button (Manual Force)

In the track details/inspector panel, add an "Identify Track" (or "Refresh Metadata") button. Clicking this button triggers the manual enrichment with the `force` parameter:

```ts
invoke('enrich_track_metadata', { trackId: track.id, force: true });
```

This bypasses standard restrictions, resetting the status to `'pending'` and forcing a re-lookup on the remote APIs. This is especially useful for newly released tracks or when improved metadata has recently been submitted to AcoustID/MusicBrainz.

### Refresh: `track-enriched` event

The backend emits `track-enriched` with `{ track_id }` after a successful enrichment. The frontend listens (in `library.svelte.ts`) and refreshes that single track's data from the DB, then updates the reactive store.

### Conflict resolution: toast + diff dialog

When enrichment completes for a track:

- **No conflicts** (all enriched fields were NULL): write silently, no UI shown.
- **Conflicts exist** (one or more enriched fields differ from existing DB values): show a toast — *"Updated metadata found for [Track Title]"* — with an "Review" button.

Clicking Review opens a **metadata diff dialog** modelled on diff3:

- Each conflicting field is shown as a row with two columns: **Current** (left) and **MusicBrainz** (right).
- Fields with no conflict are shown collapsed/greyed out but still visible.
- Each row has a toggle to select which value to keep (current or incoming), defaulting to current.
- Cover art is shown as two thumbnail images side by side if both exist.
- Buttons: **Keep All Current** / **Accept All Incoming** / **Apply Selection** (per-field merge).

The dialog can be dismissed entirely (keeping all current values). The toast auto-dismisses after ~8 seconds if ignored; the diff can be reopened from a "Pending metadata reviews" entry in the sidebar badge (if any reviews were deferred).

Pending conflicts are stored in a new `acoustid_status = 'conflict'` state so they survive app restarts and can be reviewed later.

### Settings panel

In the existing Settings view, add a toggle under a new "Network" section:

> **Fetch metadata from MusicBrainz**
> When enabled, Deep Cuts will fingerprint tracks and look up missing title, artist, album, and year from MusicBrainz. Runs once per track. No data is ever uploaded.

Backed by `get_setting` / `set_setting` IPC calls for the `acoustid_enrichment_enabled` key.

---

## What Gets Overwritten (and What Doesn't)

The enrichment is **additive and DB-only**:

- A field is only written if it is currently `NULL` in the database.
- Embedded ID3/Vorbis tags read at scan time always take precedence.
- Own songs that didn't match AcoustID get `acoustid_status = 'not_found'` and are never queried again.
- **Audio files are never modified.** Writing back to ID3/Vorbis tags is explicitly out of scope — it risks file corruption and would interfere with stale track detection (which uses file size and modification time).

---

## Open Questions

1. **AcoustID API key** — needs to be registered at acoustid.org and either hardcoded as a build-time constant or user-configurable. A hardcoded key for the app is normal practice; AcoustID's terms allow this.

2. **`fpcalc` binary distribution** — bundled as a Tauri sidecar (~1 MB per architecture). A pure-Rust Chromaprint alternative was considered but ruled out as insufficiently mature.

3. **Retry policy for `'error'` status** — on next launch? On next select? After N minutes? Suggest: retry once per app session, not per select, to avoid hammering the API on persistent network issues.

4. **User visibility** — should the UI indicate that a lookup is in progress (e.g. a subtle spinner in the player bar)? Or silently enrich in the background? Suggest: silent, since failures are harmless and success just makes fields appear.

---

## Cross-References

- **Tag system** (`tagging_ideas.md`) — MusicBrainz genre tags returned by AcoustID lookup can automatically seed the tag system as `source: 'musicbrainz'` entries, giving tracks a rich initial tag set before any local analysis runs. The tag system then becomes the unified query layer over MusicBrainz tags, Essentia tags, and user-defined tags simultaneously.
