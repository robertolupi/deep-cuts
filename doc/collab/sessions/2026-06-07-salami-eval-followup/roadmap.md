# SALAMI Structure Roadmap: Scientific Progress at AI-Collaboration Speed

Status: **FROZEN (2026-06-07)** — agreed by Codex, Gemini/agy, and Claude after amendments A-H.  
Source session: `doc/collab/sessions/2026-06-07-salami-eval-design/`  
Date: 2026-06-07

## Executive Summary

The previous SALAMI evaluation reached a useful stopping point: the current 16-bin SAX boundary
pipeline is post-processing saturated. The next improvement must raise temporal resolution and
prove that it raises the measurable ceiling under `mir_eval`, not just produce nicer-looking
boundaries.

The fastest scientific path is an offline Python experiment loop over already-cached onset peaks
and 0.2 s chroma sidecars. Dense CLAP and Rust integration come later, only after validation
results justify their cost.

## What We Treat As Settled

- **Scorer:** `mir_eval.segment.detection` is the canonical boundary scorer at +-0.5 s and
  +-3.0 s.
- **Reference anchors:** always report baseline, refined 16-bin result, grid/oracle ceiling, and
  human ceiling on the same subset.
- **Main diagnosis:** the 16-bin grid is saturated; post-processing is not the next lever.
- **Storage:** raw SSMs are not production artifacts. Recompute them from cached dense features.
- **Feature order:** onset/chroma first, dense CLAP second.
- **Holdout discipline:** the old 57-track holdout was already used once for `augment+8peaks_5s`.
  New approaches need validation-only tuning and one final protected check.

## Roadmap

### Phase 0: Evaluation Contract

Deliverable: one reproducible evaluation entry point that emits a JSON/CSV report.

Requirements:

- Fixed track IDs for train/validation/test, with split unit = track.
- No segment/window-level leakage across splits.
- Metrics at +-0.5 s and +-3.0 s with `mir_eval`.
- Per-track outputs: model F1, grid ceiling F1, human agreement F1, normalized score, boundary
  count, duration, and failure notes.
- A stable experiment manifest containing code version, input DB path, sidecar version, split
  files, parameters, random seed, and output path.

Gate to proceed: the script reproduces the archived canonical numbers within expected tolerance.

### Phase 1: Chroma/Onset SSM Prototype

Question: can dense onset/chroma novelty escape the 16-bin grid ceiling?

Dataset: SALAMI validation subset only.

Variants:

- Onset novelty only.
- Chroma SSM novelty only.
- Late fusion: normalized onset novelty + chroma SSM novelty.
- Simple boundary-count controls, so gains are not just from adding too many boundaries.

Primary metric:

- `mir_eval` boundary F1 at +-3.0 s against human annotations.

Secondary metrics:

- `mir_eval` boundary F1 at +-0.5 s.
- Boundary count error versus human annotations.
- Refined/grid and grid/human decomposition.
- Per-track normalized score against human agreement.

Gate:

- Beat the ~34% 16-bin grid/oracle at +-3.0 s on validation.
- Show a meaningful +-0.5 s lift over the archived 7.6% refined result.
- Preserve honest boundary counts; do not inflate recall with dense false positives.

### Phase 2: Validation Discipline And Ablations

Before any holdout result, freeze:

- Kernel width.
- Novelty smoothing.
- Peak-picking threshold.
- Minimum boundary spacing.
- Fusion weights.
- Boundary-count prior.

Required ablations:

- Chroma without onset.
- Onset without chroma.
- Shuffled or random feature sanity check.
- Fixed boundary-count baseline.
- 16-bin refined baseline re-run in the same script.

Report label: **validation result**, never held-out, until the config is frozen.

### Phase 3: Protected Holdout Decision

Use a held-out result only after Phase 2 chooses exactly one config.

Decision rule:

- If the old 57-track holdout is still accepted as the protected check for this new approach,
  spend it once and mark it spent for SSM work.
- If we want repeated future SSM comparisons, carve a new protected split before looking at
  results.

Held-out report must include uncertainty and per-track diagnostics, not only aggregate F1.

### Phase 4: Product Integration Gate

Only port to Rust/Tauri if validation and held-out results clear the gate.

Integration scope:

- Store compact boundary candidates and novelty scores in SQLite.
- Keep dense features in sidecars.
- Do not persist raw SSMs.
- Surface boundaries and confidence in the UI only after the pipeline output is stable.

Non-goals until proven:

- Dense CLAP recomputation across the library.
- Neural sequence labeler on 16-bin inputs.
- SoTA claims.

### Phase 5: Dense CLAP And Label Semantics

Dense CLAP becomes worthwhile only if chroma/onset SSM clears the boundary gate but label quality
or timbral segmentation remains weak.

Experiment path:

- Prototype dense CLAP on a small validation subset.
- Compare CLAP-only, chroma-only, and fused SSM novelty.
- Evaluate label/segment consistency separately from boundary detection.
- Keep SoTA language out unless evaluated on full SALAMI with published-comparable protocol.

## Collaboration Process

To keep AI-collaboration speed without scientific drift:

- Each agent owns a narrow question and writes an experiment manifest before running.
- Agents can run in parallel through MCP task claims, but shared report edits go through the
  session log and explicit ACKs.
- Any result that influences a decision must include: question, split, baseline, metric, command,
  artifact path, result label, and limitation.
- The coordinator merges only frozen artifacts: scripts, reports, and decision logs.
- Roberto's steering decisions are recorded in `session.md`, not only chat.

## Recommended Next Action

Start with Phase 0 and Phase 1:

1. Build or consolidate the canonical `mir_eval` report script — emitting **P/R/F1** (amendment A)
   plus the pairwise/label metric (B), with a committed **golden-number regression test** (F) over
   the existing `holdout_tracks.json` / `validation_tracks.json` splits (G). Bake in the
   **per-track crop offset** `max(0, duration/2 − 45)` and decide windowed-vs-full-track eval (H)
   *before* any prototype run — this is the first correctness gate.
2. Run a chroma/onset SSM prototype on validation only, against both our 16-bin oracle and a
   **stock external baseline** (D), reporting **bootstrap CIs + a paired significance test** (C).
3. Draft a validation report that explicitly decides whether the approach escapes the 16-bin
   grid ceiling — "escape" meaning a *statistically significant* win over the oracle, not a higher
   point estimate.

Do not touch the holdout, Rust integration, or dense CLAP until that validation report exists.

---

## Claude review (2026-06-07) — amendments to reach agreement

I ACK the roadmap's structure, settled-facts list, and phase gating — it faithfully carries
the archived session's conclusions and the app-first priority. The following are **additive
amendments**, each tagged to a phase. They close gaps that matter specifically because we are
iterating *fast with multiple agents*: speed multiplies both throughput and the risk of fooling
ourselves. Folding these in, I consider the roadmap agreed.

### A. Report Precision and Recall, not just F1 (→ Phase 0, 1, 2)

F1 is gameable by boundary count, and the entire archived diagnosis was a **recall** problem
(grid under-segments: 7.7 vs 14.0 GT boundaries), not placement. A single F1 hides this. **Mandate
the full P / R / F1 triple at both ±0.5 s and ±3.0 s** for every reported config. "Boundary-count
error" is necessary but not sufficient — P and R make the recall-vs-precision tradeoff legible and
stop a variant from "winning" by spraying boundaries.

### B. Keep the label/clustering metric alive as a regression guard (→ Phase 0, 1)

The 16-bin model's *strength* is grouping: pairwise clustering F1 ≈ 81% of human ceiling, far above
its boundary score. Adding off-grid boundaries can **hurt** segment labeling. Track
pairwise clustering F1 (and/or V-measure) alongside boundaries so a boundary win that regresses
labels is caught, not shipped. Labels stay "Phase 5" for *optimization*, but must be *monitored*
from Phase 1.

### C. Statistical honesty for fast iteration (→ Phase 2, 3) — the key addition

With N≈196 dual-annotator validation tracks and high per-track F1 variance, a +2 pp aggregate move
can be noise. At AI-collaboration speed we will generate many variants, so:

1. **Bootstrap 95% CIs over tracks** for every headline number (resample tracks with replacement,
   ≥1000 draws). Report `33.3% [30.1, 36.5]`, never a bare point estimate.
2. **The gate is a *significant* improvement, not a numerically higher one.** "Beat the ~34%
   oracle" → "beat it with a paired test": per-track paired bootstrap or Wilcoxon signed-rank on
   (SSM_F1 − refined_F1) across the same tracks, p < 0.05. A 34.5% that overlaps the oracle CI has
   not escaped the grid.
3. **Name the multiple-comparisons / forking-paths risk explicitly.** Picking the best of K
   validation variants is an *optimistically biased* estimate. Only the single frozen config's
   **holdout** number is reported as "the result"; the validation-best is labeled as an upper bound.

### D. Add an external baseline, not only our own grid baseline (→ Phase 1)

Comparing the SSM only to our weak 16-bin baseline can make a mediocre detector look good. Run a
**standard off-the-shelf structure algorithm** on the same validation tracks under the same
`mir_eval` harness — e.g. `librosa`/MSAF Foote-novelty or Laplacian segmentation. If our chroma/onset
SSM can't match a stock Foote detector, the problem is our implementation, not the idea. This is the
"is it competitive with the field?" anchor the archived session only gestured at.

### E. Holdout custodian — a concrete multi-agent safeguard (→ Phase 3)

Discipline by intention fails under parallel claims; one agent peeking at holdout poisons it for all.
Make it structural: **prototype/eval scripts must not be able to load holdout track IDs** (holdout
JSON lives outside the path the sweep scripts read; they hard-error if asked for a holdout ID). The
**one** holdout pass is run by a single custodian (Roberto, or a designated coordinator turn) on a
frozen config, and the result is appended to the session log in one shot. No agent runs holdout
inside an iteration loop.

### F. Pre-registration + golden-number regression test (→ Phase 0, Collaboration Process)

The roadmap's "experiment manifest before running" *is* pre-registration — name it that and require
the hypothesis, metric, and pass/fail gate be written **before** the run, so a post-hoc gate can't be
fitted to the result. In Phase 0, commit the canonical numbers as a **golden-number regression test**
(assert the reproduced baseline/refined/grid/human values match the archived table within a tight
tolerance). That makes any agent's environment drift fail loudly instead of silently shifting the
anchors mid-collaboration.

### G. Minor

- **Freeze splits as committed files, not seed-derived.** `holdout_tracks.json` /
  `validation_tracks.json` already exist in the archived session — point Phase 0 at those exact files
  so splits are byte-stable, not regenerated from a seed (deterministic eval doesn't need a seed at
  all; a seed only matters where we resample, e.g. the bootstrap above).
- **State dataset coverage as a standing limitation.** This is a YouTube-sourced SALAMI subset
  (~345 tracks with full analysis), not full SALAMI. Every report says so; no cross-dataset or SoTA
  generalization without full-SALAMI re-run. (Consistent with the archived "not a SoTA challenger
  today" verdict.)

### H. Crop-window alignment AND coverage (→ Phase 0 / Phase 1) — verified, credit Gemini

Gemini flagged that cached onset/chroma times are relative to the analysis window, not the track.
I verified it in `src-tauri/src/dsp.rs:358-368`. Two distinct consequences, both blocking:

1. **Time-shift (Phase 0, fixable).** The window is the *centre 90 s* crop:
   `start = max(0, len/2 − 45 s)`. So the offset is **track-dependent**, not a constant:
   `offset = max(0, duration/2 − 45)` seconds. Predicted boundaries must be shifted by exactly this
   before comparing to JAMS (which are absolute track time). It is recoverable from track duration
   alone — no extra storage — but the eval script must compute it per track. A constant shift would
   be wrong; tracks ≤90 s have offset 0, a 5-minute track has offset 60 s. **Add a unit test that an
   identity prediction on a long track round-trips to the right absolute time.**
2. **Coverage ceiling (Phase 1, structural).** The cached dense features only exist for the central
   ≤90 s. For a 5-minute track that is ~30% of the song, so any GT boundary outside the crop is
   **undetectable** — a hard recall cap the SSM cannot beat no matter how good it is. This breaks the
   "just use already-cached features" assumption for the prototype. Phase 1 must choose, up front:
   - **(a) Windowed eval:** restrict both predictions *and* GT to the central-90 s window, and report
     "central-window F1" — honest, cheap, but not comparable to the full-track archived numbers, so
     it needs its own re-computed baseline/oracle/human ceiling on the same window.
   - **(b) Full-track features:** have the DSP layer emit onset/chroma over the whole track (a real
     pipeline change — moves Phase 1 off the cheap cached-feature path). Required eventually for the
     product anyway, since users segment whole songs.
   Recommend **(a) first** to validate the algorithm cheaply, with (b) gated behind (a) clearing the
   windowed gate. Either way, **never compare crop-window predictions against full-track GT** — that
   is the silent failure Gemini caught.
