# DSP Architecture Recommendations: Caching Intermediate Features

To improve structural alignment and enable post-alignment snapping refinement without performance degradation, we recommend caching intermediate DSP features (specifically **beat/onset timestamps** and **chroma time-series**) during the initial `audio_analysis` pass.

---

## 1. Rationale

Currently, `run_audio_analysis` in `dsp.rs` discards frame-level features (such as spectral flux/onsets and block chroma vectors) after resolving global summary statistics (such as key, scale, and BPM). 

Storing these intermediate results provides three main benefits:
* **Zero-Recalculation Snapping**: Any boundary snapping/refinement algorithm (in Rust or external Python scripts) can snap the rigid 16-bin SAX boundaries to musical beats or onsets in **sub-millisecond time** without re-reading or re-decoding audio files.
* **Segment-Level Retrieval**: Allows section-level similarity analysis (e.g., comparing the chroma/harmonic similarity of Verse 1 in Track A vs Verse 1 in Track B).
* **UI Richness**: Enables rendering beat grids, onset markers, or chroma-based visual colors onto the frontend waveform player.

---

## 2. Recommended Caching Strategies

### Option A: `.dc.json` Sidecar Files (Recommended for Time-Series)
Since the app already caches analysis passes in a sidecar `.dc.json` next to the audio file, we can expand it to store high-resolution feature arrays.

#### Proposed JSON Schema Extension:
```json
{
  "pass_versions": {
    "audio_analysis": 1,
    "sax_alignment": 1
  },
  "dsp_features": {
    "beat_onsets": [0.45, 0.98, 1.42, 1.95, 2.47, 3.01],
    "onset_strengths": [0.85, 0.42, 0.91, 0.12, 0.64, 0.77],
    "chroma_time_step": 0.2,
    "chroma_series": [
      [0.05, 0.12, 0.81, 0.02, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
      [0.04, 0.10, 0.84, 0.02, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]
    ]
  }
}
```
* **Pros**: Prevents database bloating, easily readable by python evaluation scripts, lightweight.

---

### Option B: SQLite Database (Recommended for Relational Querying)
If relational search queries (e.g., "find tracks with high onset density during the intro") are required, the data can be stored in the database.

#### Schema Changes:
1. Add columns to the `tracks` table (using binary blobs for compression):
   ```sql
   ALTER TABLE tracks ADD COLUMN beat_onsets BLOB; -- Compressed float32 array
   ALTER TABLE tracks ADD COLUMN chroma_series BLOB; -- Compressed low-res chroma frames
   ```
2. Or create a dedicated relational table `track_onsets`:
   ```sql
   CREATE TABLE track_onsets (
       track_id INTEGER NOT NULL,
       timestamp_seconds REAL NOT NULL,
       strength REAL,
       FOREIGN KEY(track_id) REFERENCES tracks(id) ON DELETE CASCADE
   );
   CREATE INDEX idx_track_onsets_track_id ON track_onsets(track_id);
   ```

---

## 3. Implementation Blueprint (dsp.rs / audio.rs)

1. **Extend symphonia decoding outputs**:
   During `run_audio_analysis`, accumulate:
   * **Beat Onsets**: Peaks from the spectral flux autocorrelation lag mapping.
   * **Chroma Frames**: Frame-level chroma vectors accumulated in block hops.

2. **Update the Rust pipeline struct**:
   Modify `AudioAnalysisResult` in `src-tauri/src/dsp.rs`:
   ```rust
   pub struct AudioAnalysisResult {
       // ... existing fields ...
       pub beat_onsets: Vec<f32>,
       pub chroma_series: Vec<[f32; 12]>,
       pub chroma_time_step: f32,
   }
   ```

3. **Update Database/Sidecar Writer**:
   Extend `run_audio_analysis_phase` in `src-tauri/src/analysis/audio.rs` to serialize and write the vectors to `.dc.json` and/or SQLite.
