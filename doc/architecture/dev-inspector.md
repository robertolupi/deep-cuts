# Dev Inspector — Design Doc

A debug-only HUD + slide-in drawer for inspecting live app state.
Compiled out of release builds entirely (`import.meta.env.DEV` gates all markup and imports).

---

## HUD (always visible in dev)

A compact pill in the navbar, to the right of the "DEEP CUTS" wordmark and left of the
existing dev-status badge. Always reactive, no click required to update.

```
[ 1891 tracks | 47 shown | ▶ Inner Fire | analysis: 12 pending | scan: idle ]
```

| Segment | Source | Notes |
|---|---|---|
| `N tracks` | `library.trackCount` | Total in DB |
| `N shown` | `filters.filteredTracks.length` | Post-filter count |
| `▶ title` | `player.selectedTrack?.title` | Truncated to 20 chars; hidden when null |
| `analysis: N pending` | `invoke('get_pass_stats')` → sum of `pending` | Polled every 5s when analysis is running; hidden when 0 |
| `scan: idle / scanning` | `library.isScanning` | Hidden when idle |

Clicking anywhere on the pill opens the drawer.

---

## Drawer

Slides in from the right, 420px wide, full height, `z-index: 500` (above content, below toasts).
ESC or a click on the backdrop closes it.

Header bar: `⚙ Dev Inspector` + `[Dump to console]` button + `[×]` close.

Four collapsible panes, all open by default:

---

### Pane 1 — Filters

Live view of every active filter field. Fields at their default value are dimmed (opacity 0.4)
so active filters stand out immediately.

```
searchQuery       "inner fire"
semanticQuery     ""
clapQuery         ""
genreFilter       ""
bpm               20 – 250
keys              []
scale             all
vocalFilter       all
musicOnly         false
selectedTags      []
selectedDirs      [3]
similarToTrack    null
similarBlend      0.5
─────────────────────────
filteredTracks    47 / 1891
semanticIds       0
clapIds           0
```

Source: directly read from `filters.*` — all already reactive, no polling needed.

---

### Pane 2 — Current Track

Full field dump of `player.selectedTrack`. Shown as a two-column key/value table.
Long strings (path, waveform_data, enriched_metadata) are truncated with a "copy" button
that puts the full value on the clipboard.

Key fields highlighted at the top (always visible even if others are collapsed):

```
id          1042
title       Inner Fire
artist      Downspiral
bpm         138.2
key / scale A / minor
duration    214s
acoustid    found
waveform_sax  aabcddeeeedcba
```

Below that: all remaining fields in a scrollable table.

Playback sub-row (from `player`):
```
isPlaying    true
currentTime  1:23 / 3:34
```

---

### Pane 3 — Analysis Pipeline

Table of pass stats, refreshed every 3s (same data as Analysis panel but more compact).
Only passes with `pending > 0` or `failed > 0` are shown in bold; zero-activity passes are dimmed.

```
Pass              pending  in_progress  done   failed  avg_ms
audio_analysis        0          0      1891      0     412
clap                  0          0      1891      0    1840
sax                  12          1      1878      0      88
essentia              0          0      1891      3     220
…
```

A `[Recover stuck]` button calls `invoke('recover_stuck_passes')`.

---

### Pane 4 — Library / Scan

```
tauriConnected      true
trackCount          1891
tracks loaded       1891
directories         3
isScanning          false
scanProgress        100%
scanCurrentFile     —
analysisRunning     true
analysisPaused      false  (manual: false, auto: false)
```

---

### Pane 5 — Raw SQL (current track)

Available only when `player.selectedTrack` is non-null. Shows the raw database rows for that
track across every relevant table. A new **`debug_track_raw`** IPC command fetches all of
it in one round-trip.

#### Tables queried (all keyed on `track_id = ?`)

| Query | Why useful |
|---|---|
| `SELECT * FROM tracks WHERE id = ?` | Every column including large blobs (waveform_data, cover_art, enriched_metadata) |
| `SELECT * FROM track_passes WHERE track_id = ?` | Pass status, version, duration, raw_result per pass |
| `SELECT * FROM track_coords WHERE track_id = ?` | UMAP/PCA x,y coordinates |
| `SELECT t.name, tt.score, tt.discard FROM track_tags tt JOIN tags t ON tt.tag_id = t.id WHERE tt.track_id = ?` | All tags with scores |
| `SELECT tag_name FROM user_suppressed_tags WHERE track_path = ?` | User suppressions |
| `SELECT id, title, created_at FROM chat_sessions WHERE track_id = ?` | Chat sessions for this track |

`audio_embeddings` and `description_embeddings` are **excluded** — 512-float and 384-float
blobs are not human-readable and would swamp the display. The console dump includes them
as `Float32Array` for anyone who wants to inspect them in JS.

#### Display layout

Each table is a collapsible sub-section within Pane 5, collapsed by default except `tracks`
and `track_passes` which are open. Layout is the same `DevKV` two-column table used in
Pane 2, with the same truncation + copy-button treatment for long strings.

Special handling for `tracks` columns:
- `waveform_data` — show length (e.g. `128 bins`) + first 8 values as a sparkline; copy button for full JSON
- `waveform_sax` — show full string (32 chars, short enough)
- `cover_art` — show `N bytes` + a 32×32 thumbnail rendered inline via `URL.createObjectURL`
- `enriched_metadata` — pretty-print JSON, collapsed to 3 lines with a "show all" toggle

`track_passes` rows are shown as a table (not key/value) since there are multiple rows:

```
Pass             status  ver  pending  duration_ms  raw_result
audio_analysis   done    3    —        412          —
clap             done    7    —        1840         —
sax              done    1    —        88           —
essentia         failed  2    —        220          {"error": "..."}
```

`raw_result` for failed passes is shown in full (it usually contains the error message).

#### IPC command: `debug_track_raw`

New Tauri command, **debug-only** (`#[cfg(debug_assertions)]`), returns a single JSON object:

```rust
#[derive(serde::Serialize)]
struct DebugTrackRaw {
    track: serde_json::Value,          // full tracks row as object
    passes: Vec<serde_json::Value>,    // all track_passes rows
    coords: Option<serde_json::Value>, // track_coords row or null
    tags: Vec<serde_json::Value>,      // joined tag rows with score/discard
    suppressions: Vec<String>,         // suppressed tag names
    chat_sessions: Vec<serde_json::Value>,
}
```

Uses `rusqlite`'s row-to-JSON approach (iterate column names + values dynamically) so it
doesn't need to be updated when new migrations add columns — it always reflects the live
schema.

#### Refresh behaviour

Pane 5 fetches on open and has a `[Refresh]` button. It does **not** auto-poll — raw SQL
data only changes after an analysis pass completes, and the user can refresh manually.
The `track-enriched` Tauri event (emitted by AcoustID) triggers an automatic re-fetch.

---

## Console dump

`[Dump to console]` calls `console.group('Deep Cuts state snapshot')` and logs:

```js
console.log('filters',  { searchQuery, semanticQuery, ... })
console.log('player',   { selectedTrack, isPlaying, currentTime, duration })
console.log('library',  { trackCount, tracks: library.tracks.length, isScanning, ... })
console.log('ui',       { activeView, sidebarTab, ... })
console.log('analysis', passStats)   // fetched fresh via invoke
console.groupEnd()
```

`library.tracks` (full array, potentially 1891 objects) is intentionally included — it's
the most useful thing to have in the console for ad-hoc JS queries like
`tracks.filter(t => !t.artist).length`.

---

## Implementation notes

### Component layout

```
src/lib/components/dev/
  DevHud.svelte          — pill widget, imported into Navbar.svelte
  DevDrawer.svelte       — full drawer, imported into +layout.svelte (so it's always mounted)
  DevPane.svelte         — collapsible pane wrapper (title + chevron + slot)
  DevKV.svelte           — two-column key/value table row component
```

Importing into `+layout.svelte` rather than `Navbar.svelte` means the drawer can be full-height
without fighting the navbar's stacking context, and the component stays mounted (preserving
collapse state) even if the navbar re-renders.

All four files are wrapped in `{#if import.meta.env.DEV}` at the top level; Vite tree-shakes
them from the production bundle.

### Polling for pass stats

`DevDrawer` polls `invoke('get_pass_stats')` every 3s via `setInterval` inside `$effect`,
cleaned up on destroy. Only when `library.analysisRunning` is true; otherwise shows the
last known snapshot with a "(paused)" label.

The HUD shows only the summed pending count, derived from the same polled data passed down
as a prop.

### Dev menu integration

The existing right-click dev menu on the wordmark gains a second item:
`Open inspector` → sets `devDrawerOpen = true` (a module-level `$state` in `DevDrawer.svelte`,
exported so `Navbar.svelte` can set it directly).

---

## What it does NOT do

- No state mutation from the drawer (read-only; use the console dump for that)
- No track list browsing (that's the main UI's job)
- No network panel / event log (out of scope; use Chrome DevTools for that)
- Not available in release builds
