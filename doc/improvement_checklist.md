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
- [x] Add `raw_result TEXT` column to `track_passes` and populate it with structured JSON in every pass: audio_analysis (BPM/key/loudness), bpm_correction and bpm_refinement (input + decision + rule), CLAP (window positions + embedding norm), essentia (genre top-3 with scores + patch count), description_embed (embedded text or skip reason), qwen (HTTP response + parse summary).
- [x] Fix library store not reloading tracks after analysis: add `analysis-phase-complete` listener to `LibraryStore` so extracted fields become visible in the UI without a manual refresh.
- [x] Robustify Qwen response parser to handle literal `\n` escape sequences (model emits backslash-n instead of real newlines) and semicolon-delimited field separators; both formats previously parsed only the first field.

## Analysis Pass Code Organization

- [x] Split `src-tauri/src/analysis.rs` into per-pass modules, keeping orchestration separate from pass implementation details.
- [x] Introduce a single pass registry/spec for pass name, priority, version, dependencies, owned outputs, and reset behavior.
- [x] Extract shared `track_passes` lifecycle helpers for backfill, stale-version invalidation, pending-job loading, in-progress marking, DONE/FAILED updates, progress events, and sidecar saves.
- [ ] Move sidecar field ownership closer to pass definitions so adding a new pass does not require hand-updating several unrelated SQL statements.
- [x] Standardize pass job structs and result persistence paths so CLAP, Qwen, Essentia, BPM correction, and description embedding follow the same shape where practical. Added `raw_result_json` trait method so the default `run_pass` loop writes raw_result and triggers `sidecar::save` for all passes; custom runners (audio, clap, essentia) handle both inline. Eliminated all hardcoded `pass_name = '...'` strings from persistence SQL.

## Music Map Quality

- [x] Replace min/max projection normalization with percentile-clipped normalization, probably p1-p99.
- [x] Either hide projection controls whose parameters are ignored, or implement the requested algorithm/parameter behavior.
- [x] Add PCA as a fast deterministic projection option and consider making it the default if it improves global structure.
- [ ] Persist map settings such as algorithm, blend weight, and normalization percentile in `app_settings`.
- [x] Exclude or separately handle non-music tracks during projection so spoken-word/outlier content does not distort the map.
- [ ] Prototype or implement outlier satellite regions after normalization is fixed.

## Embedding & Similarity Quality

- [x] Add base-DSP silence detection storing silence regions and a `has_long_silence` flag.
- [x] Replace fixed CLAP windows at 25/50/75% with waveform-based loudest-window selection.
- [x] Version and re-run the CLAP pass when waveform-based windowing lands.
- [x] Implement blended acoustic/semantic similarity using CLAP plus description embeddings.
- [x] Add graceful fallback behavior for tracks missing description embeddings.

## User-Facing Discovery Features

- [x] Implement local semantic search over `description_embeddings`.
- [x] Add match scores or rank badges for semantic search results.
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
