# Technical Evaluation: DJ & Live Performance Features

## 1. Feature Overview & User Experience
The DJ & Live Performance suite introduces deep, technical mixing utilities to ensure flawless acoustic transitions and energy management:
* **Energy Contours & Crowd Moods**: Classifies tracks into 5 Energy Levels (1: Ambient/Warmup to 5: Peak-Time Banger) and indexes them under floor-response categories (*Euphoric*, *Gritty*, *Chill*, *Hypnotic*).
* **Harmonic UMAP Overlay**: Selecting a playing track highlights harmonically compatible keys (Camelot Wheel relations) on the 2D Music Map, filtering out timbrally jarring choices.
* **Vibe Drift Warnings**: In a playlist, the app flags any sudden, acoustic leaps between tracks with a subtle red alert indicator. Clicking it pops up 2 or 3 suggested "Bridge Tracks" that lie geometrically between the two tracks.
* **Double Drop Clash Meter**: A dual visualizer that evaluates if two tracks can be mixed simultaneously at their drops. It checks if their sub-basses or hi-hats will clash, providing EQ cut suggestions.

## 2. Technical Feasibility & Architecture

### A. Database Changes
* **Database Schema**:
  - Add `energy_level` (INTEGER) to the `tracks` table.
  - Add `spectral_profile` (BLOB - stores a compressed 24-band frequency amplitude array) to the `tracks` table.
* **Migration**: Create `12_dj_performance_columns.sql` to apply these additions.

### B. Rust Backend Services
* **Energy Estimator**: During Qwen/Essentia post-processing, map keywords (e.g. `industrial` + `aggressive` ➔ Energy 5; `acoustic` + `heartfelt` ➔ Energy 1) to populate the `energy_level` column.
* **Drift Detector**: Implement `check_playlist_vibe_drift(track_ids: Vec<i64>) -> Result<Vec<DriftAlert>, String>` which computes Euclidean distances between successive CLAP embeddings, flagging steps that exceed a dynamic threshold.
* **Double Drop Calculator**: Implement `get_double_drop_compatibility(id_a: i64, id_b: i64) -> Result<DoubleDropResult, String>` which:
  1. Loads 24-band `spectral_profile` vectors.
  2. Runs a cross-correlation check in the bass band ($<150\text{Hz}$) and high-end transient band ($>5\text{kHz}$).
  3. Returns a compatibility score ($0-100\%$) and EQ recommendations (e.g. *"Reduce bass on Track B by -6dB during double drop"*).

### C. Svelte Frontend Controls
* **Camelot Key Highlight**: Modify the `MusicMap` renderer to dynamically apply key borders or dim non-compatible Camelot nodes.
* **Crate Alerts**: Add red "drift warning" badges in the playlist track rows with an inline "+" button to insert bridge suggestions.
* **Double Drop Visualizer Panel**: A new DJ-centric dashboard showing the overlaid frequency spectrum curves of the selected track pair with a smooth, glowing compatibility meter.

## 3. Implementation Roadmap & Sizing
* **Phase 1: Core Backend & Data Models**: 3.0 dev-days (database schema migration, spectral profile compressor, vibe drift math, and double drop cross-correlation).
* **Phase 2: Svelte Interface & Visual Layers**: 3.0 dev-days (Double Drop dashboard panel, UMAP key highlighting overlays, playlist drift alerts, and bridge injection UI).
* **Phase 3: Polish, Edge Cases, & Tests**: 1.0 dev-day (refining threshold formulas, testing with multi-genre libraries).
* **Total Estimated Dev-Time**: 7.0 dev-days

## 4. Performance & Resource Impact
* **CPU / GPU Overhead**: Low. Calculating vibe drift on a playlist takes $<1\text{ms}$. Running double drop spectral overlaps takes $<3\text{ms}$ in Rust.
* **Memory Footprint**: Light. Reading two 24-band float arrays takes virtually zero RAM.
* **Database Size Impact**: Minor. Storing 24-band float arrays (96 bytes per track) adds less than $1\text{MB}$ of database storage for a library of 10,000 tracks.

## 5. Technical Uncertainty & Risk Analysis
* **Risk Level**: Medium.
* **Spectral Analysis Complexity**: Translating 24-band frequency curves into musically accurate "clash ratings" requires careful DSP filtering. If the spectral profile is taken globally rather than from the track's drop segment, the clash rating could be inaccurate. We must extract the average spectral profile from the track's *loudest 30-second section* to represent the drop.

## 6. Scoring Matrix & Priority
* **Effort Score**: 7 / 10 (7.0 dev-days total due to multiple modular sub-features)
* **Uncertainty Score**: 4 / 10 (DSP spectral mapping accuracy and relative threshold tuning)
* **Performance Impact Score**: 2 / 10 (very fast database lookups and lightweight math)
* **Wow Factor Score**: 8 / 10 (extremely unique mixing tools that give professional DJs deep acoustic analytics)
* **Priority Score**: 7.5 / 10 (blended rating)

### Scoring Rationale
This suite aggregates four powerful, modular DJ features. While the effort is higher ($7$ days), the performance impact is low, the risk is highly manageable, and the value is extremely high for live performers. Breaking this down and delivering the **Harmonic UMAP Map Matcher** and **Vibe Drift Warnings** first represents the most efficient path.
