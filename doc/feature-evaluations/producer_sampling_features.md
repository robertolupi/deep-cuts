# Technical Evaluation: Music Producer & Sampling Features

## 1. Feature Overview & User Experience
The Music Producer & Sampling suite provides essential tools for mixing, mastering, and hip hop/electronic breakbeat sampling:
* **Reference Mix Matcher & Spectral Overlays**: Drag in a commercial reference track to instantly analyze its loudness, dynamic range, and frequency spectrum. Overlay its curve against your own work-in-progress track inside the app to identify mixing issues (e.g. mud or lack of high-end).
* **Breakbeat & Groove Similarity**: Select a classic 70s drum break and click *"Find similar drum grooves"* to query the library for drum loops and breaks sharing the same timbral saturation, room acoustics, and swing. Powered by **Groove Micro-Timing Profiles**, it matches drum loops based on transient timing deviation vectors relative to a grid.
* **Crate Digger (Obscurity Index)**: Sorts library search queries by "Most Isolated / Acoustically Obscure" to surface isolated recordings, weird interludes, or unique textures.
* **Tiered Vocal Scraper**: Instantly locates vocal-free zones and vocal presence regions using high-vocal energy spectral profiling during the initial library analysis. Rather than running slow neural separation globally, heavy neural stem separation is treated as a lazy, on-demand background process when the user requests a stem download.
* **Drag-to-DAW Export**: Users can apply exact semitone transpositions and time-stretching inside the **Harmonizer Widget** and immediately drag the processed sample directly into their DAW (Ableton, Logic, Pro Tools) via a custom export button. The Rust backend dynamically compiles a high-quality (WSOLA/Rubberband) temporary WAV file on-the-fly for immediate drag-and-drop.

## 2. Technical Feasibility & Architecture

### A. Database Changes
* **Database Schema**:
  - Add `loudness_lufs` (REAL), `dynamic_range` (REAL), and `spectral_profile` (BLOB) to `tracks` table.
  - Add `groove_profile` (BLOB - stores micro-timing transient deviation vectors) to `tracks` table.
  - Add `vocal_energy_profile` (BLOB - stores local frequency ratio time-series) to `tracks` table.
  - Create `13_producer_sampling_columns.sql` database migration.

### B. Rust Backend Services
* **Ad-Hoc Reference Extractor**: Implement `analyze_external_reference(filepath: String) -> Result<ReferenceAnalysis, String>` which:
  1. Decodes the audio file to float samples.
  2. Runs Essentia extractor / BPM / Key detections on the fly.
  3. Computes the LUFS loudness and average 24-band frequency amplitude array.
  4. Returns the result **without persisting it to the library database**, keeping the database clean.
* **WSOLA & Rubberband DSP Export Engine**: Implement an audio processing service in Rust that applies WSOLA (Waveform Similarity Overlap-Add) or Rubberband time-stretching/pitch-shifting. On `dragstart` trigger:
  1. Process the source sample using the calculated pitch/stretch ratios.
  2. Compile and save a high-fidelity temporary WAV file under the app's cache directory (`/Users/rlupi/.gemini/antigravity/temp/`).
  3. Respond to Svelte with the absolute path of the generated `.wav` file to attach to the OS drag-and-drop event loop.
* **Crate Digger Spacing Logic**: Implement `get_track_obscurity_scores()` in Rust:
  * For each track, query its UMAP coordinate distance to its 10 closest neighbors. Tracks with a high average distance are isolated in space, representing acoustically unique, "obscure" files.
* **Groove & Transient Profile Extractor**: Detects onset transients, maps them against the nearest musical beat division, and registers timing offset vectors (e.g. standard deviation in milliseconds from grid beats) to define swing profiles.
* **Tiered Vocal Estimator**: During standard background analysis, calculate the spectral density ratio of vocal bands ($150\text{Hz}-4\text{kHz}$) vs background frequencies to flag sections with high vocal prominence. Only activate the ONNX neural separation engine (lazy-loading) when the user clicks "Export Vocal Stem".

### C. Svelte Frontend Controls
* **Drag-and-Drop Area**: A sleek visual drop-zone in the Settings or a dedicated "Producer Panel".
* **D3 Spectral Overlay**: A visual frequency-domain line chart plotting the 24 frequency bands of the reference track (colored cyan) and the active track (colored magenta).
* **Harmonizer & Drag-to-DAW Widget**: Shows clear numbers like `Transpose: +5 Semitones` and `Stretch: 112.5%`, complete with a "Drag to DAW" button that starts an OS-level file drag action utilizing the rendered temporary WAV path.
* **Groove Deviation Grid**: A visual dot-plot showing transient displacement from a quantized grid, highlighting if a drum break is "rushed", "laid back", or "on the grid".
* **Obscurity Sort Toggle**: Add a "Dig Mode" sorting filter to track list queries.

## 3. Implementation Roadmap & Sizing
* **Phase 1: Core Backend & Data Models**: 3.5 dev-days (ad-hoc reference decoder, obscurity k-NN distance algorithm, WSOLA/Rubberband audio rendering service, groove/vocal analysis models, and database migrations).
* **Phase 2: Svelte Interface & Visual Layers**: 3.5 dev-days (reference drop zone, D3 frequency graph overlay, transpose/stretch drag-and-drop export widget, groove timing display, and search list sorting integration).
* **Phase 3: Polish, Edge Cases, & Tests**: 1.0 dev-day (optimizing decoding of large wav files, testing drag-and-drop compatibility across macOS DAWs like Ableton Live, Logic Pro, and FL Studio).
* **Total Estimated Dev-Time**: 8.0 dev-days

## 4. Performance & Resource Impact
* **CPU / GPU Overhead**: Moderate. Decoding reference tracks and rendering pitch/stretch temp files creates short CPU spikes. Neural stem separation on-demand will utilize GPU/CPU heavily for 10–15 seconds during extraction, but because it is deferred and lazy-loaded, it never slows down the general library importing step.
* **Memory Footprint**: Moderate. Temporary audio buffers are cleared immediately after decoding and file rendering.
* **Database Size Impact**: Negligible ($<2.5\text{MB}$ for 10,000 tracks).

## 5. Technical Uncertainty & Risk Analysis
* **Risk Level**: Medium.
* **DAW Drag-and-Drop Compatibility**: Different DAWs have varying requirements for receiving files via macOS pasteboard (`NSFilenamesPboardType` vs `NSURL`). We must ensure the Tauri backend correctly populates the drag payload using standard macOS filepath properties.
* **Audio Decoding Gaps**: Dragging extremely large audio files (e.g. 24-bit 96kHz 20-minute WAV files) can cause memory spikes. The ad-hoc decoder must read audio in chunks or restrict reference analysis to files under 10 minutes.

## 6. Scoring Matrix & Priority
* **Effort Score**: 7.5 / 10 (8.0 dev-days total due to complex DSP rendering, neural lazy-loading, and OS pasteboard integration)
* **Uncertainty Score**: 4 / 10 (DAW drag-and-drop compatibility across varying software, plus ONNX runtime binding)
* **Performance Impact Score**: 3 / 10 (on-demand rendering and stem extraction will cause short CPU/GPU spikes)
* **Wow Factor Score**: 10 / 10 (dragging perfectly warped and pitch-shifted samples straight into a commercial DAW is a high-end killer feature for music producers)
* **Priority Score**: 9.0 / 10 (blended rating)

### Scoring Rationale
This suite is extremely compelling for music producers. The addition of direct **Drag-to-DAW export** with high-fidelity warping transforms Deep Cuts from a passive management system into an active creative tool. With a 10/10 Wow Factor and highly actionable lazy-loading for stem separation, this ranks as the highest-priority feature set for creative users.
