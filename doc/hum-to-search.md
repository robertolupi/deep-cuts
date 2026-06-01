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

## Preprocessing Pipeline (non-trivial)

Before CREPE sees the audio, the raw mic input needs cleaning:

1. **Normalization** — mic gain and input levels vary wildly across devices
2. **Bandpass EQ** — isolate the vocal melody range (~80–1000 Hz), reject breath noise, room rumble, and high-frequency artifacts
3. **Harmonic filtering** — a humming voice produces strong overtones that can confuse pitch detection; may need a harmonic product spectrum step

Each stage has its own failure modes and they compound. This is a meaningful chunk of work on top of the model and search infrastructure.

## Open Questions

- **Contour normalization**: hums are usually pitch-shifted relative to the original key — the embedding or distance metric needs to be transposition-invariant (e.g. compare relative intervals, not absolute f0 values)
- **Segment matching**: tracks are long; the query is a short fragment — need to decide whether to embed the whole track or sliding windows
- **Cold-start**: the analysis pass needs to run over the full library before the feature is usable — could be gated behind a user-triggered re-analysis
