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
