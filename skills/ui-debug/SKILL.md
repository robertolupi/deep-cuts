---
name: ui-debug
description: >
  Inspect, debug, and compare the Deep Cuts UI using an available browser tool.
  Use this skill whenever the user wants to inspect DOM structure, read computed
  CSS styles, take a screenshot of the app, or compare the UI before and after
  a Svelte/CSS change. Triggers on phrases like "show me the DOM", "what styles
  does X have", "screenshot the app", "compare before and after", "inspect the
  detail pane", "check the layout", or any request to verify a visual change.
  Also use proactively when finishing a CSS or Svelte refactor — capture a
  before snapshot, apply the change, then diff to confirm only intended styles changed.
---

# UI Debug Skill

This skill connects to the running Deep Cuts dev server with whichever browser automation is available. Use Chrome MCP when available; in Codex, prefer the in-app Browser plugin for localhost/file targets. The goal is the same in every environment: inspect the live UI, read DOM/accessibility structure, capture computed styles, take screenshots, and diff styles before/after a change.

## Prerequisites

- The app must be running: `npm run tauri dev` (or just `npm run dev` for frontend-only)
- A browser inspection tool must be available: Chrome MCP, Codex in-app Browser, Playwright, or a manual browser fallback
- For realistic data, use the `?local_debug=1` query parameter (see below)

## The local_debug mode

Navigating to `http://localhost:1420/?local_debug=1` activates mock IPC data
(5 fixture tracks, 2 playlists) so the full UI renders in Chrome without needing
the Tauri backend. Use this URL for all CSS/Svelte debugging. Without the param,
the library will be empty because `window.__TAURI__` is not injected in Chrome.

## Setup sequence — Chrome MCP

Run this once per session before any inspection tools:

```
1. mcp__Claude_in_Chrome__list_connected_browsers   → get deviceId
2. mcp__Claude_in_Chrome__select_browser            → connect to it
3. mcp__Claude_in_Chrome__tabs_context_mcp          → get tabId (createIfEmpty: true)
4. mcp__Claude_in_Chrome__navigate                  → http://localhost:1420/?local_debug=1
   (skip if already on that URL)
```

After setup, reuse the same `tabId` for all subsequent calls.

## Setup sequence — Codex in-app Browser

When working in Codex with the Browser plugin available:

1. Start `npm run dev` for frontend-only inspection, or `npm run tauri dev` for full Tauri behavior.
2. Navigate the in-app Browser to `http://localhost:1420/?local_debug=1` for mock IPC data.
3. Use screenshots and DOM inspection to verify the changed surface at desktop and narrow widths.
4. If a component depends on real Tauri APIs not represented by local-debug mocks, add or update the mock in `src/lib/ipc.ts` before judging the UI.

## Fallback A — Codex / no-MCP browser

Use this when no Chrome MCP extension is connected but a browser can be opened
(e.g. the Codex environment with its in-app Browser plugin).

**Start the dev server:**
```bash
# Full Tauri stack (required for Rust IPC commands):
npm run tauri

# Frontend only (sufficient for CSS / Svelte component work):
npm run dev
```

**Navigate to the mock-data URL:**
```
http://localhost:1420/?local_debug=1   # Tauri dev server default port
http://localhost:5173/?local_debug=1   # Vite-only (npm run dev)
```

The `?local_debug=1` flag activates built-in mock IPC data (5 fixture tracks,
2 playlists) so the full UI renders without a running Tauri backend.

**What to verify:**
- The five mock tracks appear in the track list with correct title / artist / duration.
- Selecting a track populates the right detail pane (cover art, metadata fields).
- The player bar renders at the bottom with transport controls visible.
- No red error banners or "Library is empty" placeholder are shown.
- Repeat at a narrow viewport (~900 px) to confirm panels don't overflow.
- Check that the theme (dark / light) matches what was changed — the CSS custom
  properties in `src/app.css` drive all colours; a single token regression shows
  everywhere.

If a component depends on a Tauri API not yet covered by local-debug mocks, add
or update the fixture in `src/lib/ipc.ts` before judging the UI.

---

## Fallback B — Playwright screenshot / DOM assertion

Use this when no interactive browser MCP is available but Node.js is present.

**One-shot screenshot** (no config file needed):
```bash
npx --yes playwright@latest screenshot \
  --browser chromium \
  "http://localhost:1420/?local_debug=1" \
  /tmp/deep-cuts-ui.png
```

Read the resulting image with the `Read` tool to inspect it visually.

**Inline script for DOM assertions** (save as `/tmp/check-ui.mjs`, run once):
```js
import { chromium } from 'playwright';

const browser = await chromium.launch();
const page = await browser.newPage();
await page.goto('http://localhost:1420/?local_debug=1');
await page.waitForSelector('.track-row', { timeout: 5000 });

const trackCount  = await page.locator('.track-row').count();
const detailPane  = await page.locator('[data-region="detail"]').isVisible();
const playerBar   = await page.locator('[data-region="player"]').isVisible();
const consoleErrs = [];
page.on('console', m => { if (m.type() === 'error') consoleErrs.push(m.text()); });

console.log({ trackCount, detailPane, playerBar, consoleErrs });
await browser.close();
```

```bash
node /tmp/check-ui.mjs
```

**What to verify:**
- `trackCount` equals the number of fixture tracks (5 for the default mock set).
- `detailPane` and `playerBar` are both `true`.
- `consoleErrs` is empty — any entry is a regression worth investigating.
- The screenshot shows correct theme colours and no layout overflow.

---

## Fallback C — Manual verification checklist

Use this when no automation tooling is available at all (pure reasoning pass or
restricted sandbox). Open a browser manually and work through this list:

**Load check**
- [ ] Dev server starts without errors (`npm run dev` or `npm run tauri`).
- [ ] `http://localhost:1420/?local_debug=1` (or `:5173`) loads without a blank
      screen or spinner that never resolves.

**Layout check**
- [ ] Left sidebar (filter/playlist panel) is visible and not collapsed.
- [ ] Track list in the centre shows mock tracks — not "Library is empty".
- [ ] Player bar is pinned to the bottom of the window.
- [ ] Right detail pane appears after clicking a track row.

**Console check**
- [ ] Open DevTools (F12 → Console); no red errors on initial load.
- [ ] Interacting with the feature under test does not produce new errors.

**Feature-under-test check**
- [ ] The changed component renders as designed (correct text, spacing, colours).
- [ ] Interactive states (hover, focus, active) look correct.
- [ ] The change does not break adjacent components in the same view.

**Theme check**
- [ ] Verify in the active theme (dark or light) that CSS custom properties
      resolve to expected values (inspect element → Computed styles).

Record which items pass / fail and any unverified states in the final response.

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
