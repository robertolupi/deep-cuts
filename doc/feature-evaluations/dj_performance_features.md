# Technical Evaluation: DJ & Live Performance Features

## 1. Feature Overview & User Experience
The DJ & Live Performance suite introduces deep, technical mixing utilities to ensure flawless acoustic transitions and energy management:
* **Energy Contours & Crowd Moods**: Classifies tracks into 5 Energy Levels (1: Ambient/Warmup to 5: Peak-Time Banger) and indexes them under floor-response categories (*Euphoric*, *Gritty*, *Chill*, *Hypnotic*).
* **Transposition-Aware Camelot Highlight**: Selecting a playing track highlights compatible Camelot keys on the 2D Music Map, expanding the search by showing compatible keys within a $\pm 1$ semitone range, complete with pitch-bend badges (e.g., `+1st` or `-1st`) indicating the target adjustments for harmonic mixing.
* **Transition Dynamics Indicators**: Reframes simple vibe drift warnings into descriptive transition alerts based on custom blend profiles ("Smooth Blend" for progressive, seamless transitions vs "Dynamic Contrast" for high-impact genre/energy shifts).
* **Double Drop Clash Meter**: A dual visualizer that evaluates if two tracks can be mixed simultaneously at their drops. Powered by **Drop-Aware Spectral Profiling**, it extracts 24-band frequency signatures from the loudest contiguous 30-second drop window rather than a global average, providing highly accurate EQ cut suggestions to prevent muddy overlaps.

## 2. Technical Feasibility & Architecture

### A. Database Changes
* **Database Schema**:
  - Add `energy_level` (INTEGER) to the `tracks` table.
  - Add `spectral_profile` (BLOB - stores a compressed 24-band frequency amplitude array) to the `tracks` table.
  - Add `drop_start_time` (REAL) and `drop_end_time` (REAL) to store the identified drop window offsets.
* **Migration**: Create `12_dj_performance_columns.sql` to apply these additions.

### B. Rust Backend Services
* **Drop-Aware Spectral Profiler**: Instead of calculating a global frequency average, this service scans the decoded audio stream using a sliding 30-second window to locate the loudest contiguous section (highest average RMS energy). The 24-band spectral profile is extracted exclusively from this "drop window".
* **Energy Estimator**: During Qwen/Essentia post-processing, map keywords (e.g. `industrial` + `aggressive` ➔ Energy 5; `acoustic` + `heartfelt` ➔ Energy 1) to populate the `energy_level` column.
* **Transition Dynamics & Drift Analyzer**: Implement `check_playlist_transitions(track_ids: Vec<i64>) -> Result<Vec<TransitionAlert>, String>` which computes Euclidean distances between successive CLAP embeddings and compares Camelot keys. Steps are classified into mix profiles:
  - **Smooth Blend**: Low embedding distance, matching or adjacent Camelot keys, low BPM delta.
  - **Dynamic Contrast**: High embedding distance or sudden key/BPM shifts that are flagged for high impact.
* **Double Drop Calculator**: Implement `get_double_drop_compatibility(id_a: i64, id_b: i64) -> Result<DoubleDropResult, String>` which:
  1. Loads drop-specific 24-band `spectral_profile` vectors.
  2. Runs a cross-correlation check in the bass band ($<150\text{Hz}$) and high-end transient band ($>5\text{kHz}$).
  3. Returns a compatibility score ($0-100\%$) and EQ recommendations (e.g. *"Reduce bass on Track B by -6dB during double drop"*).

### C. Svelte Frontend Controls
* **Transposition-Aware Camelot Highlight**: Modify the `MusicMap` renderer to dynamically apply key borders or dim non-compatible Camelot nodes. Nodes that are compatible via a $\pm 1$ semitone pitch-bend are highlighted with dynamic badges (e.g., `+1st` or `-1st`) to show the necessary pitch adjustment.
* **Transition Dynamics Indicators**: In the playlist queue, replace simple warning flags with dynamic badges displaying either "Smooth Blend" (green badge) or "Dynamic Contrast" (orange/red warning badge), with a tooltip describing transition mechanics and suggesting bridge tracks.
* **Double Drop Visualizer Panel**: A new DJ-centric dashboard showing the overlaid frequency spectrum curves of the selected track pair's drop windows with a smooth, glowing compatibility meter.

## 3. Implementation Roadmap & Sizing
* **Phase 1: Core Backend & Data Models**: 3.5 dev-days (database schema migration, drop window detector, spectral profile compressor, vibe drift math, and double drop cross-correlation).
* **Phase 2: Svelte Interface & Visual Layers**: 3.5 dev-days (Double Drop dashboard panel, UMAP key highlighting overlays with pitch-bend badges, playlist transition dynamics alerts, and bridge injection UI).
* **Phase 3: Polish, Edge Cases, & Tests**: 1.0 dev-day (refining threshold formulas, testing with multi-genre libraries).
* **Total Estimated Dev-Time**: 8.0 dev-days

## 4. Performance & Resource Impact
* **CPU / GPU Overhead**: Low. Calculating transition dynamics on a playlist takes $<2\text{ms}$. Running double drop spectral overlaps takes $<3\text{ms}$ in Rust. Locating the loudest 30-second drop window adds a brief, single-pass RMS scan during initial file scanning ($<100\text{ms}$).
* **Memory Footprint**: Light. Reading two 24-band float arrays takes virtually zero RAM.
* **Database Size Impact**: Minor. Storing 24-band float arrays and drop offsets adds less than $1.2\text{MB}$ of database storage for a library of 10,000 tracks.

## 5. Technical Uncertainty & Risk Analysis
* **Risk Level**: Medium.
* **Spectral Analysis Complexity**: Translating 24-band frequency curves into musically accurate "clash ratings" requires careful DSP filtering. If the spectral profile is taken globally rather than from the track's drop segment, the clash rating could be inaccurate. We mitigate this by extracting the spectral profile exclusively from the loudest contiguous 30-second drop window.
* **Transposition Mapping**: Pitch shifting tracks by $\pm 1$ semitone might introduce digital artifacts if the time-stretching and pitch-shifting engine is low quality. We must ensure a high-fidelity DSP engine is utilized in the player.

## 6. Scoring Matrix & Priority
* **Effort Score**: 7.5 / 10 (8.0 dev-days total due to advanced DSP scanning and dynamic UI badges)
* **Uncertainty Score**: 4 / 10 (DSP spectral mapping accuracy and relative threshold tuning)
* **Performance Impact Score**: 2 / 10 (very fast database lookups and lightweight math)
* **Wow Factor Score**: 9 / 10 (extremely unique mixing tools that give professional DJs deep acoustic analytics and transposition suggestions)
* **Priority Score**: 8.0 / 10 (blended rating)

### Scoring Rationale
This suite aggregates four powerful, modular DJ features. While the effort is moderate ($8$ days), the performance impact is low, the risk is highly manageable, and the value is extremely high for live performers. The addition of transposition-aware highlighting and drop-aware profiling guarantees that the mixing recommendations are highly professional.
