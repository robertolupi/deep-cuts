# Preparation Plan: Hum-to-Search (Query-by-Humming)

This preparation plan outlines the implementation path for Hum-to-Search (Query-by-Humming) in the Deep Cuts application. The feature allows users to record hummed/sung melodies and search their library using pitch contours.

---

## 1. Goal & Requirements

### Goal
Allow the user to search their audio library by humming or singing a melody. The application will record the hummed query, extract its pitch contour, and find matches in the database using a fast, two-stage vector-pruning and sequence-alignment pipeline.

### Core Requirements
1. **Model Download & Integration**: Add the `crepe_tiny.onnx` model (~10MB) to the download manager.
2. **Analysis Pipeline Integration**: Integrate a new `CrepePass` analysis pass that processes tracks on import/scan.
3. **Pitch Contour Extraction (CREPE)**: Run the CREPE model on 16 kHz downsampled mono audio frames. The model outputs pitch probabilities across 360 cent-spaced classes (32.7 Hz to 1976 Hz).
4. **Transposition Invariance**: Convert pitch values to cents, and mean-center the voiced frames (subtracting average pitch) so transposition/key variations are ignored.
5. **Melody Pruning (Stage 1)**: Interpolate/downsample each track's pitch contour to a fixed 128-float vector and index it in `sqlite-vec` (`track_pitch_embeddings`). Run a fast k-NN query to narrow the library to the top 100 candidates.
6. **Subsequence DTW Alignment (Stage 2)**: Perform **Subsequence Dynamic Time Warping (sDTW)** in Rust on the candidate tracks' full pitch contours. This allows a short query (e.g. 10 seconds of humming) to match any segment (like a chorus or bridge) in the candidate song.
7. **Audio Recording in UI**: Capture mic input in Svelte 5 using `getUserMedia` and the Web Audio API, then invoke the backend match handler.

---

## 2. Semantic Hit Rate

Semantic queries were run against the codebase using `knowledge_mgr.py`:
- `tools/.venv/bin/python tools/knowledge_mgr.py query "hum-to-search query by humming crepe"`
  - **Match 1**: `doc/proposals/roadmap_ideas.md` Section 8 (Score: `0.6145`) — Direct specification of CREPE, log conversion, mean centering, and DTW alignment.
  - **Match 2**: `skills/query-db/SKILL.md` (Score: `0.4937`) — Outlines query conventions.
  - **Match 3**: `doc/collab/sessions/2026-06-07-salami-eval-design/dsp_recommendations.md` (Score: `0.4508`) — Discusses storage options for time-series features (Option A sidecar vs Option B SQLite blob).
- `tools/.venv/bin/python tools/knowledge_mgr.py query "AnalysisPass execute_job save_result"`
  - **Match 1**: `skills/add-analysis-pass/SKILL.md` (Score: `0.4912`) — Detailed playbook on adding new passes, registering specs, and invalidating stale versions.
  - **Match 2**: `doc/architecture/tech.md` (Score: `0.4007`) — Lists the priorities and execution order of the modular analysis pipeline.

### Anchoring Strategy
Based on semantic queries and existing patterns:
1. **Pass Registration**: Create `CrepePass` as a standard `AnalysisPass` inside `src-tauri/src/analysis/crepe.rs`.
2. **Audio Resampling**: Reuse `crate::spectrogram::resample_to_16k` to resample track audio during analysis and user recordings during search.
3. **Database Architecture**: Avoid bloating the main `tracks` table by creating a relational table `track_pitch_contours` for full-resolution float contours (stored as binary blobs) and a virtual `track_pitch_embeddings` table for fast k-NN scans.
4. **ONNX Lifecycles**: Load the CREPE model using a `Mutex<Option<Session>>` in the new pass, matching the lazy session initialization in `src-tauri/src/embeddings.rs`.

---

## 3. Impact Assessment

### Database / Schema Changes
We will add a new database migration file: `src-tauri/migrations/33_pitch_contour_index.sql`.

```sql
-- relational table for high-res time-series f0 values
CREATE TABLE IF NOT EXISTS track_pitch_contours (
    track_id INTEGER PRIMARY KEY REFERENCES tracks(id) ON DELETE CASCADE,
    contour BLOB NOT NULL -- compressed f32 array (pitch in cents, mean-centered)
);

-- vec0 virtual table for k-NN pruning
CREATE VIRTUAL TABLE IF NOT EXISTS track_pitch_embeddings USING vec0(
    track_id INTEGER PRIMARY KEY,
    embedding float[128] -- 128-dimensional downsampled pitch representation
);
```

Register this migration in `src-tauri/src/database.rs` in `get_migrations()`.

### Rust Backend

#### 1. Model Configuration
Update `models/manifest.json` with the CREPE model specifications:
```json
"crepe": {
  "label": "CREPE Pitch Extractor",
  "files": [
    {
      "filename": "crepe_tiny.onnx",
      "url": "https://huggingface.co/rlupi/deep-cuts-models/resolve/main/crepe_tiny.onnx",
      "sha256": "4b2c12d26f...",
      "size_bytes": 11200000
    }
  ]
}
```

#### 2. The `CrepePass` Analysis Pass
Create `src-tauri/src/analysis/crepe.rs` implementing the `AnalysisPass` trait:
- **Priority**: `22` (execution placed between `clap` and `essentia`).
- **Dependencies**: `["audio_analysis"]`.
- **Inference logic**:
  1. Decode and resample audio to 16 kHz using `spectrogram::resample_to_16k`.
  2. Chop audio into overlapping 1024-sample windows (64 ms) using a **50 ms hop size** (800 samples at 16 kHz) to speed up analysis.
  3. For each window, subtract mean and divide by standard deviation (with $1e-9$ epsilon).
  4. Run normalized windows through the `crepe_tiny.onnx` session in batches.
  5. Compute f0 value: Softmax output probabilities to find the center of mass in cents:
     $$c = \sum_{i=0}^{359} p_i \cdot (1997.37 + i \cdot 20)$$
     Discard frames with confidence (max softmax prob) $< 0.30$ (unvoiced, set to `0.0`).
  6. Transposition normalization: Compute mean cent value $\mu_c$ of all voiced frames and subtract it from each voiced frame. Unvoiced frames remain `0.0`.
  7. Downsample/interpolate the contour to a fixed 128-dimensional array.
  8. Save 128-float embedding to `track_pitch_embeddings` and high-res contour to `track_pitch_contours` as a compressed binary float blob.

#### 3. Subsequence DTW Implementation
We will implement Subsequence DTW in Rust inside `src-tauri/src/dsp.rs` or a new `src-tauri/src/dtw.rs` file.

```rust
/// Computes Subsequence DTW distance between a short query and a candidate track.
/// Allows matching a short query at any position of a longer track.
pub fn subsequence_dtw(query: &[f32], candidate: &[f32]) -> f32 {
    let m = query.len();
    let n = candidate.len();
    if m == 0 || n == 0 {
        return f32::INFINITY;
    }

    // Space-optimized DP rows
    let mut dp = vec![f32::INFINITY; n + 1];
    let mut prev_dp = vec![0.0; n + 1]; // D(0, j) = 0 for subsequence matching

    for i in 1..=m {
        dp[0] = f32::INFINITY; // D(i, 0) = infinity
        for j in 1..=n {
            // Unvoiced frames (0.0) aligned with voiced frames get a high penalty
            let cost = if query[i - 1] == 0.0 || candidate[j - 1] == 0.0 {
                if query[i - 1] == 0.0 && candidate[j - 1] == 0.0 { 0.0 } else { 1200.0 } // 1 octave penalty
            } else {
                (query[i - 1] - candidate[j - 1]).abs() // difference in cents
            };
            let min_prev = prev_dp[j].min(dp[j - 1]).min(prev_dp[j - 1]);
            dp[j] = cost + min_prev;
        }
        prev_dp.copy_from_slice(&dp);
    }

    // Find the minimum cost in the last row
    let mut min_cost = f32::INFINITY;
    for j in 1..=n {
        if dp[j] < min_cost {
            min_cost = dp[j];
        }
    }

    min_cost / (m as f32) // Normalize by query length
}
```

#### 4. The Tauri IPC Command
Implement `search_by_humming` inside `src-tauri/src/commands/map.rs`:
```rust
#[tauri::command]
pub async fn search_by_humming(
    samples: Vec<f32>,
    sample_rate: u32,
    app: tauri::AppHandle,
) -> Result<Vec<TrackMatch>, String> {
    // 1. Resample query to 16 kHz
    let query_16k = crate::spectrogram::resample_to_16k(&samples, sample_rate)?;
    
    // 2. Extract normalized pitch contour & 128-float embedding
    let (query_contour, query_embedding) = extract_query_pitch_contour(&query_16k, &app)?;
    
    // 3. Search track_pitch_embeddings for top 100 candidates
    let candidates = query_top_candidates(&query_embedding, &app)?;
    
    // 4. Run sDTW for each candidate and compute scores
    let mut matches = Vec::new();
    for track_id in candidates {
        if let Some(candidate_contour) = load_candidate_contour(track_id, &app)? {
            let dtw_dist = subsequence_dtw(&query_contour, &candidate_contour);
            let score = 1.0 / (1.0 + dtw_dist / 100.0); // normalize distance to 0..1 score
            matches.push(TrackMatch { track_id, score });
        }
    }
    
    // 5. Sort matches and return top 10
    matches.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
    matches.truncate(10);
    Ok(matches)
}
```

---

### Frontend Svelte 5 / TS

#### 1. Audio Recording
Introduce a helper function in a new utility `src/lib/utils/audioRecorder.ts` that captures user recording via the mic:
- Initiates `navigator.mediaDevices.getUserMedia({ audio: true })`.
- Records using `AudioContext` and a `ScriptProcessorNode` or `AudioWorklet` to accumulate raw Float32Array PCM samples.
- Stops recording after a maximum of 12 seconds or when user clicks stop, returning the recorded samples and the sample rate of the input stream.

#### 2. Sidebar Integration
Update `src/lib/components/FilterSidebar.svelte`:
- Render a "Hum Search" microphone button inside the search container.
- Manage recording states (`idle`, `recording`, `searching`).
- Send the recorded samples to Tauri using `invoke("search_by_humming", { samples, sampleRate })`.
- If successful, set a new filter state `filters.humSearchResults` and highlight matches in the track listing.

---

## 4. Implementation Checklist

- [ ] **1. Manifest Update**: Add the `crepe` model details to `models/manifest.json`.
- [ ] **2. DB Migration**: Create `src-tauri/migrations/33_pitch_contour_index.sql` declaring `track_pitch_contours` and `track_pitch_embeddings`.
- [ ] **3. Register Migration**: Open `src-tauri/src/database.rs` and append migration `33` to `get_migrations()`.
- [ ] **4. Create Pass**: Create `src-tauri/src/analysis/crepe.rs` implementing the `AnalysisPass` trait for CREPE model execution, transposition invariance, and embedding generation.
- [ ] **5. Register Pass Submodule**: Update `src-tauri/src/analysis/mod.rs`:
  - Declare `pub mod crepe;`
  - Register `crepe::CrepePass::SPEC` in `PASS_REGISTRY`
  - Insert `crepe::CrepePass` execution step inside `PipelineManager::run()`
- [ ] **6. Sidecar Updates**: Add `CREPE` constant in `src-tauri/src/scanner/sidecar.rs` and configure the pass versions.
- [ ] **7. Expose Command**: Implement `search_by_humming` in `src-tauri/src/commands/map.rs` and register it in `tauri::generate_handler!` inside `src-tauri/src/lib.rs`.
- [ ] **8. Frontend Types**: Update `src/lib/types.ts` and `src/lib/ipc.ts` to include the `search_by_humming` command mapping and mock definitions.
- [ ] **9. Audio Recorder**: Write `src/lib/utils/audioRecorder.ts` to handle browser-based audio recording.
- [ ] **10. UI Widget**: Update `src/lib/components/FilterSidebar.svelte` with the recording interface, search triggers, and active search indicators.
- [ ] **11. Analysis Panel Tooltip**: Register the pass in `AnalysisPanel.svelte` (`PASS_ORDER`, `PASS_ROLE`, and `PASS_META`).
- [ ] **12. Run Pipeline Tests**: Run `cargo test --manifest-path src-tauri/Cargo.toml` to ensure migrations and the database setup pass validation.
