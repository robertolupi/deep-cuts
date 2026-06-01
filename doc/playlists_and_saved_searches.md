# Playlists & Saved Searches

## Two Concepts

### Saved Searches (Smart Playlists)

A saved search captures the current filter state — keyword, tags, mood radar profile, key, BPM range, folders, etc. — as a named, persistent query. Every time it is opened, it re-runs against the current library and returns fresh results.

Examples:
- "Upbeat electronic, no vocals, 120–140 BPM"
- "Tracks missing genre tag"
- "References folder, mood: aggressive > 0.7"
- "Not yet analysed"

The filter state is serialised to JSON and stored in the database. The query engine re-evaluates it on demand — there is no stored list of track IDs, so the results update automatically as the library changes (new tracks added, analysis completes, tags edited).

### Playlists (Static)

A manually curated, ordered list of track IDs. The user explicitly adds and removes tracks and can reorder them via drag-and-drop. Results do not change unless the user edits them. Tracks removed from the library are shown as stale/missing rather than silently dropped.

---

## Data Model

### `saved_searches`

```sql
CREATE TABLE saved_searches (
    id          INTEGER PRIMARY KEY,
    name        TEXT NOT NULL,
    query_json  TEXT NOT NULL,   -- serialised filter state
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL
);
```

`query_json` mirrors whatever the frontend filter store serialises — keyword, tag expressions, mood profile vertices, BPM range, key, folder IDs, etc. The backend re-executes it as a SQL query with fuzzy scoring applied in application code.

### `playlists`

```sql
CREATE TABLE playlists (
    id          INTEGER PRIMARY KEY,
    name        TEXT NOT NULL,
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL
);
```

### `playlist_tracks`

```sql
CREATE TABLE playlist_tracks (
    playlist_id  INTEGER NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,
    track_id     INTEGER NOT NULL REFERENCES tracks(id)    ON DELETE SET NULL,
    position     INTEGER NOT NULL,
    PRIMARY KEY (playlist_id, position)
);
```

`track_id` is set to NULL (rather than cascaded) when a track is removed from the library, so the playlist preserves the slot and can show a "missing" indicator.

---

## UI

### Sidebar

Both playlists and saved searches appear in the left sidebar under a **Playlists** section, below the filter controls. They are visually distinguished:

- **Saved search** — icon: funnel/magnifier. Clicking runs the query and populates the track list. An "Edit" button reopens the filter sidebar pre-populated with the saved state.
- **Playlist** — icon: ordered list. Clicking shows the static track list in a playlist view mode with drag-to-reorder.

Right-click on either opens a context menu: Rename, Duplicate, Delete.

### Adding to a playlist

- From the track list or map: right-click a track → "Add to playlist…" → picker
- From the player bar: a "+" button next to the track title
- Multi-select in the track list → right-click → "Add to playlist…"

### Saving a search

- A "Save Search…" button appears in the filter sidebar header whenever any filter is active
- Prompts for a name, then saves the current filter state

### Mood profile presets

Named radar profiles (from `mood_filtering_ideas.md`) are a specialised form of saved search — they can be stored in `saved_searches` with a `type: 'mood_profile'` field in `query_json`, or in a separate table if they need distinct UI treatment.

---

## Relationship to Tags

A saved search can filter by tag, making tags and saved searches composable:

- Tag a set of tracks `todo-edit` → saved search "Needs editing" filters on that tag
- A mood radar preset saved as a search effectively becomes a dynamic mood-based playlist

This avoids needing a separate "smart playlist" concept — saved searches *are* smart playlists.

---

## Open Questions

1. **Playlist playback order** — should playlists integrate with the player (next/previous navigates the playlist) or remain a library view only for now?

2. **Export** — M3U export of playlists is a natural ask. Low complexity, high compatibility with other players.

3. **Collaborative / sync** — out of scope for a local app, but worth noting that the data model doesn't preclude it.

4. **Search query versioning** — if the filter schema changes (new filter added, mood dimensions renamed), saved searches with old `query_json` may become invalid. Need a migration strategy or a schema version field in `query_json`.

---

## Cross-References

- **Tag system** (`tagging_ideas.md`) — saved tag queries and saved searches are the same concept at different levels. A tag expression (`mood:happy instrument:guitar`) saved as a search is a smart playlist; the two systems should share the same persistence model.
- **Statistics page** (`statistics_page.md`) — saved searches and playlists are the primary input for the statistics comparison model. The overlap/similarity section directly suggests tracks to add to a playlist based on nearest-neighbour distance across sets.
- **Map layouts** (`map_layouts.md`) — a playlist or saved search can be highlighted as an overlay on any map layout, revealing its spatial coherence and exposing acoustically adjacent tracks that aren't yet in the set.
