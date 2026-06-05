---
name: ui-debug
description: >
  Inspect, debug, and compare the Deep Cuts UI using the Chrome MCP.
  Use this skill whenever the user wants to inspect DOM structure, read computed
  CSS styles, take a screenshot of the app, or compare the UI before and after
  a Svelte/CSS change. Triggers on phrases like "show me the DOM", "what styles
  does X have", "screenshot the app", "compare before and after", "inspect the
  detail pane", "check the layout", or any request to verify a visual change.
  Also use proactively when finishing a CSS or Svelte refactor — capture a
  before snapshot, apply the change, then diff to confirm only intended styles changed.
---

# UI Debug Skill

This skill connects to the running Deep Cuts dev server via the Chrome MCP and
lets you inspect the live UI: read the DOM, capture computed CSS styles, take
screenshots, and diff styles before/after a change.

## Prerequisites

- The app must be running: `npm run tauri dev` (or just `npm run dev` for frontend-only)
- Chrome must be open with the "Claude in Chrome" extension connected
- For realistic data, use the `?local_debug=1` query parameter (see below)

## The local_debug mode

Navigating to `http://localhost:1420/?local_debug=1` activates mock IPC data
(5 fixture tracks, 2 playlists) so the full UI renders in Chrome without needing
the Tauri backend. Use this URL for all CSS/Svelte debugging. Without the param,
the library will be empty because `window.__TAURI__` is not injected in Chrome.

## Setup sequence

Run this once per session before any inspection tools:

```
1. mcp__Claude_in_Chrome__list_connected_browsers   → get deviceId
2. mcp__Claude_in_Chrome__select_browser            → connect to it
3. mcp__Claude_in_Chrome__tabs_context_mcp          → get tabId (createIfEmpty: true)
4. mcp__Claude_in_Chrome__navigate                  → http://localhost:1420/?local_debug=1
   (skip if already on that URL)
```

After setup, reuse the same `tabId` for all subsequent calls.

## Reading the DOM

Use `read_page` to get the accessibility tree. Start shallow (depth 3) and drill
into specific regions with `ref_id` when the output is too large.

```js
// Full page, shallow
read_page({ tabId, filter: "all", depth: 3 })

// Drill into a specific region by its ref_id
read_page({ tabId, filter: "all", depth: 5, ref_id: "ref_130" })

// Interactive elements only (useful for testing affordances)
read_page({ tabId, filter: "interactive", depth: 4 })
```

Key regions in Deep Cuts:
- `complementary` (first) — left filter sidebar
- `banner` — top nav bar
- `main` — track list
- `contentinfo` — player bar
- `complementary` (second) — right detail pane

## Reading computed CSS styles

Use `javascript_tool` to run `getComputedStyle()`. This returns the fully resolved
styles (after all CSS variables, cascade, and specificity are applied).

```js
// Single element by selector
const el = document.querySelector('.track-title');
const s = getComputedStyle(el);
({ fontSize: s.fontSize, fontWeight: s.fontWeight, color: s.color,
   lineHeight: s.lineHeight, letterSpacing: s.letterSpacing })

// Read a CSS custom property (design token)
getComputedStyle(document.documentElement).getPropertyValue('--color-primary').trim()

// Dump all properties you care about as an object
const el = document.querySelector('selector');
const s = getComputedStyle(el);
const props = ['fontSize','fontWeight','color','background','padding','margin',
               'borderRadius','gap','display','flexDirection','alignItems'];
Object.fromEntries(props.map(p => [p, s[p]]))
```

## Taking a screenshot

```js
computer({ action: "screenshot", tabId, save_to_disk: true })
```

Set `save_to_disk: true` to attach the image to the response so the user can see it.

## Before/after style diff workflow

This is the primary workflow for verifying CSS refactors:

**Step 1 — Snapshot before:**
```js
// Capture styles for all elements you expect to change
const snapshot = {};
for (const [name, sel] of Object.entries({ trackTitle: '.track-title', ... })) {
  const el = document.querySelector(sel);
  if (!el) { snapshot[name] = null; continue; }
  const s = getComputedStyle(el);
  snapshot[name] = { fontSize: s.fontSize, color: s.color, /* ... */ };
}
JSON.stringify(snapshot, null, 2)
```

Save the result as `before`.

**Step 2 — Apply the change** (edit the Svelte/CSS file, Vite hot-reloads automatically).

**Step 3 — Snapshot after** (same script).

**Step 4 — Diff:**
```js
// Compare before vs after objects
const before = { /* paste snapshot */ };
const after  = { /* paste snapshot */ };
const diff = {};
for (const [el, bProps] of Object.entries(before)) {
  for (const [prop, bVal] of Object.entries(bProps ?? {})) {
    const aVal = after[el]?.[prop];
    if (aVal !== bVal) {
      diff[`${el}.${prop}`] = { before: bVal, after: aVal };
    }
  }
}
diff
```

Report: what changed, what stayed the same, any unexpected changes.

## Selecting a track for detail pane inspection

The detail pane only renders when a track is selected. Click the first row:

```js
// Find the first track row and click it
computer({ action: "left_click", ref: "ref_84", tabId })
// Then re-read the detail pane region
read_page({ tabId, filter: "all", depth: 4, ref_id: "ref_130" })
```

If ref IDs have changed (they're generated per page load), use `find` instead:
```js
find({ tabId, query: "Kong track row" })
```

## Tips

- Svelte scopes class names with a hash suffix (e.g. `.track-title.s-Ah30QY_rFxYB`) —
  use the base class name (`.track-title`) in selectors, it still matches.
- Vite hot-reloads on save, so you don't need to refresh between edits.
- If the page has navigated away from `?local_debug=1`, re-navigate before inspecting.
- `browser_batch` chains multiple actions in one round-trip but cannot contain
  `read_page` on localhost on the first call in a session — call it standalone once
  to grant permission, then batch freely afterward.
