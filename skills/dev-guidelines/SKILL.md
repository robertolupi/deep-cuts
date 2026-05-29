---
name: dev-guidelines
description: Guidelines to prevent development false starts, environment mismatches, and incorrect commands in the deep-cuts monorepo
---

# Development Guidelines & Playbook

## 1. Starting the app

Launch from the project root:

```bash
npm run tauri dev          # normal start (Vite HMR + Rust compilation)
npm run tauri dev -- -- --clear-db   # if a --clear-db flag is wired up
```

Or use the scripts defined in `package.json` directly:

```bash
npm run dev    # Vite only (frontend preview, no Tauri)
npm run tauri  # full Tauri dev mode
```

**Do not** invoke `vite` or `cargo tauri` directly — always go through `npm run`.

## 2. Frontend layout

The frontend lives at the **project root** (not a `frontend/` subdirectory). All `npm` commands run from the root:

```bash
npm run build
npm run check       # svelte-check + TypeScript
npm run check:watch
```

Svelte 5 enforces strict `{@const}` placement — these must be immediate children of block tags (`{#each}`, `{#if}`, etc.), not DOM elements.

## 3. Running tests

```bash
# Rust: persistence, scanner, DSP
cargo test --manifest-path src-tauri/Cargo.toml
```

There are currently no frontend Vitest tests. Add them under `src/` if needed.

## 4. Database path

On macOS, the SQLite database lives at:
```
~/Library/Application Support/com.rlupi.deep-cuts/deep_cuts.db
```

See `skills/query-db/SKILL.md` for query patterns and safety rules.

## 5. Tauri configuration

`src-tauri/tauri.conf.json` — app identifier is `com.rlupi.deep-cuts`, product name is `Deep Cuts`.

## 6. No Python tooling

This project has no Python runtime dependency or `run_dev.py`. Everything is driven through `npm run tauri`.
