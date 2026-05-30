# Frontend Migration Plan

Three sequential phases to move from the current tab-based layout to the four-zone
design described in `doc/ui_ideas.md` and `doc/stitch_deep_cuts_ui_redesign/`.

---

## Current Architecture: Key Problems

Before planning the work it is worth naming exactly what makes the current code hard
to restructure.

**`+page.svelte` is a monolith.** It owns ~430 lines of state and logic including:
WaveSurfer lifecycle, playback controls, filter state, `filteredTracks` derivation,
theme management, toast notifications, split-pane resize, active tab, and the
`mapFocusTrackId` bridge between the player and the map. Everything else is a
prop-receiver.

**WaveSurfer is initialised in `+page.svelte` but renders in `AudioPlayer`.** The
`waveformContainer` and `spectrogramContainer` DOM refs are `$bindable` props passed
down. This coupling prevents extracting the player to a persistent bottom bar at
layout level without a complete rewrite of the binding chain.

**Filter state is duplicated.** Both `+page.svelte` and `TrackList.svelte` compute
their own `filteredTracks` derived state from identical logic. In the new design the
filter sidebar is a sibling of the content area, so filters must live in a shared
store — not in either component.

**MusicMap has its own `selectedTrack` and similarity sidebar**, disconnected from
the global selection. Clicking a dot on the map does not update the player or (in the
new design) the detail pane. The map also observes the DOM directly for theme changes
instead of reading a store.

**`Track` type is missing Qwen fields.** `ai_genre`, `ai_mood`, `ai_instruments`,
`description`, and `is_music` are stored in the database and returned by `get_tracks`
(confirmed in database.rs) but absent from `src/lib/types.ts`. The detail pane
cannot render AI data until these are added.

**`LibrarySettings` receives 14 props and callbacks from `+page.svelte`**, none of
which it needs to receive from above — it could invoke the library store directly.

---

## Phase 1: Pre-UI Refactor

**Goal:** Decompose `+page.svelte` into stores so that components can be freely
rearranged in Phase 2 without re-threading prop chains.

No visible UI changes. Every item here is a pure structural refactor; the app
should behave identically before and after.

### 1.1 — Player store (`src/lib/stores/player.svelte.ts`)

Extract from `+page.svelte`:
- State: `selectedTrack`, `isPlaying`, `currentTime`, `duration`
- WaveSurfer instance management: `wavesurfer`, `waveformContainer`,
  `spectrogramContainer` (keep as store-internal refs, not props)
- Methods: `playTrack()`, `togglePlayback()`, `resetPlayer()`,
  `handlePrevTrack()`, `handleNextTrack()`, `handleFinish()`
- Helpers: `formatDuration()`, `formatSize()`

`+page.svelte` and `AudioPlayer.svelte` both import from the store.
`AudioPlayer.svelte` stops receiving DOM container props — it owns its own
`bind:this` refs and registers them with the store on mount.

This is the most critical prerequisite. Without it, `PlayerBar` cannot be lifted to
`+layout.svelte`.

### 1.2 — Filter store (`src/lib/stores/filters.svelte.ts`)

Extract from `+page.svelte`:
- State: `searchQuery`, `genreFilter`, `minBpm`, `maxBpm`, `selectedKey`
- Derived: `filteredTracks` — single source of truth, replaces the copy in
  `TrackList.svelte`

`TrackList.svelte` removes its own `filteredTracks` derivation and reads from the
store. The filter props bound from `+page.svelte` to `TrackList` are removed.

### 1.3 — Theme store (`src/lib/stores/theme.svelte.ts`)

Extract from `+page.svelte`:
- State: `currentTheme`, `resolvedTheme`
- Methods: `setTheme()`

`MusicMap.svelte` currently observes `document.documentElement` via a
`MutationObserver` to detect theme changes. Replace with an import of `resolvedTheme`
from the store.

### 1.4 — UI store (`src/lib/stores/ui.svelte.ts`)

Extract from `+page.svelte`:
- State: `activeView` (replaces `activeTab`, values: `'table' | 'map' | 'analysis'`)
- State: `mapFocusTrackId`
- State: `toastMessage`, `toastType`, toast timeout
- Method: `showToast()`

Components that currently receive `activeTab` as a prop (HeroPanel, TrackList) read
from the store instead.

### 1.5 — Update `Track` type (`src/lib/types.ts`)

Add missing Qwen/analysis fields to the `Track` interface:
```typescript
// Qwen AI analysis
is_music: number | null;       // 1 = music, 0 = non-music
ai_genre: string | null;
ai_mood: string | null;
ai_instruments: string | null;
description: string | null;
```

Verify these are returned by the `get_tracks` IPC command in `lib.rs`. If not, add
them to the SELECT query there.

### 1.6 — De-prop `LibrarySettings`

Remove all 14 props and their matching callbacks from `+page.svelte`.
`LibrarySettings.svelte` imports `library` store directly and owns its own
`name`, `path`, `isAddLoading`, `errorMessage`, `successMessage` state internally.

### 1.7 — Slim `+page.svelte`

After the above extractions, `+page.svelte` should be reduced to mounting stores
on `onMount`, rendering the layout skeleton, and nothing else. The split-pane resize
logic and `topPaneHeight` state are removed entirely (the split pane disappears in
Phase 2).

---

## Phase 2: UI Implementation

**Goal:** Implement the four-zone layout from the Stitch mockups. Build new components,
rename/repurpose existing ones, wire everything to the stores from Phase 1.

### 2.1 — Apply Sonic Glitch design tokens

Add CSS custom properties to `app.css` (or a new `theme.css`):
```css
--color-surface: #121318;
--color-surface-low: #1a1b21;
--color-surface-container: #1e1f25;
--color-primary: #00f0ff;        /* Cyber Cyan — replaces --color-accent-cyan */
--color-secondary: #fe00fe;      /* Studio Pink — replaces --color-accent-magenta */
--color-on-surface: #e3e1e9;
--color-outline: #849495;
--color-border-glass: rgba(255, 255, 255, 0.08);
--color-glow-cyan: rgba(0, 240, 255, 0.4);
--color-glow-pink: rgba(255, 0, 255, 0.3);
--font-ui: 'Inter', sans-serif;
--font-mono: 'JetBrains Mono', monospace;
--sidebar-width: 260px;
--detail-pane-width: 320px;
--player-bar-height: 80px;
```

Existing `--color-accent-cyan: #00f2fe` is close enough to `#00f0ff` that a
find-and-replace across component styles is sufficient.

### 2.2 — `+layout.svelte` — mount PlayerBar

```svelte
<script>
  import PlayerBar from '$lib/components/PlayerBar.svelte';
</script>

<div class="app-shell">
  <div class="app-content">{@render children()}</div>
  <PlayerBar />
</div>
```

`PlayerBar` is always mounted, always visible. Playback persists across view switches.

### 2.3 — `PlayerBar.svelte` (new)

Extracted from the center column of `AudioPlayer.svelte`. Reads from the player store.

- Left: vinyl icon, track title, artist
- Center: WaveSurfer waveform (48px) + spectrogram (48px, toggleable). Both rendered
  here so they stay time-aligned. The spectrogram toggle expands the bar with a CSS
  height transition; state persisted to localStorage.
- Right: prev/play-pause/next, time readout, spectrogram toggle icon, "Find Similar"
  button (sets `mapFocusTrackId` in UI store), volume.

The `waveformContainer` and `spectrogramContainer` refs live here. WaveSurfer is
initialised here, not in `+page.svelte`.

### 2.4 — `FilterSidebar.svelte` (new)

Reads/writes the filter store. Collapsed by default, toggled by a button or the View
menu.

Sections (in order):
1. Search input with AI mode toggle (`⚡`)
2. Genre chips (derived from `library.tracks`, most frequent genres shown, overflow
   into an expand button)
3. Camelot key grid (4×6 grid of key buttons — improvement over the current dropdown,
   adopted from the Stitch mockup)
4. BPM range slider (reuses `RangeSlider.svelte`)
5. Energy level selector (1–5, shown only when `ai_genre` data is present)
6. Mood sliders (shown only when Essentia mood data is present)
7. Format checkboxes
8. Sort dropdown (Default / BPM ↑↓ / Key / Duration / Obscurity)

Active filters are shown as dismissable chips at the top of the sidebar.

### 2.5 — `TrackDetailPane.svelte` (new)

Reads `selectedTrack` from the player store. Shown on the right side of the layout.
Collapses to an overlay if the window is too narrow.

Sections (top to bottom):
1. Track header: title, artist, album, year, format badge
2. Technical specs: sample rate, bit depth, bitrate, channels, size (JetBrains Mono)
3. Mood bars: Essentia mood scores (shown only when present)
4. AI description: Qwen prose (shown only when `description` is non-null)
5. "Sounds vs. Feels" slider + ranked similar tracks list (calls
   `get_similar_tracks_blended` IPC — see `doc/feature-evaluations/sounds_feels_similarity_slider.md`)
6. File path (monospace, click to reveal in Finder)
7. Lyrics / Comments (collapsed by default)

Empty state: vinyl graphic + "Select a track to explore" when `selectedTrack` is null.

### 2.6 — `TableView.svelte` (rename / refactor `TrackList.svelte`)

- Remove the filter toolbar row entirely — filters now live in `FilterSidebar`.
- Read `filteredTracks` from the filter store instead of computing locally.
- Keep the table structure, waveform thumbnails, load-more pagination.
- Track selection calls `player.playTrack(track)` directly.
- Remove `activeTab` prop.

### 2.7 — `MapView.svelte` (refactor `MusicMap.svelte`)

- Remove the internal similarity sidebar — it moves to `TrackDetailPane`.
- Selecting a dot calls `player.playTrack()` + sets the global `selectedTrack`,
  populating the detail pane.
- Add a floating map toolbar (top of the canvas) for map-specific controls:
  Density Contours toggle, Instrument Spotlight, Pathfinding mode.
  Currently these live in a clunky sidebar inside MusicMap — promote them to
  the floating toolbar shown in the Stitch mockup.
- Map receives `mapFocusTrackId` from the UI store instead of as a prop.
- Replace `MutationObserver` theme detection with `resolvedTheme` from theme store.
- Wire the algorithm/parameter controls (already present as `$state` in MusicMap)
  to the settings configuration.

### 2.8 — `AnalysisView.svelte` (refactor `AnalysisPanel.svelte`)

- Replace fictional pass names with real ones, presented with user-friendly labels:
  | Internal name | Display label |
  |---|---|
  | `audio_analysis` | Waveform & Metadata |
  | `bpm_correction` | BPM Detection |
  | `bpm_refinement` | BPM Refinement |
  | `clap` | Acoustic Embeddings (CLAP) |
  | `qwen` | AI Description (Qwen) |
  | `description_embed` | Semantic Embeddings |
  | `essentia` | Genre, Mood & Loudness |
- Add estimated remaining time calculation based on per-pass throughput (the
  `throughputBaseline` mechanism is already there — just surface the estimate).
- The right pane showing the currently-processing track with live mood bars is shown
  in the mockup — add this.

### 2.9 — `ConfigurationPage.svelte` (rename `LibrarySettings.svelte`)

Self-contained after Phase 1.6. Becomes a view accessed via Settings tab (or later,
via native menu). Add:
- Duplicates card (scan button + grouped list from `track_relationships` table)
- Map configuration card (algorithm selector, parameter sliders, blend weight — using
  the settings schema from `doc/music_map_improvements.md`)

### 2.10 — `+page.svelte` — four-zone layout

```svelte
<div class="app-layout">
  <Navbar />                          <!-- view toggle + settings button -->
  <div class="workspace">
    <FilterSidebar />
    <main class="content-area">
      {#if activeView === 'table'}   <TableView />
      {:else if activeView === 'map'} <MapView />
      {:else}                         <AnalysisView />
      {/if}
    </main>
    <TrackDetailPane />
  </div>
</div>
```

`Navbar.svelte` is slimmed to just the view toggle buttons (Table / Map / Analysis)
and a Settings button, with the brand mark.

---

## Phase 3: Post-Conversion Cleanup

**Goal:** Remove dead code, consolidate styles, and verify nothing is left over from
the old architecture.

### 3.1 — Delete obsolete components

| Component | Reason |
|---|---|
| `HeroPanel.svelte` | Replaced by empty state in `TrackDetailPane` |
| `AudioPlayer.svelte` | Split into `PlayerBar` + `TrackDetailPane` |

### 3.2 — Remove dead state from `+page.svelte`

- `topPaneHeight`, `isResizing`, `preDetailsHeight`, split-pane mouse handlers
- `showDetails`, `toggleDetails()`
- `waveformContainer`, `spectrogramContainer` (moved to PlayerBar)
- `isPlaying`, `currentTime`, `duration`, `wavesurfer` (moved to player store)
- `searchQuery`, `genreFilter`, `minBpm`, `maxBpm`, `selectedKey` (moved to filter store)
- `selectedTrack` (moved to player store)
- `mapFocusTrackId` (moved to UI store)
- `currentTheme`, `resolvedTheme`, `setTheme` (moved to theme store)
- Toast state (moved to UI store)
- `filteredTracks` (moved to filter store)
- `formatDuration`, `formatSize` (moved to player store or a utils file)

### 3.3 — Remove duplicated `filteredTracks` from `TrackList.svelte`

Confirmed removed in Phase 2.6, but verify no remnants.

### 3.4 — CSS consolidation

- Replace all instances of `--color-accent-cyan: #00f2fe` with `--color-primary`
- Replace `--color-accent-magenta` / `--ff007f` with `--color-secondary`
- Remove per-component `<style>` blocks that redefine shared variables
- Move all layout constants (sidebar width, player bar height, etc.) to the
  single token definition in `app.css`

### 3.5 — Verify `Track` type coverage

After Phase 1.5, confirm that every field returned by `get_tracks` in `lib.rs`
is represented in `types.ts`. Any field present in the DB but absent from the
type silently returns `undefined` in the frontend.

### 3.6 — Remove `activeTab` prop threading

Verify no component still receives `activeTab` as a prop. The only consumer should
be the UI store.

### 3.7 — Remove MusicMap's internal similarity panel

Confirmed removed in Phase 2.7. Check that `similarTracks` state and the
`search_similar_tracks_audio` IPC call are removed from `MapView.svelte` — they
now live in `TrackDetailPane`.

---

## Component Summary

| Current | Phase | Action | New Name |
|---|---|---|---|
| `+layout.svelte` | 2.2 | Add PlayerBar mount | — |
| `+page.svelte` | 1, 2.10 | Gut to layout skeleton | — |
| `Navbar.svelte` | 2.10 | Slim to view toggle + settings | — |
| `HeroPanel.svelte` | 3.1 | Delete | — |
| `AudioPlayer.svelte` | 2.3, 2.5, 3.1 | Split then delete | `PlayerBar` + `TrackDetailPane` |
| `TrackList.svelte` | 2.6 | Remove filters, read store | `TableView` |
| `MusicMap.svelte` | 2.7 | Remove similarity sidebar, floating toolbar | `MapView` |
| `AnalysisPanel.svelte` | 2.8 | Real pass names, remaining time | `AnalysisView` |
| `LibrarySettings.svelte` | 1.6, 2.9 | De-prop, add duplicates + map config | `ConfigurationPage` |
| `RangeSlider.svelte` | — | Keep as-is | — |
| *(new)* | 2.4 | — | `FilterSidebar` |
| *(new)* | 2.5 | — | `TrackDetailPane` |
| *(new)* | 2.3 | — | `PlayerBar` |

## Store Summary

| Store | New | Contains |
|---|---|---|
| `library.svelte.ts` | No (keep as-is) | tracks, directories, scan state |
| `player.svelte.ts` | Yes | selectedTrack, WaveSurfer, playback controls |
| `filters.svelte.ts` | Yes | searchQuery, genre/key/bpm filters, filteredTracks |
| `theme.svelte.ts` | Yes | currentTheme, resolvedTheme, setTheme |
| `ui.svelte.ts` | Yes | activeView, mapFocusTrackId, toast |

---

## Phase 4: Frontend Testability

**Goal:** Make the frontend testable in isolation — no Tauri process, no real audio
files, no backend. Tests run in CI with `npm test`.

### 4.1 — Why the current code is untestable

Every meaningful piece of logic is blocked by one of three hard Tauri dependencies:

- `invoke()` and `listen()` from `@tauri-apps/api` — called directly in stores and
  components, crash immediately outside a Tauri WebView context.
- `convertFileSrc()` — converts a filesystem path to a `asset://` URL, meaningless
  in JSDOM.
- `WaveSurfer.create()` — creates a Web Audio API context. JSDOM does not implement
  the Web Audio API.

There is currently no abstraction layer over any of these, and no test runner
configured.

### 4.2 — Toolchain setup

Install as dev dependencies:

```
vitest                         # Vite-native test runner
@vitest/coverage-v8            # Code coverage
@testing-library/svelte        # Component rendering (Svelte 5 compatible from v5)
@testing-library/jest-dom      # DOM assertion matchers (.toBeVisible, .toHaveText…)
jsdom                          # DOM environment for component tests
```

Add a `test` block to `vite.config.ts`:

```typescript
test: {
  environment: 'jsdom',
  setupFiles: ['src/test/setup.ts'],
  include: ['src/**/*.{test,spec}.{ts,svelte}'],
  globals: true,
}
```

Add `src/test/setup.ts`:

```typescript
import '@testing-library/jest-dom';
import { vi } from 'vitest';

// Mock Tauri IPC globally — every test gets a clean slate via vi.mocked()
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
  convertFileSrc: vi.fn((path: string) => `mock-asset:///${path}`),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(),
}));

// Mock WaveSurfer — Web Audio API is not available in jsdom
vi.mock('wavesurfer.js', () => ({
  default: {
    create: vi.fn(() => ({
      load: vi.fn(),
      play: vi.fn(),
      pause: vi.fn(),
      playPause: vi.fn(),
      destroy: vi.fn(),
      on: vi.fn(),
      getDuration: vi.fn(() => 240),
      getCurrentTime: vi.fn(() => 0),
      setOptions: vi.fn(),
    })),
  },
}));

vi.mock('wavesurfer.js/dist/plugins/spectrogram.esm.js', () => ({
  default: { create: vi.fn(() => ({})) },
}));
```

Add a `test` script to `package.json`:

```json
"test": "vitest run",
"test:watch": "vitest",
"test:coverage": "vitest run --coverage"
```

### 4.3 — IPC mocking strategy

Rather than building a full service-layer abstraction (which would require changing
all store signatures), Vitest's module mocking is used. The `@tauri-apps/api/core`
module is mocked globally in `setup.ts` (see 4.2). Individual tests configure the
mock return values per-call:

```typescript
import { invoke } from '@tauri-apps/api/core';
import { vi } from 'vitest';

vi.mocked(invoke).mockImplementation((cmd: string) => {
  if (cmd === 'get_tracks') return Promise.resolve(MOCK_TRACKS);
  if (cmd === 'get_watched_directories') return Promise.resolve(MOCK_DIRS);
  return Promise.resolve(null);
});
```

This keeps stores unchanged and avoids adding indirection purely for tests.

The one exception is the **player store**: WaveSurfer needs a DOM container ref to
initialise. The store should check `if (typeof window === 'undefined') return` before
calling `WaveSurfer.create()`, and in tests the mock WaveSurfer is used automatically
via the global mock in `setup.ts`.

### 4.4 — Mock data fixtures (`src/test/fixtures.ts`)

A single fixture file exports factory functions with realistic defaults and
per-test overrides:

```typescript
import type { Track, WatchedDirectory } from '$lib/types';

export function createTrack(overrides: Partial<Track> = {}): Track {
  return {
    id: 1,
    watched_directory_id: 1,
    path: '/music/test_track.flac',
    filename: 'test_track.flac',
    size_bytes: 45_000_000,
    last_modified: 1700000000,
    duration_seconds: 240,
    sample_rate: 44100,
    bitrate: 1411,
    channels: 2,
    bit_depth: 24,
    title: 'Test Track',
    artist: 'Test Artist',
    album: 'Test Album',
    genre: 'Rock---Alternative Rock',
    year: 2020,
    track_number: 1,
    track_total: 12,
    disc_number: 1,
    disc_total: 1,
    album_artist: 'Test Artist',
    composer: null,
    comment: null,
    bpm: 128,
    lyrics: null,
    waveform_data: null,
    key: 'A',
    scale: 'minor',
    key_strength: 0.85,
    loudness_lufs: -14.2,
    loudness_range: 8.1,
    detected_genre: 'Rock---Alternative Rock',
    detected_vocal: 'instrumental',
    detected_vocal_confidence: 0.91,
    mood_happy: 0.3,
    mood_sad: 0.1,
    mood_aggressive: 0.7,
    mood_relaxed: 0.2,
    mood_party: 0.5,
    mood_acoustic: 0.1,
    mood_electronic: 0.6,
    is_music: 1,
    ai_genre: 'rock, alternative rock',
    ai_mood: 'aggressive, energetic',
    ai_instruments: 'electric guitar, bass guitar, drums',
    description: 'A high-energy alternative rock track driven by distorted guitars.',
    ...overrides,
  };
}

export function createDir(overrides: Partial<WatchedDirectory> = {}): WatchedDirectory {
  return { id: 1, name: 'Test Library', path: '/music', ...overrides };
}

// Pre-built collections for common scenarios
export const MOCK_TRACKS: Track[] = [
  createTrack({ id: 1, title: 'Track One', bpm: 90, key: 'C', scale: 'major', genre: 'Jazz' }),
  createTrack({ id: 2, title: 'Track Two', bpm: 140, key: 'A', scale: 'minor', genre: 'Rock---Heavy Metal' }),
  createTrack({ id: 3, title: 'Track Three', bpm: 110, key: 'F', scale: 'major', genre: 'Jazz', description: null }),
  createTrack({ id: 4, title: 'Silent Track', bpm: null, key: null, scale: null, is_music: 0 }),
];

export const MOCK_DIRS: WatchedDirectory[] = [createDir()];
```

### 4.5 — What to unit test (stores and utils)

These have no DOM dependency and run as pure Vitest unit tests.

**`src/lib/utils/mapMath.ts`** — already pure functions, easiest to test first:
```typescript
// resolveTrackColor returns correct colour for genre / camelot / bpm modes
// camelotMap contains expected codes for all 24 keys
// BPM interpolation clamps correctly at boundaries
```

**`src/lib/stores/filters.svelte.ts`** — the most valuable test target. All filter
combinations, edge cases, and the `filteredTracks` derivation:
```typescript
// searchQuery matches title, artist, album, filename
// genreFilter is case-insensitive and partial
// BPM range excludes tracks with null BPM when range is narrowed
// Key filter excludes tracks missing key or scale
// Multiple filters combine with AND logic
// filteredTracks resets displayLimit when any filter changes
// Clearing all filters returns the full track list
```

**`src/lib/stores/player.svelte.ts`** — state transitions using the mocked WaveSurfer:
```typescript
// playTrack sets selectedTrack and calls WaveSurfer.load
// togglePlayback calls playPause on the WaveSurfer instance
// handleNextTrack advances to the next track in filteredTracks
// handlePrevTrack wraps around to the last track at index 0
// resetPlayer nulls selectedTrack and destroys the WaveSurfer instance
// handleFinish auto-advances (calls handleNextTrack)
```

**`src/lib/stores/theme.svelte.ts`**:
```typescript
// setTheme('dark') writes to localStorage and sets data-theme on <html>
// setTheme('system') reads prefers-color-scheme
// resolvedTheme is 'dark' or 'light', never 'system'
```

### 4.6 — What to component test

These use `@testing-library/svelte` and the Tauri IPC mock from `setup.ts`.

**`FilterSidebar.svelte`**:
```typescript
// Renders search input
// Typing in search updates the filter store
// Clicking a genre chip toggles the genre filter
// Active filters appear as chips above the sidebar
// Clicking a chip's × clears that filter
// Key grid: clicking a key button sets selectedKey in the store
// BPM slider updates minBpm / maxBpm
```

**`TableView.svelte`** (replaces TrackList):
```typescript
// Renders one row per track in filteredTracks
// Shows playing bars animation on the active row
// Shows "No matching tracks" empty state when filteredTracks is empty
// Shows "Library is empty" empty state when library.tracks is empty
// "Load More" button appears when filteredTracks.length > displayLimit
// Clicking a row calls player.playTrack()
```

**`TrackDetailPane.svelte`**:
```typescript
// Shows empty state when selectedTrack is null
// Shows title, artist, album when selectedTrack is set
// Hides mood bars section when mood fields are all null
// Hides AI description section when description is null
// Shows AI description when description is present
// Shows Qwen genre/mood/instruments when present
// "Reveal in Finder" button exists and has correct title attribute
```

**`PlayerBar.svelte`**:
```typescript
// Shows track title and artist when selectedTrack is set
// Play button renders when isPlaying is false
// Pause button renders when isPlaying is true
// Clicking play/pause calls player.togglePlayback()
// Time readout shows formatted currentTime / duration
// Spectrogram is hidden by default
// Spectrogram toggle button expands the bar
```

### 4.7 — Where testing work fits in the phases

Testing infrastructure and fixtures are set up **once at the start of Phase 1** —
before any store extraction begins. This way each store is written with a test
alongside it, and regressions are caught immediately as `+page.svelte` is
dismantled.

| When | Work |
|---|---|
| Start of Phase 1 | Install toolchain, write `setup.ts`, write `fixtures.ts` |
| Phase 1.1 (player store) | Write player store unit tests |
| Phase 1.2 (filter store) | Write filter store unit tests — highest value |
| Phase 1.3 (theme store) | Write theme store unit tests |
| Phase 1.5 (types.ts) | Update fixture `createTrack` with new Qwen fields |
| Phase 2 (each new component) | Write component test alongside each component |
| Phase 3 (cleanup) | Verify test coverage hasn't dropped; delete tests for deleted components |

### 4.8 — What not to test

- **`MusicMap` / `MapView` canvas rendering** — D3 canvas operations are meaningless
  in JSDOM. Test the data transformations (coordinate scaling, colour resolution,
  `resolveTrackColor`) as unit tests on pure functions instead. Canvas rendering
  is validated visually via the Streamlit prototype.
- **WaveSurfer waveform rendering** — mocked entirely; visual quality is validated
  by running the app.
- **IPC command implementations** — those are Rust and covered by `cargo test`.
- **Full end-to-end flows** — out of scope for this phase. A Playwright suite wired
  to a dev Tauri build is a natural Phase 5, but not a prerequisite for the UI
  migration.

---

## Phase 5: LLM Computer Use Tests

**Goal:** Cover the rendering surface that jsdom cannot reach — canvas, WebAudio, and
design-system compliance — using an LLM agent that looks at actual screenshots of the
running app.

### Why it fits here

Three categories of UI behaviour are untestable with Vitest + jsdom:

| Behaviour | Why jsdom fails | LLM agent can… |
|---|---|---|
| D3 canvas map rendering | Canvas 2D context not implemented | Verify selected dot has cyan halo; contours appear when toggled |
| WaveSurfer waveform | Web Audio API not implemented | Verify waveform renders; progress cursor moves on play |
| Spectrogram expand animation | CSS transitions not run | Verify bar height increases when toggle is clicked |
| Sonic Glitch design compliance | No visual output | Verify colours, spacing, font treatment match mockup |

Traditional Playwright screenshot diffs catch pixel regressions but cannot reason
semantically. They break on any cosmetic change and cannot say *why* something looks
wrong. An LLM agent produces assertions like "the selected track row has a left cyan
border and the row background is slightly lighter than neighbouring rows" — robust to
minor visual drift and meaningful when they fail.

### Setup

The test suite spins up the Vite dev server with a `VITE_MOCK_TAURI=true` environment
variable. When that flag is set, a thin shim in `src/lib/services/mock-ipc.ts`
intercepts all `invoke` calls and returns fixture data from `src/test/fixtures.ts`
instead of hitting the Tauri backend. This means the full Svelte app renders in a
normal browser with realistic data, no Tauri process needed.

```typescript
// src/lib/services/mock-ipc.ts  (loaded only when VITE_MOCK_TAURI=true)
import { MOCK_TRACKS, MOCK_DIRS } from '../test/fixtures';

export function mockInvoke(cmd: string): Promise<unknown> {
  const responses: Record<string, unknown> = {
    'get_tracks': MOCK_TRACKS,
    'get_watched_directories': MOCK_DIRS,
    'get_track_count': MOCK_TRACKS.length,
    'get_theme': 'dark',
    // add as needed
  };
  return Promise.resolve(responses[cmd] ?? null);
}
```

The Chrome MCP tools (`mcp__Claude_in_Chrome`) can then drive the live dev server,
take screenshots, and verify rendered output semantically.

### Test cases

**Map view:**
- Toggle Density Contours on → contour lines appear over the dots
- Select a track → that dot has a visible cyan outer glow
- Apply a genre filter → non-matching dots visually dimmer than matching ones
- Click "Find Similar" in player bar → map focuses and highlights the correct dot

**Player bar:**
- Load a track → waveform renders with visible bars (not blank)
- Click play → progress cursor is visible moving left-to-right
- Toggle spectrogram → bar expands downward, frequency gradient visible

**Design compliance (run once after Phase 2 lands):**
- Primary interactive elements use `#00f0ff` (Cyber Cyan)
- AI-generated content uses `#fe00fe` (Studio Pink) accent
- Track detail pane mood bars animate on track load
- JetBrains Mono is used for BPM, key, sample rate, file path values
- Glass panels have visible backdrop blur against the dark background

### Integration with CI

LLM computer use tests are slow and require a running browser — they are not part of
`npm test`. They run as a separate `npm run test:visual` script, invoked manually
before a release or after a significant UI change. They do not block `cargo test` or
the Vitest suite.

---

## Branching Strategy

The two questions of "parallel UIs?" and "long-lived branch?" are related and worth
addressing explicitly.

### Why a parallel old/new UI is not worth it

Maintaining both UIs simultaneously would require:
- Keeping the old `+page.svelte` prop chains working while Phase 1 extracts state
  into stores — doubling the work of every store extraction.
- Two CSS systems (current theme tokens + Sonic Glitch tokens) co-existing.
- Routing (`/` vs `/v2`) that would need to be stripped out after the migration.

This is a single-user desktop app, not a public web product. There is no user base to
protect from a UI transition. The cost of parallelism outweighs the benefit.

### Recommended approach: two-phase landing on `main`

**Phase 1 lands directly on `main` as individual PRs**, one per store extraction.
Each PR leaves the app visually identical and fully functional. These are net
improvements to the code structure regardless of the UI redesign and should not be
gated on it. They also unblock the testability work.

**Phase 2 opens a short-lived `ui-redesign` branch** once all Phase 1 PRs are
merged. The layout is rebuilt on the branch and merged as a single PR when the
four-zone layout is functional end-to-end. The branch should not live longer than
1–2 weeks given the pace of development (68 commits landed in a short period — the
redesign is well within reach quickly with LLM-assisted coding).

**Phases 3 and 4 (cleanup and tests) follow immediately** on `main` after the merge.

### Handling the ongoing Qwen pass

The Qwen analysis pass is still running against the live database. Phase 1 and Phase
2 changes touch only frontend files — no Rust analysis code, no DB schema, no IPC
commands. The pass runs uninterrupted throughout the redesign.
