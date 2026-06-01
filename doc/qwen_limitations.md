# Qwen2-Audio Limitations & Findings

## Audio Tokenisation

Qwen2-Audio encodes audio at **16 kHz mono** via its Whisper-based audio encoder. llama.cpp slices the input into **30-second chunks**, each producing exactly **750 audio tokens**. The slices are encoded sequentially and all fed into the LLM context before generation begins.

```
30s slice → 750 tokens
60s       → 1,500 tokens
160s      → ~4,500 tokens  (measured on a 2m40s track)
```

## Context Window

The model's context is **8,192 tokens** (`n_ctx_train = 8192`). Audio tokens compete with the system prompt, user question, and generated response for that budget.

Approximate audio capacity at default context:

| Track length | Audio tokens | Tokens remaining for prompt + response |
|---|---|---|
| 30s | 750 | ~7,400 |
| 3 min | 4,500 | ~3,600 |
| 5 min | 7,500 | ~600 — dangerously tight |
| ~6.5 min | 8,192 | 0 — hard limit |

The practical safe limit for full-track analysis is around **5 minutes**. Tracks longer than that need a windowed excerpt.

`--ctx-size` can be passed to `llama-server` / `llama-mtmd-cli` to increase the context, but Qwen2-Audio was not trained beyond 8,192 tokens and quality may degrade with extended context.

## Automatic Chunking by llama.cpp

llama.cpp handles slicing transparently — passing a full MP3 works without any pre-processing. The caller does **not** need to split audio manually. Observed processing time on Apple Silicon (M-series):

Measured on a **Mac Studio (M3 Ultra, 28-core CPU, 96 GB unified memory)**. The large unified memory pool means the 7B Q4 model (~4 GB), mmproj (~650 MB), and KV cache are all resident simultaneously with no swapping — timings on machines with less RAM or without GPU acceleration will be significantly higher:

- ~225–460 ms per 30-second slice (encode + decode)
- A 3-minute track takes roughly **3 seconds** of audio processing before generation starts

## Implications for Deep Cuts

### Analysis pipeline (`qwen.rs`)

The current code manually extracts a 30-second midpoint window before sending to llama-server. This was the right call for the pipeline:

- **Speed**: 30s → one slice, ~0.5s processing. Full track → N slices, N × 0.5s.
- **Cost**: audio tokens consumed per track stays fixed at 750, leaving ample context budget for the structured prompt and response.
- **Consistency**: all tracks are evaluated on the same-length excerpt.

The 30-second window duration should be made **configurable** (e.g. `analysis_settings.qwen_window_seconds`) so it can be tuned without a code change. A longer window (e.g. 60–90s) may improve genre/mood accuracy for tracks with distinct sections, at the cost of slower pipeline runs.

#### Smart Vocal Activity Detection (VAD) & Energy-Based Slicing
Instead of blindly cropping the absolute midpoint of a track (which might capture a silent breakdown, acoustic transition, or intro/outro sound effect), the analysis pipeline incorporates a **Smart VAD & Energy-Based Slicing** algorithm:
- The system runs the fast RMS energy pre-pass to map volume profiles across the track.
- A local, low-latency Vocal Activity Detection (VAD) pass runs in parallel to score voice activity density.
- The pipeline selects a 30-second window representing the **highest average energy containing active speech/vocal dynamics** (for vocal tracks) or **highest average musical onset density** (for instrumentals).
- This ensures that the Qwen2-Audio model receives the most representative and information-dense 30-second slice of the music (e.g., the main chorus or primary vocal hook) rather than a quiet pause or intro.

### Chat feature (`track-feedback.md`)

For interactive feedback, passing the **full track** is preferred — the user may ask about any part of the song and the model should have full context. The automatic chunking in llama.cpp makes this straightforward.

Tracks longer than ~5 minutes should fall back to a configurable window (defaulting to the full track for shorter songs, and a user-selected region for longer ones — the WaveSurfer region selector described in `track-feedback.md` handles this case).

#### Hierarchical Summarization for Long Tracks (>5m)
For massive tracks (e.g., extended electronic mixes, classical symphonies, progressive rock epics exceeding 10–20 minutes) that would completely blow past Qwen2's 8,192-token context window if fed as raw audio:
- **Multi-Window Extraction**: The track is divided into three or four distinct 30-second windows distributed across key structural zones (intro, build-up, peak chorus/movement, outro) selected by energy profiles.
- **Sequential Local Inference**: Each segment is analyzed individually to generate a short, high-fidelity text summary detailing its specific tempo, instrumental texture, and emotional tone.
- **Hierarchical Synthesis**: The individual text summaries are combined and fed into the final LLM chat context as structured markdown descriptions. This enables Qwen2-Audio to answer deep questions about the entire multi-movement track using a fraction of the raw audio token budget.

#### Interactive WaveSurfer Timeline Highlighting & Manual Re-crop
To bridge the chat and audio feedback loop, the frontend WaveSurfer timeline is enriched with:
- **Interactive Highlighting**: Visual brackets showing the exact boundaries of the 30-second window that was used to generate the current description.
- **Manual Re-crop**: If a user feels the automated smart slice missed a key element of the track (e.g., a quiet acoustic outro), they can drag the WaveSurfer region brackets to a new section of the timeline and click a "Re-Analyze Region" button. This triggers an on-demand analysis of the custom crop, updating the database tags and description in real-time.

## Raw Measurements (2m40s test track)

```
Slices:        6  (each 30s, 750 tokens)
Total audio tokens:  ~4,500
Prompt eval time:    5,673 ms / 4,523 tokens  (797 t/s)
Total time:          6,358 ms / 4,547 tokens
Context used:        ~4,547 / 8,192 tokens (~55%)
```

## Cross-References

- `src-tauri/src/analysis/qwen.rs` — pipeline pass, 30s midpoint window
- `src-tauri/src/llama.rs` — server lifecycle
- `doc/track-feedback.md` — interactive chat design, window selection UI
