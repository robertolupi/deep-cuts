# Playlist & Saved Search View Enhancements

This document details user interface and layout improvements for playlist and saved search views in Deep Cuts, focusing on transition analysis, reordering, and smart suggestions.

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
