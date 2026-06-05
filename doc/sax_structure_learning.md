# SAX Structure Learning — Model Options

## Context

The hand-tuned centroid approach (7 fixed (energy, repetition) centroids derived from 153
Downspiral tracks) has two known problems:

1. **Downspiral bias** — centroids reflect one artist's structural conventions, not the library.
2. **Discrete cost quantization** — edit distance scores clump at identical values because
   substitution costs between similar centroids happen to be equal.

The goal is to replace manual calibration with a learned model that generalises across genres.

Available features per segment (16 segments per track):
- `energy` — SAX letter mapped to [0, 1] (a=0, b=0.25, c=0.5, d=0.75, e=1.0)
- `rep_score` — normalised SSM repetition score [0, 1]
- `position` — fractional position in track [0, 1] (free, always available)

Available data:
- **1890 tracks**, all with `waveform_data` + `waveform_sax` + 16-segment SSM → 30,240 segments, no labels
- **153 Downspiral tracks** with `lyrics.txt` section labels → ~2,400 weakly-labeled segments
  (labels are position-approximate: line number / total lines ≈ fractional position in track)

---

## Option A — Unsupervised: Gaussian Mixture Model

**Idea:** Replace the 7 hand-tuned centroids with a GMM trained on all 30,240 segments in 2D
(energy × repetition). Natural clusters emerge from the full library, not from Downspiral alone.

**Why better than K-means:**
- Soft cluster assignments give continuous probabilities → edit distance costs become continuous
  naturally, breaking the quantization problem without any additional heuristics.
- Elliptical covariance captures the fact that Chorus has low energy-σ but high rep-σ (Outro),
  whereas a sphere (K-means) cannot.

**Training:** `sklearn.mixture.GaussianMixture(n_components=7)` on all segments. ~30 lines.

**Label alignment:** Use the 153 Downspiral tracks to name components post-hoc. For each
labeled segment, record which GMM component fired most often. Component 4 → mostly Chorus
positions → call it C. No gradient descent on the labeled data.

**Limitation:** Still treats each segment independently — no sequence context. A Chorus segment
that follows two Verse segments is treated the same as an isolated Chorus segment.

**Effort:** ~30 minutes. Good first validation: if GMM clusters are clean, the problem is
geometry. If not, the 2D feature space is insufficient.

---

## Option B — Self-supervised: Contrastive Segment Embeddings

**Idea:** The SSM already provides free supervision: if segments *i* and *j* within a track
have high cosine similarity, they are probably the same section type. Train a tiny MLP to
embed each segment such that SSM-similar segments are nearby in embedding space.

**Architecture:**
```
Input: (energy, rep_score, position) → 3D
MLP:   Linear(3→16) → ReLU → Linear(16→8) → L2-normalise → 8D embedding
Loss:  Contrastive (or NT-Xent / cosine similarity loss)
       Positive pair: segments with SSM cosine sim > 0.8 within same track
       Negative pair: segments with SSM cosine sim < 0.3, or from different tracks
```

**Training data:** All 1890 tracks, zero manual labels. The SSM is the teacher.

**Label alignment:** After training, cluster the 8D embeddings (K-means or GMM, K=7).
Use the 153 Downspiral weak labels to name clusters: find which cluster each labeled segment
falls into, vote by plurality.

**Why better than A:**
- The 8D embedding can capture structure that 2D (energy, rep) cannot — e.g. a Bridge is
  energetically similar to Chorus but contextually unique (low repetition across the track).
- The learned geometry reflects the full library, not our manual priors.

**Limitation:** Still per-segment, no sequence context.

**Effort:** ~2–3 hours. Requires PyTorch or a numpy contrastive loss implementation.

---

## Option C — Semi-supervised: Small Sequence Classifier

**Idea:** A tiny sequence model (1D CNN or LSTM) takes the full 16-step feature sequence as
input and predicts a label for each step. Trained on the 153 Downspiral tracks with weak labels.

**Architecture:**
```
Input:  (16 × 3) — energy, rep_score, position per segment
Model:  Conv1d(3→16, kernel=3) → ReLU → Conv1d(16→16, kernel=3) → ReLU → Linear(16→7)
Output: (16 × 7) — per-segment label probabilities
Params: ~1,000
Loss:   Cross-entropy only at labeled positions (sparse supervision)
```

**Key advantage over A and B:** Sequence context. A Chorus prediction after I→V→V is more
confident than an isolated Chorus prediction. This should fix the classical false positives
(Tchaikovsky's loud movements don't follow an I→V→C transition pattern).

**Data augmentation:** Time-warp the 16-step sequence (randomly resample 12–20 steps → 16)
to simulate structural variation and prevent overfitting on 153 tracks.

**Training data:** 153 labeled tracks → ~2,400 labeled segments (weak, position-approximate).

**Limitation:** Small labeled set; may overfit to Downspiral conventions unless regularised
heavily. Needs validation on held-out non-Downspiral tracks.

**Effort:** ~2–3 hours.

---

## Option C+ — Pretrain B, Fine-tune C (recommended long-term)

**Idea:** Two-stage training combining the best of B and C.

1. **Pretrain (self-supervised, all 1890 tracks):** Train the contrastive MLP from Option B
   to produce 8D segment embeddings. The geometry reflects the full library.

2. **Fine-tune (semi-supervised, 153 tracks):** Replace the 3D input of Option C's sequence
   classifier with the 8D pretrained embeddings. Train the sequence model on the Downspiral
   weak labels. The pretrained geometry gives the sequence model a head start; the weak labels
   only provide structural interpretation, not raw cluster geometry.

**Why this is the most principled approach:**
- All unlabeled data shapes the embedding geometry.
- The 153 labels provide section naming and sequence context, not cluster calibration.
- Adding more labeled tracks (from any artist) incrementally improves the fine-tuning without
  retraining the self-supervised stage.

**Effort:** 4–6 hours total (B + C in sequence).

---

## Decision tree

```
Start → Run GMM (Option A, 30 min)
         │
         ├── GMM clusters align cleanly with section types?
         │       Yes → Use GMM soft assignments as drop-in for centroids.
         │             Add sequence context later with Option C on top.
         │
         └── GMM clusters are messy / Pre-Chorus still bleeds into Chorus?
                 │
                 └── 2D feature space is insufficient.
                     Run Option B (contrastive, 2–3h).
                     If B embeddings cluster cleanly → proceed to C+.
```

---

## Files

- Feature computation: `tools/sax_structure_explorer.py`, inline experiment scripts
- Weak labels: `~/Downloads/MP3 Songs/*/lyrics.txt` (153 tracks)
- Related docs: `doc/sax_structural_search.md`, `doc/sax_structure.md`
- Experiment results: `doc/sax_structural_search.md` § Prototype experiment results
