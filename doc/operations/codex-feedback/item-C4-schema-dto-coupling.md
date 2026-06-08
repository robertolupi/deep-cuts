# C4: Reduce Schema and DTO Coupling

Source: [codebase-improvements.md](codebase-improvements.md)

## Problem

`Track` is very wide and is mapped with large positional `SELECT` lists in `src-tauri/src/database.rs`. Recent schema work added fields such as `structure_cluster_id`, and the frontend type boundary can drift. A new analysis column can break unrelated queries or silently disappear at the frontend boundary.

## Goal

1. Add migration invariant tests asserting expected columns, indexes, and virtual tables after all migrations run.
2. Introduce smaller query-specific Rust DTOs for list rows, detail rows, map rows, and analysis rows.
3. Keep one canonical TypeScript type source for IPC response DTOs.

## Files to touch

- `src-tauri/src/database.rs` — `Track` struct, `SELECT` queries
- `src-tauri/src/commands/` — commands that map `Track` to IPC responses
- `src/lib/types.ts` — TypeScript DTO alignment
- New test module for migration schema invariants

## Notes

This is a refactor with a real blast radius. Agree on the DTO split strategy before touching queries. The migration invariant tests are lower risk and can be done first as a standalone sub-task.

## Implemented outcome

Scope landed: goals **1** (migration invariant tests) and the de-coupling half of **2**, keeping one canonical `Track`/`types.ts` (goal **3**). The thin-list / wide-detail DTO split was **deferred** — `get_tracks` returns the full wide `Track` and `TrackDetailPane.svelte` reads heavy fields (waveform/sax) straight out of that in-memory list, so a true split forces a frontend detail-fetch rework whose payoff isn't yet measured. Map/analysis rows already use narrow DTOs (`MappedTrackPoint` in `commands/map.rs`, `{id, watched_directory_id}` in `statistics.rs`), so those paths were left as-is.

What changed in `src-tauri/src/database.rs`:

- The three hand-aligned positional `SELECT … row.get(N)` blocks (`find_all`, `find`, and the CRUD test) were replaced by a single `db_row_mapping!(Track { … })` macro that generates `Track::COLUMN_LIST` (canonical `SELECT` columns) and `Track::from_row` (maps **by column name**). Adding a column is now two compiler-checked edits — the struct field and the macro list — with no positional index to keep aligned. A name in only one of the two places fails to compile, so drift is caught at build time.
- `find_all`/`find` build their SQL from `COLUMN_LIST.join(", ")` and map with `Self::from_row`.
- New `database::tests` migration-invariant tests assert expected tables, indexes, and virtual tables (vec0/fts5) exist after all migrations, plus that every `Track`-mapped column exists on the `tracks` table (subset check — the table legitimately has unmapped columns like `bpm_raw`, `onsets`, `cover_art`).

**Rejected approach — `serde_rusqlite`:** would have collapsed the mapping to one place (the struct alone), but every published version pins an incompatible `rusqlite` (`0.39.x`→`0.36`, `0.43`→`0.40`; ours is `0.38`), and `libsqlite3-sys`'s `links = "sqlite3"` allows only one copy. Adopting it would force bumping the whole rusqlite/migration/sqlite-vec stack plus the MSRV — disproportionate to this refactor. The dependency-free macro was chosen instead.

Skill docs updated: `skills/db-migration/SKILL.md` and `skills/add-analysis-pass/SKILL.md` now describe adding a `tracks` column via the macro list.

## Follow-up: thin-list / wide-detail DTO split (deferred)

**Status:** not started — waiting on in-progress frontend refactoring, which changes the same data-flow this split depends on.

Goal **2**'s remaining piece: split the wide `Track` into a thin `TrackListRow` (no heavy blobs — `waveform_data`, `waveform_sax`, `sax_alignment*`, `silence_regions`, `lyrics`) returned by `get_tracks`, and a wide `TrackDetail` returned by `get_track` on selection.

Why it's deferred (not just sequencing):

- Today `get_tracks` returns the full wide `Track[]`, the store holds it as one `tracks` array (`src/lib/stores/library.svelte.ts`), and `TrackDetailPane.svelte` reads heavy fields **straight out of that in-memory array** — there is no separate detail fetch. A real split therefore forces a frontend change (fetch detail on row selection), which collides with the parallel frontend refactor.
- The payoff (smaller list payload) is currently **unmeasured**. Before doing this, measure the `get_tracks` payload size / deserialization cost on a realistic library; if it isn't a real problem, this can stay closed.

When unblocked, the macro from this pass makes the Rust side cheap: define `TrackListRow` with its own `db_row_mapping!` list (subset of `Track`'s columns) and a matching `from_row`; point `get_tracks` at it. `Track`/`get_track` stay as the detail path. Re-check this against the post-refactor frontend before starting.
