# Skills Improvements

Date: 2026-06-06

This is the active skills/process backlog from the Codex feedback pass. Completed items are archived in [completed-improvements.md](completed-improvements.md) so future readers can focus on open work.

## 1. Make UI skills portable across agent environments

`skills/ui-debug/SKILL.md` assumes Chrome MCP and the Claude in Chrome extension. This is useful for that environment but brittle for Codex, Gemini, or a browser plugin.

Add fallback sections:

- Codex in-app Browser workflow for localhost or `?local_debug=1`.
- Playwright/browser fallback for screenshots and DOM checks.
- Manual verification checklist when no browser automation is available.

Keep the skill outcome-oriented: what to verify, not only which tool to click.

## 2. Add lightweight doc/skill linting (partial)

The collab sessions part is done (`tools/lint_collab.py`, commit `c64973d`). Remaining:

- missing proposal metadata in `doc/*.md`;
- duplicate or conflicting instructions between skills and protocol docs;
- direct Tauri imports in frontend code outside `$lib/ipc`;
- `filter_map(|r| r.ok())` in Rust production code.

This does not need to block every commit initially. Start as a report command agents can run before large changes.
