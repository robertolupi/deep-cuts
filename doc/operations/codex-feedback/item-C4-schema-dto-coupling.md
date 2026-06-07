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
