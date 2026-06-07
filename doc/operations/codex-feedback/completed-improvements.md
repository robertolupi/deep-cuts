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
