# UI Redesign Checklist

Derived from `doc/frontend_migration_plan.md` and the implementation plan artifact.
Work through phases in order. Each section can be paused and resumed.
Phases 1 and 3–4 land directly on `main`. Phase 2 uses a `ui-redesign` branch.

Last session: 2026-05-30

---

## Phase 0 — Design Tokens ✅ COMPLETE (commit f7c6bc1)

- [x] Add `--sg-*` Sonic Glitch token system to `app.css` (dark / light / accessible)
- [x] Dark: Cyber Cyan `#00f0ff` primary, Studio Pink `#fe00fe` secondary, deep indigo surfaces
- [x] Light: DAW warm-neutral palette (Logic/Ableton-inspired: stone surfaces, teal/plum accents)
- [x] Accessible: darker cyan `#00cccc`, black text on filled buttons, all fx suppressed
- [x] Add JetBrains Mono to Google Fonts import
- [x] Add layout constants: `--sg-sidebar-width`, `--sg-detail-pane-width`, `--sg-player-bar-height`
- [x] Backward-compat shim aliases (`--bg-main` → `--sg-surface` etc.)
- [x] Purple primary replaced by cyan across all shim-consuming components

---

## Phase 1 — Pre-UI Refactor (store extractions, invisible changes)

Each item is a separate commit on `main`. App looks and behaves identically throughout.

### 1.1 — Player store (`src/lib/stores/player.svelte.ts`) ✅ commit 85822ff
- [x] Create `src/lib/utils/format.ts` (`formatDuration`, `formatSize`)
- [x] Create `src/lib/stores/player.svelte.ts` with state + methods
- [x] Move `selectedTrack`, `isPlaying`, `currentTime`, `duration` into store
- [x] Move `wavesurfer` + `waveformContainer` / `spectrogramContainer` into store
- [x] Move `playTrack()`, `togglePlayback()`, `resetPlayer()`, `handlePrevTrack()`, `handleNextTrack()` into store
- [x] Update `AudioPlayer.svelte`: remove all bindable props, import store, bind DOM refs on mount
- [x] Update `+page.svelte`: remove extracted state, import player store
- [x] Vitest harness: setup.ts mocks, fixtures.ts, format.test.ts (10), player.test.ts (15) — 25/25 ✓

### 1.2 — Filter store (`src/lib/stores/filters.svelte.ts`) ✅ complete
- [x] Create `src/lib/stores/filters.svelte.ts` with state + derived `filteredTracks`
- [x] Move `searchQuery`, `genreFilter`, `minBpm`, `maxBpm`, `selectedKey` into store
- [x] Move `filteredTracks` `$derived.by(…)` into store
- [x] Remove duplicated `filteredTracks` derivation from `TrackList.svelte`
- [x] Remove filter props bound from `+page.svelte` → `TrackList`
- [x] Update `+page.svelte`: remove filter state

### 1.3 — Theme store (`src/lib/stores/theme.svelte.ts`) ✅ complete
- [x] Create `src/lib/stores/theme.svelte.ts`
- [x] Move `currentTheme`, `resolvedTheme`, `setTheme()` into store
- [x] Move system-preference `$effect` listener into store
- [x] Replace `MutationObserver` in `MusicMap.svelte` with theme store import
- [x] Update `Navbar.svelte`: remove `currentTheme`/`onThemeChange` props, import store
- [x] Update `+page.svelte`: remove theme state, call `theme.init()` in `onMount`

### 1.4 — UI store (`src/lib/stores/ui.svelte.ts`)
- [ ] Create `src/lib/stores/ui.svelte.ts`
- [ ] Move `activeTab` → `activeView: 'table' | 'map' | 'analysis' | 'settings'`
- [ ] Move `mapFocusTrackId` into store
- [ ] Move `showToast()`, toast state (`errorMessage`, `successMessage`, timeout) into store
- [ ] Update `HeroPanel.svelte` and `TrackList.svelte` to read `activeView` from store
- [ ] Update `+page.svelte`: remove ui state

### 1.5 — Update `Track` type
- [ ] Add `is_music`, `ai_genre`, `ai_mood`, `ai_instruments`, `description` to `src/lib/types.ts`
- [ ] Verify `get_tracks` IPC in `lib.rs` returns these fields (check DB schema)

### 1.6 — De-prop `LibrarySettings`
- [ ] `LibrarySettings.svelte` imports `library` store directly (remove 14 props + callbacks)
- [ ] Own `name`, `path`, `isAddLoading`, `errorMessage`, `successMessage` state internally
- [ ] Remove all props from `+page.svelte` → `LibrarySettings`

### 1.7 — Slim `+page.svelte`
- [ ] Remove split-pane resize logic (`topPaneHeight`, `isResizing`, mouse handlers)
- [ ] Remove `showDetails` / `toggleDetails()`
- [ ] Verify `+page.svelte` contains only layout skeleton + store `init()` calls
- [ ] Commit "refactor: slim +page.svelte — Phase 1 complete"

---

## Phase 2 — UI Implementation (on `ui-redesign` branch)

Open branch once all Phase 1 commits are on `main`:
```
git checkout -b ui-redesign
```

### 2.1 — Sonic Glitch design tokens ✅ DONE in Phase 0

### 2.2 — `+layout.svelte` — app shell + PlayerBar mount
- [ ] Create `src/routes/+layout.svelte` with `app-shell` grid
- [ ] Import and mount `PlayerBar` at layout level

### 2.3 — `PlayerBar.svelte` (new)
- [ ] Left: vinyl icon, track title, artist (from player store)
- [ ] Center: WaveSurfer waveform (48px) + spectrogram (48px, toggleable, spring-animates)
- [ ] Right: prev/play-pause/next, time readout, spectrogram toggle, "Find Similar", volume
- [ ] WaveSurfer initialised here (moved from `AudioPlayer.svelte` / player store)
- [ ] Spectrogram toggle state persisted to `localStorage`
- [ ] Background: `var(--sg-waveform-bg)` — recessed from glass panels above

### 2.4 — `FilterSidebar.svelte` (new)
- [ ] Search input + `⚡ AI` mode toggle
- [ ] Active filter dismissable chips
- [ ] Genre chips (most frequent, overflow expand)
- [ ] Camelot key grid (4×6)
- [ ] BPM range slider (reuse `RangeSlider.svelte`)
- [ ] Energy level selector (1–5, shown when ai_genre present)
- [ ] Mood sliders (shown when Essentia mood data present)
- [ ] Format checkboxes
- [ ] Sort dropdown
- [ ] Collapsible

### 2.5 — `TrackDetailPane.svelte` (new)
- [ ] Empty state (vinyl + prompt) when no track selected
- [ ] Track header: title, artist, album, year, format badge
- [ ] Technical specs in `var(--sg-font-mono)`
- [ ] Mood bars (hidden when all null)
- [ ] AI description prose (hidden when `description` null)
- [ ] "Sounds vs. Feels" slider + ranked similar tracks list
- [ ] File path (monospace, click → reveal in Finder)
- [ ] Lyrics / Comments (collapsed by default)
- [ ] Collapsible pane

### 2.6 — `TableView.svelte` (refactor `TrackList.svelte`)
- [ ] Remove filter toolbar (filters now in `FilterSidebar`)
- [ ] Read `filters.filteredTracks` from store
- [ ] Track click → `player.playTrack()` directly
- [ ] Remove `activeTab` prop
- [ ] Keep columns, waveform thumbnails, load-more

### 2.7 — `MapView.svelte` (refactor `MusicMap.svelte`)
- [ ] Remove internal similarity sidebar (→ `TrackDetailPane`)
- [ ] Dot selection → `player.playTrack()` + `player.selectedTrack`
- [ ] Add floating map toolbar (Density Contours, Instrument Spotlight, Pathfinding)
- [ ] Read `ui.mapFocusTrackId` from store (remove prop)
- [ ] Theme detection via theme store (not `MutationObserver`)

### 2.8 — `AnalysisView.svelte` (refactor `AnalysisPanel.svelte`)
- [ ] Real pass display names (audio_analysis, bpm_correction, clap, essentia, qwen, description_embed)
- [ ] Estimated remaining time display
- [ ] Currently-processing track panel (right) with live mood bars

### 2.9 — `ConfigurationPage.svelte` (refactor `LibrarySettings.svelte`)
- [ ] Restyle to Sonic Glitch
- [ ] Add Duplicates card
- [ ] Add Map Configuration card

### 2.10 — `+page.svelte` — four-zone layout
- [ ] `FilterSidebar` | `main content (Navbar + view switch)` | `TrackDetailPane`
- [ ] CSS: `grid-template-columns: var(--sg-sidebar-width) 1fr var(--sg-detail-pane-width)`
- [ ] Slim `Navbar.svelte` to view toggle buttons + Settings

---

## Phase 3 — Post-Conversion Cleanup

- [ ] Delete `HeroPanel.svelte`
- [ ] Delete `AudioPlayer.svelte`
- [ ] Remove split-pane CSS from `app.css`
- [ ] Remove `activeTab` prop from any remaining components
- [ ] Remove `MusicMap`'s internal `similarTracks` state
- [ ] Consolidate CSS — remove per-component `--sg-*` redefinitions
- [ ] Remove backward-compat shim aliases from `app.css`
- [ ] Verify `Track` type coverage matches all fields returned by `get_tracks`

---

## Phase 4 — Testing Completeness

- [ ] Filter store: all filter combinations, BPM null handling, AND logic
- [ ] Player store: state transitions, next/prev wrap-around, auto-advance on finish
- [ ] Theme store: switching, localStorage persistence, system preference
- [ ] `mapMath.ts`: `resolveTrackColor` for all three modes, Camelot key lookups
- [ ] `FilterSidebar`: chip dismissal, key grid interaction
- [ ] `TableView`: renders tracks, empty states, load-more
- [ ] `TrackDetailPane`: conditional section visibility
- [ ] `PlayerBar`: play/pause state, time display, spectrogram toggle
- [ ] Run `npm run check` — zero TypeScript errors
- [ ] Run `cargo test --manifest-path src-tauri/Cargo.toml` — all tests green

---

## Phase 5 — LLM Computer Use Tests (pre-release, manual)

- [ ] Set up `VITE_MOCK_TAURI=true` mode with `src/lib/services/mock-ipc.ts`
- [ ] Map: selected dot has cyan halo; contours appear when toggled
- [ ] Map: out-of-filter dots visually dimmed (not hidden)
- [ ] Player: waveform renders with visible bars; progress cursor moves on play
- [ ] Player: spectrogram visible and time-aligned when toggled
- [ ] Design: primary interactive elements use Cyber Cyan; AI content uses Studio Pink
- [ ] Design: JetBrains Mono for BPM, key, sample rate, file path
- [ ] Design: glass panels have visible backdrop blur
