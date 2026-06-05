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

### ⚠️ Stale binary problem

`npm run tauri dev` sometimes serves a **stale Rust binary** — it may not recompile even when source files have changed. Symptoms: code fixes appear to have no effect, or analysis results are inconsistent with source changes.

**Diagnose: check if the binary is older than the source:**

```bash
stat -f "%Sm %N" src-tauri/target/debug/deep-cuts
stat -f "%Sm %N" src-tauri/src/embeddings.rs   # or whichever file was changed
```

If the binary timestamp is **earlier** than the source file, it's stale.

**Fix: force a rebuild before launching:**

```bash
cargo build --manifest-path src-tauri/Cargo.toml
npm run tauri dev
```

This is especially important after editing files in `src-tauri/src/` (e.g. `embeddings.rs`, `analysis.rs`) where a stale binary produces silently wrong results with no error message.

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

Frontend tests use Vitest + `@testing-library/svelte` + jsdom. Run them with:

```bash
npm test           # single run
npm run test:watch # watch mode
```

Test files live alongside source files as `*.test.ts` (e.g. `src/lib/components/Foo.test.ts`). The full stack — `vitest`, `@testing-library/svelte`, `@testing-library/jest-dom`, `jsdom` — is already installed.

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
