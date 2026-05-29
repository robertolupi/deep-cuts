# Agent Instructions & Project Skills

Welcome to the Deep Cuts repository — a clean-room Tauri/Rust/Svelte 5 desktop app for managing audio file collections. We use custom agent skills and project documentation to standardize development tasks.

## Custom Skills Directory

All custom skills are located in the [skills/](skills/) directory.

Before starting any task related to the following areas, you **MUST** read the corresponding `SKILL.md` file and follow its instructions:

1. **IPC Commands**
   - **Path**: [skills/add-ipc-command/SKILL.md](skills/add-ipc-command/SKILL.md)
   - **Use when**: Adding or editing Tauri/IPC endpoints between the frontend and Rust backend.

2. **Check Prototype**
   - **Path**: [skills/check-prototype/SKILL.md](skills/check-prototype/SKILL.md)
   - **Use when**: Looking up how a feature was implemented in `music-intelligence` or `music-index`.

3. **Database Migrations**
   - **Path**: [skills/db-migration/SKILL.md](skills/db-migration/SKILL.md)
   - **Use when**: Creating, running, or debugging database schema changes.

4. **Development Guidelines**
   - **Path**: [skills/dev-guidelines/SKILL.md](skills/dev-guidelines/SKILL.md)
   - **Use when**: Onboarding, running the app, or checking general repository conventions.

5. **Querying the Database**
   - **Path**: [skills/query-db/SKILL.md](skills/query-db/SKILL.md)
   - **Use when**: Formulating database queries or interacting with the SQLite database directly.

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

## Tech Stack

- **Frontend**: Svelte 5, SvelteKit, TypeScript, D3.js, WaveSurfer.js
- **Backend**: Rust, Tauri 2, rusqlite + rusqlite_migration, sqlite-vec, ort (ONNX Runtime)
- **Bundle ID**: `com.rlupi.deep-cuts`
- **Dev command**: `npm run tauri` (from project root)
