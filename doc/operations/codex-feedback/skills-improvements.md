# Skills Improvements

Date: 2026-06-06

This is the active skills/process backlog from the Codex feedback pass. Completed items are archived in [completed-improvements.md](completed-improvements.md) so future readers can focus on open work.

## 1. Strengthen `add-analysis-pass`

The skill is already valuable and reflects the trait-based pipeline. Add explicit guardrails from this review:

- Do not use `filter_map(|r| r.ok())` for DB reads in production paths.
- Batch passes must update pass state for success, skip, and not-applicable cases.
- Batch passes must log metrics with `run_id`, or explicitly document why metrics do not apply.
- "Not enough data" is a completed/skipped state, not a reason to rerun forever.
- No-`track_id` tables require custom reset behavior and tests.
- Any pass that adds a frontend-visible field must update TypeScript DTOs and mock data.

## 2. Strengthen `add-ipc-command`

The skill tells agents how to add Rust commands and invoke them from the frontend, but it should also require the app's wrapper boundary:

- Add new commands to `$lib/ipc`'s typed command map.
- Add local-debug mock behavior when the command affects visible UI.
- Prefer `$lib/ipc` imports over direct `@tauri-apps/api/core` imports.
- Add a test for command args/result shape when feasible.
- For push events, document event name, payload type, lifecycle, and unlisten ownership.

## 3. Make UI skills portable across agent environments

`skills/ui-debug/SKILL.md` assumes Chrome MCP and the Claude in Chrome extension. This is useful for that environment but brittle for Codex, Gemini, or a browser plugin.

Add fallback sections:

- Codex in-app Browser workflow for localhost or `?local_debug=1`.
- Playwright/browser fallback for screenshots and DOM checks.
- Manual verification checklist when no browser automation is available.

Keep the skill outcome-oriented: what to verify, not only which tool to click.

## 4. Tighten `svelte-component` and `ui-design`

Add checklists that match current risks:

- Components should use `$lib/ipc` instead of direct Tauri imports.
- Long components should extract pure helper functions before they exceed broad feature ownership.
- Stores must be idempotent if they register listeners.
- Component styles must use `--sg-*` tokens and avoid inline color styles.
- New UI work should include light and accessible theme checks.

## 5. Add lightweight doc/skill linting

A small local script could catch recurring process issues:

- stale flat files under `doc/collab/sessions/` when protocol expects `session.md` inside directories;
- missing proposal metadata in `doc/*.md`;
- duplicate or conflicting instructions between skills and protocol docs;
- command examples that fail from the documented working directory;
- direct Tauri imports in frontend code outside `$lib/ipc`;
- `filter_map(|r| r.ok())` in Rust production code.

This does not need to block every commit initially. Start as a report command agents can run before large changes.
