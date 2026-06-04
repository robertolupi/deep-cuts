# Roadmap Ideas & Deferred Brainstorms

This document consolidates various unimplemented, brainstorming, and deferred feature proposals for the Deep Cuts application. These ideas are not currently scheduled or committed for implementation.

---

## 1. DJ & Live Performance Features

### Concept
Advanced mixing utilities to support harmonic mixing, transition analysis, and drop compatibility:
- **Energy Levels**: Classify tracks into 5 Energy Levels (1: Ambient/Warmup to 5: Peak-Time Banger) and index under floor-response categories (*Euphoric*, *Gritty*, *Chill*, *Hypnotic*).
- **Transposition-Aware Camelot Highlight**: Highlight compatible keys on the 2D Music Map within a $\pm 1$ semitone range, complete with pitch-bend badges (e.g. `+1st` / `-1st`).
- **Transition Dynamics Indicators**: Custom blend profile alerts ("Smooth Blend" vs "Dynamic Contrast") signaling energy/genre drift.
- **Double Drop Clash Meter**: Evaluates drop compatibility using **Drop-Aware Spectral Profiling** (extracting 24-band frequency signatures from the loudest contiguous 30-second window) to suggest EQ cuts.

---

## 2. Pathfinding Playlists & Transitions

### Concept
Generate cohesive transitional playlists by mapping geometric "journeys" through the UMAP music map:
- **Map Waypoints**: Let users select start and end tracks, then drag waypoint path lines on the 2D map to route the playlist through specific acoustic regions.
- **A* Pathfinding**: Resolve playlist sequence using a search algorithm that steps through neighboring tracks to form a smooth transition.
- **Cross-Genre Bridges**: Flag major boundaries where distance thresholds are exceeded and recommend bridge techniques (e.g., *Echo-out*, *Tempo-ramping*, or *Power Intro*).

---

## 3. Music Producer & Sampling Features

### Concept
Tools dedicated to sampling, crate digging, and DAW compatibility:
- **Breakbeat & Groove Similarity**: Query the library for drum loops and breaks sharing the same timbral saturation, room acoustics, and swing using transient timing deviation vectors relative to a grid (**Groove Micro-Timing Profiles**).
- **Crate Digger (Obscurity Index)**: Sort library search results by "acoustically obscure / isolated" positions to surface unique textures or rare breakbeats.
- **Lazy Stem Extraction**: Leverage high-vocal energy spectral profiling during library scan, then defer heavy neural stem separation (demixing) to an on-demand download request.
- **Metadata Writeback**: Write analyzed BPM and key back to physical files' ID3/Vorbis/MP4 tags using `lofty`, making them immediately readable by modern DAWs (Ableton, Logic) without re-analysis.

---

## 4. Sonic DNA & Multimodal QA

### Concept
Research-level audio intelligence capabilities:
- **Sonic DNA & DTW**: Timbral evolution mapping over whole durations using sliding-window CLAP extractions plotted on a timeline (showing intros, drops, vocal entries). Use Dynamic Time Warping (DTW) to align timelines of different tracks to spot cross-track similarities.
- **Acoustic EQ Prefiltering**: Isolate low-pass (<150Hz), high-pass (>2kHz), or band-pass (300Hz-3kHz) regions before generating spectrograms to compare tracks strictly by drum groove, percussive swing, or vocal texture.
- **Multimodal Chat QA**: Sidebar conversational chat utilizing local Qwen2-Audio to answer questions about the delivery of the singer, tension builds, or structure.

---

## 5. Sounds vs. Feels Similarity Slider

### Status
- **Backend**: Implemented in `src-tauri/src/commands/map.rs` via `search_similar_tracks_audio` and `blended_embedding_distance` (blending CLAP acoustic and MiniLM description embeddings).
- **Frontend**: **Not Implemented**. Needs a slider UI in Svelte 5 to dynamically control the `clap_weight` parameter, morphing the similar track search results in real time with animations.

---

## 6. UMAP Density Contours & Map Layering

### Status
- **Implemented**: `src/lib/components/MusicMap.svelte` computes and renders background density contour layers using `d3.contourDensity()` reactive state.
- **Not Implemented**: Lasso-to-playlist selection, floating labels for acoustic regions mapped via HDBSCAN clustering, and Web Worker offloading for contour computations to prevent main-thread lag.

---

## 7. Tagging Systems

### Concept
A unified query layer over all tags (Essentia, MusicBrainz, Qwen, and user-defined tags):
- Schema proposal: `tags` and `track_tags` tables.
- visual namespaces (e.g. `mood:`, `genre:`, `instrument:`, `source:`).
- Autocomplete, boolean logic search inputs, and parent-child tags hierarchy.

---

## 8. Hum-to-Search (Query-by-Humming)

### Concept
Record user humming or mumbling via `getUserMedia` and match it against the library:
- **CREPE Pitch Contour Model**: Run CREPE ONNX model locally to extract fundamental frequency (f0) contours.
- **Transposition Invariance**: Normalize pitch sequences using log conversion and mean centering (subtracting average pitch) to allow transposition-free matching.
- **DTW Alignment**: Perform initial k-NN pruning via `sqlite-vec`, then run DTW in Rust to align the candidate list with the tempo-warped query sequence.

---

## 9. BPM Detection Improvements

### Concept
Address octave errors and envelope pollution in ambient intros, vocals, or spoken-word segments:
- **Block Slicing & Voting**: Slice audio into 10-second segments, ignore silent/ambient zones via a silence mask, run autocorrelation, and let blocks vote on the tempo.
- **Fuzzy Genre Post-Correction**: Refactor hard thresholds to Gaussian probability profiles matching genres (e.g., Essentia, Qwen, or iTunes style metadata) to select the correct multiplier (half/double/raw).

---

## 10. Qwen Additional Questions

### Concept
Expanded Qwen VLM prompts to extract additional fields to store in the database:
- `lyrics_language` (en, es, etc.).
- `energy_level` (1-5).
- `listening_context` (club, workout, ambient).
- `era_decade` (70s, 80s, modern).
- `danceability` (low, mid, high).
