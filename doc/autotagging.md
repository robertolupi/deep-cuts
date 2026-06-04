# Auto-Tagging System

Deep Cuts builds a rich tag vocabulary for every track through cooperating analysis passes. Tags are stored in a normalized `tags` / `track_tags` schema (see `database.rs`) where each row carries a `source` column that identifies which pass wrote it. When a pass is re-run it deletes its own source rows and rewrites them from scratch, leaving tags from other sources untouched.

---

## Tag Schema

```sql
CREATE TABLE tags (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    name            TEXT NOT NULL UNIQUE,
    normalized_name TEXT NOT NULL UNIQUE
);

CREATE TABLE track_tags (
    track_id INTEGER NOT NULL,
    tag_id   INTEGER NOT NULL,
    source   TEXT NOT NULL,   -- 'essentia' or 'qwen'
    score    REAL,            -- pass-specific confidence or distance score
    discard  INTEGER NOT NULL DEFAULT 0,  -- soft-deletion flag (reserved for future use)
    PRIMARY KEY (track_id, tag_id),
    FOREIGN KEY (track_id) REFERENCES tracks(id) ON DELETE CASCADE,
    FOREIGN KEY (tag_id)   REFERENCES tags(id)   ON DELETE CASCADE
);
```

---

## Pass 1 — CLAP (source: none — embeddings only)

**File**: [`src-tauri/src/analysis/clap.rs`](../src-tauri/src/analysis/clap.rs)
**Priority**: 20 (depends only on `audio_analysis`)

The CLAP pass encodes every track as a 512-dimensional audio embedding using the CLAP (Contrastive Language-Audio Pretraining) model and stores it in the `audio_embeddings` table. **This pass writes no tags.** The embedding is used by downstream passes for audio-text similarity verification.

Three audio windows are sampled per track (targeting the highest-energy region of the waveform profile) and their embeddings are pooled into a single representative vector. The result is stored as a raw float32 blob in little-endian byte order.

---

## Pass 2 — Essentia (source: `essentia`)

**File**: [`src-tauri/src/analysis/essentia.rs`](../src-tauri/src/analysis/essentia.rs)
**Priority**: 40 (depends on `audio_analysis`, `bpm_correction`)

Essentia runs pre-trained ONNX classifiers on a mel-spectrogram of each track and writes two tag namespaces:

### `vocals` namespace
Requires ≥ 0.80 confidence from the voice/instrumental classifier:
- `vocals:present` — track has singing
- `vocals:instrumental` — track is instrumental

### `mood` namespace
Any Essentia mood probability ≥ 0.75 fires a tag:
- `mood:sad`, `mood:aggressive`, `mood:relaxed`, `mood:party`, `mood:acoustic`, `mood:electronic`

---

## Pass 3 — Qwen (source: `qwen`)

**File**: [`src-tauri/src/analysis/qwen.rs`](../src-tauri/src/analysis/qwen.rs)
**Priority**: 50 (depends on `audio_analysis`, `bpm_correction`, `clap`, `essentia`)

Qwen2-Audio listens to 30 seconds of audio (centered on the highest-energy window) and runs a **multi-turn conversation** with the local llama-server. Each step extends the same conversation so later answers build on earlier context.

### Conversation steps

| Step | Prompt (abbreviated) | Output stored | Tag namespace |
|------|----------------------|---------------|---------------|
| `feel` | *(initial message — genre/feel, tempo, key context)* | `tracks.ai_mood` | `feel` |
| `instruments` | List instruments you can clearly hear | `tracks.ai_instruments` | `inst` |
| `description` | Two to three prose sentences describing the track | `tracks.description` | — |

If any step returns Chinese characters or the description is generic/invalid, the attempt is retried up to three times. On step failure the best partial result is saved rather than discarding everything.

### CLAP similarity verification

After all steps complete, the description text is embedded with CLAP's text encoder and its cosine similarity to the track's audio embedding is computed. If similarity ≥ 0.28 the result passes immediately; otherwise the attempt with the highest similarity across the three retries is kept. This acts as a consistency check — descriptions that don't match the audio at all (e.g. from a server hallucination) are deprioritised.

### Tag cleaning

Raw model output passes through `clean_qwen_tags` which strips label prefixes (e.g. `FEEL:`, `INSTRUMENTS:`), lowercases, trims whitespace, and filters tokens against an instrument/genre whitelist before being split on commas into individual tag labels.

---

## Pass ordering and dependency graph

```
audio_analysis
    └─ bpm_correction
            └─ clap (embeddings only) ────────────────┐
            └─ essentia ──────────────────────────────┤
                                                      ▼
                                             qwen (feel / inst / description)
                                                      └─ description_embed
```

---

## Tag namespaces summary

| Namespace | Written by | Meaning |
|-----------|-----------|---------|
| `inst`    | qwen | Instruments heard in the track |
| `vocal`   | essentia | Voice presence / type |
| `feel`    | qwen | Emotional character |
| `mood`    | essentia | Mood classifier output |

---

## Alternatives Considered

### CLAP concept tagging (discarded)

**Approach**: Use the CLAP audio embedding to tag tracks by measuring cosine similarity between the track embedding and a set of text concept embeddings. Concepts were drawn from AudioSet labels mapped to the `inst`, `vocal`, and `feel` namespaces (~37 labels). Each concept was embedded by averaging three prompt templates (`"a song featuring {}"`, `"music with {}"`, `"{}"`). Per-track tags were selected using a sqlite-vec KNN query (`SELECT concept_idx, distance FROM _clap_concepts WHERE embedding MATCH ? AND k = 15`), writing the nearest concepts as `source='clap'` tags.

**Earlier variant**: A z-score approach computed the full `n × m` (tracks × concepts) dot-product matrix in Python and tagged tracks scoring ≥ 1.5 standard deviations above the library mean per concept. This was replaced with per-track KNN because z-scores are not comparable across concepts and the batch approach was fragile.

**Why it failed**:
1. **Low precision**: When Qwen2-Audio was asked to confirm CLAP's proposed tags for each track, it discarded ~91% of them. The distance score from the KNN query was not predictive of whether Qwen would agree — the score distribution for confirmed vs. discarded tags was nearly identical (mean difference ~0.006).
2. **Vocabulary mismatch**: CLAP uses AudioSet-derived label names (`inst:electric guitar`, `inst:synthesizer`) while Qwen generates free-form labels (`inst:guitar`, `inst:synth`). There was zero exact-label overlap, making Precision/Recall/F1 evaluation degenerate (all TPs = 0).
3. **`feel` namespace unreliable**: CLAP's ability to distinguish emotional feel via text-audio similarity was too noisy to be useful in a library context where the user will filter by feel.

**Conclusion**: The signal-to-noise ratio of CLAP concept tagging is too low to surface to users. The embedding itself remains valuable for audio-text similarity verification (used by the Qwen pass) and will likely be useful for future similarity search / nearest-neighbour features.

---

### Qwen CLAP validation step (discarded)

**Approach**: After Qwen completed its own `feel` / `instruments` / `description` steps, a fourth turn was added asking it to confirm which CLAP-proposed tags were correct: *"An audio similarity model also suggested these tags … which are correct?"*. Confirmed tags were kept; the rest were soft-deleted (`discard = 1`).

**Why it failed**: The fundamental issue was the vocabulary mismatch described above. Even when Qwen did confirm tags, the confirmed set was ~9% of all CLAP proposals — too sparse to be useful, and the confirmed labels still didn't align with Qwen's own free-form instrument names. The extra LLM turn added latency (≈15 s/track at local inference speed) for negligible gain.

---

### Qwen `tags_context` step (discarded)

**Approach**: A conversation step asking Qwen to suggest 2–3 listening context tags (e.g. `context:driving`, `context:studying`).

**Why it failed**: The outputs were far too generic and inconsistent — the same track would get entirely different context tags across runs, and many tags bore no relationship to the actual audio. Removed without replacement.

---

### Qwen `tags_vocals` step (discarded)

**Approach**: A conversation step asking Qwen to identify voice type and lyrics language, writing to the `vocal` namespace.

**Why it failed**: Language detection was frequently wrong — Italian-language songs were tagged as English, and other non-English tracks were mislabelled. The `vocals:present` / `vocals:instrumental` signal is already handled more reliably by the Essentia classifier. Removed; `vocal` tags now come only from Essentia.
