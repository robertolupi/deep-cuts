# Completed Improvements

Date: 2026-06-06

This file archives completed items from the Codex feedback backlog so active recommendation files stay focused on open work. Keep entries concise: what was completed, where it landed, and which commit made it durable.

## Skills Improvements

### Generated skill discovery

**Completed:** 2026-06-06  
**Commit:** `aa1b60a docs: add dynamic skill discovery`

`AGENTS.md`, `CLAUDE.md`, and `GEMINI.md` now point agents to the generated [skills/INDEX.md](../../../skills/INDEX.md). The index is generated from `skills/*/SKILL.md` frontmatter by `tools/generate_skill_index.py`.

### Collaboration protocol source of truth

**Completed:** 2026-06-06  
**Commit:** `90a50d3 docs: simplify collaboration skill`

[skills/bot-collab/SKILL.md](../../../skills/bot-collab/SKILL.md) is now a lightweight launcher/checklist. Protocol details live in [doc/collab/PROTOCOL.md](../../collab/PROTOCOL.md).

### Python guidance drift

**Completed:** 2026-06-06  
**Commit:** `7aacf75 docs: reorganize documentation taxonomy`

[skills/dev-guidelines/SKILL.md](../../../skills/dev-guidelines/SKILL.md) now states that the app has no Python runtime dependency, while Python remains supported for tools, experiments, model export, and validation scripts through `tools/.venv/bin/python` and [skills/using-python/SKILL.md](../../../skills/using-python/SKILL.md).

### Strengthen `add-ipc-command`

**Completed:** 2026-06-07  
**Commit:** `0b6e325 docs(skills): strengthen add-ipc-command with mock system guidance`

[skills/add-ipc-command/SKILL.md](../../../skills/add-ipc-command/SKILL.md) now covers the `$lib/ipc` wrapper boundary: Step 3 documents the two-file mock system (`mock-data.ts` + `MOCK_RESPONSES` in `ipc.ts`), import boundary rules (✓/✗ examples), push event metadata documentation, and a restructured checklist with ipc.ts marked required. Common mistakes table updated.

### Tighten `svelte-component` and `ui-design`

**Completed:** 2026-06-07  
**Commit:** `f09e5b1 docs(skills): add CSS token and cross-skill checklist items to svelte-component and ui-design`

[skills/svelte-component/SKILL.md](../../../skills/svelte-component/SKILL.md) had IPC boundary and store idempotency coverage already; added hardcoded hex/rgba row to Common mistakes pointing to the design token reference. [skills/ui-design/SKILL.md](../../../skills/ui-design/SKILL.md) already had comprehensive CSS token coverage; added IPC import and store idempotency items to the verification checklist, cross-referencing `svelte-component`.

## Docs and Process Improvements

### Normalize collaboration session storage

**Completed:** 2026-06-07  
**Commit:** `c64973d chore(collab): normalize session layout, add linter, update protocol`

Flat session files migrated to `YYYY-MM-DD-slug/session.md` directories. Two non-dated design files moved to `doc/architecture/`. `tools/lint_collab.py` added (errors on flat `.md` files, warns on sessions inactive >30 days). [doc/collab/PROTOCOL.md](../../collab/PROTOCOL.md) updated: sessions must be directories, participants are opt-in per session from the roster.

## Codebase Improvements

### Harden startup failure behavior (C4)

**Completed:** 2026-06-07  
**Commit:** `06a9a7b fix(startup): show explanatory dialog on DB initialization failure`

`src-tauri/src/lib.rs` now shows a native `rfd` dialog with an explanatory message ("Deep Cuts could not open its database. Check that the application data directory is writable. Error: {detail}") and calls `process::exit(1)` on main DB init failure, instead of silently panicking via Tauri's generic wrapper. Metrics DB remains optional.

### Fix library store lifecycle (F3)

**Completed:** 2026-06-07  
**Commit:** `8a81f88 fix(stores): make library.init() idempotent and add dispose()`

`src/lib/stores/library.svelte.ts`: added `initialized` guard to prevent duplicate listener registration, `unlisteners[]` array collecting all unlisten functions, and `dispose()` method for test/hot-reload cleanup. `initialized` resets on init failure to allow retries. Pre-existing cross-store coupling on `player.selectedTrack` noted with a TODO comment.

## Skills Improvements

### Strengthen `add-analysis-pass` (S1)

**Completed:** 2026-06-07  
**Commit:** `d1f3b65 docs: skills guardrails, doc sync checklist, proposal frontmatter`

`skills/add-analysis-pass/SKILL.md`: added metrics/run_id common-mistake row, expanded DTO row to name `mock-data.ts` explicitly, expanded no-`track_id` row to require tests. Four of the six guardrails (filter_map, batch status, "not enough data", no-track_id reset) were already present.

## Docs and Process Improvements

### Add proposal lifecycle metadata (D1)

**Completed:** 2026-06-07  
**Commit:** `d1f3b65 docs: skills guardrails, doc sync checklist, proposal frontmatter`

Added YAML lifecycle frontmatter (`status`, `owner`, `last_verified`, etc.) to five proposal/research docs: `sax_structure_learning.md` (active), `sax_structural_search.md` (active), `music_map_improvements.md` (proposed), `playlist_view_enhancements.md` (active), `feature_feasibility_analysis.md` (proposed).

### Doc sync checklist for feature commits (D5)

**Completed:** 2026-06-07  
**Commit:** `d1f3b65 docs: skills guardrails, doc sync checklist, proposal frontmatter`

Added `## Doc Sync` section to `CLAUDE.md` after "Testing & Committing" with a five-item GFM checklist: migration docs, analysis pass skill, IPC mocks, implemented-differently notes, removed-feature status.

### Fix cd-based command examples (D6)

**Completed:** 2026-06-07  
**Note:** `models/` is gitignored — `models/README.md` was fixed locally but cannot be committed.

Fixed four `cd tools` + bare `python` patterns in `models/README.md` to use `tools/.venv/bin/python` from repo root. Skills directory was already correct. The bad-example block in `doc/operations/codex-feedback/docs-approach-improvements.md` was intentionally left unchanged (it illustrates the anti-pattern).
