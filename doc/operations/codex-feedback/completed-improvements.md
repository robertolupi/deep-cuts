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

## Codebase Improvements

### Analysis pass invariant tests (C1)

**Completed:** 2026-06-07
**Commit:** `10d1f34 test(analysis): add pass invariant tests (C1)`

Four invariant tests in `analysis::tests`:
- `invariant_registry_all_fields_non_empty`: every PASS_REGISTRY entry has non-empty name/description and all dependency names exist in the registry.
- `invariant_run_order_respects_dependencies`: every dependency appears before its dependent in the concrete PipelineManager run order.
- `invariant_reset_cascade_clears_dependents`: resetting `clap` also resets `qwen` downstream.
- `invariant_sax_batch_pass_writes_track_passes`: `needs_run()` correctly reflects PENDING vs DONE state.
Note: `execute()` cannot be called in unit tests (requires `AppHandle<Wry>` with ONNX/audio IO); status invariants are verified via direct DB updates.

### Typed CommandMap + import routing (F1 complete)

**Completed:** 2026-06-07
**Commits:** `8e25954` (F1a import routing), `5577200` (F1b CommandMap)

F1a: All 21 files importing `@tauri-apps/api` directly now go through `$lib/ipc`. Added re-exports for `convertFileSrc`, `getVersion`, `openUrl`, `UnlistenFn`.

F1b: `CommandMap` type added to `src/lib/ipc.ts` mapping all 86 commands to args/result types. `invoke()` is now overloaded with a typed path for CommandMap keys and a backward-compatible escape hatch. 73 commands have precise types; 13 have `// TODO: tighten`. New interfaces: `SemanticSearchResult`, `PassStat`, `ChatSession`, `ChatMessage`, `ChatSearchResult`, `ResumableFile`, `M3UTrackInfo`. Skills `add-ipc-command` and `svelte-component` updated with CommandMap checklist items.
