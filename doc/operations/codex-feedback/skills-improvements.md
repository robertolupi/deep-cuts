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

## S3a. Lint missing proposal metadata

Extend or add a script that checks `doc/**/*.md` proposal and research files for missing lifecycle frontmatter (`status`, `owner`, `last_verified`). Report files without frontmatter as warnings.

## S3b. Lint direct Tauri imports in frontend

Script that scans `src/` for `from "@tauri-apps/api"` outside of `src/lib/ipc.ts`. Report each occurrence as an error.

## S3c. Lint `filter_map(|r| r.ok())` in Rust

Script that greps `src-tauri/src/` for `filter_map(|r| r.ok())` and reports each hit with file and line. Silent error-swallowers in production DB paths.
