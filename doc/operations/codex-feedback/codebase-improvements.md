# Codebase Improvements

Date: 2026-06-06

## Backend and Pipeline

### 1. Make analysis pass lifecycle invariants testable

The analysis pipeline has a good `PASS_REGISTRY` in `src-tauri/src/analysis/mod.rs`, but execution order is still encoded separately in `PipelineManager::run()`. The `add-analysis-pass` skill even warns that priority does not determine execution order.

Add tests that assert:

- Every `PASS_REGISTRY` entry is executed or intentionally excluded.
- Dependencies precede dependents in the concrete run order.
- Resetting a pass clears owned columns, owned tables, owned tags, and dependent passes.
- Batch passes update `track_passes` status consistently.

This directly addresses the recent pattern where a feature touches migrations, registry metadata, run order, sidecars, IPC registration, frontend fields, and docs in one commit.

### 2. Fix batch pass status semantics

`structure_cluster` has two risky states:

- It can return early when too few tracks are available without making progress on `track_passes`.
- It can mark all `structure_cluster` pass rows done, even rows outside the loaded/applicable set.

Make batch-pass completion first-class. `run_batch_pass()` currently ignores `run_id`, emits only phase events, and leaves status/metrics policy to each pass. The helper or trait should standardize:

- success, skip, and failure status writes;
- metrics logging using `run_id`;
- coarse pause/cancel checkpoints;
- behavior for "not enough data" and "not applicable" tracks.

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

## Frontend and IPC

### 5. Route all Tauri calls through `$lib/ipc`

`src/lib/ipc.ts` provides mock/local-debug support, but many components import `@tauri-apps/api/core` directly. That weakens browser-only UI debugging and tests.

Make `$lib/ipc` the only invoke/listen import for app code, except for APIs it intentionally re-exports such as `convertFileSrc` if needed.

Next step:

- Define a `CommandMap` type from command name to args/result.
- Type `invoke` as `invoke<K extends keyof CommandMap>(cmd: K, args: CommandMap[K]["args"])`.
- Require each new IPC command to update the wrapper, mock response when applicable, and at least one frontend test or store test.

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

**Progress (2026-06-07, commit `feae60d`):** 18 of 32 components fixed. 14 remaining: CollapsiblePane, DuplicatesPanel, FilterSidebar, MoodRadar, MusicMap, NetworkSettingsCard, PlayerBar, RangeSlider, SettingsCard, StatisticsPanel, DevKV, DevPane, +layout.svelte.

The Sonic Glitch token system is strong, but components still contain hardcoded hex/RGBA colors and inline styles. This undermines light and accessible themes.

Update code and skills so new UI work requires:

- no inline color styles;
- no component-specific hardcoded color literals unless explicitly mapped to a design token;
- canvas palettes read from computed CSS variables;
- high-contrast checks for new controls.

**D3.js caveat:** D3 canvas/SVG rendering cannot consume `var(--sg-*)` CSS variables directly. The correct workaround is to resolve tokens at render time via `getComputedStyle(document.documentElement).getPropertyValue('--sg-primary').trim()`. Do not replace D3 hex literals with raw hex — replace them with `getComputedStyle` lookups so theme-switching still works. This also means canvas components must re-read colors on theme change (listen for `html[data-theme]` attribute mutations or re-render on a theme store change).

## Testing Priorities

1. Rust: pass registry/order/reset invariant tests.
2. Rust: migration schema tests for all current columns/tables/indexes.
3. Rust: batch-pass skip/status tests, especially `structure_cluster`.
4. Frontend: typed IPC wrapper tests with local-debug mocks.
5. Frontend: `FilterSidebar`, `TrackDetailPane`, `MusicMap`, `AnalysisPanel`, and update-banner workflow tests.
6. End-to-end smoke: launch frontend in `?local_debug=1` and verify table/map/settings/analysis/chat surfaces render without Tauri.
