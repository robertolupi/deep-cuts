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
    lib.rs       — IPC command handlers, managed state, app entry
    database.rs  — schema migrations, DB init, structs
    scanner/     — recursive audio file scanner (mod.rs, fs.rs, metadata.rs, db.rs)
    main.rs      — binary entry point
static/          — static assets
skills/          — project-specific agent playbooks
```

## Testing & Committing

Always run `cargo test --manifest-path src-tauri/Cargo.toml` after Rust changes. **Never commit — wait for the user to explicitly say so after manual testing.**

## Tech Stack

- **Frontend**: Svelte 5, SvelteKit, TypeScript, D3.js, WaveSurfer.js
- **Backend**: Rust, Tauri 2, rusqlite + rusqlite_migration, sqlite-vec, ort (ONNX Runtime)
- **Bundle ID**: `com.rlupi.deep-cuts`
- **Dev command**: `npm run tauri` (from project root)
