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

<!-- SKILLS-START (auto-generated, do not edit) -->

## Available Skills

Read the matching `SKILL.md` before starting any task that touches the area described.

| Skill | Description |
|---|---|
| `add-analysis-pass` | Checklist and guide for adding a new analysis pass to the trait-based modular pipeline |
| `add-ipc-command` | Pattern for adding a new Tauri IPC command (request/response or push event) in the deep-cuts monorepo |
| `add-tauri-sidecar` | How to bundle an external binary and its dylib dependencies with the Tauri app, patch rpaths, sign everything, and resolve the path at runtime |
| `bot-collab` | Pattern for multi-agent collaboration sessions in the deep-cuts repository |
| `bump-dev-version` | Bump the app version in Cargo.toml after a release to start the next dev cycle |
| `db-migration` | Safe pattern for adding SQLite schema migrations in the deep-cuts Rust/rusqlite_migration stack |
| `dev-guidelines` | Guidelines to prevent development false starts, environment mismatches, and incorrect commands in the deep-cuts monorepo |
| `git-commits` | Rules for writing commit messages in the deep-cuts repository — subject line format, body content, scope naming, and what to omit |
| `how-to-experiment` | Experimental protocol for Deep Cuts research, prototypes, model evaluations, threshold tuning, ablations, metric comparisons, and claims about accuracy or quality. Use before running or interpreting experiments so bots preserve train/validation/test boundaries, avoid leakage, compare against baselines, and report results honestly. |
| `query-db` | How to locate and query the deep-cuts production SQLite database |
| `query-metrics-db` | How to locate and query the deep-cuts pipeline metrics SQLite database |
| `release-build` | End-to-end checklist for building a signed macOS release — bump manifest min_app_version, verify sidecars, build, and inspect the .app |
| `svelte-component` | Conventions for writing Svelte 5 components in deep-cuts — runes, stores, props, event listeners, and file layout |
| `ui-debug` | Inspect, debug, and compare the Deep Cuts UI using an available browser tool. Use this skill whenever the user wants to inspect DOM structure, read computed CSS styles, take a screenshot of the app, or compare the UI before and after a Svelte/CSS change. Triggers on phrases like "show me the DOM", "what styles does X have", "screenshot the app", "compare before and after", "inspect the detail pane", "check the layout", or any request to verify a visual change. Also use proactively when finishing a CSS or Svelte refactor — capture a before snapshot, apply the change, then diff to confirm only intended styles changed. |
| `ui-design` | Guidelines for the Sonic Glitch design system, theme variables, and multi-theme readability (dark, light, high-contrast) |
| `using-python` | How to run Python scripts and install packages in the deep-cuts project |
| `write-docs` | Guidelines for creating, updating, reorganizing, and reviewing Deep Cuts documentation, including doc taxonomy, lifecycle status, protected public-link paths, proposal handling, and link verification. |

<!-- SKILLS-END -->
