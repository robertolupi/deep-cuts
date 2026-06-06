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

5. **Querying the Metrics Database**
   - **Path**: [skills/query-metrics-db/SKILL.md](skills/query-metrics-db/SKILL.md)
   - **Use when**: Inspecting pipeline performance data, latency stats, failure logs, or the `metrics.db` file.

6. **Adding an Analysis Pass**
   - **Path**: [skills/add-analysis-pass/SKILL.md](skills/add-analysis-pass/SKILL.md)
   - **Use when**: Adding a new pass to the analysis pipeline (e.g. essentia, qwen, bpm_correction).

7. **Release Build**
   - **Path**: [skills/release-build/SKILL.md](skills/release-build/SKILL.md)
   - **Use when**: Producing a signed macOS release build, verifying the bundle, or publishing.

8. **Bump Dev Version**
   - **Path**: [skills/bump-dev-version/SKILL.md](skills/bump-dev-version/SKILL.md)
   - **Use when**: Advancing `Cargo.toml` to the next version after a release has shipped.

9. **Bundling an External Binary (Tauri Sidecar)**
   - **Path**: [skills/add-tauri-sidecar/SKILL.md](skills/add-tauri-sidecar/SKILL.md)
   - **Use when**: Bundling a new external executable (e.g. fpcalc, llama-server) with the Tauri app.

10. **Svelte Components & Stores**
   - **Path**: [skills/svelte-component/SKILL.md](skills/svelte-component/SKILL.md)
   - **Use when**: Writing or editing Svelte 5 components, stores, or frontend reactive logic.

11. **UI Debugging & Style Inspection**
   - **Path**: [skills/ui-debug/SKILL.md](skills/ui-debug/SKILL.md)
   - **Use when**: Inspecting DOM structure, reading computed CSS styles, taking screenshots, or comparing UI before/after a Svelte/CSS change via the Chrome MCP.

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

## File System Rules

**Never search in global directories** (`~/Library`, `~/Documents`, `/`, etc.). All project files live under `/Users/rlupi/src/deep-cuts/`. For database access, always use the `skills/query-db/SKILL.md` skill — it specifies the correct DB path.

## Tech Stack

- **Frontend**: Svelte 5, SvelteKit, TypeScript, D3.js, WaveSurfer.js
- **Backend**: Rust, Tauri 2, rusqlite + rusqlite_migration, sqlite-vec, ort (ONNX Runtime)
- **Bundle ID**: `com.rlupi.deep-cuts`
- **Dev command**: `npm run tauri` (from project root)
