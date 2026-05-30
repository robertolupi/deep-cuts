# Qwen2-Audio Description Pass — Design Document

## Overview

Two new analysis passes that build on top of CLAP:

1. **`qwen` (priority 30)** — sends audio to a local Qwen2-Audio-7B model via `llama-server`, stores a prose description in `tracks.description`
2. **`description_embed` (priority 40)** — embeds that description with all-MiniLM-L6-v2 (already in use), stores a 384-d vector in `description_embeddings`

The UMAP/similarity map then blends CLAP (512-d) + description embedding (384-d) → 896-d concatenated vector, giving the map a semantic layer on top of acoustic similarity.

---

## llama-server Lifecycle

**Key constraint**: `llama-server` loads ~5 GB of weights into RAM/GPU. It must only be alive while the `qwen` pass is actively processing tracks. It must be shut down immediately when the batch is done.

### Start
- Called at the beginning of the `qwen` phase in `PipelineManager::run()`
- `spawn_llama_server()` tries known paths in order: `/opt/homebrew/bin/llama-server`, `/usr/local/bin/llama-server`, `llama-server`
- After spawn, poll `http://127.0.0.1:10086/health` every 250 ms, up to 120 s (model load can be slow on CPU)
- If the process exits prematurely, surface the error and mark all pending `qwen` passes as `FAILED`

### Shutdown
- **RAII guard**: a `LlamaServerGuard` struct wraps `Child`. On `drop()` it calls `child.kill()` then `child.wait()` to reap the process
- The guard is created at the start of the `qwen` phase and dropped when the phase ends (success, error, or panic)
- If `llama-server` was already running externally on port 10086 when we started (detected via health check), we do **not** kill it on exit

### Concurrency
- `qwen` runs **single-threaded** (one inference at a time — model is memory-heavy)
- A `Mutex<()>` inference lock prevents concurrent requests if we ever parallelize later

---

## Model Path Configuration

All models — CLAP ONNX files, mel weights, Qwen2-Audio GGUFs — resolve through a single configurable `model_path` directory stored in `app_settings`:

```sql
-- key: 'model_path', value: absolute path to the models directory
SELECT value FROM app_settings WHERE key = 'model_path';
```

`get_model_path(filename, app)` in `embeddings.rs` is extended to check this setting as an additional candidate directory, in between the Tauri resource bundle and the dev fallbacks. This way CLAP and Qwen model files are all found by the same function with no per-model path settings.

### Resolution order (all models)
1. Tauri resource bundle `{resource_dir}/models/{filename}`
2. App data dir `{app_data_dir}/models/{filename}`
3. **`app_settings.model_path`** / `{filename}` (user-configurable via Settings UI)
4. Dev fallbacks: `models/{filename}`, `../models/{filename}`

### Qwen2-Audio filenames
- `Qwen2-Audio-7B-Instruct.Q4_K_M.gguf` (~4.7 GB)
- `Qwen2-Audio-7B-Instruct.mmproj-Q8_0.gguf` (~0.3 GB)

Downloaded from HuggingFace repo `mradermacher/Qwen2-Audio-7B-Instruct-GGUF` via `tools/download_models.py`.

If either GGUF file is not found, all pending `qwen` passes are marked `FAILED` with an actionable error message pointing to `download_models.py` and the Settings page.

---

## Database Changes

### Migration: new `tracks` columns
```sql
ALTER TABLE tracks ADD COLUMN is_music INTEGER;     -- 1/0/NULL
ALTER TABLE tracks ADD COLUMN ai_genre TEXT;
ALTER TABLE tracks ADD COLUMN ai_mood TEXT;
ALTER TABLE tracks ADD COLUMN ai_instruments TEXT;
ALTER TABLE tracks ADD COLUMN description TEXT;     -- prose from DESCRIPTION field
```

### Migration: `description_embeddings` virtual table
```sql
CREATE VIRTUAL TABLE IF NOT EXISTS description_embeddings USING vec0(
    track_id INTEGER PRIMARY KEY,
    embedding float[384]
);
```

### `track_passes` backfill (at analysis start, same as existing passes)
```sql
INSERT OR IGNORE INTO track_passes (track_id, pass_name, priority, status)
    SELECT id, 'qwen', 30, 'pending' FROM tracks;

INSERT OR IGNORE INTO track_passes (track_id, pass_name, priority, status)
    SELECT id, 'description_embed', 40, 'pending' FROM tracks;
```

Note: `description_embed` is backfilled at analysis start but only becomes runnable once `qwen` is done — it reads from `tracks.description`, so a track with no description yet is a no-op (marked `DONE` with a null embedding).

### `reset_pass("qwen")`
Clears `tracks.description` and `description_embeddings` for all tracks, resets both `qwen` and `description_embed` passes to `pending`.

---

## `qwen` Pass — Audio Processing

1. Decode audio → mono, native sample rate (`dsp::decode_audio_to_mono`)
2. Resample to 16 kHz (`dsp::resample_to_16k` — already exists)
3. Take a 30-second window centred on the track midpoint
4. Encode to WAV bytes in memory (`encode_audio_to_wav`)
5. Base64-encode

### Prompt

Fields read from `tracks` (all set by `audio_analysis`, which runs first):
- `bpm`, `key`, `scale` — fall back to 120 BPM / C major if NULL
- `genre` — omit the genre hint line if NULL or empty

```
The measured tempo of this track is approximately {bpm:.0} BPM and the detected key is {key} {scale}.
[if genre set] The file metadata tags this track as "{genre}", though that label may be broad or imprecise.
Listen carefully and respond using ONLY the following format, one field per line, nothing else:

MUSIC: yes or no (is this music, as opposed to speech, podcast, sound effects, or silence?)
GENRE: genre and subgenre in a few words
MOOD: mood and emotional feel in a few words
INSTRUMENTS: main instruments, comma-separated
DESCRIPTION: two to three sentences of plain prose describing the track
```

### Response format & parsing

Expected response:
```
MUSIC: yes
GENRE: Progressive metal, post-metal
MOOD: Dark, introspective, melancholic
INSTRUMENTS: Electric guitar, distorted bass, drums, synthesizer pads
DESCRIPTION: A brooding progressive metal track with heavy, down-tuned guitars layered over shimmering synth textures. The rhythm section drives an unrelenting pulse while melodic leads surface between dense chord walls. Emotionally heavy and cinematic, evoking isolation and tension.
```

Parsing: iterate lines, split on first `: `, match key case-insensitively. Unknown lines are ignored. If a field is missing from the response, store NULL. The genre hint nudges Qwen toward the right semantic neighbourhood without overriding its own listening — the "broad or imprecise" qualifier prevents it from anchoring too hard on a coarse tag like "Rock" when the track is actually doom metal.

### DB columns added to `tracks`

```sql
ALTER TABLE tracks ADD COLUMN is_music INTEGER;         -- 1 = music, 0 = non-music, NULL = not yet run
ALTER TABLE tracks ADD COLUMN ai_genre TEXT;            -- e.g. "Progressive metal, post-metal"
ALTER TABLE tracks ADD COLUMN ai_mood TEXT;             -- e.g. "Dark, introspective"
ALTER TABLE tracks ADD COLUMN ai_instruments TEXT;      -- e.g. "Electric guitar, drums"
ALTER TABLE tracks ADD COLUMN description TEXT;         -- prose paragraph
```

### `description_embed` input

The text fed to the sentence embedder is a concatenation of all structured fields (excluding `MUSIC`), giving the embedding a rich semantic signal:

```
Genre: Progressive metal, post-metal. Mood: Dark, introspective, melancholic. Instruments: Electric guitar, distorted bass, drums, synthesizer pads. Progressive metal track with heavy, down-tuned guitars layered over shimmering synth textures...
```

If `is_music = 0`, the `description_embed` pass is skipped entirely.

### HTTP Request
```
POST http://127.0.0.1:10086/v1/chat/completions
{
  "messages": [{
    "role": "user",
    "content": [
      { "type": "input_audio", "input_audio": { "data": "<base64>", "format": "wav" } },
      { "type": "text", "text": "<prompt>" }
    ]
  }]
}
```
Timeout: 120 s per track.

### DB Write
```sql
UPDATE tracks SET
    is_music     = ?1,
    ai_genre     = ?2,
    ai_mood      = ?3,
    ai_instruments = ?4,
    description  = ?5
WHERE id = ?6
```

---

## `description_embed` Pass

1. Read `tracks.description` for the track
2. If NULL or empty → mark pass `DONE`, write nothing to `description_embeddings`
3. Otherwise → call `embeddings::run_sentence_embed(&description, Some(&app))` (384-d, already implemented)
4. Store result:
```sql
INSERT OR REPLACE INTO description_embeddings (track_id, embedding) VALUES (?1, ?2)
```

---

## Pipeline Sequencing in `PipelineManager::run()`

```
Phase 1 — audio_analysis    (parallel, N threads)
Phase 2 — clap              (single-threaded, sequential)
Phase 3 — qwen              (single-threaded; llama-server started before, killed after)
Phase 4 — description_embed (single-threaded)
```

Each phase queries for its pending jobs at the start of the phase (not at pipeline start), so tracks that failed earlier phases are naturally skipped.

### llama-server guard scope
```
start of Phase 3:   spawn llama-server → LlamaServerGuard created
  process all qwen jobs...
end of Phase 3:     LlamaServerGuard dropped → child.kill() + child.wait()
start of Phase 4:   description_embed runs (no llama-server needed)
```

---

## UMAP / Similarity Blending

In `commands/map.rs`, when building vectors for UMAP or KNN:

- If a track has both CLAP and description embedding: concatenate L2-normalised CLAP (512-d) + L2-normalised description (384-d) → 896-d vector
- If a track has only CLAP (description not yet run, or returned null): use CLAP only, zero-padded to 896-d (or just use the 512-d CLAP vector — TBD)
- `vec_distance` queries against `audio_embeddings` remain 512-d only; blended vectors are used only for UMAP layout

---

## `download_models.py` Update

Add Qwen2-Audio GGUF download to the existing `tools/download_models.py` (or create it if it doesn't exist), downloading from `mradermacher/Qwen2-Audio-7B-Instruct-GGUF` into `models/`.

---

## New Rust Code Locations

| Component | Location |
|-----------|----------|
| `LlamaServerGuard`, `spawn_llama_server`, `ensure_llama_server`, `resolve_llama_paths` | `src-tauri/src/llama.rs` (new file) |
| `qwen` and `description_embed` pass handlers | `src-tauri/src/analysis.rs` (new phases) |
| `reset_pass` update for `qwen` | `src-tauri/src/commands/analysis.rs` |
| DB migrations | `src-tauri/src/database.rs` |
| UMAP blending | `src-tauri/src/commands/map.rs` |

---

## Decisions

1. **Fallback for UMAP when description embedding is missing** — use CLAP only (512-d) for tracks that have no description embedding. Do not zero-pad; mixed-dimension libraries are handled by branching at map-build time.
2. **llama-server install check** — check for `llama-server` in PATH at app startup and surface a warning in the Settings page if not found. Do not block analysis; just inform the user before they attempt the qwen pass.
3. **Timeout per track** — keep 120 s for now. Measure actual inference time on the target machine during first real run and adjust the constant accordingly.
