---
name: tauri-mocks
description: Adopt @tauri-apps/api/mocks for frontend unit tests alongside a typed CommandMap
metadata:
  type: project
---

# Tauri Mocks — Missing Frontend Unit Test Infrastructure

Date: 2026-06-07

## Observation

We do not use `@tauri-apps/api/mocks`. Our current mock system (`LOCAL_DEBUG` + `MOCK_RESPONSES` in `ipc.ts` + `mock-data.ts`) is a live browser debugging workflow — it lets you run the UI in a real browser without a Tauri backend. This is worth keeping.

`@tauri-apps/api/mocks` serves a different purpose: it intercepts `invoke`/`listen` in a vitest/jest test runner, enabling automated unit tests for stores and components that call IPC without a running backend.

## Gap

There are no frontend unit tests for any store or component that calls IPC. This means regressions in command wiring, response handling, or reactive state updates are only caught by manual testing.

## Recommendation

Adopt `@tauri-apps/api/mocks` as a **separate initiative**, bundled with the typed `CommandMap` backlog item. The right moment is when the typed command surface is added to `ipc.ts` — tests written at the same time as the types will validate the typed surface from day one.

**Why:** bundle with typed CommandMap
**Cost of error:** silent regressions in IPC-driven UI state, caught only by manual testing
