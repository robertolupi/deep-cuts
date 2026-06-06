# Codex Feedback Index

Date: 2026-06-06

This directory collects a codebase, skills, and docs review of Deep Cuts. The review used a local survey of the repository, recent git history, and three focused sub-agent reviews: Rust/Tauri backend, Svelte frontend, and skills/docs process.

## Files

- [codebase-improvements.md](codebase-improvements.md) — backend, frontend, testing, IPC, schema, and analysis-pipeline recommendations.
- [skills-improvements.md](skills-improvements.md) — project skill updates that would make future agent work more reliable.
- [docs-approach-improvements.md](docs-approach-improvements.md) — improvements to proposal docs, collaboration logs, and research-to-product workflow.
- [completed-improvements.md](completed-improvements.md) — completed feedback items moved out of the active backlog.

## Highest-Value Improvements

1. Add invariant tests around analysis pass registration, execution order, dependency order, reset behavior, and batch-pass status updates.
2. Standardize frontend Tauri access through `$lib/ipc` and make the IPC boundary typed by command name.
3. Stop silently dropping SQLite row-mapping errors with `filter_map(|r| r.ok())` in production paths.
4. Split the largest Svelte components and stores into smaller components plus pure tested helper modules.
5. Add lifecycle metadata to proposal docs: `status`, `owner`, `last_verified`, `implemented_by`, and `superseded_by`.
6. Add lightweight repo hygiene checks for docs, skills, IPC imports, and SQLite row-error swallowing.

## Git History Signals

Recent commits show features landing across many surfaces at once. For example, `47f82cb` added structure clustering and touched Rust analysis, command registration, docs, frontend stores/components, and the analysis-pass skill. `39cb908` added SAX alignment with migrations plus filters/detail UI. `4702f77` added HDBSCAN coloring while dropping `waveform_fingerprint`. This pattern makes invariant tests and stronger skills more valuable than broad style cleanup.

The worktree already had an unrelated `src-tauri/Cargo.lock` version change before this review. This feedback originally added files under `doc/codex-feedback/`; they now live under `doc/operations/codex-feedback/`.
