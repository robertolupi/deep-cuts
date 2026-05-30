# UI Redesign Proposal for Deep Cuts

## Context

The current UI is a 4-tab layout (Dashboard, Music Map, Analysis, Settings) with a split pane on the Dashboard for the audio player and track list. The planned feature set (7 feature areas) will add: a semantic search mode, a "Sounds vs. Feels" blending sidebar, UMAP density contour overlays and instrument filters, a DJ Vibe Drift panel, a Pathfinding Playlist builder, a Producer spectral overlay tool, and a Harmonizer widget.

Adding these into the existing tab structure would create a cluttered Dashboard, an overloaded Music Map, and a filter toolbar that does not scale. The proposal below describes a ground-up layout redesign.

---

## Current UI: What Works & What Doesn't

**Works well:**
- The glass-panel aesthetic and cyberpunk color palette are distinctive and coherent.
- The MusicMap is the app's visual centerpiece and deserves more screen real estate.
- The Navbar is clean; four tabs is navigable today.

**Pain points:**
- **The player disappears when you navigate away from Dashboard.** Switching to Music Map or Analysis silently drops the waveform and controls.
- **The split pane compresses both halves.** Player and tracklist compete for the same vertical space and there is nowhere to put a "Sounds vs. Feels" sidebar or a song detail panel.
- **The TrackList filter toolbar doesn't scale.** It already has four controls. Adding semantic search, energy level, and Dig Mode sort pushes it past readability.
- **Analysis and Music Map are siloed tabs** despite features like UMAP contours, Camelot highlights, and Vibe Drift all belonging on the map.
- **No visible playlist queue.** The app auto-advances through filteredTracks but there is no persistent queue, which DJ and pathfinding features require.
- **Settings is a top-level tab** despite being an infrequent destination.

---

## Proposed Layout

```
┌──────────────────────────────────────────────────────────────┐
│  Menu Bar      File · View · Library · Window · Help         │
├────────────┬─────────────────────────────┬───────────────────┤
│            │                             │                   │
│  FILTER    │   Main Content              │   TRACK DETAIL    │
│  SIDEBAR   │                             │   PANE            │
│            │   [⊞ Table]  [⬡ Map]       │                   │
│  🔍 Search │                             │   Title           │
│            │   Table view: track rows    │   Artist · Album  │
│  Genre     │   — or —                    │                   │
│  Key       │   Map view: UMAP scatter    │   Waveform        │
│  BPM       │                             │   Spectrogram     │
│  Energy    │   (both show the same       │                   │
│  Mood      │   filtered data set)        │   Metadata grid   │
│  Format    │                             │   Mood bars       │
│  …         │                             │   AI description  │
│            │                             │   Sounds/Feels    │
│            │                             │                   │
├────────────┴─────────────────────────────┴───────────────────┤
│  [◀◀] [▶] [▶▶]  ━━━━━━━━●━━━  Track Title · Artist  2:14 / 4:52 │
└──────────────────────────────────────────────────────────────┘
```

The layout has four zones: a native menu bar, a left filter sidebar, a central content area that switches between table and map views, a right track detail pane, and a persistent bottom player bar.

---

## Zone 1: Menu Bar

A native macOS/Windows menu bar replaces the current Navbar tab strip.

**Suggested menus:**
- **File**: Reveal in Finder, Export Sidecar Files, Export Playlist
- **View**: Toggle Filter Sidebar, Toggle Detail Pane, Switch to Table View, Switch to Map View, Show Analysis Panel, Theme submenu
- **Library**: Scan Now, Add Folder, Manage Folders (→ Configuration page)
- **Window**: standard window controls
- **Help**: About

**Settings / Configuration** moves out of the tab strip entirely. Library management (add/remove watched folders, scan progress) lives in a Configuration page opened from the Library menu. This frees the four-tab strip and reduces it to zero — navigation is now entirely through the menu bar and the Table/Map toggle in the content area.

The current Navbar brand mark ("DEEP CUTS" shimmer) can be retained as a non-interactive title in the menu bar or window title.

---

## Zone 2: Filter Sidebar (Left)

A collapsible left sidebar housing all facet filters. It applies to both the table view and the map view — the same `filteredTracks` derived store drives both presentations.

**Sections:**
- **Search** — text input at the top, with an `⚡ AI` toggle button to switch to semantic NLP mode. In AI mode the input label changes to "Describe the vibe…" and results are ranked by cosine similarity with match percentage badges.
- **Genre** — tag cloud or checkboxes showing all genres in the current library, with counts. Clicking a genre chip adds it as an active filter.
- **Key** — grid of musical keys (chromatic layout or alphabetical list), with harmonic compatibility highlight when a track is selected.
- **BPM** — dual-handle range slider with preset buttons (Slow / Mid / Fast / V. Fast).
- **Energy Level** — five-step selector (Ambient → Peak-Time), populated by the Essentia/Qwen energy classification pass.
- **Mood** — sliders or range selectors for Happy, Aggressive, Relaxed, etc., shown only when Essentia mood data is present.
- **Format** — checkboxes for file format (FLAC, MP3, WAV, AIFF…).
- **Sort** — dropdown: Default, BPM ↑↓, Key, Duration, Obscurity (Crate Digger mode).

Active filters are summarized as dismissable chips at the top of the sidebar below the search input, so the user can see and clear all active constraints at a glance.

The sidebar is resizable (draggable right edge) and collapsible via the View menu or a toggle button. Default width: ~220px.

---

## Zone 3: Main Content Area (Table / Map Toggle)

A pair of view-mode buttons in the top-right corner of this zone switches between table and map. The active filtered track set is shared — switching views does not reset filters.

**Table View** — the current TrackList grid, promoted to full height without a split pane above it. Columns: #, Title, Waveform thumbnail, Artist, Album, Duration, BPM, Key, Format. Clicking a row selects the track (populates the detail pane) and plays it.

**Map View** — the current MusicMap UMAP scatter, also promoted to full height. Map-specific controls (density contours toggle, instrument spotlight, Camelot key rings, pathfinding mode) appear as a compact floating toolbar anchored to the top of this zone, visible only in Map view.

Selecting a dot on the map selects the track and populates the detail pane, same as clicking a table row. Filters applied in the sidebar dim (but do not hide) out-of-filter dots on the map, preserving spatial context.

The Analysis panel (currently a separate tab) becomes a secondary view mode button: `[⊞ Table] [⬡ Map] [📊 Analysis]`, or it is accessible from the View menu.

---

## Zone 4: Track Detail Pane (Right)

A fixed-width right pane showing rich information about the currently selected track. This is where the metadata and AI analysis that currently live in the collapsible "Details" section of AudioPlayer land permanently, without needing a toggle.

**Contents (top to bottom):**
- **Track header**: title, artist, album, year, format badge.
- **Technical specs**: sample rate, bit depth, bitrate, channels, file size.
- **Mood bars**: Essentia mood scores (Happy, Sad, Aggressive, Relaxed, Party, Acoustic, Electronic) rendered as thin horizontal bars.
- **AI description**: the Qwen-generated prose description of the track (scrollable text block).
- **"Sounds vs. Feels" slider**: the blended similarity slider (Proposal 2 from the original brainstorm). Below it, a ranked list of the top 5–8 similar tracks. Changing the slider weight triggers a debounced IPC call to `get_similar_tracks_blended`.
- **File path**: monospace code block, clickable to reveal in Finder.
- **Lyrics / Comments**: shown when present, collapsed by default.

When no track is selected, the pane shows a placeholder with a vinyl graphic and a prompt to select a track from the list or map.

The pane is resizable (draggable left edge) and collapsible. Default width: ~300px.

---

## Zone 5: Bottom Player Bar

A persistent slim bar pinned to the bottom of the window, always visible regardless of the content view. Playback never disappears on navigation.

**Left section** — track identity: album art placeholder (or animated vinyl), title, artist.

**Center section** — WaveSurfer waveform and spectrogram, stacked vertically so they share the same horizontal time axis and stay perfectly aligned. The waveform is always visible (~48px). The spectrogram is toggleable via a small button (e.g. a spectrum icon) on the right side of this section — when enabled, it expands the bar downward (~48px additional) rather than overlapping. Below both: previous / play-pause / next buttons and a time readout (`2:14 / 4:52`).

**Right section** — action buttons: spectrogram toggle, "Find Similar" (focus the detail pane's similarity list), "Reveal in Finder", volume knob or mute toggle.

Clicking anywhere on the waveform in the bar still scrubs playback position. The spectrogram toggle state is persisted to localStorage so the user's preference survives app restarts. The bar height adjusts smoothly between compact (waveform only) and expanded (waveform + spectrogram) states.

---

## Component Mapping (Current → New)

| Current | New Home |
|---|---|
| `Navbar.svelte` | Native menu bar + Table/Map toggle buttons |
| `HeroPanel.svelte` | Removed; replaced by empty-state in detail pane |
| `AudioPlayer.svelte` | Split: player controls → `PlayerBar.svelte`; metadata/mood/spectrogram → `TrackDetailPane.svelte` |
| `TrackList.svelte` | `TableView.svelte` (full height, no top pane above it) |
| `MusicMap.svelte` | `MapView.svelte` (full height, floating map toolbar) |
| `AnalysisPanel.svelte` | `AnalysisView.svelte` (third view mode button) |
| `LibrarySettings.svelte` | `ConfigurationPage.svelte` (opened from Library menu) |
| *(new)* | `FilterSidebar.svelte` |
| *(new)* | `TrackDetailPane.svelte` |
| *(new)* | `PlayerBar.svelte` |

---

## Open Questions

- **Sidebar placement confirmation**: left is assumed (consistent with Finder/iTunes column browser pattern). Right would mirror the detail pane but create competing sidebars.
- **Table ↔ Map toggle style**: icon buttons in the content area header, or keyboard shortcuts only (⌘1 / ⌘2)?
- **Semantic search in sidebar**: replace the existing text input with a mode-toggle, or keep both inputs separate (one text, one AI)?
- **Map view dimming vs. hiding**: when filters are active, should out-of-filter tracks be dimmed on the map (preserving spatial context) or hidden entirely (cleaner)?
- **Player bar height**: should the bar animate its height change when the spectrogram is toggled (CSS transition), or snap instantly?
