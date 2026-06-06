---
name: how-to-experiment
description: Experimental protocol for Deep Cuts research, prototypes, model evaluations, threshold tuning, ablations, metric comparisons, and claims about accuracy or quality. Use before running or interpreting experiments so bots preserve train/validation/test boundaries, avoid leakage, compare against baselines, and report results honestly.
---

# How To Experiment

Use this skill whenever work involves measuring whether an idea is good: model training,
classifier accuracy, embedding quality, search relevance, threshold tuning, UI heuristics,
performance changes, ablations, or comparisons between approaches.

The purpose is not bureaucracy. It is to prevent fast prototype loops from producing numbers
that look convincing but do not generalize. Deep Cuts has already hit this failure mode: the
SAX sequence tagger reported **99.27% accuracy on the same 740 tracks used for training** in
`doc/collab/sessions/2026-06-06-sax-transformer/session.md`; the follow-up in
`future-ideas.md` correctly marked that number as untrustworthy until evaluated on a held-out
split.

## Minimum Protocol

Before running the experiment, write down:

1. **Question**: the specific claim being tested.
2. **Unit of split**: usually track, album, artist, or library. Never split by segment/window if
   segments from the same track can leak into both train and test.
3. **Metric**: the primary metric and any acceptance threshold.
4. **Baseline**: the current production behavior, simple heuristic, or prior model.
5. **Frozen evaluation set**: what data is reserved for final reporting.
6. **Artifacts**: scripts, command lines, seeds, dataset versions, and output files.

If any of these are missing, call the run exploratory and do not make generalization claims.

## Data Splits

Use three sets when tuning is involved:

- **Train**: fit model weights, thresholds, centroids, clusters, or prompt examples.
- **Validation**: choose hyperparameters, early stopping, thresholds, prompt variants, UI
  constants, and feature combinations.
- **Test / holdout**: final one-time estimate. Do not tune on it.

Default split for enough data: **70% train / 10% validation / 20% test**, stratified when useful
by genre, artist, label class, duration bucket, or library source.

For small data:

- Prefer grouped cross-validation over repeatedly peeking at one tiny holdout.
- Keep a final untouched holdout if the result will drive production behavior.
- Report uncertainty: per-class counts, confidence intervals, or "N is too small" notes.

For music data, prevent leakage aggressively:

- Split by **track** at minimum.
- Split by **artist or album** when evaluating generalization to unseen music styles.
- Keep derived windows, SAX segments, embeddings, lyrics labels, and cached pass outputs on the
  same side of the split as their source track.
- If near-duplicates or alternate masters exist, keep them in the same split.

## Test Set Rules

The test set is not a dashboard knob.

- Do not inspect test errors while choosing parameters.
- Do not rerun many variants and report the best test score.
- Do not add "just one fix" after seeing test failures without moving back to validation and
  creating a new final holdout.
- If the test set was used for decisions, say so and rename it to validation.

Acceptable use of test results:

- Final report after the approach is frozen.
- Regression check for a previously frozen production model.
- External benchmark evaluation with no fine-tuning on that benchmark.

## Baselines And Ablations

Every experiment needs a comparison.

- **Baseline**: current app behavior, a simple rule, random/majority class, or previous model.
- **Ablation**: remove one suspected shortcut or feature at a time.
- **Sanity check**: shuffle labels, use random embeddings, or test a trivial predictor when
  appropriate.

Deep Cuts example: for structural labels, `position` can be a shortcut. A model using
`energy + rep_score + position` must be compared against `energy + rep_score` without
`position` before claiming it learned musical structure.

## Metrics

Pick the metric that matches the task, not the one that is easiest to compute.

- Classification: report overall accuracy plus per-class precision/recall/F1 and class counts.
- Imbalanced labels: prefer macro F1, balanced accuracy, or per-class recall over raw accuracy.
- Ranking/search: use fixed query sets with precision@K, recall@K, MRR, nDCG, or blinded human
  judgments.
- Segmentation/boundaries: use boundary F-measure with a stated tolerance when comparing to
  music-structure benchmarks; fixed-frame accuracy may be useful internally but is not the
  standard public claim.
- Performance: separate cold start, warm run, per-track latency, throughput, memory, and model
  load time.

When a metric is a proxy, say what it does not prove.

## Reporting Template

Use this shape in docs, session logs, or handoffs:

```markdown
## Experiment: <name>

Question:
Dataset:
Split unit:
Split:
Leakage controls:
Baseline:
Variants:
Primary metric:
Secondary metrics:
Commands/scripts:
Result:
Decision:
Limitations:
Artifacts:
```

## Result Labels

Use precise labels:

- **Exploratory**: useful signal, no protected holdout, okay for idea generation.
- **Validation result**: tuned on validation data, useful for choosing a variant.
- **Held-out result**: final result on untouched test data.
- **Production regression**: checks that a frozen behavior did not degrade.

Avoid unsupported wording:

- Do not say "99% accurate" if the number is training accuracy.
- Do not say "better" without naming the baseline and metric.
- Do not say "generalizes" without an unseen-track, unseen-artist, unseen-library, or external
  benchmark evaluation.

## When Speed Matters

Fast loops are fine if labeled correctly.

For a quick prototype:

1. Run on a small sample.
2. Compare against one baseline.
3. Save the script and seed.
4. Mark the result **exploratory**.
5. List the minimum next step needed before production use.

Do not let a quick loop silently become the evidence for a production decision.
