# C2: Fix Batch Pass Status Semantics

Source: [codebase-improvements.md](codebase-improvements.md)

## Problem

`structure_cluster` has two risky states:

- It can return early when too few tracks are available without making progress on `track_passes`.
- It can mark all `structure_cluster` pass rows done, even rows outside the loaded/applicable set.

`run_batch_pass()` currently ignores `run_id`, emits only phase events, and leaves status/metrics policy to each pass.

## Goal

Make batch-pass completion first-class. The helper or trait should standardize:

- success, skip, and failure status writes;
- metrics logging using `run_id`;
- coarse pause/cancel checkpoints;
- behavior for "not enough data" and "not applicable" tracks.

## Files to touch

- `src-tauri/src/analysis/mod.rs` — `run_batch_pass()`, `BatchAnalysisPass` trait
- `src-tauri/src/analysis/` — `structure_cluster` pass (and any other `BatchAnalysisPass` impls)
- `src-tauri/src/commands/analysis.rs` — verify IPC surface stays consistent

## Notes

This requires understanding current `structure_cluster` semantics and agreeing on the right skip/done/fail states before writing code. Expect back-and-forth.
