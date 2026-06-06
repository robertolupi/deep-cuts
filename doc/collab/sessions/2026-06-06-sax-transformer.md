# Session: SAX Transformer (Approach B)

**Date:** 2026-06-06  
**Participants:** Roberto, Gemini, Meta  
**Goal:** Research and design a neural sequence model (recurrent network or tiny transformer) to classify 16-segment audio envelopes (using energy, repetition score, and track position features) into structural labels (Intro, Verse, Pre-Chorus, Chorus, Bridge, Outro, End) trained on Genius lyrics section markers, to be exported via ONNX for Rust-side inference and Viterbi search.

---

## [Gemini, 10:53]

We are dividing our efforts. We are proceeding with **Approach A** (client-side DTW block search & UI composer), while passing this session to Meta to study and design **Approach B** (Neural Sequence Classifier + Viterbi).

To help Meta understand the two paths, we've created a sample file [2026-06-06-sax-transformer.json](file:///Users/rlupi/src/deep-cuts/doc/collab/sessions/2026-06-06-sax-transformer.json) containing 5 tracks with title, artist, genre, `waveform_sax`, `waveform_fingerprint`, and raw `waveform_data` arrays.

### Approach A: DTW + Visual Block Query Language (Current Development)
- **Goal**: Align user-composed block queries (e.g. `[Intro] -> [Verse] -> [Chorus] -> [Outro]`) to a track's 32-character SAX envelope string (e.g. `aaaaaaaaccccccccceeeeeeeeebbbbbb`) using **Dynamic Time Warping (DTW)**.
- **Representation**:
  - Blocks are mapped to target SAX letters: `Intro` -> `a`, `Break`/`Outro` -> `b`, `Verse`/`Pre-Chorus` -> `c`, `Bridge` -> `d`, `Chorus`/`Drop` -> `e`, and `···` -> `*` (wildcard, zero cost).
  - Search runs in TypeScript client-side over all library tracks in ~2ms, sorting results by normalized DTW distance.

### Approach B: Neural Sequence Classifier + Viterbi (Research/Handoff)
- **Goal**: Instead of comparing raw 1D SAX letters, we train a sequence model (like a GRU or tiny Transformer) to output section probabilities for 16 equal-length segments of a track, then use a Viterbi decoder to find the optimal query alignment.
- **Inputs per Segment (16 steps)**:
  1. `energy`: SAX letter mapped to `[0.0, 1.0]` (a=0.0, b=0.25, c=0.5, d=0.75, e=1.0).
  2. `rep_score`: Normalized Self-Similarity Matrix (SSM) repetition score in `[0.0, 1.0]`.
  3. `position`: Fractional track position `[0.0, 1.0]` (i.e. `step_index / 16`).
- **Target Classes (7)**: `[Intro, Verse, Pre-Chorus, Chorus, Bridge, Outro, End]`.
- **Training Labels**: Extracted from community lyrics tags in scraped `lyrics.txt` files (e.g., `[Verse 1]`, `[Chorus]`), mapped to track position ratios.

### Tasks for Meta:
1. **Model Architecture**: Design a neural sequence classifier (GRU, 1D CNN, or sequence-to-sequence Transformer) to map a `16 x 3` input sequence to a `16 x 7` label probability distribution.
2. **Genius Label Alignment**: Propose a method to preprocess and align character/line offsets of Genius lyrics section headers to the 16 temporal audio segments.
3. **ONNX Deployment**: Describe how to compile the model to ONNX, handle sequence padding or fixed size inputs (16 steps), and run it within the Rust backend via `ort` (ONNX Runtime).
4. **Viterbi Formulating**: Define the Viterbi recurrence relation to align a block search sequence (e.g., `[Intro, Chorus, Outro]`) against a `16 x 7` log-probability matrix.

**→ Handoff:** Study the model architecture and training pipeline for the neural sequence classifier using the Genius lyrics dataset, delivering a PyTorch-based proposal and ONNX export strategy.
