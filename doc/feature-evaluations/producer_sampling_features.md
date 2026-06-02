# Technical Evaluation: Music Producer & Sampling Features

## 1. Feature Overview & User Experience
The Music Producer & Sampling suite provides essential tools for mixing, mastering, and hip hop/electronic breakbeat sampling:
* **Breakbeat & Groove Similarity**: Select a classic 70s drum break and click *"Find similar drum grooves"* to query the library for drum loops and breaks sharing the same timbral saturation, room acoustics, and swing. Powered by **Groove Micro-Timing Profiles**, it matches drum loops based on transient timing deviation vectors relative to a grid.
* **Crate Digger (Obscurity Index)**: Sorts library search queries by "Most Isolated / Acoustically Obscure" to surface isolated recordings, weird interludes, or unique textures.
* **Tiered Vocal Scraper**: Instantly locates vocal-free zones and vocal presence regions using high-vocal energy spectral profiling during the initial library analysis. Rather than running slow neural separation globally, heavy neural stem separation is treated as a lazy, on-demand background process when the user requests a stem download.
* **BPM & Key Metadata Writeback**: After analysis, let the user write the detected BPM and key back to the file's ID3/Vorbis/MP4 tags. Modern DAWs (Ableton, Logic) read these natively, so the producer's library is immediately usable without re-analysis inside the DAW. This is preferable to in-app pitch/tempo warping — the DAW will do a far better job of the actual warping once it has accurate metadata.

## 2. Technical Feasibility & Architecture

### A. Database Changes
* **Database Schema**:
  - Add `groove_profile` (BLOB - stores micro-timing transient deviation vectors) to `tracks` table.
  - Add `vocal_energy_profile` (BLOB - stores local frequency ratio time-series) to `tracks` table.
  - Create `13_producer_sampling_columns.sql` database migration.

### B. Rust Backend Services
* **Metadata Writeback**: Implement `write_track_metadata(track_id, bpm, key)` using `lofty` (or `id3`/`metaflac`) to write BPM and key to the file's tags in-place. Should write to standard fields (`TBPM`, `TKEY` for ID3; `BPM`/`KEY` for Vorbis; `tmpo`/`©key` for M4A). Offer a preview of what will be written before committing.
* **Crate Digger Spacing Logic**: Implement `get_track_obscurity_scores()` in Rust:
  * For each track, query its UMAP coordinate distance to its 10 closest neighbors. Tracks with a high average distance are isolated in space, representing acoustically unique, "obscure" files.
* **Groove & Transient Profile Extractor**: Detects onset transients, maps them against the nearest musical beat division, and registers timing offset vectors (e.g. standard deviation in milliseconds from grid beats) to define swing profiles.
* **Tiered Vocal Estimator**: During standard background analysis, calculate the spectral density ratio of vocal bands ($150\text{Hz}-4\text{kHz}$) vs background frequencies to flag sections with high vocal prominence. Only activate the ONNX neural separation engine (lazy-loading) when the user clicks "Export Vocal Stem".

### C. Svelte Frontend Controls
* **Metadata Writeback Button**: In the track detail pane, a "Write to file" button next to BPM and key fields. Shows a confirmation dialog with the exact tag fields that will be written.
* **Groove Deviation Grid**: A visual dot-plot showing transient displacement from a quantized grid, highlighting if a drum break is "rushed", "laid back", or "on the grid".
* **Obscurity Sort Toggle**: Add a "Dig Mode" sorting filter to track list queries.

## 3. Implementation Roadmap & Sizing
* **Phase 1 — Metadata Writeback**: 1.0 dev-day (lofty integration, IPC command, confirmation UI). High value, low risk.
* **Phase 2 — Core Analysis**: 3.0 dev-days (obscurity k-NN distance algorithm, groove/vocal analysis models, database migrations).
* **Phase 3 — Svelte Interface**: 2.5 dev-days (groove timing display, search list sorting integration, vocal estimator display).
* **Phase 4 — Polish & Tests**: 1.0 dev-day.
* **Total Estimated Dev-Time**: ~7.5 dev-days

## 4. Performance & Resource Impact
* **CPU / GPU Overhead**: Low for writeback and obscurity scoring. Neural stem separation on-demand will utilize GPU/CPU heavily for 10–15 seconds during extraction, but because it is deferred and lazy-loaded, it never slows down the general library importing step.
* **Memory Footprint**: Low. Metadata writeback operates on file handles, not decoded audio.
* **Database Size Impact**: Negligible ($<2.5\text{MB}$ for 10,000 tracks).

## 5. Technical Uncertainty & Risk Analysis
* **Risk Level**: Low–Medium.
* **Tag Format Coverage**: `lofty` supports ID3v2, Vorbis, MP4, and AIFF tags. Edge cases may exist for obscure container formats (e.g. WavPack, Musepack) — these can be skipped gracefully with a user-visible warning.
* **File Permissions**: Writing back to files in watched directories requires the app to have write access. On macOS this is generally fine but worth surfacing clearly if it fails.

## 6. Scoring Matrix & Priority
* **Effort Score**: 5 / 10
* **Uncertainty Score**: 2 / 10
* **Performance Impact Score**: 1 / 10
* **Wow Factor Score**: 8 / 10 (producers immediately get a correctly-tagged library usable in any DAW)
* **Priority Score**: 8.5 / 10

### Scoring Rationale
Removing the DAW warping engine significantly reduces complexity and risk with no meaningful loss — modern DAWs do this better anyway. The metadata writeback feature is a natural complement to Deep Cuts' analysis pipeline: the app already knows the BPM and key accurately, and writing that back to the file closes the loop for producers who work across multiple tools.
