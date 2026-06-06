# CLAP Window Selection — Research & Findings

## Background

CLAP embeddings are computed by running the audio encoder on a 10-second mel-spectrogram window. For tracks longer than 10 seconds we must choose which part(s) of the track to sample. The embedding quality — and therefore the accuracy of similarity search and semantic features — depends heavily on this choice.

Our current implementation (`src-tauri/src/embeddings.rs`, `run_clap_inference_pooled`) runs the CLAP audio encoder **three times independently** on three 10-second windows, then **average-pools** the resulting 512-dim embeddings and L2-normalises the result.

---

## Current Algorithm: Top-3 Loudest Spaced

`select_clap_window_pcts` in `embeddings.rs`:

1. Parse the waveform envelope stored in `tracks.waveform_data` (128-bin JSON array of RMS energy values).
2. Rank bins by descending energy.
3. Greedily pick the top-3 bins that are each at least ~10 s apart from any already-selected bin.

**Problem identified:** For tracks that build to a climax near the end (common in pop, electronic, rock), all three windows cluster in the final 20–30% of the track. Analysis of 50 real tracks showed the third pick was ≥ 0.85 (85% into the track) in the vast majority of cases — often landing in the outro.

---

## Proposed Algorithm: Adaptive (Tercile + Temporal Spread)

### Key insight: coefficient of variation as a loudness-flatness detector

The coefficient of variation (CV = σ/μ) of the waveform envelope cleanly separates two track archetypes:

- **Dynamic tracks (CV ≥ 0.25, ~82% of library):** genuine loudness variation across the track. Energy-based selection is meaningful — louder sections tend to be more musically representative.
- **Flat-loudness tracks (CV < 0.25, ~18% of library):** modern brickwall mastering pushes RMS to near-constant levels. Energy differences between bins are noise, not signal. Energy-based selection degenerates to arbitrary picks.

### Branch 1 — Dynamic tracks: Tercile selection

Split the waveform bins into three energy terciles (low / mid / high). Pick the loudest bin from each tercile that satisfies the minimum spacing constraint. This guarantees:
- One sample from a quieter passage (verses, breakdowns)
- One sample from a mid-energy section (chorus, hook)
- One sample from the loudest section (drop, climax)

The three windows span the track's dynamic range rather than all competing for the same peak.

### Branch 2 — Flat-loudness tracks: Temporal spread

For tracks where the waveform CV is below threshold, ignore energy entirely and use fixed temporal anchors: **15%, 50%, 85%**. These avoid the intro (first ~10%) and outro (last ~10%) while covering early, middle, and late sections of the track body.

### Listening test results (June 2025)

Script: `scripts/compare_clap_windows.py`

**Round 1 — 10 random tracks, current vs. tercile-only:**
- Tercile clearly superior: 3 tracks (11222, 11732, 9315)
- Tie / both kept: 7 tracks
- Current superior: 0 tracks

**Round 2 — 10 flat-loudness tracks (CV < 0.25), current vs. adaptive:**
- Adaptive clearly superior: 1 track (11118)
- Tie: 9 tracks
- Current superior: 0 tracks

Interpretation: for truly flat-loudness tracks, any three well-spaced positions produce roughly equivalent embeddings — the temporal spread doesn't make things worse, but the gains are about avoiding pathological clustering rather than audible quality improvement. Still worth implementing.

Notable egregious current-algorithm failures:
- Track 11511 (Nádegas A Declarar, CV=0.101): current picks 0.80 / 0.94 / 0.99 — all three windows in the last minute of a 5-minute track.
- Track 12160 (1000 ördög, CV=0.130): current picks 0.61 / 0.72 / 0.76 — three windows within a 14-second span.
- Track 11381 (Screaming for Vengeance, CV=0.107): current picks 0.02 / 0.61 / 0.92 — first window lands at 6 seconds, likely in the intro.

---

## Order Sensitivity

**Finding: order does not matter for our implementation.**

The CLAP paper's fusion pathway stacks 4 windows as a tensor and processes them through attention-based convolutional fusion (AFF/iAFF) — that *is* order-sensitive.

However, we export and use the **non-fusion ONNX pathway**: each window is processed independently through the full audio encoder, producing one embedding per window. We then average-pool in Rust. A sum is commutative — order is irrelevant.

---

## Future direction: Per-section CLAP embeddings (structure-guided)

### Motivation

The tercile and temporal-spread algorithms are indirect proxies for song structure — they use
energy as a stand-in for "this is a verse" or "this is a chorus." Once the structural
classifier (MLP from `doc/research/sax_structure_learning.md`) produces reliable per-segment labels,
we can be explicit: sample one CLAP window per detected section, centered within that
section's run.

### Data model

Rather than one pooled embedding per track, store one embedding per detected structural
section run in a separate table:

```sql
CREATE TABLE section_embeddings (
    id INTEGER PRIMARY KEY,
    track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    section_type TEXT NOT NULL,   -- 'I','V','P','C','B','O','E', or NULL for tercile fallback
    position_start REAL NOT NULL, -- fractional position [0,1] in track
    position_end   REAL NOT NULL,
    confidence     REAL NOT NULL, -- MLP probability of the assigned label
    embedding      BLOB NOT NULL  -- 512-dim float32 CLAP vector (sqlite-vec)
);
```

A track with consolidated section runs `[I, V, C, V, C, B, O]` produces 7 rows. A track
where MLP confidence is low falls back to the tercile/temporal-spread algorithm and stores
3 rows with `section_type = NULL`.

### Window placement

For each consolidated section run (adjacent segments with the same label, after confidence
filtering):

- **Center the 10-second window** at the midpoint of the run.
- If the run is shorter than 10 seconds, center within it and pad; if longer, center in the
  densest (highest confidence) sub-segment.
- Skip the first 5% and last 5% of any `Intro` or `Outro` run to avoid fade-in/fade-out
  artifacts.

### Query patterns

**Whole-track similarity (current "sounds like"):**
Reconstruct by averaging all section embeddings for a track, weighted by section duration.
Equivalent to the current pooled embedding, but derived from structure-aware windows.

**Section-specific similarity:**
```sql
-- "Find tracks whose chorus sounds like X"
SELECT track_id, MIN(distance) AS best_chorus_distance
FROM section_embeddings
WHERE section_type = 'C'
GROUP BY track_id
ORDER BY best_chorus_distance;
```

**Cross-section queries:**
"Find tracks whose verse sounds like this track's chorus" — query the verse embedding of all
tracks against the chorus embedding of the seed. No mainstream music app exposes this.

### Blending with tercile embeddings

Since CLAP is cheap, both approaches can coexist:

- **Structure-guided** (section_type IS NOT NULL): used for section-specific and
  cross-section queries; also feeds the pooled whole-track embedding.
- **Tercile fallback** (section_type IS NULL): always computed as a safety net; used when
  MLP confidence is low or labels haven't been computed yet.

A confidence-weighted blend: `α × structure_pooled + (1−α) × tercile_pooled` where α = mean
label confidence across all section embeddings for the track. When the MLP is uncertain,
tercile dominates; when confident, structure-guided dominates.

### Pipeline dependency

```
audio_analysis  →  sax + repetition vector (waveform_labels)
waveform_labels →  MLP structural classifier  →  section_type assignments
section_types   →  CLAP pass  →  section_embeddings rows
```

CLAP becomes the final pass. For new tracks, tercile embeddings can be computed immediately;
structure-guided embeddings are added once the MLP pass completes. The `section_embeddings`
table can be populated incrementally without invalidating existing tercile rows.

### Prerequisite

Reliable MLP labels. At 51% overall accuracy (current D2 model, Downspiral-only training)
this would degrade CLAP quality for ~half the library. Target: retrain on Genius-expanded
dataset (400–600 labeled tracks) and validate Chorus recall ≥ 50% before switching
structure-guided windows on by default.

---

## Future direction: CLAP native fusion pathway

The CLAP training code (`CLAP/src/laion_clap/training/data.py`) implements a 4-window fusion scheme:

```
mel_fusion = stack([mel_shrink, mel_chunk_front, mel_chunk_middle, mel_chunk_back])
```

Where `mel_shrink` is the **entire track's mel-spectrogram compressed to the standard input size**. This is fed through the HTSAT encoder's `PatchEmbed` layer with a 2D convolutional fusion head, producing a single embedding in one forward pass.

**Advantage over our approach:** The shrink window gives the model a coarse global view of the entire track's structure — something our 3-window average has no equivalent of. For tracks with strong structural variation (long intros, breakdowns, outros) the model can "see" the whole arc.

**Disadvantage:** The front/middle/back windows are fixed temporal positions, with no content-awareness. The attention mechanism can learn to downweight a bad window but cannot replace it.

**Why we can't use it today:** Our exported ONNX model uses the non-fusion pathway. The fusion model has additional convolutional layers for the attention mechanism — it's architecturally different and would require a new ONNX export and validation pass.

**Ideal future state:** Use the fusion ONNX model but substitute our adaptive window selection for the fixed front/middle/back picks. The shrink window would be kept as-is (full-track thumbnail). This would combine the structural awareness of fusion with the content-awareness of our selection algorithm.

---

## Implementation notes

- CV threshold of 0.25 splits the library ~82/18. Worth re-evaluating if the library composition changes significantly.
- `scripts/compare_clap_windows.py` generates side-by-side WAV excerpts for any track ID for manual evaluation.
- After implementing the new algorithm, the `clap` pass version (`pass_version::CLAP` in `scanner/sidecar.rs`) must be bumped to trigger a full re-run.
- The `track_coords` table (UMAP projections) will also need to be regenerated after re-embedding.
