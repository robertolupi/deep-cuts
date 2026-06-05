# Changelog

All notable changes to Deep Cuts will be documented here.

## [0.1.6] — 2026-06-05

### Features

- **Pipeline Pause/Resume Controls**: Added manual pause/resume functionality to the analysis pipeline from the Analysis Panel, along with a "Paused" badge indicator.
- **Automatic Chat Session Pausing**: Automatically pauses the analysis pipeline when entering a chat session to avoid resource conflicts, and automatically resumes it upon navigating away. Includes descriptive toast notifications and inline banner updates.
- **Instruct Qwen Prompt**: Updated the track description analysis prompt to instruct Qwen not to repeat the BPM or key when answering.

---

## [0.1.5] — 2026-06-05

### Features

- **Custom User Tags & Suppression**: Introduced the ability for users to append manual custom tags (defaulting to the `"user"` source namespace) and suppress automatic tags from the analysis pipeline. User tags and suppressions are persisted across scans via the database and backed up in `.dc.json` sidecar files.
- **Visual Distinction for User Tags**: Styled custom user tags in the track detail pane with fully filled backgrounds using the prefix theme color, bold dark-contrast text, and a glowing outer border for clear visual distinction from automated hollow tags.
- **Suppress & Restore Tag Workflows**: Added quick suppression for automated tags via right-click (represented as strikethrough/dimmed chips) and a restoration dropdown menu in the track details pane to easily unsuppress them.
- **Reusable Autocomplete Component**: Refactored the local autocompletion implementations in both the filter sidebar and the track detail pane into a unified, reusable `TagsAutocomplete.svelte` component.
- **Align Tag Search Styles**: Standardized input fields by introducing a `borderless` mode to `Autocomplete.svelte`, ensuring the sidebar tag filter input fits cleanly within its parent wrapper without double borders.

### Fixes

- **User Tag Promotion Conflict**: Handled SQLite conflicts gracefully when a user adds/promotes an existing tag, ensuring the database record updates without key violations.

---

## [0.1.4] — 2026-06-04

### Features

- **Autotagging & Unified Tags UI**: Introduced a system to automatically emit namespace-prefixed tags from multiple analysis stages. Added a unified Tags section in the track detail pane displaying colored chips that filter the library on click. Includes synonym caching, prompt example cleanup, and instrument hallucination guards.
- **Pipeline Metrics Database & Diagnostics Drawer**: Implemented automatic latency and status tracking per track/pass in a local `metrics.db`. Added a diagnostics drawer under Library Settings featuring average latency cards, a recent failures log, and a D3 Gantt trace view aggregated by pass for large libraries.
- **Saved Search Smart Auto-Naming**: Automatically generates descriptive, human-readable names for saved searches based on active filters, complete with Vitest unit tests.
- **Analysis Coverage Panel Stats**: Added detailed Qwen metadata coverage statistics to the library Analysis Coverage panel.

### Fixes

- **TrackList Pagination Reactivity Loop**: Resolved a bug where the TrackList pagination would reset unexpectedly due to a reactivity loop in the `selectedTrack` state effect.
- **Duration Formatting in UI**: Replaced raw seconds with formatted `H:MM:SS` duration strings in the track list, and added millisecond formatting for sub-second durations in the metrics inspector.

---

## [0.1.3] — 2026-06-03

### Features

- **Downgrade llama-server for Metal Audio Stability**: Reverted the bundled `llama-server` to version `b9432` to resolve a critical upstream Metal regression (in `b9433+`) that caused Qwen2-Audio prompts to enter an infinite `@@@` token loop.
- **Improved Music Map Projections & Layouts**: Added *Mood Centroid* projection, *Genre Wheel* circle layout, *Mood coloring scheme* with auto-color toggling, and generalized similarity queries with customizable sonic/description blend weighting.
- **Logarithmic Contour Heatmap Overlays**: Integrated D3 density contour overlays on the Music Map to visualize density clusters of sonic similarity.
- **Fuzzy Gaussian BPM Correction & Joint Preprocessing**: Implemented fuzzy Gaussian correction passes and joint Key/BPM preprocessing to improve alignment and fallback reliability.
- **IDF-Scaled Hybrid Search**: Implemented term-frequency (IDF) scaling for hybrid text/vector searches, combining CLAP audio similarity and textual descriptions.
- **Loudest Region & Analysis Window Markers**: Added visual markers in the player waveform showing the exact 10-second high-energy window analyzed by Essentia/Qwen.
- **Non-Music Skip Optimization**: Automatically checks if a track is non-music (derived from Essentia genre probabilities) and skips heavy spectrogram and AI passes.
- **App Log Opener & License Viewer**: Added an application log file opener and a view of the bundled AGPL-3.0 license directly in the Settings drawer.
- **Faceted & Extended Search**: Composer field is now fully indexed and matched by text filters. Collapsible legend added to the Music Map with automatic hiding on pan/zoom.
- **Mood Filtering**: Seven mood axes (happy, sad, aggressive, relaxed, party, acoustic, electronic) exposed as dual-handle histogram range sliders in the filter sidebar, backed by Essentia mood scores.
- **Mood Radar**: Spider/radar chart visualisation in the track detail pane showing the full mood profile of the selected track.
- **Chat Session Persistence**: Chat conversations are now saved to the database with full-text search and a session picker, so you can return to previous Q&A sessions for any track.
- **Statistics Panel — Set Source Selector**: The Statistics page now lets you scope comparisons to the full library, the current filter, or a specific folder.
- **Reset Analysis Pass**: A per-track menu in the track detail pane lets you clear and re-run any individual analysis pass (BPM, key, Essentia, Qwen, etc.) without re-scanning the whole library.
- **Energy-Based Window Selection for Qwen & Essentia**: Both the Qwen2-Audio description pass and the Essentia classifier pass now pick the most energetically interesting (loudest) 10-second audio window rather than a fixed offset, improving description and tag quality on tracks with long or quiet intros.
- **CLAP-Based Qwen Validation & On-Demand Resampling**: Automatically validates generated Qwen descriptions against the track's CLAP audio embedding. If similarity falls below a calibrated threshold (0.28), it triggers an on-demand resampling pass with a lower temperature (0.2) and strict prompts to automatically correct hallucinations and language slippage.
- **Pass Dependencies & Cascading Resets**: Configured the Qwen analysis pass to depend on CLAP in the pipeline database. Resetting a track's CLAP pass now automatically cascades to clear and re-verify its Qwen descriptions and tags.
- **Save Filtered Results as Playlist**: A new "Save to Playlist" button directly in the filter sidebar allows users to quickly save the current filtered search results into a new or existing playlist.
- **Clickable Metadata Filters**: Artist and Album fields in the track detail pane are now clickable, allowing users to quickly filter the music library by the selected artist or album.
- **Pipeline Execution Optimization**: Reordered analysis phases so that `essentia` and `bpm_refinement` run before `qwen` and `description_embed`, ensuring that refined BPM, key, and mood statistics are available during description generation.

### Fixes

- **Scope Reset Pass**: Fixed the reset analysis pass action to correctly scope the database and column resets to the current track only (preventing multi-track reset bugs).
- **Theme Visibility & Contrast**: Improved the visibility and contrast of WaveSurfer waveforms and the Mood Radar spider chart when using the light theme.
- **Pagination**: Replaced load-more behavior with pagination to avoid performance degradation when updating search filters on large libraries.
- **Bounds-Aware Windows**: Fixed bounds-checking for Qwen, Essentia, and CLAP analysis windows on very short audio files.
- **UI & Theme Enhancements**: Integrated theme-aware waveform styling in the chat pane, unified Update/Network cards, and disabled chat input during active pipeline analysis.

---

## [0.1.2]

### Changes

- Sidecar file writing (`.dc.json`) is now **opt-in** (disabled by default); toggle in Settings.
- Fixed notarization issues blocking macOS Gatekeeper approval.
- App version is now derived automatically from `Cargo.toml` at compile time.
- Updated app icon.

---

## [0.1.1]

### Features

- **AcoustID Metadata Enrichment**: Fingerprints tracks against the AcoustID/MusicBrainz database to fill in missing title, artist, album, and year metadata. Includes cover art fetching with local database caching.
- **Playlists**: Save and load named playlists; export any playlist or filtered set as a native M3U file via the macOS file dialog.
- **Statistics Page**: Compare audio feature distributions (BPM, key, loudness, genre, mood) between the full library and the current filter set, with interactive histograms.
- **WaveSurfer Region Selector in Chat**: Draw a region on the waveform to send only a specific segment to the Qwen2-Audio model, reducing context window usage on long tracks.

### Fixes

- Cap chat audio at 4 minutes to stay within `llama-server` context budget.
- Resolve `llama-server` audio processing bugs and restore SSE streaming with a waiting indicator.
- Smooth-scrolling UI improvements and miscellaneous Svelte 5 / compiler warning fixes.

---

## [0.1.0]

Initial public release.

### Features

- Recursive library scanner with sidecar (`.dc.json`) persistence
- Audio analysis pipeline: BPM (with correction and refinement), key, loudness (EBU R128), waveform, duration
- Essentia ONNX classifier: 400-class Discogs genre, vocal/instrumental detection, seven mood axes
- CLAP audio embeddings (512-dim) stored in `sqlite-vec` for KNN similarity search
- Qwen2-Audio-7B local descriptions via `llama-server`
- all-MiniLM-L6-v2 description embeddings for semantic text search
- Music Map: UMAP 2D projection of CLAP embeddings, D3 zoom/pan, filter-aware, KNN inspection pane
- Full-text and faceted filtering: genre, key, BPM range, vocals, folder, similarity
- WaveSurfer.js waveform and spectrogram player with track detail pane
- **Local Multimodal AI Chat**: Interactive Q&A with any track using Qwen2-Audio-7B-Instruct, with real-time SSE token streaming
- **In-app Model Downloader**: Download, verify, and monitor all neural network models directly inside the app with resumable transfers
- **Startup Update Checks**: Automatic update notifications on launch
- Bundled `llama-server` sidecar with relocation-safe shared libraries
- Duplicates view: CLAP-based similarity detection with clickable track rows
- Sonic Glitch design system with dark, light, and accessible high-contrast themes
- macOS sandbox-compatible, fully offline
