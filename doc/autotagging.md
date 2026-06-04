# Auto-Tagging System

Deep Cuts builds a rich tag vocabulary for every track through three cooperating analysis passes. Tags are stored in a normalized `tags` / `track_tags` schema (see `database.rs`) where each row carries a `source` column that identifies which pass wrote it. When a pass is re-run it deletes its own source rows and rewrites them from scratch, leaving tags from other sources untouched.

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
    source   TEXT NOT NULL,   -- 'clap', 'essentia', or 'qwen'
    PRIMARY KEY (track_id, tag_id),
    FOREIGN KEY (track_id) REFERENCES tracks(id) ON DELETE CASCADE,
    FOREIGN KEY (tag_id)   REFERENCES tags(id)   ON DELETE CASCADE
);
```

---

## Pass 1 — CLAP (source: `clap`)

**File**: [`src-tauri/src/analysis/clap.rs`](../src-tauri/src/analysis/clap.rs)  
**Priority**: 20 (runs early, depends only on `audio_analysis`)

The CLAP pass encodes every track as a 512-dim audio embedding using the CLAP model and stores it in `audio_embeddings`. After all tracks are processed it runs a library-wide **concept tagging** step:

1. Embeds each concept in `CONCEPT_MAP` using three text prompt templates (`"a song featuring {}"`, `"music with {}"`, `"{}"`), averaged and L2-normalised.
2. Computes the dot-product similarity of every (track, concept) pair, building an `n × m` matrix.
3. **Z-scores each concept column** across the library. Tracks that score ≥ 1.5 standard deviations above the library mean for a concept receive that tag (capped at 15 tags per track).
4. Deletes all existing `source='clap'` rows and writes the new set.

The concept map covers ~65 AudioSet labels mapped to four namespaces:

| Namespace | Examples |
|-----------|---------|
| `inst`    | acoustic guitar, piano, synthesizer, drums, violin, saxophone, … |
| `vocal`   | male, female, choir, rap, opera, beatbox, … |
| `feel`    | angry, happy, sad, tender, exciting, … |

CLAP tags are provisional — they are validated and potentially pruned by the Qwen pass that runs later.

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

Qwen2-Audio listens to the actual audio and runs a **multi-turn conversation** with the local llama-server. Each step extends the same conversation so later answers build on earlier context, reducing repetition.

### Conversation steps

| Step | Prompt (abbreviated) | Output stored | Tag namespace |
|------|----------------------|---------------|---------------|
| `feel` | *(initial system message — genre/feel)* | `tracks.ai_mood` | `feel` |
| `instruments` | List instruments you can clearly hear | `tracks.ai_instruments` | `inst` |
| `description` | Two to three prose sentences describing the track | `tracks.description` | — |
| `tags_vocals` | Voice type and lyrics language | — | `vocal` |
| `tags_context` | 2–3 suitable listening context tags | — | `context` |

If any step returns Chinese characters or the description is generic/invalid, the attempt is retried. On step failure the best partial result is saved rather than discarding everything.

### CLAP validation follow-up

After the main steps complete, Qwen reads the current `source='clap'` tags from the database and is asked to confirm which ones are correct given the audio it just heard. Only confirmed tags are re-written as `source='clap'`; the rest are discarded. This cross-pass validation ensures CLAP's statistical guesses are grounded by audio understanding.

### Tag cleaning

Raw model output passes through `clean_qwen_tags` which strips label prefixes (e.g. `FEEL:`, `INSTRUMENTS:`), lowercases, trims whitespace, and filters tokens against an instrument/genre whitelist before being split on commas into individual tag labels.

---

## Pass ordering and dependency graph

```
audio_analysis
    └─ bpm_correction
            └─ clap ──────────────────────────────┐
            └─ essentia ──────────────────────────┤
                                                  ▼
                                              qwen (validates clap, writes feel/inst/vocal/context)
                                                  └─ description_embed
```

---

## Tag namespaces summary

| Namespace | Written by | Meaning |
|-----------|-----------|---------|
| `inst`    | clap, qwen | Instruments heard in the track |
| `vocal`   | clap, essentia, qwen | Voice type / language |
| `feel`    | clap, qwen | Emotional character |
| `mood`    | essentia | Mood classifier output |
| `context` | qwen | Suitable listening situations |
