# Future Ideas: SAX Transformer Improvements

## 1. Ablate the `position` feature

The current model takes three inputs per segment: `energy`, `rep_score`, and `position` (fractional track position). `position` is a shortcut — it lets the model learn "choruses statistically appear around 40–60% through a track in our library" rather than discovering structure from audio alone.

**Experiment**: drop `position` from the feature vector and retrain. Measure how much accuracy falls.
- If accuracy drops significantly: `position` was doing most of the work; the sequence model is weaker than it looks.
- If accuracy holds: `energy` + `rep_score` are genuinely carrying the signal — that's the interesting result worth reporting.

The distinction matters: a model that discovers sequence is musically meaningful and generalises to unseen libraries. A model that leans on position is a statistical shortcut tied to this specific dataset.

## 2. Run a proper held-out evaluation on the existing dataset

The 99.27% training accuracy was measured with no train/val/test split — the model was evaluated on the same 740 tracks it was trained on. This number is currently untrustworthy as a measure of generalisation.

**Minimum viable experiment**:
- Split the 740-track library into train (70%) / val (10%) / test (20%) *before* training, stratified by genre if possible.
- Report frame accuracy on the held-out test set only.
- Use the val set for early stopping and hyperparameter tuning, not the test set.

This is a prerequisite for any honest comparison — against a public benchmark or against ablations (idea 1). Without it, all other accuracy numbers are suspect.

## 3. Evaluate against a public benchmark

Standard benchmarks for music structure analysis:
- **SALAMI** (~1400 tracks, dual human annotations) — freely available, the main reference dataset.
- **RWC Popular** (100 tracks) — available via RWC dataset request.

The standard metric is **boundary detection F-measure at ±0.5s and ±3s tolerance**, not frame accuracy. The current fixed 16-segment framing outputs boundaries at fixed positions, which is a structural mismatch — variable-length boundary prediction would be needed to compete fairly.

SALAMI is also useful as a zero-shot generalisation test: train on our 740-track library, evaluate on SALAMI without fine-tuning. A large gap between the two would confirm the model is overfitting to library-specific patterns (e.g. position shortcuts).

## 4. Replace SAX energy with CLAP embeddings as input

The 3-feature input (SAX energy, rep_score, position) is much weaker than what SALAMI-benchmarked systems use. We already compute 512-dim CLAP embeddings per track.

**Idea**: compute CLAP embeddings over 16 equal-length segments of each track (sliding window inference), then feed the sequence of segment embeddings into the GRU or Transformer instead of the hand-crafted features. CLAP captures timbre, instrumentation, and energy jointly — a much richer signal.

This would be a novel combination (symbolic SAX fingerprint for search + CLAP-based sequence model for structural labeling) and a plausible SoTA attempt on SALAMI worth writing up.
