# Deep Cuts Improvement Checklist

This checklist captures concrete follow-up work found during the repository review. Keep items small enough to tackle one by one.

## Correctness & Test Health

- [x] Add `@types/node` or adjust `tsconfig.json` so `npm run check` has zero warnings.
- [x] Add regression tests for `musicOnly` using `detected_genre.startsWith("Non-Music")` as the source of truth.
- [x] Add tests for malformed `waveform_data` so the track list cannot crash on corrupt JSON.
- [x] Add map command tests proving exposed projection parameters are either honored or intentionally ignored.

## Analysis Pipeline Reliability

- [x] Mark CLAP preprocessing failures as `FAILED` instead of only logging them; failed prep jobs are currently moved to `IN_PROGRESS` without a consumer sentinel.
- [x] Replace `unwrap()` calls in long-running analysis worker paths with recoverable error handling where a poisoned lock or bad row should not kill the whole phase.
- [x] Make all phase-level early returns emit enough state for the UI to show a specific failure reason, not just `analysis-complete`.
- [x] Add a user-facing recovery action for stuck `IN_PROGRESS` rows beyond implicit reset on the next run.
- [x] Review pass reset behavior so each reset clears all derived outputs owned by that pass, including vector-table orphans.

## Analysis Pass Code Organization

- [ ] Split `src-tauri/src/analysis.rs` into per-pass modules, keeping orchestration separate from pass implementation details.
- [ ] Introduce a single pass registry/spec for pass name, priority, version, dependencies, owned outputs, and reset behavior.
- [ ] Extract shared `track_passes` lifecycle helpers for backfill, stale-version invalidation, pending-job loading, in-progress marking, DONE/FAILED updates, progress events, and sidecar saves.
- [ ] Move sidecar field ownership closer to pass definitions so adding a new pass does not require hand-updating several unrelated SQL statements.
- [ ] Standardize pass job structs and result persistence paths so CLAP, Qwen, Essentia, BPM correction, and description embedding follow the same shape where practical.

## Music Map Quality

- [ ] Replace min/max projection normalization with percentile-clipped normalization, probably p1-p99.
- [x] Either hide projection controls whose parameters are ignored, or implement the requested algorithm/parameter behavior.
- [ ] Add PCA as a fast deterministic projection option and consider making it the default if it improves global structure.
- [ ] Persist map settings such as algorithm, blend weight, and normalization percentile in `app_settings`.
- [ ] Exclude or separately handle non-music tracks during projection so spoken-word/outlier content does not distort the map.
- [ ] Prototype or implement outlier satellite regions after normalization is fixed.

## Embedding & Similarity Quality

- [x] Add base-DSP silence detection storing silence regions and a `has_long_silence` flag.
- [x] Replace fixed CLAP windows at 25/50/75% with waveform-based loudest-window selection.
- [x] Version and re-run the CLAP pass when waveform-based windowing lands.
- [x] Implement blended acoustic/semantic similarity using CLAP plus description embeddings.
- [x] Add graceful fallback behavior for tracks missing description embeddings.

## User-Facing Discovery Features

- [ ] Implement local semantic search over `description_embeddings`.
- [ ] Add match scores or rank badges for semantic search results.
- [ ] Add a "Sounds vs Feels" slider in the detail/player area for similarity recommendations.
- [ ] Add duplicate/remix detection using CLAP similarity thresholds and title/artist heuristics.
- [ ] Add pathfinding playlists after map quality is improved.

## Frontend Robustness & UX

- [x] Move `JSON.parse(track.waveform_data)` out of the Svelte render path and use a safe helper/cache.
- [x] Add error toasts for failed similarity searches instead of silently clearing loading state.
- [x] Keep selected/playing track visible when filters change, or explicitly show when playback is outside the current filter set.
- [ ] Review icon-only and inline-SVG buttons for consistency with the app design system.
- [x] Add focused component tests around `TrackList`, `MusicMap`, and `AnalysisPanel` behavior.

## Build & Configuration

- [x] Define a `coreml` Cargo feature or remove the stale `#[cfg(feature = "coreml")]` branches.
- [x] Fill in `authors` and `repository` metadata in `src-tauri/Cargo.toml`.
- [x] Decide whether `ort` should keep `download-binaries` for production builds or vendor/runtime-check model dependencies explicitly.
- [x] Add a documented model path setting UI if the intended `app_settings.model_path` flow is still desired.
