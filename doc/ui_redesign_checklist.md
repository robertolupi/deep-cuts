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

### 1.4 — UI store (`src/lib/stores/ui.svelte.ts`) ✅ complete
- [x] Create `src/lib/stores/ui.svelte.ts`
- [x] Move `activeTab` → `activeView: 'table' | 'map' | 'analysis' | 'settings'`
- [x] Move `mapFocusTrackId` into store
- [x] Move `showToast()`, toast state (`errorMessage`, `successMessage`, timeout) into store
- [x] Update `HeroPanel.svelte` and `TrackList.svelte` to read `activeView` from store
- [x] Update `+page.svelte`: remove ui state

### 1.5 — Update `Track` type ✅ complete
- [x] Add `is_music`, `ai_genre`, `ai_mood`, `ai_instruments`, `description` to `src/lib/types.ts`
- [x] Verify `get_tracks` IPC in `lib.rs` returns these fields (check DB schema)

### 1.6 — De-prop `LibrarySettings` ✅ complete
- [x] `LibrarySettings.svelte` imports `library` store directly (remove 14 props + callbacks)
- [x] Own `name`, `path`, `isAddLoading` state internally; toast via `ui.showToast()`
- [x] Remove all props from `+page.svelte` → `LibrarySettings`

### 1.7 — Slim `+page.svelte` ✅ complete
- [x] Remove split-pane resize logic (`topPaneHeight`, `isResizing`, mouse handlers)
- [x] Remove `showDetails` / `toggleDetails()`
- [x] `+page.svelte` contains only layout skeleton + store `init()` calls
- [x] Commit "refactor: slim +page.svelte — Phase 1 complete"

---

## Phase 2 — UI Implementation (on `ui-redesign` branch)

Open branch once all Phase 1 commits are on `main`:
```
git checkout -b ui-redesign
```

### 2.1 — Sonic Glitch design tokens ✅ DONE in Phase 0

### 2.2 — `+layout.svelte` — app shell + PlayerBar mount ✅ complete
- [x] Create `src/routes/+layout.svelte` with `app-shell` flex-column
- [x] Import and mount `PlayerBar` at layout level
- [x] Move `library.init()` + `theme.init()` here (out of `+page.svelte`)

### 2.3 — `PlayerBar.svelte` (new) ✅ complete
- [x] Left: vinyl thumb (spinning when playing), track title + artist (JetBrains Mono), Find Similar button
- [x] Center: prev/play-pause/next transport + waveform row (WaveSurfer 48px + spectrogram 48px)
- [x] Right: reveal-in-finder, spectrogram toggle
- [x] WaveSurfer containers registered with player store via `register()` on mount
- [x] Spectrogram toggle state persisted to `localStorage` (`deep-cuts-spectrogram`)
- [x] Background: `var(--sg-waveform-bg)` — recessed from glass panels above
- [x] `player.playTrack()` / `handlePrevTrack()` / `handleNextTrack()` — args removed; store imports `theme` + `filters` directly

### 2.4 — `FilterSidebar.svelte` (new) ✅ complete
- [x] Search input + genre autocomplete
- [x] Active filter dismissable chips
- [x] Key note grid (12-note multi-select OR) + scale toggle (All/Maj/Min)
- [x] BPM range slider + presets
- [x] Vocals filter (All / Vocals / Instrumental)
- [x] Music-only toggle (requires is_music = 1)
- [x] Collapsible

### 2.5 — `TrackDetailPane.svelte` (new) ✅ complete
- [x] Empty state (vinyl + prompt) when no track selected
- [x] Track header: title, artist, album, year, genre, format badge, spinning vinyl
- [x] Technical specs grid (all fields including key strength, loudness range, track/disc, composer, album artist)
- [x] Mood bars (Essentia — hidden when all null)
- [x] AI description prose + colour-coded tags (genre=pink, mood=amber, instruments=cyan)
- [x] Essentia classifier section (type, genre, vocal + confidence)
- [x] "Find sounds similar" CLAP-based filter button
- [x] File path (monospace, click → reveal in Finder)
- [x] Lyrics / Comments (always expanded with separate headers)

### 2.6 — `TableView.svelte` (refactor `TrackList.svelte`) ✅ complete
- [x] Remove filter toolbar (filters now in `FilterSidebar`)
- [x] Read `filters.filteredTracks` from store
- [x] Remove `tracks` prop — reads `library.tracks` directly
- [x] Drop legacy `glass-panel` / `bottom-pane-scroller` classes

### 2.7 — `MapView.svelte` (refactor `MusicMap.svelte`) ✅ complete
- [x] Full-size canvas filling workspace
- [x] Remove internal similarity sidebar, details panel, inline audio player
- [x] Dot selection → `player.playTrack()` (TrackDetailPane shows details)
- [x] visibleTracks filtered by `filters.filteredTracks` IDs
- [x] Floating toolbar: track count, color-coding toggle, algo params, Recompute button
- [x] Hover tooltip follows cursor
- [x] FilterSidebar visible in both table and map views
- [x] Theme-aware toolbar colors (--sg-surface-* tokens)

### 2.8 — `AnalysisPanel.svelte` ✅ complete
- [x] Human-readable pass labels with one-line descriptions
- [x] Passes sorted by pipeline execution order
- [x] Per-pass colour accent (theme-aware: pastel in light mode)
- [x] ETA display per pass + global remaining time
- [x] "processing" tag pulses on active pass
- [x] Model warning panel restyled

### 2.9 — `LibrarySettings.svelte` ✅ complete
- [x] Restyle to Sonic Glitch tokens
- [x] Global Toast component added to layout (replaces per-page inline alerts)

### 2.10 — `Navbar.svelte` ✅ complete
- [x] Slim 44px bar: wordmark left, icon+label view toggles centre, theme picker right
- [x] All legacy classes removed, fully scoped styles

---

## Phase 3 — Post-Conversion Cleanup ✅ complete

- [x] Delete `HeroPanel.svelte`
- [x] Delete `AudioPlayer.svelte`
- [x] Remove split-pane, glass-panel, shimmer-text, old navbar, hero-panel, audio-player-pane CSS from `app.css` (−228 lines)
- [x] Update stale comments in `player.svelte.ts`
- [x] `MusicMap` internal similarity sidebar removed
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
