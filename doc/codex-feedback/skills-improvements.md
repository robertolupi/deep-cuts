# Skills Improvements

Date: 2026-06-06

## 1. Update `AGENTS.md` or generate a skill index

`AGENTS.md` lists a subset of skills, but the repo now contains additional workflows that affect correctness:

- `add-analysis-pass`
- `add-tauri-sidecar`
- `bot-collab`
- `bump-dev-version`
- `release-build`
- `svelte-component`
- `ui-debug`
- `using-python`
- `query-metrics-db`

Either update `AGENTS.md` to list all mandatory triggers or add a generated skill index checked by CI/doc-lint. Future agents should not have to discover key workflows by chance.

## 2. Make `doc/collab/PROTOCOL.md` the collaboration source of truth

`doc/collab/PROTOCOL.md` now requires structured handoffs with `Task`, `Context`, and `Deliverable`. `skills/bot-collab/SKILL.md` still contains older one-line handoff examples.

Recommended change:

- Keep protocol details only in `doc/collab/PROTOCOL.md`.
- Slim `skills/bot-collab/SKILL.md` to: when to use it, which file to read, the minimum startup checklist, and verification rules.
- Add a note that session files are working logs and durable decisions must be promoted into normal `doc/` files or skills.

## 3. Strengthen `add-analysis-pass`

The skill is already valuable and reflects the trait-based pipeline. Add explicit guardrails from this review:

- Do not use `filter_map(|r| r.ok())` for DB reads in production paths.
- Batch passes must update pass state for success, skip, and not-applicable cases.
- Batch passes must log metrics with `run_id`, or explicitly document why metrics do not apply.
- "Not enough data" is a completed/skipped state, not a reason to rerun forever.
- No-`track_id` tables require custom reset behavior and tests.
- Any pass that adds a frontend-visible field must update TypeScript DTOs and mock data.

## 4. Strengthen `add-ipc-command`

The skill tells agents how to add Rust commands and invoke them from the frontend, but it should also require the app's wrapper boundary:

- Add new commands to `$lib/ipc`'s typed command map.
- Add local-debug mock behavior when the command affects visible UI.
- Prefer `$lib/ipc` imports over direct `@tauri-apps/api/core` imports.
- Add a test for command args/result shape when feasible.
- For push events, document event name, payload type, lifecycle, and unlisten ownership.

## 5. Resolve Python guidance drift

`skills/dev-guidelines/SKILL.md` says "No Python tooling", while `skills/using-python/SKILL.md`, `tools/`, and model export workflows clearly use Python.

Replace the statement with:

> The app has no Python runtime dependency. Python is used only for tools, experiments, model export, and validation scripts. Use `tools/.venv/bin/python`, never system Python.

Then make README examples match the same rule and run from the repository root.

## 6. Make UI skills portable across agent environments

`skills/ui-debug/SKILL.md` assumes Chrome MCP and the Claude in Chrome extension. This is useful for that environment but brittle for Codex, Gemini, or a browser plugin.

Add fallback sections:

- Codex in-app Browser workflow for localhost or `?local_debug=1`.
- Playwright/browser fallback for screenshots and DOM checks.
- Manual verification checklist when no browser automation is available.

Keep the skill outcome-oriented: what to verify, not only which tool to click.

## 7. Tighten `svelte-component` and `ui-design`

Add checklists that match current risks:

- Components should use `$lib/ipc` instead of direct Tauri imports.
- Long components should extract pure helper functions before they exceed broad feature ownership.
- Stores must be idempotent if they register listeners.
- Component styles must use `--sg-*` tokens and avoid inline color styles.
- New UI work should include light and accessible theme checks.

## 8. Add lightweight doc/skill linting

A small local script could catch recurring process issues:

- stale flat files under `doc/collab/sessions/` when protocol expects `session.md` inside directories;
- missing proposal metadata in `doc/*.md`;
- duplicate or conflicting instructions between skills and protocol docs;
- command examples that fail from the documented working directory;
- direct Tauri imports in frontend code outside `$lib/ipc`;
- `filter_map(|r| r.ok())` in Rust production code.

This does not need to block every commit initially. Start as a report command agents can run before large changes.
