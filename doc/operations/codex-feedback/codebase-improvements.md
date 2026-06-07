# Codebase Improvements

Date: 2026-06-06

## Backend and Pipeline

### 1. Fix batch pass status semantics

`structure_cluster` has two risky states:

- It can return early when too few tracks are available without making progress on `track_passes`.
- It can mark all `structure_cluster` pass rows done, even rows outside the loaded/applicable set.

Make batch-pass completion first-class. `run_batch_pass()` currently ignores `run_id`, emits only phase events, and leaves status/metrics policy to each pass. The helper or trait should standardize:

- success, skip, and failure status writes;
- metrics logging using `run_id`;
- coarse pause/cancel checkpoints;
- behavior for "not enough data" and "not applicable" tracks.

### 2. Stop swallowing SQLite row errors

Production code frequently uses `filter_map(|r| r.ok())` on SQLite row iterators. That hides schema drift, corrupt rows, and mapping bugs. Replace it with `collect::<Result<Vec<_>, _>>()` or explicit logging and error propagation.

High-priority areas:

- analysis batch loaders such as `sax`, `sax_alignment`, `structure_cluster`;
- commands returning UI data such as `structure`, `statistics`, `metrics`, `playlists`;
- scanner sidecar restore/export code.

Tests should include at least one deliberate malformed row or incompatible query shape to prove failures are visible.

### 3. Harden startup failure behavior

`src-tauri/src/lib.rs` logs main DB initialization failures but continues setup. Later IPC commands expect managed DB state to exist, so a startup problem can become delayed and confusing runtime failures.

Return a setup error if the main database cannot initialize. Metrics can remain optional, but it should enter an explicit degraded state that the UI and logs can distinguish from normal operation.

### 4. Reduce schema and DTO coupling

`Track` is very wide and is mapped with large positional `SELECT` lists in `src-tauri/src/database.rs`. Recent schema work added fields such as `structure_cluster_id`, and the frontend type boundary can drift.

Recommended path:

- Add migration invariant tests asserting expected columns, indexes, and virtual tables after all migrations.
- Introduce smaller query-specific Rust DTOs for list rows, detail rows, map rows, and analysis rows.
- Keep one canonical TypeScript type source for IPC response DTOs.

This reduces the risk that a new analysis column breaks unrelated queries or silently disappears at the frontend boundary.

### 5. Make manifest/download errors explicit

Manifest and model download code swallows or collapses several failure modes into generic errors. Downloads also run blocking `ureq` IO inside async flow.

Improve this by:

- validating requested model group keys before download starts;
- emitting structured event payloads for network, cache, parse, resume, checksum, and filesystem failures;
- moving large blocking IO into `spawn_blocking` or a dedicated blocking worker;
- adding tests for cache write failure, invalid group key, interrupted resume, and manifest parse fallback.

## Frontend and IPC

### 6. Split oversized UI surfaces

The largest Svelte files mix data fetching, derivation, rendering, canvas/SVG logic, styling, and IPC:

- `TrackDetailPane.svelte`
- `MusicMap.svelte`
- `FilterSidebar.svelte`
- `AnalysisPanel.svelte`
- `ChatPanel.svelte`
- `StatisticsPanel.svelte`

Start with `FilterSidebar` and `filters.svelte.ts`. Extract pure modules for filter application, saved-search serialization, structure matching, sorting, and semantic/CLAP result reduction. Pure modules are cheaper to test and reduce Svelte rune coupling.

### 7. Fix store lifecycle coupling

`library.init()` registers listeners but does not retain unlisten functions. Repeated init calls can duplicate event handlers. The library store also mutates `player.selectedTrack` after enrichment, which creates hidden cross-store coupling.

Add:

- `initialized` guard;
- `dispose()` that calls every unlisten function;
- a dedicated track-refresh method that player/detail state can subscribe to or call explicitly.

### 8. Bring component CSS back to tokens

The Sonic Glitch token system is strong, but components still contain hardcoded hex/RGBA colors and inline styles. This undermines light and accessible themes.

Update code and skills so new UI work requires:

- no inline color styles;
- no component-specific hardcoded color literals unless explicitly mapped to a design token;
- canvas palettes read from computed CSS variables;
- high-contrast checks for new controls.

## Testing Priorities

1. Rust: migration schema tests for all current columns/tables/indexes.
2. Rust: batch-pass skip/status tests, especially `structure_cluster`.
3. Frontend: typed IPC wrapper tests with local-debug mocks.
4. Frontend: `FilterSidebar`, `TrackDetailPane`, `MusicMap`, `AnalysisPanel`, and update-banner workflow tests.
5. End-to-end smoke: launch frontend in `?local_debug=1` and verify table/map/settings/analysis/chat surfaces render without Tauri.
