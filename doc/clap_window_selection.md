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
