# UI Redesign Checklist

Derived from `doc/frontend_migration_plan.md`. Work through phases in order.
Phases 1 and 3–4 land directly on `main`. Phase 2 uses a `ui-redesign` branch.

---

## Phase 1 — Pre-UI Refactor (store extractions, invisible changes)

Each item is a separate PR. App looks and behaves identically throughout.

### Toolchain & fixtures (prerequisite for all testing)
- [ ] Install `vitest`, `@vitest/coverage-v8`, `@testing-library/svelte`, `@testing-library/jest-dom`, `jsdom`
- [ ] Add `test` block to `vite.config.ts`
- [ ] Write `src/test/setup.ts` (mock `@tauri-apps/api/core`, `@tauri-apps/api/event`, `wavesurfer.js`)
- [ ] Write `src/test/fixtures.ts` (`createTrack()`, `createDir()`, `MOCK_TRACKS`, `MOCK_DIRS`)
- [ ] Add `test`, `test:watch`, `test:coverage` scripts to `package.json`

### 1.1 — Player store (`src/lib/stores/player.svelte.ts`)
- [ ] Move `selectedTrack`, `isPlaying`, `currentTime`, `duration` into store
- [ ] Move `wavesurfer` instance + `waveformContainer` / `spectrogramContainer` refs into store
- [ ] Move `playTrack()`, `togglePlayback()`, `resetPlayer()`, `handlePrevTrack()`, `handleNextTrack()` into store
- [ ] Move `formatDuration()`, `formatSize()` into store (or `src/lib/utils/format.ts`)
- [ ] Update `AudioPlayer.svelte` to own its DOM refs and register them with the store on mount
- [ ] Update `+page.svelte` to remove extracted state
- [ ] Write unit tests for player store state transitions

### 1.2 — Filter store (`src/lib/stores/filters.svelte.ts`)
- [ ] Move `searchQuery`, `genreFilter`, `minBpm`, `maxBpm`, `selectedKey` into store
- [ ] Move `filteredTracks` derived state into store (single source of truth)
- [ ] Remove duplicated `filteredTracks` derivation from `TrackList.svelte`
- [ ] Remove filter props bound from `+page.svelte` to `TrackList`
- [ ] Write unit tests for all filter combinations and edge cases

### 1.3 — Theme store (`src/lib/stores/theme.svelte.ts`)
- [ ] Move `currentTheme`, `resolvedTheme`, `setTheme()` into store
- [ ] Replace `MutationObserver` in `MusicMap.svelte` with store import
- [ ] Write unit tests for theme switching and localStorage persistence

### 1.4 — UI store (`src/lib/stores/ui.svelte.ts`)
- [ ] Move `activeTab` → `activeView` (`'table' | 'map' | 'analysis'`) into store
- [ ] Move `mapFocusTrackId` into store
- [ ] Move toast state and `showToast()` into store
- [ ] Update `HeroPanel.svelte` and `TrackList.svelte` to read `activeView` from store

### 1.5 — Update `Track` type
- [ ] Add `is_music`, `ai_genre`, `ai_mood`, `ai_instruments`, `description` to `src/lib/types.ts`
- [ ] Verify `get_tracks` IPC command in `lib.rs` returns these fields
- [ ] Update `createTrack()` fixture with new fields

### 1.6 — De-prop `LibrarySettings`
- [ ] Remove all 14 props and callbacks passed from `+page.svelte`
- [ ] `LibrarySettings.svelte` imports `library` store directly
- [ ] `LibrarySettings.svelte` owns its own `name`, `path`, `isAddLoading`, `errorMessage`, `successMessage` state

### 1.7 — Slim `+page.svelte`
- [ ] Remove split-pane resize logic (`topPaneHeight`, `isResizing`, mouse handlers)
- [ ] Remove `showDetails` / `toggleDetails()`
- [ ] Verify `+page.svelte` contains only layout skeleton and store initialisation

---

## Phase 2 — UI Implementation (on `ui-redesign` branch)

Open branch once all Phase 1 PRs are merged.

### 2.1 — Sonic Glitch design tokens
- [ ] Add CSS custom properties to `app.css` (surface, primary cyan, secondary pink, typography, spacing tokens)
- [ ] Replace `--color-accent-cyan: #00f2fe` → `--color-primary: #00f0ff` across all component styles
- [ ] Replace magenta accent → `--color-secondary: #fe00fe`
- [ ] Add `--sidebar-width: 260px`, `--detail-pane-width: 320px`, `--player-bar-height: 80px`

### 2.2 — `+layout.svelte` — mount PlayerBar
- [ ] Add `app-shell` layout wrapper
- [ ] Import and mount `PlayerBar` at layout level (persistent across all views)

### 2.3 — `PlayerBar.svelte` (new)
- [ ] Left: vinyl icon, track title, artist
- [ ] Center: WaveSurfer waveform (48px, always visible) + spectrogram (48px, toggleable)
- [ ] Spectrogram toggle expands bar height with CSS transition; state persisted to localStorage
- [ ] Right: prev/play-pause/next, time readout, spectrogram toggle icon, "Find Similar" button, volume
- [ ] WaveSurfer initialised here (moved from `+page.svelte`)
- [ ] Write component tests (play/pause state, time display, spectrogram toggle)

### 2.4 — `FilterSidebar.svelte` (new)
- [ ] Search input with AI mode toggle (`⚡`)
- [ ] Genre chips (most frequent genres, overflow expand button)
- [ ] Camelot key grid (4×6, replaces key dropdown)
- [ ] BPM range slider (reuse `RangeSlider.svelte`)
- [ ] Energy level selector (1–5, shown only when ai_genre data present)
- [ ] Mood sliders (shown only when Essentia mood data present)
- [ ] Format checkboxes
- [ ] Sort dropdown (Default / BPM ↑↓ / Key / Duration / Obscurity)
- [ ] Active filters shown as dismissable chips at top
- [ ] Collapsible (toggle button or View menu)
- [ ] Write component tests (chip toggling, key grid, filter store updates)

### 2.5 — `TrackDetailPane.svelte` (new)
- [ ] Empty state (vinyl graphic + prompt) when `selectedTrack` is null
- [ ] Track header: title, artist, album, year, format badge
- [ ] Technical specs in JetBrains Mono (sample rate, bit depth, bitrate, channels, size)
- [ ] Mood bars (hidden when all mood fields null)
- [ ] AI description prose (hidden when `description` null)
- [ ] "Sounds vs. Feels" blending slider + ranked similar tracks list
- [ ] File path (monospace, click to reveal in Finder)
- [ ] Lyrics / Comments section (collapsed by default)
- [ ] Collapsible pane
- [ ] Write component tests (conditional section visibility)

### 2.6 — `TableView.svelte` (refactor `TrackList.svelte`)
- [ ] Remove filter toolbar row (filters now in `FilterSidebar`)
- [ ] Read `filteredTracks` from filter store
- [ ] Track selection calls `player.playTrack()` directly
- [ ] Remove `activeTab` prop
- [ ] Keep table columns, waveform thumbnails, load-more pagination
- [ ] Write component tests (row rendering, empty states, load-more)

### 2.7 — `MapView.svelte` (refactor `MusicMap.svelte`)
- [ ] Remove internal similarity sidebar (moved to `TrackDetailPane`)
- [ ] Selecting a dot calls `player.playTrack()` and sets global `selectedTrack`
- [ ] Add floating map toolbar: Density Contours toggle, Instrument Spotlight, Pathfinding mode
- [ ] Read `mapFocusTrackId` from UI store (remove prop)
- [ ] Replace `MutationObserver` theme detection with theme store import
- [ ] Wire algorithm/parameter controls to settings configuration

### 2.8 — `AnalysisView.svelte` (refactor `AnalysisPanel.svelte`)
- [ ] Replace fictional pass names with real display labels (see migration plan §2.8 table)
- [ ] Add estimated remaining time display (throughput baseline already computed)
- [ ] Add currently-processing track panel on the right with live mood bars

### 2.9 — `ConfigurationPage.svelte` (refactor `LibrarySettings.svelte`)
- [ ] Rename and restyle to match Sonic Glitch
- [ ] Add Duplicates card (scan button + relationship list)
- [ ] Add Map Configuration card (algorithm selector, parameter sliders, blend weight)

### 2.10 — `+page.svelte` — four-zone layout
- [ ] Render `FilterSidebar` + main content area + `TrackDetailPane`
- [ ] Main content switches between `TableView`, `MapView`, `AnalysisView` based on UI store
- [ ] Slim `Navbar.svelte` to view toggle buttons (Table / Map / Analysis) + Settings button

---

## Phase 3 — Post-Conversion Cleanup

- [ ] Delete `HeroPanel.svelte`
- [ ] Delete `AudioPlayer.svelte`
- [ ] Remove remaining dead state from `+page.svelte` (split-pane, showDetails, etc.)
- [ ] Remove `activeTab` prop from any remaining components
- [ ] Remove `MusicMap`'s internal `similarTracks` state and `search_similar_tracks_audio` call (moved to detail pane)
- [ ] Consolidate CSS — remove per-component redefinitions of shared variables
- [ ] Verify `Track` type coverage matches all fields returned by `get_tracks`
- [ ] Confirm no component still receives `activeTab` as a prop

---

## Phase 4 — Testing Completeness

- [ ] Filter store: all filter combinations, BPM null handling, AND logic across filters
- [ ] Player store: state transitions, next/prev wrap-around, auto-advance on finish
- [ ] Theme store: switching, localStorage persistence, system preference
- [ ] `mapMath.ts`: `resolveTrackColor` for all three modes, Camelot key lookups
- [ ] `FilterSidebar`: chip dismissal, key grid interaction
- [ ] `TableView`: renders tracks, empty states, load-more
- [ ] `TrackDetailPane`: conditional section visibility for all null/non-null combinations
- [ ] `PlayerBar`: play/pause state, time display, spectrogram toggle
- [ ] Run `npm test` — all tests green
- [ ] Run `npm run test:coverage` — review coverage report

---

## Phase 5 — LLM Computer Use Tests (pre-release, manual)

- [ ] Set up `VITE_MOCK_TAURI=true` mode with `src/lib/services/mock-ipc.ts`
- [ ] Map: selected dot has cyan halo; contours appear when toggled
- [ ] Map: out-of-filter dots are visually dimmed (not hidden)
- [ ] Player: waveform renders with visible bars; progress cursor moves on play
- [ ] Player: spectrogram visible and time-aligned with waveform when toggled
- [ ] Design compliance: primary interactive elements use Cyber Cyan; AI content uses Studio Pink
- [ ] Design compliance: JetBrains Mono used for BPM, key, sample rate, file path
- [ ] Design compliance: glass panels have visible backdrop blur
