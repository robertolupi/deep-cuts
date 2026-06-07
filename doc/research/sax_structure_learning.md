---
status: active
owner: Roberto
last_verified: 2026-06-07
implemented_by:
superseded_by:
related_code:
related_skills:
---

# SAX Structure Learning — Model Options

## Current State

This document is a model-research backlog. The current app uses SAX/alignment/cluster analysis, but no production learned section-label model was found.

| Area | Status | Evidence / Notes |
| :--- | :--- | :--- |
| Hand-engineered SAX/structure features | Implemented | Energy-envelope SAX, alignment, and structure clustering are present in the app. |
| Weak-label experiments | Active research | The Downspiral/lyrics workflow is useful evidence, but it is not a production dependency. |
| GMM / contrastive / sequence-model options | Need human review | These are alternative research paths. Pick one deliberately before adding model code or schema. |
| Production learned labels | Not implemented | No shipped classifier, model artifact, or inference path for learned section labels was found. |

---

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

## Option D — Weak-supervision MLP + pseudo-label loop (recommended next step)

Based on GMM result (Option A confirmed feature space is insufficient for hard boundaries)
and Meta AI review. The core insight: **output probabilities, not labels**. Soft scores fix
the discrete cost problem automatically.

### D1 — Logistic regression baseline (try first, 30 min)

Input: `[energy, rep_score, position]` — 3 features, already available per segment.
Output: softmax over 7 classes (I, V, P, C, B, O, E).

Why try this before MLP: if logistic regression on 3 features already separates P/C/V better
than the hand-tuned centroids, the feature set is sufficient and nonlinearity isn't needed.
The GMM failure may have been a geometry problem (wrong covariance structure) rather than a
feature problem.

```python
from sklearn.linear_model import LogisticRegression
from sklearn.preprocessing import StandardScaler

X = np.array([[energy, rep, position], ...])   # labeled segments from Downspiral
y = [label, ...]

clf = LogisticRegression(C=1.0, max_iter=500, class_weight='balanced')
clf.fit(X_train, y_train)
probs = clf.predict_proba(X_test)  # shape (n_segments, 7) — continuous, no ties
```

Label smoothing for noisy alignment: use `LabelEncoder` + mix 10% uniform prior into
the one-hot targets before fitting, or use sklearn's `class_weight='balanced'`.

### D2 — MLP with pseudo-label loop (if LR insufficient)

**Architecture:**
```
Input:  [energy, rep_score, position, sax_onehot_5] → 8D
Layer1: Linear(8→32) → ReLU → Dropout(0.3)
Layer2: Linear(32→16) → ReLU
Output: Linear(16→7) → Softmax
Params: ~2,000
```

**Training loop (semi-supervised):**
1. Train on 153 Downspiral tracks (label smoothing ε=0.1 for noisy alignment).
2. Run inference on 1,737 unlabeled tracks; keep predictions with confidence > 0.7 as pseudo-labels.
3. Retrain on original labels + pseudo-labels. One loop is enough at this scale.

The model learns to merge P and C where the library doesn't support the distinction, or keep
them separate where it does. No manual calibration.

### D3 — Replace edit distance with -log probability + Viterbi

Once a soft classifier exists (LR or MLP), replace the Levenshtein DP entirely:

**Score a query pattern against a track:**
```
For query [I, V, C, V, C, O] and track with per-segment probability matrix P (16×7):
  score(track) = min-cost alignment via Viterbi over -log P[:, label_idx]
  + transition penalty for staying in same section (encourages segment runs)
```

This is forced alignment — exactly how speech recognition aligns a phoneme sequence to
a probability matrix. Benefits:
- **Continuous scores** — costs are -log probabilities, no two tracks tie.
- **Tiebreaker is free** — confidence is baked into the cost.
- **Time boundaries** — Viterbi backtrace gives the actual segment positions where each
  block was matched, enabling the "show why a track matched" waveform overlay in the UI.
- **Transition penalties** — can encode "chorus lasts at least 2 segments" without rules.

**Implementation sketch:**
```python
def viterbi_align(log_probs, query_label_indices, transition_penalty=0.5):
    """
    log_probs: (n_seg, n_classes) — log P(label | segment)
    query_label_indices: list of class indices for the query
    Returns: (total_cost, [matched_segment_indices])
    """
    n, m = log_probs.shape[0], len(query_label_indices)
    dp = np.full((n, m), np.inf)
    bp = np.zeros((n, m), dtype=int)
    dp[0, 0] = -log_probs[0, query_label_indices[0]]
    for i in range(1, n):
        for j in range(m):
            # stay on same query position (skip track segment)
            stay = dp[i-1, j] + 0.5  # insertion cost
            # advance query (match this segment to query[j])
            cost = -log_probs[i, query_label_indices[j]]
            advance = dp[i-1, j-1] + cost if j > 0 else np.inf
            if stay < advance:
                dp[i, j] = stay; bp[i, j] = j
            else:
                dp[i, j] = advance; bp[i, j] = j - 1
    # backtrack ...
    return dp[-1, -1], backtrack(bp)
```

---

## GMM result (Option A — June 2025)

**Outcome: feature space insufficient for hard boundaries.**

- 3 Chorus components, 0 useful Intro/Pre-Chorus/Bridge/Outro/End components
- Verse: 55% accuracy, Chorus: 79%, all others: 0%
- Root cause: Pre-Chorus centroid (0.447 energy, 0.845 rep) and Chorus (0.647, 0.875)
  are too close; brickwalled masters collapse the gap further
- Classical false positives: quiet Goldberg variations look like Verse in 2D

**Conclusion:** The 3D feature space can support soft boundaries but not hard ones. Move to
Option D1 (logistic regression) to test whether a learned soft boundary is sufficient, before
investing in contrastive pretraining (Option B/C+).

---

## Decision tree (updated)

```
GMM (Option A) ✅ DONE — feature space insufficient for hard clusters
         │
         └── Option D1: logistic regression on [energy, rep, position]
                  │
                  ├── LR accuracy >> GMM (>50% on Intro/Outro/Bridge)?
                  │       Yes → Feature set is sufficient. Use LR probabilities
                  │             + Viterbi alignment (D3). Done.
                  │
                  └── LR still collapses P/C/V?
                           │
                           ├── Option D2: MLP + pseudo-label loop
                           │     (adds nonlinearity + unlabeled data)
                           │
                           └── Option B/C+: contrastive pretraining
                                 (richer features, if 3D is truly insufficient)
```

---

## Experiment results summary (June 2025)

### Option A — GMM (failed)
Three components collapsed to Chorus. Intro/Pre-Chorus/Bridge/Outro/End: 0% accuracy.
**Verdict:** Hard boundaries don't work in 3D. Feature space is sufficient but needs soft classifier.

### Option D1 — Logistic Regression (partial success)
- Overall accuracy: **38%**
- Intro: 96% recall ✅ (position signal dominates)
- Outro: 68%, Bridge: 54% — better than GMM
- Verse: 9% recall ❌ — still confused with everything
- Chorus: 28% recall — worse than GMM (balanced weighting sacrificed majority class)
- Query results: continuous costs ✅, but "I Left My Heart in San Francisco" topping
  both classic pop and drop queries — not enough discriminative power
- **Verdict:** Feature set sufficient, model capacity insufficient. Move to MLP.

### Option D2 — sklearn MLP (32→16→7, sample-weighted) ✅
- Overall accuracy: **51%** (+13pp over LR)
- Intro: 92% recall ✅
- Bridge: 79% (+25pp), Outro: 71%, End: 68%, Pre-Chorus: 54%
- Verse: 40% (+31pp) ✅ — big win from nonlinearity
- Chorus: 32% — still weakest; most acoustically diverse class
- Viterbi costs fully continuous, no ties
- Query highlights:
  - **Build [I,V,C]**: Rammstein "Amour", Céline Dion, downspiral — diverse, appropriate ✅
  - **Ends quietly [C,C,E]**: Vangelis "Love Theme from Blade Runner", NIN "Head Down" ✅
  - **Classical false positives** persist in Drop/pop queries — Baroque repetition looks
    like verse-chorus in 2D. Known limitation, not blocking.
- **Verdict:** Good enough for a first block composer. Remaining issues are data-limited,
  not architecture-limited.

### Convergence warning
sklearn MLP hit max_iter=500 without full convergence. Increase to 1000 or switch to
`solver='adam'` with `early_stopping=True` for the production model.

---

## Next: expand labeled data via Genius API

**Problem:** 153 labeled tracks (all Downspiral) biases the MLP toward one artist's
structural conventions. Chorus recall (32%) suffers most — needs more diverse examples.

**Plan:** Fetch lyrics with section labels from the Genius API for all library tracks,
store them alongside audio, and retrain the MLP on the expanded dataset.

### Why Genius
- Only major service where community lyrics include structural markers (`[Verse 1]`,
  `[Chorus]`, `[Bridge]`) in the same `[Label]` format as Downspiral's lyrics.txt files
- Free API, generous rate limit (~1 req/sec), personal/educational use permitted
- API docs: https://docs.genius.com

### What we expect to gain
- Pop, rock, electronic tracks: good label coverage — these are the genres where section
  labels are most reliable and most needed for training
- Classical, jazz, ambient, instrumental: few or no Genius entries — that's fine, those
  genres are structurally distinct and may need separate handling anyway
- Target: 400–600 additional labeled tracks → MLP Chorus recall should recover to 50%+

### Implementation plan

1. **Register a Genius API app** (user action, ~2 min at genius.com/api-clients)
   — produces a client access token, no OAuth needed for read-only search

2. **Write `tools/fetch_genius_lyrics.py`**
   - Query Genius search API: `GET /search?q={title} {artist}`
   - Parse top result: match on artist name similarity to avoid wrong-song hits
   - Fetch lyrics page, extract section-labeled text
   - Save to `lyrics.txt` alongside the audio file (same format as Downspiral)
   - Skip tracks that already have `lyrics.txt`
   - Skip tracks with no artist/title metadata
   - Rate-limit to 1 req/sec

3. **Validate a sample** — spot-check 20 fetched lyrics files for label quality
   before retraining

4. **Retrain MLP** on full labeled set (153 Downspiral + N Genius)
   — expect Verse and Chorus recall to improve most

5. **Re-run queries** and compare against current D2 results

### Schema note
No DB changes needed — lyrics live on disk alongside audio files, same as Downspiral.
The training pipeline already discovers them via `Path(track.path).parent / "lyrics.txt"`.

---

## Files

- Feature computation: `tools/sax_structure_explorer.py`, inline experiment scripts
- Weak labels: `~/Downloads/MP3 Songs/*/lyrics.txt` (153 tracks)
- Related docs: `doc/research/sax_structural_search.md`, `doc/research/sax_structure.md`
- Experiment results: `doc/research/sax_structural_search.md` § Prototype experiment results
