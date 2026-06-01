# Hum/Mumble Melody Search

Query-by-humming: record a few seconds of a user humming or mumbling a melody and find matching tracks in the library using vector similarity.

## Why It Fits the Current Stack

- **sqlite-vec** — nearest-neighbor search over melody embeddings is already the right primitive
- **ort (ONNX Runtime)** — can run the same embedding model for both library indexing and live query inference
- **Analysis pipeline** — a new pass handles the per-track embedding extraction at scan time
- **Tauri / Web Audio API** — `getUserMedia` gives mic access in the frontend with no extra native dependencies

## Approach

### Model: CREPE (pitch contour)

[CREPE](https://github.com/marl/crepe) is the recommended starting point:

- Extracts a fundamental frequency (f0) contour over time — exactly what humming encodes
- Small model (~30 MB), fast inference
- ONNX export is available
- Pitch-only representation is robust to the timbre difference between a hummed query and an instrument

The pitch contour can be encoded as a fixed-length embedding (e.g. quantized f0 sequence or summary statistics) and stored in a `sqlite-vec` table.

**Alternative:** MERT or music2vec for a full audio embedding — more robust to rhythm/timbre variation but heavier and ONNX export is less mature. Worth revisiting once CREPE is proven insufficient.

## Components to Build

1. **Analysis pass** (`add-analysis-pass` skill)
   - Run CREPE over each track, extract the f0 contour
   - Encode as a fixed-dimension vector
   - Store in a new `melody_embeddings` sqlite-vec table keyed by `track_id`

2. **Query endpoint** (new IPC command)
   - Accept raw PCM audio from the frontend (a few seconds of humming)
   - Run CREPE inference on the query audio
   - Encode to the same vector format
   - Return top-N nearest neighbors from `melody_embeddings`

3. **Frontend UI**
   - Record button (hold-to-record or toggle) using `getUserMedia`
   - Send recorded audio buffer to the IPC command
   - Display results like any other search

### Sliding Window Chunking for sqlite-vec
Because `sqlite-vec` requires a fixed vector dimension for indexing, the system cannot index whole-track variable-length f0 contours directly. To solve this:
- **Overlapping 5-Second Frames**: During indexing, each track is split into 5-second sliding windows with a 50% overlap (2.5-second step).
- **1-to-Many Segment Table**: Each 5-second segment is embedded into a fixed-dimension vector (e.g., 128 dimensions representing the downsampled f0 curve) and stored in a specialized child table `track_melody_segments`:
  ```sql
  CREATE TABLE track_melody_segments (
      segment_id   INTEGER PRIMARY KEY,
      track_id     INTEGER REFERENCES tracks(id),
      start_offset REAL,  -- start time in seconds
      embedding    F32_VEC_128
  );
  ```
- **Aggregation**: A query hum is also encoded into a 128-dimensional vector representing its 5-second pitch envelope. `sqlite-vec` performs a rapid K-NN search to find the nearest matching 5-second segments, and results are aggregated by `track_id` (using maximum similarity score or sum of segment ranks) to find the best overall matching tracks.

### Real-Time Visual Pitch Envelope Feedback
To provide a responsive UX during melody recording, the frontend uses the Web Audio API to extract real-time pitch estimates (using a lightweight autocorrelation algorithm in a web worker). The Svelte frontend renders a live, glowing canvas line showing the user's pitch envelope in real time. This reassures the user that their voice is being captured correctly and guides them to hum cleanly.

---

## Preprocessing Pipeline (non-trivial)

Before CREPE sees the audio, the raw mic input needs cleaning:

1. **Normalization** — mic gain and input levels vary wildly across devices
2. **Bandpass EQ** — isolate the vocal melody range (~80–1000 Hz), reject breath noise, room rumble, and high-frequency artifacts
3. **Harmonic filtering** — a humming voice produces strong overtones that can confuse pitch detection; may need a harmonic product spectrum step

### Relative Pitch Interval Transposition Invariance
Hummers rarely sing in the original key of the track. To make the search transposition-invariant, absolute f0 frequencies (measured in Hz) must be neutralized:
- **Logarithmic Pitch Conversion**: Absolute frequencies are converted to logarithmic semitones:
  `p = 12 * log2(f0 / 440.0) + 69`
- **Mean Centering (Interval Subtraction)**: The average pitch value over the 5-second window is subtracted from each point in the pitch curve:
  `p_norm[i] = p[i] - mean(p)`
  This centers the contour around zero, transforming absolute pitches into relative intervals. Any pitch shift or transposition offset is canceled out, allowing a hum in C-major to match an original recording in F#-major perfectly.

### Dynamic Time Warping (DTW) Alignment
While `sqlite-vec` performs the initial fast K-NN pruning, human humming has variable speed and tempo fluctuations that standard L2/Cosine vector distance cannot fully capture.
- **Two-Stage Search**: The system uses `sqlite-vec` to prune the library down to the top 100 candidate tracks.
- **DTW Rescoring**: In Rust, a secondary pass runs Dynamic Time Warping (DTW) alignment on the raw, variable-length pitch contours of the top 100 candidates against the user's raw query pitch contour. DTW finds the optimal non-linear alignment in time, accounting for tempo speedups or slow-downs, and outputs a highly accurate similarity score used to produce the final search rankings.

---

## Open Questions

- **Cold-start**: the analysis pass needs to run over the full library before the feature is usable — could be gated behind a user-triggered re-analysis
- **Acoustic feedback loop**: preventing desktop speaker output from leaking into the microphone if the user hums while music is playing (echo cancellation).
