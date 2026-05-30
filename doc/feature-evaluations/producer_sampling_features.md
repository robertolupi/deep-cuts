# Technical Evaluation: Music Producer & Sampling Features

## 1. Feature Overview & User Experience
The Music Producer & Sampling suite provides essential tools for mixing, mastering, and hip hop/electronic breakbeat sampling:
* **Reference Mix Matcher & Spectral Overlays**: Drag in a commercial reference track to instantly analyze its loudness, dynamic range, and frequency spectrum. Overlay its curve against your own work-in-progress track inside the app to identify mixing issues (e.g. mud or lack of high-end).
* **Breakbeat & Groove Similarity**: Select a classic 70s drum break and click *"Find similar drum grooves"* to query the library for drum loops and breaks sharing the same timbral saturation, room acoustics, and swing.
* **Crate Digger (Obscurity Index)**: Sorts library search queries by "Most Isolated / Acoustically Obscure" to surface isolated recordings, weird interludes, or unique textures.
* **Vocal & Instrumental Scraper**: Instantly locates clean, vocal-free sections featuring solo instruments (e.g. solo piano or strings) ready to sample.
* **Pitch & Tempo Sampler Harmonizer**: Select a sample and dynamically calculate the exact semitone transpose steps and time-stretch percentages to fit your current active DAW project (e.g., G-Minor at 90 BPM).

## 2. Technical Feasibility & Architecture

### A. Database Changes
* **Database Schema**:
  - Add `loudness_lufs` (REAL), `dynamic_range` (REAL), and `spectral_profile` (BLOB) to `tracks` table.
  - Create `13_producer_sampling_columns.sql` database migration.

### B. Rust Backend Services
* **Ad-Hoc Reference Extractor**: Implement `analyze_external_reference(filepath: String) -> Result<ReferenceAnalysis, String>` which:
  1. Decodes the audio file to float samples.
  2. Runs Essentia extractor / BPM / Key detections on the fly.
  3. Computes the LUFS loudness and average 24-band frequency amplitude array.
  4. Returns the result **without persisting it to the library database**, keeping the database clean.
* **Crate Digger Spacing Logic**: Implement `get_track_obscurity_scores()` in Rust:
  * For each track, query its UMAP coordinate distance to its 10 closest neighbors. Tracks with a high average distance are isolated in space, representing acoustically unique, "obscure" files.
* **Harmonizer Calculator**: Implement a simple math helper mapping pitch transposition:
  $$\Delta_{\text{semitones}} = 12 \cdot \log_2 \left( \frac{f_{\text{target}}}{f_{\text{source}}} \right)$$
  and time stretching ratio:
  $$\text{Stretch Ratio} = \frac{\text{BPM}_{\text{target}}}{\text{BPM}_{\text{source}}} \cdot 100\%$$

### C. Svelte Frontend Controls
* **Drag-and-Drop Area**: A sleek visual drop-zone in the Settings or a dedicated "Producer Panel".
* **D3 Spectral Overlay**: A visual frequency-domain line chart plotting the 24 frequency bands of the reference track (colored cyan) and the active track (colored magenta).
* **Harmonizer Widget**: Shows clear numbers like `Transpose: +5 Semitones` and `Stretch: 112.5%` with a "Copy to Clipboard" button.
* **Obscurity Sort Toggle**: Add a "Dig Mode" sorting filter to track list queries.

## 3. Implementation Roadmap & Sizing
* **Phase 1: Core Backend & Data Models**: 2.5 dev-days (ad-hoc reference decoder, obscurity k-NN distance algorithm, database migration, and transposing math).
* **Phase 2: Svelte Interface & Visual Layers**: 2.5 dev-days (reference drop zone, D3 frequency graph overlay, transpose calculator display, and search list sorting integration).
* **Phase 3: Polish, Edge Cases, & Tests**: 1.0 dev-day (handling file formats, optimizing decoding of large wav files).
* **Total Estimated Dev-Time**: 6.0 dev-days

## 4. Performance & Resource Impact
* **CPU / GPU Overhead**: Low. Decoding and analyzing a dragged reference track takes 1–3 seconds of CPU on the fly. Normal library queries run on pre-computed indices and execute instantly.
* **Memory Footprint**: Low. Temporary audio buffers are cleared immediately after decoding.
* **Database Size Impact**: Negligible ($<1.5\text{MB}$ for 10,000 tracks).

## 5. Technical Uncertainty & Risk Analysis
* **Risk Level**: Low.
* **Obscurity Math Consistency**: If the UMAP projection changes, the obscurity score will shift. This is expected and natural.
* **Audio Decoding Gaps**: Dragging extremely large audio files (e.g. 24-bit 96kHz 20-minute WAV files) can cause memory spikes. The ad-hoc decoder must read audio in chunks or restrict reference analysis to files under 10 minutes.

## 6. Scoring Matrix & Priority
* **Effort Score**: 6 / 10 (6.0 dev-days total)
* **Uncertainty Score**: 3 / 10 (simple physics equations for pitch/stretch; straightforward SQL sort on k-NN distance)
* **Performance Impact Score**: 2 / 10 (short CPU spike when importing reference tracks)
* **Wow Factor Score**: 9 / 10 (delivers immediate, highly practical tools for beatmakers and audio engineers)
* **Priority Score**: 8.5 / 10 (blended rating)

### Scoring Rationale
This suite is extremely compelling for music producers. The **Crate Digger Mode** and **Transpose Calculator** can be built in less than 2 days with near-zero uncertainty, yielding immediate high value. The **D3 Spectral Overlay** represents a slightly larger effort (3 days) but has a spectacular "wow" factor, justifying the 8.5 priority rating.
