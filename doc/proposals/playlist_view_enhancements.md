---
status: active
owner: Roberto
last_verified: 2026-06-07
implemented_by:
superseded_by:
related_code:
related_skills:
---

# Playlist & Saved Search View Enhancements

## Current State

Playlist storage, ordering, and saved-search smart auto-naming are implemented. Drag-to-reorder has schema and command support but no polished UI. The energy sparkline, visual BPM/key transition badges, TSP transition optimizer, and vibe-based recommendation panel are all unimplemented proposals pending design review.

---

## Acceptance Criteria

- **User-visible:** Tracks in a playlist can be reordered by drag-and-drop with smooth animated transitions (`animate:flip`); the new order persists after closing and reopening the playlist.
- **User-visible:** A continuous energy sparkline stitched from 128-bin waveform envelopes is displayed below the playlist track list, showing the energy narrative arc of the set.
- **User-visible:** BPM and key-compatibility badges appear between consecutive tracks; green badges for harmonic-compatible transitions, red for incompatible key changes; tempo slope indicators show BPM changes step-by-step.
- **User-visible:** An "Optimize Transitions" action reorders selected tracks in a playlist using TSP over the structural similarity sub-matrix, returning a smoothed sequence.
- **User-visible:** When saving a search, an auto-suggested name is pre-filled based on the active filter state (genre, BPM range, mood); the user can accept or override it.
- **User-visible:** A "Suggested Additions" sidebar panel recommends library tracks based on the CLAP embedding centroid, mood vector, and BPM/key profile of the current playlist/saved search.
- **Data model:** No new tables required; relies on existing `playlists`, `playlist_tracks`, `map_coordinates` (embeddings), and `tracks` (BPM, key, waveform_data) columns.
- **IPC / frontend boundary:** Drag-to-reorder persists via the existing playlist reorder command; a new or updated IPC endpoint is needed to expose the TSP optimizer result; recommendation surface requires a new `suggest_playlist_tracks` command returning ranked `Track` objects.
- **Tests:** Unit test for auto-naming logic covering genre+BPM+mood combinations; integration test for the TSP optimizer confirming it returns a valid permutation; Rust test that CLAP centroid computation returns the correct mean vector for a small fixture set.
- **Local verification:** Drag a track in a playlist, restart the app, confirm order is preserved; open saved-search save dialog and confirm auto-name is populated; click "Optimize Transitions" and verify badge colors change.
- **Theme / accessibility:** Drag handles must be operable via keyboard (arrow-key reorder); transition badges must carry accessible labels/tooltips, not just color.

---

## Implementation Status

This page mixes shipped playlist infrastructure with UI ideas that still need design review.

| Area | Status | Evidence / Notes |
| :--- | :--- | :--- |
| Playlist storage and ordering | Implemented | `playlists` and `playlist_tracks` store ordered tracks, and backend playlist commands support reorder operations. |
| Drag-to-reorder interface | Partially implemented | The data model and command surface support ordering. A dedicated polished drag/drop list flow still needs review against the current UI. |
| Saved search smart auto-naming | Implemented | Filter-based naming is implemented in the saved-search flow. |
| Energy wave sparkline | Need human review | The 128-bin waveform data exists, but no playlist-level stitched sparkline UI was found. |
| Visual connection badges | Need human review | BPM/key data exists, but transition badge UI is still proposal material. |
| Transition pathfinder optimizer | Need human review | Structural similarity data is emerging, but automatic playlist ordering has not been productized. |
| Vibe-based recommendations | Need human review | Embeddings exist; a playlist recommendation surface was not found. |

---

## 1. Drag-to-Reorder Interface
- Implement an interactive drag-and-drop ordering interface in the Svelte 5 frontend.
- Uses Svelte's native `animate:flip` and HTML5 Drag and Drop APIs to let users rearrange tracks manually.

---

## 2. Dynamic Energy Wave Sparkline
- Splicing the 128-point envelopes of the playlist's tracks end-to-end creates a continuous "Energy Timeline" sparkline.
- Displayed below the playlist track list, this graph shows the visual narrative arc (builds, tension, release) of the set.

---

## 3. Visual Connection Badges (Vibe Guide)
To help DJs audit transition compatibility step-by-step:
- **Harmonic Badges**: Check key compatibility between consecutive tracks (e.g. $9A \rightarrow 10A$). Render a glowing green bridge badge for compatible transitions, or a red warning badge for incompatible key changes.
- **Tempo Ramps**: Draw slope indicators showing tempo changes from track to track (flat for same tempo, steep step for tempo jumps).

---

## 4. Transition Pathfinder Sequence Optimizer
- For any set of selected tracks in a playlist, the user can click "Optimize Transitions".
- The system extracts the $K \times K$ structural similarity sub-matrix from the library's global $M M^T$ matrix.
- Solving the Traveling Salesperson Problem (TSP) / Hamiltonian path over the structural distances reorders the tracks automatically to ensure the smoothest arrangement transitions from start to finish.

---

## 5. Saved Search Smart Auto-Naming
When creating or saving a search, the app should assist the user by suggesting a descriptive name automatically:
- **How it works**: Inspects the active filters in `filters.svelte.ts` (e.g., genre: Electronic, BPM range: 120-130, mood: happy).
- **Format**: Generates structured suggestions, e.g. `"Upbeat Electronic (120-130 BPM)"` or `"Classical Minor (60-80 BPM)"`. This reduces naming overhead when saving filter states as smart playlists.

---

## 6. Vibe-Based Playlist Recommendations
Suggest tracks to expand playlists or saved searches based on the common traits of the existing tracks:
- **How it works**: Evaluates the centroid (average coordinates/vectors) of the tracks inside the playlist/saved search in the CLAP embedding space, Qwen mood vector space, and average BPM/keys.
- **Implementation**: Renders a "Suggested Additions" panel in the sidebar, recommending tracks from the library that share these common features.
