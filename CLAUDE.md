# Agent Instructions & Project Skills

Welcome to the Deep Cuts repository — a clean-room Tauri/Rust/Svelte 5 desktop app for managing audio file collections. We use custom agent skills and project documentation to standardize development tasks.

## Custom Skills Directory

All custom skills are located in the [skills/](skills/) directory.

The generated [skills/INDEX.md](skills/INDEX.md) lists every skill by extracting frontmatter from `skills/*/SKILL.md`. Before starting any task, inspect that index and read every `SKILL.md` whose description matches the work.

Regenerate the index after adding, removing, renaming, or changing the frontmatter for a skill:

```bash
tools/.venv/bin/python tools/generate_skill_index.py
```

## Repository Layout

```
src/             — Svelte 5 frontend (SvelteKit, TypeScript)
src-tauri/       — Rust Tauri backend
  src/
    lib.rs       — managed state, app entry, generate_handler registration
    database.rs  — schema migrations, DB init, structs
    commands/    — IPC command handlers (one file per domain)
    scanner/     — recursive audio file scanner (mod.rs, fs.rs, metadata.rs, db.rs)
    analysis/    — analysis pipeline passes and orchestrator
    main.rs      — binary entry point
static/          — static assets
skills/          — project-specific agent playbooks
```

## Testing & Committing

Always run `cargo test --manifest-path src-tauri/Cargo.toml` after Rust changes. **Never commit — wait for the user to explicitly say so after manual testing.**

## Doc Sync

Before marking a feature complete, check each item that applies:

- [ ] **Migration changed** → update related design docs in `doc/`.
- [ ] **Analysis pass changed** → update `skills/add-analysis-pass/SKILL.md`.
- [ ] **IPC command changed** → update mock responses in `src/lib/mock-data.ts` and `src/lib/ipc.ts`.
- [ ] **Implemented differently than planned** → add an "Implemented outcome" note to the proposal doc.
- [ ] **Feature removed** → set `status: superseded` in the proposal doc's frontmatter.

## File System Rules

**Never search in global directories** (`~/Library`, `~/Documents`, `/`, etc.). All project files live under `/Users/rlupi/src/deep-cuts/`. For database access, always use the `skills/query-db/SKILL.md` skill — it specifies the correct DB path.

## Tech Stack

- **Frontend**: Svelte 5, SvelteKit, TypeScript, D3.js, WaveSurfer.js
- **Backend**: Rust, Tauri 2, rusqlite + rusqlite_migration, sqlite-vec, ort (ONNX Runtime)
- **Bundle ID**: `com.rlupi.deep-cuts`
- **Dev command**: `npm run tauri` (from project root)
