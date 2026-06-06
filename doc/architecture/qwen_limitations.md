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

The pipeline extracts a 30-second window centered on the **highest-energy bin** from the stored waveform profile (`embeddings::select_best_energy_window_pct`), falling back to the track midpoint when no waveform data is available. This ensures the model sees the most musically dense section rather than a silent intro or outro.

- **Speed**: 30s → one slice, ~0.5s processing. Full track → N slices, N × 0.5s.
- **Cost**: audio tokens consumed per track stays fixed at 750, leaving ample context budget for the structured prompt and response.
- **Consistency**: all tracks are evaluated on the same-length excerpt.

The 30-second window duration should be made **configurable** (e.g. `analysis_settings.qwen_window_seconds`) so it can be tuned without a code change. A longer window (e.g. 60–90s) may improve genre/mood accuracy for tracks with distinct sections, at the cost of slower pipeline runs.

### Chat feature

Implemented in `src-tauri/src/commands/chat.rs` and `src/lib/components/ChatPanel.svelte`. The user selects an audio region via a WaveSurfer timeline; the backend slices that exact region and sends it to llama-server. For tracks longer than ~5 minutes the token budget becomes tight — the region selector lets the user pick which section to analyse rather than exhausting the full context.

#### Hierarchical Summarization for Long Tracks (>5m) — future work
For tracks exceeding ~6.5 minutes (beyond the 8,192-token hard limit even with a minimal prompt), a possible approach is to analyse three or four 30-second windows distributed across structural zones (intro, build-up, chorus, outro), summarise each individually, then synthesise the text summaries into a single chat context. Not yet implemented.

## Raw Measurements (2m40s test track)

```
Slices:        6  (each 30s, 750 tokens)
Total audio tokens:  ~4,500
Prompt eval time:    5,673 ms / 4,523 tokens  (797 t/s)
Total time:          6,358 ms / 4,547 tokens
Context used:        ~4,547 / 8,192 tokens (~55%)
```

## Cross-References

- `src-tauri/src/analysis/qwen.rs` — pipeline pass, energy-based 30s window
- `src-tauri/src/embeddings.rs` — `select_best_energy_window_pct` (shared with CLAP)
- `src-tauri/src/llama.rs` — server lifecycle
- `src-tauri/src/commands/chat.rs` — interactive `ask_qwen` IPC command
- `src/lib/components/ChatPanel.svelte` — chat UI with WaveSurfer region selector
