# Agent Instructions & Project Skills

Welcome to the Deep Cuts repository — a clean-room Tauri/Rust/Svelte 5 desktop app for managing audio file collections. We use custom agent skills and project documentation to standardize development tasks.

## Custom Skills Directory

All custom skills are located in the [skills/](skills/) directory.

Before starting any task related to the following areas, you **MUST** read the corresponding `SKILL.md` file and follow its instructions:

1. **IPC Commands**
   - **Path**: [skills/add-ipc-command/SKILL.md](skills/add-ipc-command/SKILL.md)
   - **Use when**: Adding or editing Tauri/IPC endpoints between the frontend and Rust backend.

2. **Database Migrations**
   - **Path**: [skills/db-migration/SKILL.md](skills/db-migration/SKILL.md)
   - **Use when**: Creating, running, or debugging database schema changes.

3. **Development Guidelines**
   - **Path**: [skills/dev-guidelines/SKILL.md](skills/dev-guidelines/SKILL.md)
   - **Use when**: Onboarding, running the app, or checking general repository conventions.

4. **Querying the Database**
   - **Path**: [skills/query-db/SKILL.md](skills/query-db/SKILL.md)
   - **Use when**: Formulating database queries or interacting with the SQLite database directly.

5. **Querying the Metrics / Telemetry Database**
   - **Path**: [skills/query-metrics-db/SKILL.md](skills/query-metrics-db/SKILL.md)
   - **Use when**: Inspecting pipeline performance data, latency stats, failure logs, or the `telemetry.db` file.

6. **Adding an Analysis Pass**
   - **Path**: [skills/add-analysis-pass/SKILL.md](skills/add-analysis-pass/SKILL.md)
   - **Use when**: Adding a new pass to the analysis pipeline (e.g. essentia, qwen, bpm_correction).

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
