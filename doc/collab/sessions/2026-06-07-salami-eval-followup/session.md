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

---

## [Meta, 21:07]

Checked the follow-up session — it's frozen after the three ACKs, with the last handoff asking Codex to mark it FROZEN, which he did. The roadmap amendments A–H are now the contract.

Here is the function-signature draft for `tools/evaluate_salami_phase0.py` that implements exactly what you specified — dual-mode, offset, window filtering, bootstrap, Wilcoxon — without writing the full bodies (design-review role).

### `tools/evaluate_salami_phase0.py` — signatures

```python
from pathlib import Path
from typing import Literal, TypedDict
import numpy as np

class BoundaryScores(TypedDict):
    precision: float
    recall: float
    f1: float

def calculate_crop_offset(duration: float, window: float = 90.0) -> float:
    """Return max(0, duration/2 - window/2). Implements the centre-crop logic from dsp.rs."""
   ...

def to_absolute_time(times_crop: list[float], offset: float) -> list[float]:
    """Add offset to every timestamp from sidecar (crop-relative → track-absolute)."""
   ...

def filter_to_window(times_abs: list[float], start: float, end: float) -> list[float]:
    """Keep only boundaries inside [start, end]. Used for Option A windowed mode."""
   ...

def load_track(track_id: str, db_path: Path) -> dict:
    """
    Load duration, JAMS GT, and model predictions.
    Returns {'duration': float, 'gt_abs': [...], 'pred_crop': [...]}
    """
   ...

def score_mireval(pred_abs: list[float], gt_abs: list[float], tolerances: tuple[float,float]=(0.5,3.0)) -> dict[str, BoundaryScores]:
    """Run mir_eval.segment.detection for ±0.5s and ±3.0s. Returns {'0.5': {...}, '3.0': {...}}"""
   ...

def bootstrap_ci(scores: np.ndarray, n_resamples: int = 2000, alpha: float = 0.05) -> tuple[float,float,float]:
    """Return (mean, lower, upper) 95% CI via resampling tracks with replacement."""
   ...

def paired_wilcoxon(a: np.ndarray, b: np.ndarray) -> dict:
    """Wilcoxon signed-rank on per-track paired differences. Returns {'stat':..., 'p':..., 'mean_diff':...}"""
   ...

def evaluate_split(
    track_ids: list[str],
    db_path: Path,
    mode: Literal['legacy','windowed'] = 'windowed',
    window: float = 90.0
) -> dict:
    """
    Dual-mode execution:
    - legacy: score full-track pred vs full-track GT (for reproducing archived 21.8%/33.3%)
    - windowed: apply offset, then filter both pred and GT to [offset, offset+window]
    Returns per-track scores and aggregates.
    """
   ...

def run_phase0(
    split_json: Path,
    db_path: Path,
    mode: Literal['legacy','windowed'] = 'windowed',
    n_bootstrap: int = 2000
) -> dict:
    """
    Phase 0 entry point. Implements roadmap Phase 0 contract:
    - fixed splits, no leakage
    - P/R/F1 triple at both tolerances
    - bootstrap CIs
    - Wilcoxon vs baseline and vs oracle
    - golden-number regression check
    """
   ...
```

This keeps the implementation narrow — Codex/Claude fill the bodies, you keep the statistical contract intact, and the legacy mode gives you the regression test against the exact numbers from the design session.

---

## [Gemini, 21:08]

Quoting the handoff I picked up from Meta:
> **Task:** Codex/Claude: Fill the bodies of these function signatures, keeping the statistical contract intact.

I ACK Meta's function signatures for `tools/evaluate_salami_phase0.py`.
I have created the skeleton file at [evaluate_salami_phase0.py](file:///Users/rlupi/src/deep-cuts/tools/evaluate_salami_phase0.py) containing these signatures and design contracts.

I will hand off to Codex and Claude to claim this task and implement the full function bodies.

**→ Handoff:**
**Task:** Codex / Claude: Claim the task and implement the function bodies in `tools/evaluate_salami_phase0.py` according to the design contract.
**Context:** `tools/evaluate_salami_phase0.py`, `roadmap.md`, `session.md` (this log).
**Deliverable:** Fully implemented and verified `tools/evaluate_salami_phase0.py` that runs all golden-number regression tests successfully in Legacy mode.

## [Roberto, 21:10]

(Scribed by Claude.) Decision on the deliverable and the Phase-0 follow-on:

1. **The roadmap is the deliverable, and it is done.** `roadmap.md` is FROZEN and agreed by Codex,
   Gemini/Agy, and Claude (amendments A–H), with Meta's design review. Roberto's stated task —
   "agree on next steps and draft a roadmap" — is satisfied.
2. **Phase-0 implementation: Codex implements; Claude reviews/verifies.** The open task to fill
   the bodies of `tools/evaluate_salami_phase0.py` goes to **Codex**. Claude stays in
   **review/verify** role (checks the implementation, golden-number regression, and statistical
   contract A–H rather than writing the bodies). App-first priority: this research-track work is
   opportunistic, not a commitment to ship.

## [Claude, 21:11]

Per Roberto's decision above, I am **not** claiming the Phase-0 task — routing it to Codex and
moving to review/verify. When Codex's `tools/evaluate_salami_phase0.py` is implemented, I will
verify against the frozen contract, specifically:

- **Legacy mode reproduces the archived numbers** within the golden-number tolerances
  (baseline 21.8% / refined 33.3% / human 71.5% @3 s, ±0.5%) — and **fails loudly without the
  per-track crop offset** (amendment F, H).
- **Windowed mode never compares crop predictions to full-track GT**, and recomputes its *own*
  baseline/oracle/human ceilings on the central-90 s window (amendment H) — the archived full-track
  33.3% is *not* a valid windowed anchor.
- **P/R/F1 triple** at both tolerances (A), **pairwise/label metric** present as a regression guard
  (B), **bootstrap CIs + paired Wilcoxon** wired into the gate (C), and **holdout IDs not loadable**
  by the eval path (E).

**→ Handoff:**
**Task:** Codex — claim the open task and implement the bodies of `tools/evaluate_salami_phase0.py`
to the frozen contract; ping Claude when it runs Legacy-mode golden-number tests green.
**Context:** `tools/evaluate_salami_phase0.py` (skeleton), `roadmap.md` (FROZEN, amendments A–H),
this log. Validation/holdout splits are the committed `*_tracks.json` in the archived design session.
**Deliverable:** Implemented `tools/evaluate_salami_phase0.py` passing Legacy golden-number tests →
Claude verifies before any windowed/Phase-1 numbers are trusted.

## [Codex, 21:18]

Quoting the handoff I picked up from Claude:
> **Task:** Codex — claim the open task and implement the bodies of `tools/evaluate_salami_phase0.py`
> to the frozen contract; ping Claude when it runs Legacy-mode golden-number tests green.
> **Deliverable:** Implemented `tools/evaluate_salami_phase0.py` passing Legacy golden-number tests →
> Claude verifies before any windowed/Phase-1 numbers are trusted.

Claimed MCP task `627f20f4086249ecadaa681a910554a3` as `codex` and implemented
`tools/evaluate_salami_phase0.py`.

What is implemented:

- Dual execution modes:
  - `legacy`: reproduces archived full-track anchors.
  - `windowed`: applies central-crop semantics and recomputes its own central-window anchors.
- Track-dependent crop offset helper: `max(0, duration / 2 - window / 2)`.
- Boundary scoring through `mir_eval.segment.detection`, reporting P/R/F1 at +-0.5 s and +-3.0 s.
- Aligned dual-annotator aggregate reporting for baseline/refined/oracle/human comparisons.
- Pairwise label F1 regression guard.
- Bootstrap confidence intervals over tracks.
- Paired Wilcoxon tests for refined-vs-baseline and refined-vs-oracle.
- Structural holdout guard: normal CLI use rejects `holdout_tracks.json` unless `--allow-holdout`
  is explicitly passed for a custodian run.
- JSON artifact output via `--json-out`.

Verification run:

```text
tools/.venv/bin/python tools/evaluate_salami_phase0.py --mode legacy --golden --n-bootstrap 100
```

Result: **golden-number regression PASSED** on the aligned dual-annotator subset (N=196):

- Baseline F1@3s: 21.82%
- Refined F1@3s: 33.26%
- Human F1@3s: 71.53%

Also ran:

```text
tools/.venv/bin/python tools/evaluate_salami_phase0.py --mode windowed --n-bootstrap 100
```

Result: windowed mode completed and produced separate central-window anchors (not comparable to
the archived full-track 33.3% anchor), with aligned dual-annotator subset N=194.

Holdout guard check:

```text
tools/.venv/bin/python tools/evaluate_salami_phase0.py --split-json doc/collab/sessions/2026-06-07-salami-eval-design/holdout_tracks.json --mode legacy --n-bootstrap 1
```

Result: exits with `error: holdout split is protected; pass --allow-holdout only for the custodian's frozen run`.

Note from Roberto during implementation: re-analysis is currently running, so current DB-backed
windowed numbers are smoke-test outputs, not authoritative research results.

**→ Handoff:**
**Task:** Claude — review/verify `tools/evaluate_salami_phase0.py` against your checklist and the
frozen A-H roadmap contract.
**Context:** `tools/evaluate_salami_phase0.py`, this session entry, and the commands/results above.
**Deliverable:** Verification ACK or concrete fixes before the Phase 0 task is marked complete.

## [Claude, 21:24] — VERIFICATION ACK

Quoting the handoff I picked up:
> **Task:** Claude — review/verify `tools/evaluate_salami_phase0.py` against your checklist and the frozen A-H roadmap contract.
> **Deliverable:** Verification ACK or concrete fixes before the Phase 0 task is marked complete.

I read the full implementation and **re-ran all three commands myself** (not just trusting Codex's
report). **VERIFIED — Phase 0 is sound. ACK to mark the task complete.**

### Independently reproduced

- **Legacy `--golden`: PASSED.** baseline 21.82%, refined 33.26%, oracle 34.06%, human 71.53% @3 s;
  refined 7.61% / baseline 3.76% @0.5 s; pairwise label F1 46.13%. Every value lands on the archived
  canonical table within ±0.5 pp. N=229 (aligned dual-annotator subset N=196). ✅
- **DB path** is the canonical `query-db` location, read-only. No CLAUDE.md violation.
- **Holdout guard:** confirmed real exit code **2** with `error: holdout split is protected` (Codex's
  pipe-masked exit was a measurement artifact; the guard genuinely hard-errors). I did **not** run
  `--allow-holdout` — that single frozen pass is the custodian's, not a review run. ✅

### Contract coverage (A–H)

| | Status |
|---|---|
| **A** P/R/F1 triple, both tolerances | ✅ reported everywhere |
| **B** pairwise/label regression guard | ✅ `pairwise_label_f1` aggregated with CI |
| **C** bootstrap CIs + paired Wilcoxon gate | ✅ both wired; `refined_vs_oracle` + `refined_vs_baseline` |
| **D** external Foote/MSAF baseline | ⏳ **Phase 1 scope, not Phase 0** — correctly deferred (flag, not a defect) |
| **E** holdout custodian / IDs unloadable | ✅ `_reject_holdout` hard-errors by default |
| **F** pre-registration + golden-number test | ✅ `_assert_golden_numbers` + `--golden` |
| **G** committed split files | ✅ points at the archived `*_tracks.json` |
| **H** crop offset + windowed anchors | ✅ track-dependent offset; windowed recomputes its own ceilings |

### Two findings worth recording (not blockers)

1. **The significance gate already earns its keep — it confirms saturation statistically.**
   Legacy `refined vs oracle @3 s`: **p=0.989, mean_diff −0.008** → refined does **not**
   significantly beat the grid oracle. This is amendment C turning the archived prose ("post-processing
   saturated") into a *tested* claim. Good.

2. **Windowed mode quantifies the resolution headroom — and a caveat to interpret it honestly.**
   Windowed oracle jumps to **64.68% @3 s** (legacy 34.06%), near the windowed human ceiling 69.64%,
   and `refined vs windowed-oracle` is now **p=1.2e-23, mean_diff −0.307** (refined far below).
   *Why:* the windowed oracle projects GT to 16 bins over a ~90 s span instead of over a full ~5 min
   track, so its effective bin size is much finer — that resolution boost is the whole point. **Caveat
   for Phase 1:** this 64.68% is a fair target *only* for a detector that actually operates at
   window/fine resolution (the dense 0.2 s SSM is exactly that), and windowed numbers must **never** be
   compared to legacy 34%. The refined→windowed-oracle gap (34%→65%) **is** the resolution headroom the
   roadmap predicted — now measured.

Caveat noted (Roberto): re-analysis is running, so DB-backed numbers are smoke outputs. The `--golden`
gate is the guardrail — re-run it before trusting any post-re-analysis number; if it ever fails, the
DB state drifted from the archived anchors.

### Optional polish (non-blocking, for whoever does Phase 1)

- Add the **amendment D external baseline** (librosa/MSAF Foote) as a variant when Phase 1 starts.
- Consider asserting golden at **±0.5 s** too (currently only @3 s is in `GOLDEN_LEGACY_DUAL_ANNOTATOR`).
- A tiny **unit test** for `calculate_crop_offset` round-trip (t=10 s in crop → 70 s abs on a 300 s
  track) would lock amendment H against regressions, per Meta's skeleton note.

**→ Handoff:**
**Task:** Codex — you're clear to mark task `627f20f4086249ecadaa681a910554a3` **complete**. Phase 0
is verified. The optional polish items above are Phase-1 starters, not blockers. No further action
needed from me unless Roberto re-prioritizes the research track.
**Context:** `tools/evaluate_salami_phase0.py` (verified), `roadmap.md` (FROZEN), this log.
**Deliverable:** Closed Phase-0 task; roadmap + verified eval harness stand as the session's output.

## [Roberto, 21:21]

Roberto asked whether Meta's participation was worth the coordination overhead, because including
Meta requires pushing context to a public GitHub repo and slows the loop down to human relay speed.

Process decision:

- Meta was useful in this session for **milestone design review**: shaping the evaluation contract,
  turning the frozen roadmap into concrete Phase 0 requirements, and drafting function signatures.
- Meta should **not** be in every implementation/debugging loop. The public-repo relay is too
  cumbersome when agents with local repo/runtime access can iterate faster.
- Recommended pattern going forward: use Codex + Claude/Gemini for normal execution and verification;
  invite Meta only for higher-level scientific framing, roadmap review, architecture critique, or
  "are we fooling ourselves?" checks before committing to a research direction.

## [Codex, 21:25] — Lay Summary of Phase 0 Results

Roberto asked for an ELI5 explanation of the Phase 0 results. Summary:

Think of the app trying to mark "this is where the song changes section."

There are two different games now:

### 1. Old full-song game

The old system divides the whole song into only **16 big chunks**. For a 5-minute song, each
chunk is around 19 seconds. That means the app can only place section changes on a very coarse
grid.

Results:

- Basic 16-chunk model: about **22%**
- Refined model with extra novelty peaks: about **33%**
- Best possible 16-chunk grid: about **34%**
- Human-vs-human agreement: about **72%**

ELI5: the refined model is basically squeezing everything possible out of the old chunky ruler.
It gets almost all the score that a 16-chunk ruler can get, but humans are using a much finer
ruler.

So the old approach is not "badly tuned"; it is **out of resolution**.

### 2. New windowed game

The audio features we currently cache only cover the **middle 90 seconds** of each song, not the
whole song. So we must score only that middle window, otherwise we are asking the model to find
boundaries in parts of the song it literally cannot see.

When we score fairly inside that 90-second window:

- Refined old model: about **34%**
- Windowed grid/oracle ceiling: about **65%**
- Human agreement: about **70%**

ELI5: in the middle 90 seconds, the "best possible" score is much higher because the ruler is
effectively finer. That shows there is real headroom if we build the dense chroma/onset SSM
detector.

Big takeaway:

- The old 16-bin full-song method is maxed out.
- The next improvement is not smarter post-processing.
- The next improvement is **higher-resolution boundary detection**, starting with the central
  90-second window.
- The script now protects us from fooling ourselves: it checks old known numbers, blocks
  accidental holdout use, reports precision/recall/F1, and uses statistical tests.

Caveat: Roberto said re-analysis is running, so current DB-backed numbers are smoke-test results.
The important part is that the evaluation harness now reproduces the archived golden numbers and
separates old full-track scoring from new windowed scoring.

## [Roberto, 21:26]

Correction/clarification on the "re-analysis is running" caveat: the data was not merely
mid-refresh. At the time Codex was ready to interpret the results, several relevant passes were
still missing or incomplete:

- Essentia
- structure clustering
- BPM refinement
- Qwen audio LLM
- description embedder

So current DB-backed Phase 0/windowed numbers should be treated as harness smoke tests until those
passes have finished and the golden checks are rerun against the final analysis state.

## [Claude, 21:24] — Plain-language summary (ELI5, scribed at Roberto's request)

Roberto asked for a layman explanation of the results. Recording it here so the session has a
non-jargon version of what we found.

**What the app is trying to do.** Automatically chop a song into its sections — intro / verse /
chorus / bridge / outro — and mark *when* each starts. Like auto-generating chapter markers for a song.

**How we score it.** The SALAMI dataset has section boundaries that real humans marked by hand. We
compare where the app puts a boundary vs where humans put one. A hit = the app's marker lands within
a few seconds of a human's. We use two difficulty levels: **±3 s** ("close enough") and **±0.5 s**
("nearly perfect").

**The key @3s numbers:**
- Two **humans** vs each other: **~71%** — even trained people only agree 71% of the time, because
  "where does the chorus *really* start?" is genuinely fuzzy. So ~71%, not 100%, is the realistic best.
- The **app today**: **~33%** — roughly halfway to as-good-as-a-human. Not broken, but room to grow.
- **Best the current design could ever do**: **~34%** — the app is already basically at its own ceiling.

**Three takeaways:**
1. **33% isn't as bad as it sounds** — the goalpost is ~71% (human agreement), not 100%.
2. **We've squeezed out everything the current design can give, and now we can *prove* it.** The
   significance test showed the clever post-processing tricks make no *real* difference vs the
   design's ceiling — it's luck, not signal. So polishing this approach further is wasted effort; we
   need a fundamentally different method.
3. **We found *why* it's stuck, and it's fixable.** The current method uses a **coarse ruler** — it
   can only place boundaries at 16 evenly-spaced slots (like folding the song into 16 equal pieces).
   Real choruses rarely start exactly on a fold. When we re-ran the test with a **finer ruler** (the
   windowed mode), the achievable ceiling jumped **34% → 65% @3s**, almost up to the human ~70%.

**Headline:** the app's "brain" for grouping sections is actually decent — what holds it back is
**measurement precision, not intelligence.** Don't build a bigger AI model; instead detect boundaries
at fine time-resolution (the dense-SSM prototype). The numbers now tell us that's the lever worth
pulling, and roughly how much it could gain.

**Honest caveats:** measured on a YouTube-sourced SALAMI subset; the app is mid-re-analysis; and the
finer-ruler **65% is a target to aim for, not a result achieved**. It's the prize, not the trophy.

## [Roberto, 21:29]

Two observations that reframe the bottleneck and the path forward (scribed by Claude, with Claude's
analysis recorded under it):

1. **The real gate is the *arbitrary resolution I set for `waveform_data`* (128 points), not only the
   16-bin SAX grid.** There are two stacked resolution limits.
2. **`128 → 16` looks like the layer widths of a neural network.** Two questions: (a) is this pipeline
   really a neural network by other (less efficient) means? (b) can we do meta-parameter optimization
   to find the sweet spot?

### Claude's response — verified bottleneck

Confirmed in code. Two stacked, hand-set resolution gates:
- **Gate 1 — 16-bin SAX grid:** baseline boundaries are quantized to `duration/16` (~11–19 s/bin).
- **Gate 2 — 128-pt `waveform_data`:** `dsp.rs:354` `downsample_profile(&rms_energies, 128)`. The
  refinement's novelty peaks come from this 128-pt envelope (`refine_salami_boundaries.py:103`
  `ranked_novelty_peaks`), so refined boundaries can't beat `duration/128` ≈ **1.4 s (3 min) / 1.9 s
  (4 min) / 2.3 s (5 min)**. This is *exactly* why ±0.5 s is pinned at ~7% — you can't hit ±0.5 s on a
  ~2 s candidate grid. The **65% windowed oracle is NOT gated by `waveform_data`** (it's a GT ceiling),
  so the prize stands; only the current detector is stuck on the 128-pt grid.

Already-cached features blow past both gates: **onsets ~23 ms** (~100× finer), **chroma 0.2 s** (~10×).

### Claude's response — is it a neural network?

Right intuition, precise name. A fixed cascade of *filter → nonlinearity → pool* at multiple scales is
a **scattering transform** (Mallat/Andén) — provably "a CNN with untrained, hand-designed filters,"
and a published audio technique. So structurally it IS a conv-net's early layers, built by hand. What
makes it *not* a neural network is the defining property: **nothing is learned** — 128/16/RMS/FFT/cosine
are all fixed and task-agnostic. (Nuance: 128 and 16 are partly *parallel* representations merged at the
end, not a clean feedforward stack — the "two layers" is looser than it looks.)

- **Less efficient?** Two opposite answers. *Task*-efficiency: yes, hand-designed averages aren't tuned
  to boundary detection. *Data*-efficiency: no — with ~200–345 tracks a trained end-to-end net would
  memorize (Meta's over-parameterization warning). The fixed pipeline is the data-poor-regime choice,
  on purpose.

### Claude's response — HPO sweet spot, with the trap

Yes, but: **the hyperparameters ARE the parameters.** Sweeping `128`/`16`/kernel-width/peak-count/
fusion-weights and picking the best is gradient-free optimization of a fixed architecture = *training
the model by hand with a bad optimizer.* Consequences:
- **Forking-paths in resolution-space (amendment C).** Every knob swept on validation is a DOF fitted
  to validation; sweep many and the holdout collapses. HPO must be pre-registered, validation/dev only,
  **report the number of configs tried**, one holdout pass at the end.
- **It exposes the real decision.** Gradient descent beats grid/Bayesian search per-parameter, so the
  moment many knobs need sweeping, that's the signal you've hit the data ceiling and should switch to a
  *tiny trained* model (scattering features → small head). HPO over a fixed pipeline is the substitute
  for training in the data-poor regime; it stops paying off exactly when you have enough data to train.
- **Principled first sweep = the bottleneck's own axis:** novelty-source resolution
  (`128` → `0.2 s` chroma → `23 ms` onset + intermediate downsamplings) × SAX bin count. Two axes, not a
  6-knob free-for-all. The windowed-oracle result (16-bins-over-90 s = 65% vs over-full-track = 34%) is
  already one point on that curve — we're extending a curve, not starting blind.

### Proposed Phase-1a (cheapest experiment, smaller than the full SSM)

**Swap the novelty source.** Feed `ranked_novelty_peaks` the cached 23 ms onset envelope / 0.2 s chroma
instead of the 128-pt `waveform_data`; re-run on validation; measure ±0.5 s and ±3 s vs the recomputed
windowed ceilings. Isolates Roberto's hypothesis with almost no new code. **Caveat:** finer resolution
removes the *ceiling* but only pays off if novelty is *informative* at that scale — raw energy at 23 ms
is jittery, which is why the SSM uses chroma self-similarity (harmonic change) rather than energy. So
Phase-1a tests "does removing the 128-pt cap help at all," and the SSM (Phase 1) is what turns "can place
precisely" into "places correctly."

**→ Handoff:**
**Task:** When the research track is prioritized, run **Phase-1a** (swap novelty source: 128-pt
waveform → cached onsets/chroma) on validation before the full SSM, and frame any resolution/bin sweep
as pre-registered HPO (report #configs, validation/dev only, one holdout pass). Fold these as a Phase-1
amendment when `roadmap.md` is unfrozen. Note: Roberto's [21:26] caveat — several passes (Essentia,
structure clustering, BPM refinement, Qwen, description embedder) were incomplete, so rerun the golden
checks once the analysis state is final before trusting any Phase-1a numbers.
**Context:** `src-tauri/src/dsp.rs:354` (128-pt downsample), `tools/refine_salami_boundaries.py:103`
(`ranked_novelty_peaks`), `tools/evaluate_salami_phase0.py` (windowed ceilings), cached `onsets` +
sidecar `chroma_series` from `dsp_caching_implemented.md`. App-first: parked/opportunistic.
**Deliverable:** Phase-1a validation result (does escaping the 128-pt cap lift ±0.5 s?) + a pre-registered
resolution×bin HPO plan, when prioritized.

## [Claude, 21:34] — Brainstorm kickoff: "untrained-CNN" angle (with Codex + Agy)

At Roberto's request, opening a brainstorm with Codex and Agy on the neural-network framing — including
his example that **PyTorch / `ort` (ONNX Runtime, already shipped for CLAP) would be more efficient than
our handcrafted code** at some of this. Seed sent to both via the collab MCP (`type: brainstorm`).

**Framing.** The pipeline (`128`-pt waveform envelope → `16`-bin SAX, + cosine SSM + checkerboard novelty)
is structurally a **scattering transform = an untrained, hand-designed CNN front-end**. So the question is
sharper than "use a NN?": *how much hand-DSP should become tensor/learned ops, and where does it pay off
vs over-engineer?*

**Threads on the table:**
1. **Efficiency** — SSM = `L2norm(X) @ L2norm(X).T`; novelty = a 1D conv. Tensor one-liners (torch
   prototype / ONNX+`ort` deploy), BLAS-fast vs hand Rust loops. Or is per-track compute so small that
   `ort` overhead loses and the real hotspot is CLAP forward passes? *Where is the hotspot?*
2. **Data-efficient sweet spot** — keep the front-end fixed (no data to train it), replace the hand-tuned
   readout (kernel width / threshold / fusion / 8-peaks-5s) with a **tiny differentiable head** trained
   with sample weights = human agreement. "HPO done right" = gradient descent over a few params.
3. **Differentiable HPO** — learnable kernel width/fusion + soft-NMS peak-picking → end-to-end
   differentiable boundary loss ("the hyperparameters ARE parameters"). Fruitful or rabbit-hole?
4. **Buy vs build** — pretrained ONNX structure models (All-in-One, beat-this, MSAF refs) we could just
   *run* as the external baseline (amendment D), rather than train anything.

**Guardrails:** tiny data (~200–345 tracks) → CV + holdout-custodian + must beat fixed-features+Foote;
app-first → prototype/train in PyTorch, export ONNX, run via `ort` in Rust; don't conflate "tensors for
speed" (low-risk, do now) with "train a model" (data-limited, gated).

**Asks:** Codex = runtime/impl lens (where ort/torch beats hand Rust; minimal SSM-as-tensor prototype;
droppable pretrained ONNX model). Agy/Gemini = modeling lens (is fixed-features+tiny-head the right
data-efficient target; safe head + training protocol on this N). Both: kill bad ideas fast.

**→ Handoff:**
**Task:** Codex + Agy — respond to the brainstorm seed (sent via collab MCP, `type: brainstorm`) with
your lens; flag what's fruitful vs over-engineering. Roberto is relaying the seed manually too.
**Context:** this turn, the [Roberto, 21:29] analysis above, `scoping_dense_embeddings_ssm.md`.
**Deliverable:** Each peer's take on the four threads → a short synthesis of which (if any) are worth a
prototype, kept consistent with app-first + the frozen roadmap's experimental discipline.

## [Codex, 21:37] — Brainstorm Response and Phase 0 Task Closure

First, closing the previous implementation loop: Claude verified `tools/evaluate_salami_phase0.py`
and cleared MCP task `627f20f4086249ecadaa681a910554a3`. I marked the task complete through the
collab MCP.

Codex runtime/implementation take on the "untrained CNN / scattering transform" brainstorm:

### 1. Where tensors pay off

For the dense SSM prototype, express the math as tensor operations in Python first:

- `X = normalize(features)`
- `SSM = X @ X.T`
- novelty = diagonal scan / checkerboard correlation, expressible as convolution-like tensor ops

That is the right prototype path because it makes the algorithm short, inspectable, and easy to
compare across feature choices. It also avoids premature Rust complexity.

Deployment via ONNX Runtime is **not automatically a win**. For a 90-second chroma window at 0.2 s
steps, `N` is only about 450 frames, so an SSM is about 200k similarities. Rust `ndarray` or BLAS may
already be plenty. `ort` setup/copy overhead could dominate unless the model also includes a larger
learned head or dense CLAP. The real likely hotspot is dense CLAP forward passes, not SSM matrix math.

Recommendation: prototype in PyTorch/NumPy; profile; deploy the smallest proven path. Do not choose
`ort` just because the math resembles a neural net.

### 2. Minimal prototype worth building

The smallest useful prototype is **not** a trained model. It is:

1. central-window chroma/onset feature loader,
2. tensor SSM + novelty curve,
3. fixed Foote/checkerboard peak picker,
4. evaluation through `evaluate_salami_phase0.py` windowed mode,
5. comparison against the 16-bin refined baseline, the windowed oracle, and an external Foote/MSAF
   baseline.

This proves whether the SSM-as-tensor path has signal before any training.

### 3. Tiny trained head: maybe, but only after fixed SSM baseline

Fixed features + tiny trained readout is the data-efficient sweet spot **if** the fixed SSM has
signal but threshold/kernel/fusion tuning becomes the bottleneck.

Safe parameter budget on ~200-345 tracks:

- fusion weights over onset/chroma/(maybe CLAP),
- one or two kernel/smoothing parameters,
- threshold or top-k/boundary-count prior,
- optional tiny logistic/MLP head over local novelty features.

Keep it under tens of parameters, use grouped CV, report configs tried, and weight tracks by human
agreement. Anything end-to-end over dense embeddings is likely overfit theater at this N.

### 4. Differentiable HPO: mostly rabbit hole for now

Soft-NMS / soft-argmax / differentiable peak picking is elegant, but likely too much machinery before
we prove fixed SSM beats the external baseline. Use Optuna/grid search over a few pre-registered knobs
first. Switch to differentiable HPO only if:

- the fixed SSM clearly has signal,
- the same few hyperparameters keep dominating,
- and validation variance is under control.

### 5. Buy vs build

For amendment D, first run an off-the-shelf baseline rather than hunt for a deployable ONNX model:

- MSAF/Foote-style novelty segmentation if available in the Python environment,
- otherwise a small local Foote implementation over chroma.

Pretrained ONNX structure-boundary models may exist, but they are a research scan item, not a blocker.
Even if found, they must run under the same Phase 0 harness and beat fixed-features+Foote before they
matter.

### Codex recommendation

Fruitful:

1. Phase-1a: swap novelty source from 128-pt waveform to cached onset/chroma.
2. Phase-1b: tensor SSM prototype in Python.
3. Add external Foote/MSAF baseline.
4. Profile before deciding Rust `ndarray` vs ONNX Runtime.

Likely over-engineering now:

- ONNX deployment of the SSM before profiling,
- differentiable peak picking before fixed SSM works,
- any medium/large trained model on this dataset size,
- end-to-end training over CLAP/chroma without a much larger dataset.

Per Roberto's decision, agy/Gemini should do the final synthesis after peer responses.

**→ Handoff:**
**Task:** Agy/Gemini — synthesize the brainstorm responses into a short recommendation: what to
prototype, what to defer, and what to kill.
**Context:** Claude's brainstorm seed, Codex's runtime take above, and any Gemini/modeling response.
**Deliverable:** Final synthesis for Roberto; keep it aligned with app-first priority and the frozen
Phase 0/roadmap guardrails.

## [Roberto, 21:37]

(Scribed by Claude — logged late; my live entry kept racing peer writes and the session closed first.
Recording Roberto's steering act per `bot-collab`/`PROTOCOL.md`, since it drove this final phase.)

Roberto's directive: **if NNs are too heavy, consider other optimization methods** — e.g. ant colony,
amoeba (Nelder-Mead); "make a list then brainstorm"; and **Agy owns the final synthesis.**

Claude's contribution (relayed to peers via collab MCP, credited in the Codex 21:38 and Gemini 21:39
turns below): the **key reframe** that mir_eval F1 is non-differentiable over only ~2–8 knobs → a
**black-box / derivative-free** regime (no gradients/NN/big-data), split into **P1 hyperparameter
tuning** (Nelder-Mead / Bayesian-opt / CMA-ES / random-search baseline) vs **P2 boundary selection**
(combinatorial → Viterbi/DP, with ACO redirected there, not to continuous HPO). Full list lives in the
peer responses that follow; not duplicated here.

## [Codex, 21:38] — Optimizer Add-On

Claude asked for the runtime/implementation take on derivative-free optimizers (`amoeba`, Bayesian
optimization, CMA-ES, ant colony, nevergrad, random search), with agy owning final synthesis.

Installed-package check:

- `scipy` is installed.
- `optuna` is not installed.
- `nevergrad` is not installed.

Recommendation for the first optimizer bake-off: **use SciPy + random search**, not a new optimizer
dependency yet.

Why:

- The first useful tuning space should be tiny: novelty source, smoothing/kernel width, threshold or
  top-k, min boundary spacing, and maybe one fusion weight. That is about 2-6 knobs.
- `mir_eval` F1 is noisy and non-differentiable, so gradients are not useful.
- Each eval is cheap enough once features are cached, but validation-set reuse is the real risk. The
  optimizer must be pre-registered and the number of trials reported.
- SciPy gives `optimize.differential_evolution` and `optimize.minimize(method="Nelder-Mead")` without
  adding dependencies. Random search is the required baseline that any fancy optimizer must beat.

Suggested first bake-off:

1. Random search: 50 trials.
2. SciPy differential evolution: about 60-100 function evaluations.
3. Optional Nelder-Mead local polish from the best random/DE point: <=30 evaluations.

Budget: keep the whole bake-off under about **150 validation evaluations**. If 150 evals are not enough
to find a stable win over the fixed baseline, the search surface is probably too noisy or the features
are not informative enough. Do not keep increasing trials until validation improves.

When to add Optuna:

- Add Optuna only if we need conditional/categorical search spaces and want study persistence/pruning.
- It is a good later choice, but for the first pass the dependency cost and extra machinery are not
  needed.

Ant colony:

- Not worth it for continuous HPO.
- For boundary selection, formulate it as a dynamic-programming/Viterbi path problem instead. If the
  objective decomposes over candidate boundaries with transition/duration costs, DP/Viterbi is exact
  and easier to test. Ant colony is heuristic machinery where we likely have an exact structured
  solution.

Codex synthesis input:

- Prototype fixed SSM first.
- Add random search + SciPy DE as the first optimizer layer.
- Keep trial counts small and pre-registered.
- Use DP/Viterbi for combinatorial boundary selection, not ant colony.
- Defer Optuna/nevergrad/differentiable HPO until a fixed-feature baseline has proven signal.

**→ Handoff:**
**Task:** Agy/Gemini — include this optimizer recommendation in the final brainstorm synthesis.
**Context:** Claude's optimizer add-on, Codex package check, and the Phase 0 guardrails.
**Deliverable:** Final synthesis: optimizer path should be random-search baseline + SciPy first;
Optuna later if needed; DP/Viterbi instead of ant colony for boundary selection.

---

## [Gemini, 21:39] — Final Brainstorm Synthesis: Untrained CNN & Optimization

Quoting the handoff I picked up:
> **Task:** Agy/Gemini — synthesize the brainstorm responses into a short recommendation: what to prototype, what to defer, and what to kill.
> **Deliverable:** Final synthesis for Roberto; keep it aligned with app-first priority and the frozen Phase 0/roadmap guardrails.

Having reviewed the contributions from Claude (categorization of HPO vs Boundary Selection) and Codex (package check and runtime constraints), here is the finalized synthesis and recommendation.

### 1. What to PROTOTYPE (Immediate Phase 1 Scope)

* **Phase 1a (Novelty Source Swap)**: Run the cheapest resolution check by swapping the novelty source in `refine_salami_boundaries.py` from the 128-pt `waveform_data` to the cached 23 ms onsets / 0.2 s chroma. This isolates the resolution ceiling hypothesis with minimal code changes.
* **Phase 1b (Tensor SSM in Python)**: Build the PyTorch/NumPy-based offline SSM prototype. Python tensor operations are the correct environment to verify the Foote checkerboard kernel algorithm before writing any Rust code.
* **External Foote Baseline (Amendment D)**: Implement or run a standard MSAF/Librosa Foote baseline to serve as our anchor comparison, ensuring our custom SSM beats a stock off-the-shelf detector.
* **SciPy + Random Search for P1 (Hyperparameter Optimization)**: 
  * Codex verified that `optuna` and `nevergrad` are **not** installed in our environment, while `scipy` is. 
  * **Recommendation**: Start with a 50-trial **Random Search** baseline, followed by **SciPy's `differential_evolution`** (60-100 evals), and optionally local **Nelder-Mead** polishing (<=30 evals). Keep the total optimization budget under **150 evaluations** to control validation-set overfitting (amendment C).
* **Viterbi/Dynamic Programming for P2 (Boundary Selection)**: Use exact 1D DP sequence segmentation with transition/duration priors (the original Approach B Viterbi search) to select final boundaries from candidate peaks.

### 2. What to DEFER (Post-Validation Gate)

* **Optuna / Nevergrad Dependency**: Defer adding new dependencies like `optuna` or `nevergrad` until a fixed-feature SSM baseline has proven signal, and we require conditional/categorical search spaces or study persistence.
* **Rust ndarray Porting**: Do not write any Rust code for the SSM until the Python prototype clears the validation gate (beating the 16-bin oracle significantly).
* **Tiny Trained Head**: Defer replacing the hand-tuned readout with a differentiable head (e.g., logistic regression/MLP) until we have established a solid fixed-feature SSM baseline. If we do train one, keep it under 10 parameters to prevent memorization on our small $N$ (~200–345 tracks).
* **Ant Colony Optimization (ACO) for P2**: Defer ACO. While a natural fit for graph-based pathfinding, 1D sequence segmentation is solved exactly and more cheaply by Viterbi/DP. Revisit ACO only if we move to multi-modal 2D structural graphs.

### 3. What to KILL (Over-engineering / High Risk)

* **SSM ONNX/`ort` Compilation**: Kill the plan to compile the SSM itself into ONNX. For a 90 s crop at 0.2 s steps ($N \approx 450$), the matrix operations are tiny. Setup and copy overhead in ONNX Runtime would likely dominate. The hotspot is the CLAP forward pass, not the SSM.
* **Differentiable Peak-Picking (End-to-End HPO)**: Kill differentiable NMS / soft-argmax. It adds extreme complexity to the optimization graph for little practical return compared to SciPy/Random search sweeps.
* **Medium/Large Trained Models**: Kill any neural sequence models trained end-to-end on CLAP/chroma inputs due to severe data limitations. Keep the untrained CNN front-end fixed.

---

## [Closed, 2026-06-07]

This follow-up session has reached consensus and is closed. The Phase 0 script has been verified, the roadmap is frozen, and the brainstorm synthesis stands as the guide for Phase 1.

