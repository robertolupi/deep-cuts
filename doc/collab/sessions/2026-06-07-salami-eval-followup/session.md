# SALAMI Eval Follow-Up: Scientific Roadmap at Collaboration Speed

## Participants

| Handle | Identity |
|---|---|
| **Roberto** | Human, project owner |
| **Codex** | OpenAI Codex coding agent |
| **Claude** | Anthropic Claude |
| **Gemini** | Google Gemini |

## [Roberto, 20:34]

Roberto started a new follow-up from the archived
`doc/collab/sessions/2026-06-07-salami-eval-design/` session.

Task: agree on next steps and draft a roadmap.

Deliverable: an agreed report about improving the SALAMI/structure pipeline in a scientific
manner, with proper experiments, while keeping the high-throughput AI-collaboration loop.

## [Codex, 20:34]

I reviewed the full archived session plus the frozen SSM scoping note and the implemented DSP
caching outcome. Key carry-forward facts:

- Canonical scorer is `mir_eval.segment.detection`, with greedy matching retained only as a
  sanity check.
- The `augment+8peaks_5s` refinement saturated the 16-bin grid: refined/grid is about 97.7%
  at +-3 s on the dual-annotator validation subset.
- The remaining gap is architectural, not post-processing: grid/human is about 47.6% at +-3 s,
  and refined/human is about 46.5%.
- The main scientific lever is raising temporal resolution and measuring whether that raises
  the grid/human ceiling. Bigger sequence models on 16-bin inputs are premature.
- The frozen SSM plan says: do not persist raw SSMs; run chroma-first; add dense CLAP only if
  chroma/onset underperforms.
- DSP caching has already landed: compact onsets in SQLite, 0.2 s chroma series in sidecars,
  no boundary detector yet.

I created `roadmap.md` as a draft report with a staged plan:

1. Freeze the evaluation contract and artifact layout.
2. Build the first offline onset/chroma SSM prototype using already-cached features.
3. Run controlled validation experiments against the 16-bin oracle and human ceiling.
4. Spend holdout only once after the config is frozen, or carve a new held-out slice if the
   existing holdout is no longer clean for the new approach.
5. Only then decide whether to port to Rust, add dense CLAP, or revisit neural label models.

MCP smoke-test step: I am sending this session path and report draft to `claude` and `agy` using
the shared collab MCP. Requested response: amend or ACK the roadmap, with special attention to
experimental leakage, split discipline, metric gates, and collaboration mechanics.

**-> Handoff:**
**Task:** Claude and Gemini: review `roadmap.md`, amend or ACK it, and identify any scientific
or collaboration-process gaps before we call it agreed.
**Context:** `doc/collab/sessions/2026-06-07-salami-eval-design/session.md`,
`doc/collab/sessions/2026-06-07-salami-eval-design/scoping_dense_embeddings_ssm.md`,
`doc/collab/sessions/2026-06-07-salami-eval-design/dsp_caching_implemented.md`, and this
follow-up session.
**Deliverable:** Final agreed roadmap report, or concrete edits needed to reach agreement.

## [Codex, 20:38]

MCP coordination smoke test status:

- Sent direct `handoff` messages from `codex` to `claude` and `agy`.
- Received an `agy -> codex` `handoff` confirming the new follow-up session and asking me to
  review/collaborate on the roadmap deliverable.
- Claimed the open MCP queue task from `agy`:
  `0a522a641dcc4ca6b45e96ba703f583c`.
- No substantive roadmap ACK/counterproposal has arrived yet.
- After waiting, released the queue task back to open status so Claude or Gemini can claim it
  next instead of waiting for Codex's lease to expire.

ACK: the shared MCP mailbox and task queue are both functioning for this session.

**-> Handoff:**
**Task:** Claude and Gemini/agy: review `roadmap.md`, amend or ACK it, and decide whether it is
ready to mark as the agreed report.
**Context:** `roadmap.md`, the archived salami design session, and the MCP task
`0a522a641dcc4ca6b45e96ba703f583c`.
**Deliverable:** Peer ACKs or concrete edits; after that Codex can mark the queue task complete.

## [Gemini, 20:38]

Quoting the handoff I picked up:
> **Task:** Claude and Gemini: review `roadmap.md`, amend or ACK it, and identify any scientific or collaboration-process gaps before we call it agreed.
> **Context:** `doc/collab/sessions/2026-06-07-salami-eval-design/session.md`, `doc/collab/sessions/2026-06-07-salami-eval-design/scoping_dense_embeddings_ssm.md`, `doc/collab/sessions/2026-06-07-salami-eval-design/dsp_caching_implemented.md`, and this follow-up session.
> **Deliverable:** Final agreed roadmap report, or concrete edits needed to reach agreement.

### Review & ACK of `roadmap.md`
I fully **ACK** the proposed `roadmap.md`. It provides the correct staging and rigorous scientific gates required to evaluate the SSM prototype.

### Key Gap Identified: Analysis Crop Window Offset
Checking `doc/collab/sessions/2026-06-07-salami-eval-design/dsp_caching_implemented.md`, there is a critical timing detail we must account for:
* *"Times for both features (onsets and chroma) are seconds from the start of the analysis window (the centre 90 s crop used for key/BPM), not from the start of the track."*
* Human annotations in SALAMI (JAMS files) are relative to the **entire track**.
* **Impact**: When comparing the predicted boundaries from onset/chroma time-series against JAMS annotations, we will have a systematic time-shift mismatch equal to the start offset of the analysis window unless we shift the predictions back into the absolute timeline of the track.
* **Resolution**: In Phase 0, we must ensure the evaluation script reads the analysis window start offset (or computes it from the track duration / pipeline settings) and adds this offset to all predicted boundary timestamps.

### Handoff
I will pass the turn to Claude to review this offset issue, add any necessary modifications to the roadmap, and finalize the agreement.

**→ Handoff:**
**Task:** Claude: Review the crop window offset alignment issue, add any final suggestions or amendments to `roadmap.md`, and proceed to finalize/freeze the roadmap.
**Context:** `doc/collab/sessions/2026-06-07-salami-eval-followup/session.md`, `roadmap.md`, and `dsp_caching_implemented.md`.
**Deliverable:** Final frozen `roadmap.md` and session.md entry showing Claude's final review and ACK.

## [Claude, 20:42]

Quoting the handoff I picked up (from Codex via MCP + Gemini's in-log handoff):
> **Task:** Review `roadmap.md`, amend or ACK it, identify scientific/collaboration-process gaps; review the crop-window offset; finalize.
> **Deliverable:** Final agreed roadmap, or concrete edits.

**ACK Codex's `roadmap.md`** — structure, settled-facts list, and phase gating faithfully carry
the archived design session and the app-first priority. Good work; it's the right spine.

**ACK Gemini's crop-offset catch — and I verified it in code.** `src-tauri/src/dsp.rs:358-368`: the
analysis window is the *centre 90 s* crop (`start = max(0, len/2 − 45 s)`). So Gemini is right there's
a timeline mismatch vs absolute-time JAMS, and I'd sharpen it two ways: (1) the offset is
**track-dependent** — `max(0, duration/2 − 45)`, not a constant (0 for ≤90 s tracks, 60 s for a
5-min track); recoverable from duration, no new storage. (2) Bigger consequence: cached onset/chroma
**only cover the central ≤90 s**, so GT boundaries outside the crop are *undetectable* — a hard recall
cap that breaks the "just reuse cached features" assumption. Phase 1 must pick windowed-eval (cheap,
needs its own re-computed ceilings) vs full-track feature emission (a real pipeline change) up front,
and **never** score crop-window predictions against full-track GT.

I folded **eight additive amendments (A–H)** into `roadmap.md` — all rigor/safeguard increases, no
structural rewrites:
- **A** P/R/F1 triple, not bare F1 (the diagnosis was a *recall* problem; F1 hides it).
- **B** keep pairwise/label metric as a regression guard (model's 81%-of-human strength is grouping;
  adding boundaries can hurt it).
- **C** *(key)* bootstrap 95% CIs + a **paired significance test** for the gate — "beat the oracle"
  means *significantly*, not a higher point estimate; plus name the forking-paths bias (validation-best
  is upper-biased, only frozen-config holdout is "the result").
- **D** add a **stock external baseline** (librosa/MSAF Foote), not only our own weak grid baseline.
- **E** **holdout custodian** — eval scripts structurally cannot load holdout IDs; one custodian runs
  the single holdout pass. Discipline-by-intention fails under parallel agent claims.
- **F** name the manifest as **pre-registration** (gate written before the run) + a committed
  **golden-number regression test** so agent environment drift fails loudly.
- **G** freeze splits as the existing committed JSON files (not seed-derived); state the
  YouTube-subset dataset coverage as a standing limitation.
- **H** the verified crop offset + coverage issue above.

Net: roadmap is **agreed by Codex + Gemini + Claude**. I did not unilaterally freeze it — A–H are new
content Codex/Agy haven't ACKed yet. Once they give a one-line ACK on the amendments (they're additive,
unlikely contested), the report is frozen.

**→ Handoff:**
**Task:** Codex + Agy — one-line ACK (or pushback) on amendments A–H in `roadmap.md`, especially **C**
(significance gate) and **H** (windowed-vs-full-track decision for Phase 1). On ACK, mark the report
FROZEN and complete the open MCP task.
**Context:** `roadmap.md` (Claude review section + updated Status/Recommended Next Action), this
session log, `src-tauri/src/dsp.rs:358-368`.
**Deliverable:** Two ACKs → frozen `roadmap.md`, or concrete counter-edits.

## [Gemini, 20:44]

Quoting the handoff I picked up:
> **Task:** Codex + Agy — one-line ACK (or pushback) on amendments A–H in `roadmap.md`, especially **C** (significance gate) and **H** (windowed-vs-full-track decision for Phase 1). On ACK, mark the report FROZEN and complete the open MCP task.
> **Context:** `roadmap.md` (Claude review section + updated Status/Recommended Next Action), this session log, `src-tauri/src/dsp.rs:358-368`.
> **Deliverable:** Two ACKs → frozen `roadmap.md`, or concrete counter-edits.

I fully **ACK amendments A–H** in `roadmap.md` with the following responses:
* **A–G**: Agreed. The additions of P/R/F1 triples, clustering/label monitors, bootstrap CIs, paired significance Wilcoxon tests, external MSAF/Foote baselines, pre-registration rules, and the split evaluation hygiene are excellent.
* **H (Crop Offset / Coverage)**: I agree with choosing option **(a) Windowed evaluation** first for Phase 1. It allows us to validate the algorithm cheaply within the central-90s crop window using recomputed baseline/oracle/human ceilings, and we will only implement **(b) Full-track features** in the DSP pipeline after Phase 1 successfully clears the gate.

This delivers Gemini's ACK. I will notify Codex and Claude via the mailbox.

**→ Handoff:**
**Task:** Codex: Review peer ACKs. If you also ACK amendments A–H, update the status of `roadmap.md` to FROZEN and mark the open task complete.
**Context:** `roadmap.md`, this session log.
**Deliverable:** Finished and frozen `roadmap.md`, closed task `0a522a641dcc4ca6b45e96ba703f583c`.

## [Codex, 20:42]

Quoting the handoff I picked up:
> **Task:** Codex: Review peer ACKs. If you also ACK amendments A–H, update the status of `roadmap.md` to FROZEN and mark the open task complete.
> **Context:** `roadmap.md`, this session log.
> **Deliverable:** Finished and frozen `roadmap.md`, closed task `0a522a641dcc4ca6b45e96ba703f583c`.

ACK Claude's amendments A-H and Gemini/agy's ACK, including the Phase 1 choice to start with
**windowed evaluation** over the central analysis crop before paying for full-track feature
emission. The additions strengthen the roadmap without changing its spine:

- P/R/F1 and label-regression monitoring prevent a boundary-count-only win from hiding damage.
- Bootstrap CIs, paired significance tests, pre-registration, and golden-number regression tests
  make fast parallel experiments harder to over-interpret.
- External baseline and holdout-custodian rules give the roadmap an honest comparison point and
  structural protection against accidental holdout peeking.
- The crop-offset/coverage correction is essential: Phase 1 must recompute central-window
  baseline/oracle/human ceilings and must never compare crop-window predictions to full-track
  ground truth.

Updated `roadmap.md` status to **FROZEN (2026-06-07)**. The remaining MCP queue task
`0a522a641dcc4ca6b45e96ba703f583c` is complete.

---

## [Meta, 21:02]

The three agents did converge — fast — and they left you with a frozen roadmap, not just chat.

I pulled both files you linked:

- **session.md** shows the live handoffs: Codex drafted the spine, Gemini ACK'd and caught the timing bug, Claude verified it in code and added the statistical safeguards, then all three signed off.
- **roadmap.md** is now marked **FROZEN (2026-06-07)** — agreed by Codex, Gemini/agy, and Claude after amendments A–H.

### What they actually agreed on

**The diagnosis from the last eval:**
- Canonical scorer is `mir_eval.segment.detection` at ±0.5s and ±3.0s
- `augment+8peaks_5s` saturated the 16-bin grid: refined/grid ≈ 97.7% at ±3s
- But the ceiling vs human is low: grid/human ≈ 47.6%, refined/human ≈ 46.5%
- Conclusion: post-processing is done — you need higher temporal resolution, not bigger models on 16-bin inputs

**What’s already in the codebase:**
- DSP caching landed: compact onsets in SQLite, 0.2s chroma series in sidecars, no boundary detector yet

**The critical fix Gemini spotted, Claude proved:**
- Cached features are timed from the *centre 90s crop*, not track start: "seconds from the start of the analysis window (the centre 90 s crop)"
- Claude verified in `src-tauri/src/dsp.rs:358-368`: start = max(0, len/2 − 45s)
- That means two problems: (1) you must shift predictions by `offset = max(0, duration/2 − 45)` per track, and (2) features only cover ≤90s, so any GT boundary outside that window is undetectable — a hard recall cap

### The eight amendments that made it scientific

Claude folded these in, Gemini and Codex both ACK'd:

- **A** — report P/R/F1 triple, not just F1 (the problem was recall, not placement)
- **B** — keep pairwise clustering F1 as regression guard (your 16-bin model's strength is grouping)
- **C** — bootstrap 95% CIs + paired significance test — "beat the oracle" means statistically significant, not +0.5pp
- **D** — add stock external baseline (librosa/MSAF Foote), not just your own grid
- **E** — holdout custodian: eval scripts physically cannot load holdout IDs; one person runs the single holdout pass
- **F** — pre-registration + golden-number regression test so environment drift fails loudly
- **G** — freeze splits as committed JSON files, note this is a YouTube SALAMI subset (~345 tracks)
- **H** — the crop-offset/coverage fix above

### Recommended next action (already in roadmap)

They didn't leave it vague:

1. Build the canonical `mir_eval` script emitting P/R/F1 plus the pairwise metric, with a committed golden-number test, and bake in the per-track crop offset before any prototype run
2. Run the chroma/onset SSM prototype on validation only, against both the 16-bin oracle *and* the external Foote baseline, reporting bootstrap CIs + paired test
3. Decide "windowed eval first" (restrict GT to central 90s, recompute ceilings) vs "full-track features" — they recommend (a) windowed first to validate cheaply

Bottom line: the collaboration loop worked — in ~10 minutes they went from "we're saturated" to a frozen, statistically-rigorous plan that explicitly protects you from the multi-agent failure modes (peeking at holdout, comparing crop predictions to full-track GT, and chasing noise).

---

## [Meta, 21:05]

The three-agent convergence gave you a frozen contract, not just ideas. I pulled all five docs you linked and extracted the exact numbers they agreed to treat as ground truth. Here's the Phase 0 design pattern Meta would hand to Codex/Claude for implementation — architecture first, code second.

### Phase 0 eval script — design skeleton

**Purpose:** one reproducible entry point that reproduces the archived SALAMI numbers *exactly*, then adds the statistical rigor from amendments A–H.

#### 1. Input contract

- Read `validation_tracks.json` and `holdout_tracks.json` as frozen splits (no seed regeneration)
- For each track load:
  - duration from DB (needed for offset)
  - JAMS annotations (absolute track time)
  - model predictions (initially the 16-bin SAX boundaries from `sax_alignment_boundaries`)
  - sidecar `dsp_features.onsets` and `chroma_series` if present

#### 2. Track-dependent crop offset — the bug Gemini caught

Both cached features are timed from the centre 90s analysis window, not track start: "seconds from the start of the analysis window (the centre 90 s crop)"

Claude verified the exact formula in `dsp.rs:358-368`: `start = max(0, len/2 − 45 s)`

Implement as:
```python
offset = max(0.0, duration/2 - 45.0) # seconds
pred_abs = [t + offset for t in pred_crop]
chroma_times_abs = [t + offset for t in chroma_times_crop]
```
Do **not** use a constant 45s — tracks ≤90s have offset 0, a 5-min track has offset 60s. Add a unit test that a synthetic boundary at t=10s in the crop round-trips to 70s absolute on a 300s track.

#### 3. Option A — windowed evaluation (enforced)

Because cached features only cover the central ≤90s, you must never compare crop predictions to full-track GT. Roadmap mandates: "restrict both predictions *and* GT to the central-90 s window"

Implementation:
- window_start = offset
- window_end = offset + 90.0
- gt_windowed = [b for b in gt if window_start <= b <= window_end]
- pred_windowed = [b for b in pred_abs if window_start <= b <= window_end]
- Recompute *all* ceilings on this same window — baseline, grid oracle, human agreement. Do not reuse the full-track 29.36% number.

#### 4. mir_eval core

Use `mir_eval.segment.detection` as canonical scorer. For each track compute:
- P, R, F1 at ±0.5s and ±3.0s (amendment A — report triple, not F1 alone)
- pairwise clustering F1 for label regression guard (amendment B)

Aggregate by mean over tracks, but keep per-track vectors for statistics.

#### 5. Bootstrap 95% CI resampling

Amendment C requires: "Bootstrap 95% CIs over tracks for every headline number (resample tracks with replacement, ≥1000 draws)"

Pattern:
```python
def bootstrap_ci(scores, n_boot=2000):
    boots = [np.mean(resample(scores)) for _ in range(n_boot)]
    return np.percentile(boots, [2.5, 97.5])
```
Report as `33.3% [30.1, 36.5]`, never bare point.

#### 6. Paired Wilcoxon significance gate

"The gate is a *significant* improvement, not a numerically higher one... paired test: per-track paired bootstrap or Wilcoxon signed-rank"

Pattern:
```python
diffs = ssm_f1_per_track - refined_f1_per_track
stat, p = wilcoxon(diffs)
pass_gate = (p < 0.05) and (mean_diff > 0)
```
This prevents a +0.5pp noise win from clearing the ~34% oracle.

#### 7. Golden numbers for regression test

Pulling the exact validation values from the 2026-06-07 design session (mir_eval canonical, dual-annotator N=196):

From the scoping doc table:
- **Baseline (16-bin)**: 21.8% @3s, 3.8% @0.5s
- **Refined augment+8peaks_5s**: 33.3% @3s, 7.6% @0.5s
- **Grid/oracle ceiling**: ~34% @3s, ~6.6% @0.5s
- **Human ceiling**: 71.5% @3s, 64.6% @0.5s

From the original greedy run (for backward compatibility check):
- Baseline: 3.18% @0.5s, 18.04% @3s
- Refined: 6.52% @0.5s, 28.99% @3s
- Holdout refined: 4.65% @0.5s, 29.00% @3s
- Human consensus (greedy): 61.61% @3s, 55.74% @0.5s

Claude's recomputed normalized set (same 196 tracks):
- Baseline 18.11%, Refined 28.87%, Grid 28.29%, Human 61.74% @3s

**Regression test spec (Phase 0 gate):**
- Run script on frozen validation split with windowed GT
- Assert |mir_eval_baseline_3s - 0.218| < 0.005
- Assert |mir_eval_refined_3s - 0.333| < 0.005
- Assert |mir_eval_human_3s - 0.715| < 0.005
- Fail loudly if offset not applied (test will drift by ~30-60s)

This implements amendment F — "golden-number regression test" — so environment drift fails before any experiment runs.

#### 8. Output artifact

Emit one JSON per run with:
- manifest (code hash, DB path, split files, params, seed)
- per-track: duration, offset, window, P/R/F1@0.5, P/R/F1@3, boundary count, human_F1
- aggregates with CIs
- significance test results vs baseline and vs oracle

---

**→ Handoff:**
**Task:** Want me to turn this into the actual `tools/evaluate_salami_phase0.py` function signatures next, or should we first confirm the windowed ceilings need to be recomputed from scratch (since the archived 33.3% was full-track)?

---

## [Gemini, 21:06]

Quoting the handoff I picked up from Meta:
> Want me to turn this into the actual `tools/evaluate_salami_phase0.py` function signatures next, or should we first confirm the windowed ceilings need to be recomputed from scratch (since the archived 33.3% was full-track)?

### Decision: Recomputing Windowed Ceilings and Dual-Mode Execution
Yes, **the windowed ceilings must be recomputed from scratch**. Since the archived results (like 33.3% refined @3s) were scored on the full track without applying crop offsets or filtering predictions, restricting evaluation to the central-90s crop window changes the baseline boundaries. 

To solve this cleanly, we decide that the Phase 0 script should support two execution modes:
1. **Full-Track (Legacy Mode)**: Evaluates predictions without the crop offset or window filtering. This mode will be used to run the golden-number regression tests and verify that our `mir_eval` harness exactly reproduces the archived session's aggregate baselines (e.g. F1 of 21.8% baseline, 33.3% refined).
2. **Windowed (Corrected Mode)**: Applies the track-dependent crop offset and filters both JAMS and predicted boundaries to the central 90s window. This will establish our *new* windowed ceilings (baseline, oracle, human) that will act as the true anchors for Phase 1.

I endorse the design skeleton and ask Meta AI to draft the function signatures for `tools/evaluate_salami_phase0.py` next, incorporating this dual-mode logic.

**→ Handoff:**
**Task:** Meta: Draft the function signatures for `tools/evaluate_salami_phase0.py` incorporating the dual-mode execution (Legacy vs Windowed), offset calculation, window filtering, bootstrap resampling, and Wilcoxon testing.
**Context:** `roadmap.md`, `session.md` (this log), and the design skeleton.
**Deliverable:** Function signatures draft for `tools/evaluate_salami_phase0.py`.


