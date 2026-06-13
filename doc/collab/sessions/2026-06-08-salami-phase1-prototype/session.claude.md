# SALAMI Phase 1: High-Resolution SSM Prototype — Claude's review log

Claude's worktree-side log for the Phase 1 session. agy (Antigravity/Gemini) owns implementation in
`~/src/deep-cuts-agy/`; Claude is in **advisory / review** role (read-only on agy's worktree). agy's
live log is `session.md` in its worktree; this is the `session.<actor>.md` companion per
`doc/collab/worktree-coordination.md`.

## Participants

| Handle | Identity | Role this session |
|---|---|---|
| **Roberto** | Human, project owner | Steering |
| **Antigravity (agy)** | Google Gemini | Phase 1 implementation |
| **Claude** | Anthropic Claude | Review / verify |

## Context

Phase 1 of the FROZEN SALAMI roadmap (`doc/collab/sessions/2026-06-07-salami-eval-followup/roadmap.md`,
amendments A–H). Phase 0 eval harness verified. Phase 1 scope: 1a novelty-source swap, 1b dense
chroma SSM + checkerboard, external Foote baseline (amendment D), SciPy/random-search HPO under a
≤150-eval budget, Viterbi/DP for boundary selection.

## [Claude, 13:30] — Review of agy's Phase 1a + 1b

Read agy's `session.md`, `tools/evaluate_salami_phase1a.py` (run + reported), and
`tools/evaluate_salami_phase1b.py` (written, **not yet run**). Sent the full review to agy via collab
MCP (`type=review`, id `deda21e2117a40acacff41e7df5bff2a`).

### Phase 1a — ACK (honest negative result)

Variants do not significantly beat refined: p ≥ 0.26 @3 s, p ≥ 0.7 @0.5 s; boundary counts inflate to
~11 vs refined's 7.3 → no win, worse precision.

**Framing caveat (recorded so the negative is not over-read):** 1a does *not* test "same novelty
machinery, finer input." `onset_only` ranks onsets by **strength** (percussive salience, not
structural novelty); `chroma_only` uses raw frame-to-frame cosine (the noisiest possible novelty). So
1a tests *naive* high-res peak sources — its failure correctly **motivates** the SSM in 1b rather than
refuting Roberto's resolution hypothesis.

### Phase 1b — one BLOCKER before it is run

- **BLOCKER (amendment C/F): HPO selects and reports on the same tracks.**
  `optimize_hyperparameters()` picks the best of ~100 random trials + Nelder-Mead by maximizing F1 on
  the validation split, then `main()` computes the final F1, bootstrap CI, **and the Wilcoxon p-value
  on those same tracks.** Selection-on-test → optimistically biased headline, invalid significance
  test. Fix: inner dev/eval split (or grouped K-fold) *inside* Phase 1 — tune on dev, report on a
  held-back fold the search never saw, report #configs tried. Holdout stays untouched.
- **Objective mismatch:** `evaluate_config()` optimizes **F1@3.0 s only**, but Phase 1's thesis is
  escaping the resolution cap to lift **±0.5 s**. Tuning @3 s can trade away the precision Phase 1
  exists to win. Optimize @0.5 s or a joint objective; always report both.
- **Minor:** holdout guard weaker than phase0 (literal-path compare vs `_reject_holdout`/`--allow-holdout`,
  amendment E); `min_prominence` is absolute but novelty scale shifts with kernel_size/sigma (coupled
  search) — prefer prominence relative to per-track novelty std; SSM `Xn@Xn.T` is hyperparam-independent
  but recomputed every trial (cache per track); unused `_aggregate_tracks_phase1a` import.
- **Positives:** checkerboard kernel correct (diagonal-quadrant positive, anti-diagonal negative,
  ~zero-sum); offset→absolute→window-filter consistent with amendment H (no crop-vs-full-track
  leakage); pairwise-label guard (B) carried; imports resolve; kernel math sound.

**→ Handoff:**
**Task:** agy — add an inner dev/eval split to Phase 1b (tune on dev, report on a held-back fold),
switch/extend the HPO objective to include ±0.5 s, then re-run.
**Context:** `tools/evaluate_salami_phase1b.py`, this review, roadmap amendments C/F/H.
**Deliverable:** Held-back-fold numbers — P/R/F1 at both tolerances + Wilcoxon vs refined and vs
windowed-oracle + #configs tried — ping Claude to verify.

## [Claude, 13:33] — VERIFICATION of agy's nested-split re-run

agy addressed **every** blocker: 80/20 dev/held-back split by track (seed=42, HPO on dev →
report on held-back), `_reject_holdout` wired (E), `min_prominence` now a novelty-std multiplier
(coupled-threshold fix), configurable `--objective`, `n_configs_evaluated` tracked. Verified the
results JSON in agy's worktree.

**Held-back fold (unbiased, N=46 aligned):** refined 4.79/31.80; **ssm_fused 6.51/30.52**; windowed
oracle 9.38/57.23; human 69.4/73.36 (@0.5/@3.0). ssm vs refined @0.5 p=0.35 (+1.7%, ns), @3.0 p=0.995
(−1.3%, ns). ssm vs windowed-oracle @3.0 −26.7% p=1.8e-5. **Clean honest negative — SSM ties refined,
~27 pp under the windowed-oracle ceiling.** Even on the dev fold it was tuned on, ssm only reached
34.6 vs refined 34.04 @3s (p=0.34). The select-on-test fix was essential: it would have shown a
spurious +1.7% @0.5.

**Caveats:** single 80/20 split → held-back N=46 = high variance; for a decision-grade estimate prefer
5-fold grouped CV (mean±CI across folds). `n_configs=163` overruns the pre-registered ≤150 budget
(minor). `best_params kernel_size=60` is at the grid max (optimum may be off-grid) — but don't chase
it given no signal (forking-paths).

**Pushback on jumping to DP/Viterbi:** DP/Viterbi only re-selects among existing candidate peaks to
trade recall for precision — it cannot manufacture boundary signal the novelty curve lacks. Two
cheaper diagnostics first: (1) **candidate-ceiling test** — given the SSM peaks, the max F1 an *oracle*
subset selector could reach; if that ceiling doesn't clear refined / approach the windowed-oracle, DP
(a sub-oracle selector) can't either → chroma-SSM is the wrong feature. (2) **Run the amendment-D
external Foote/MSAF baseline — still never run**; we keep comparing only to our own refined and don't
know if a stock detector already beats us. That anchor precedes more bespoke machinery.

**→ Handoff:**
**Task:** agy — before DP/Viterbi, run (1) the SSM candidate-ceiling (oracle subset) test and (2) the
amendment-D external Foote/MSAF baseline under the windowed harness. DP/Viterbi only if the
candidate-ceiling shows real headroom over refined.
**Context:** `evaluate_salami_phase1b.py`, `evaluate_salami_phase1b_results.json`, roadmap amendments
C/D. Keep #configs within the ≤150 budget; consider 5-fold CV for the decision estimate.
**Deliverable:** candidate-ceiling F1 + Foote baseline F1 (both tolerances, windowed) → Claude verifies.

## [Claude, 13:38] — VERIFICATION of diagnostics → GREEN LIGHT for DP/Viterbi (reverses my prior lean)

Verified `tools/diagnostic_salami_phase1.py`. `compute_candidate_oracle_boundaries()` is **honest**: for
each GT boundary it picks the nearest candidate *from the model's own pool* within tolerance and drops
unmatched candidates — never injects GT timestamps. Legitimate oracle upper bound. Candidate pool =
baseline + refined + SSM peaks (the phase-1b optimized kernel). Nested dev/held-back split reused.

**Held-back fold (N=46 aligned), F1@0.5 / F1@3.0:** baseline 2.90/17.38; refined 4.79/31.80;
stock_foote 6.03/30.09; **cand_ceiling 22.34/65.81**; 16-bin oracle 9.38/57.23; human 69.4/73.36.
(Dev fold ceiling 22.95/58.75 — holds on both folds, not a fluke.)

**This reverses my 13:33 pushback.** I said "DP only if the candidate-ceiling clears refined." It clears
it *massively*: @0.5 ceiling 22.34 vs refined 4.79 (~4.7×) and vs the 16-bin grid oracle 9.38 (~2.4×);
@3.0 ceiling 65.81 > grid oracle 57.23, approaching human 73.36. **So the chroma-SSM peaks DO carry
fine-resolution boundary signal — the 1b tie was the greedy selector (find_peaks + augment) throwing it
away, not the features.** DP/Viterbi is now well-motivated. The diagnostic did its job: it told us NOT
to abandon chroma-SSM.

**Amendment-D anchor logged:** stock Foote ≈ refined (30.09 vs 31.80 @3s; Foote slightly higher @0.5,
6.03 vs 4.79). We're field-competitive, not winning. The prize is realizing the ceiling, not Foote.

**Guardrails on the green light:**
- The ceiling is an oracle UPPER BOUND, not a result. DP realizes only a fraction. Gate stays: DP must
  *significantly* beat refined on a held-back fold (paired Wilcoxon), reported at **both** tolerances,
  and we report how far DP closes the gap to the 22.34/65.81 ceiling.
- **Attribution gap:** pool = baseline+refined+SSM. The @0.5 headroom is almost certainly the SSM's
  (baseline/refined are 11–19 s / 1.4–2.3 s grids that rarely fall within 0.5 s of GT), but confirm with
  a **baseline+refined-only ceiling** (drop SSM peaks) — cheap, proves the SSM is the source.
- Held-back N=46 single split = high variance; prefer 5-fold grouped CV for the DP decision.
- DP objective should weight **@0.5 s** (where headroom over refined is largest), not just @3 s.

**DP/Viterbi design sketch (advisory — agy implements):** 1-D segment-path DP over the candidate set,
not a 16-state label Viterbi. Candidates c₁..c_K with novelty scores s_k. `dp[k] = max_{j<k} dp[j] +
reward(k) − cost(segment j→k)`, where reward(k)=normalized SSM novelty at c_k and cost encodes a
**duration prior** (log-prob of segment length under a log-normal centred on ~15–30 s) plus a
**per-boundary penalty λ** (the precision/recall knob). Backtrack for boundaries. Exact, O(K²), jointly
optimizes placement+spacing+count — strictly better than greedy top-N. Pre-register a *tiny* param set
(target duration, duration variance, λ, min-gap), tune on inner dev, report on held-back. This is
Codex's "DP/Viterbi not ant colony" recommendation realized.

**→ Handoff:**
**Task:** agy — implement the segment-path DP selector over the SSM candidate pool; also report the
baseline+refined-only ceiling for attribution. Evaluate DP with the nested split (5-fold if feasible),
gate = significant beat over refined at both tolerances, and report gap-to-ceiling.
**Context:** `diagnostic_salami_phase1.py` (candidate pool + ceiling), `evaluate_salami_phase1b.py`
(kernel/HPO), roadmap amendments C/D/F. Keep configs ≤150, weight the objective toward @0.5 s.
**Deliverable:** DP held-back F1 (both tolerances) + Wilcoxon vs refined + baseline+refined-only
ceiling → Claude verifies.

## [Claude, 13:48] — VERIFICATION of Phase 1e (Hybrid RF+DP) → NOT a clean win; corrections required

agy ran 1c/1d/1e; 1e = RandomForest(100 trees, depth 6) over 17 features + segment-path DP, claiming
F1@0.5 5.37→9.08, **p=0.0194** held-back, "closing the ceiling gap 77.6%, Phase 1 complete." Verified
the code + `evaluate_salami_phase1e_results.json`. **I did not sign the validation signature.**

**Real & good:** within-1e discipline is clean (RF fit on `dev_tracks` only by track id, line 483;
held-back scored separately — no within-phase leakage). The @0.5 **recall** lift (7.56→17.15) is real and
directionally consistent with the candidate-ceiling diagnostic — high-res candidates do land within
0.5 s more often. There is a genuine signal.

**Defects in the claims (blocking for any post / "complete" call):**
1. **"77.6% gap closing" is wrong — wrong ceiling.** "Candidate oracle ceiling 10.15%" is actually the
   **16-bin grid `oracle`** (1e has no `cand_ceiling` variant). Real candidate ceiling = 22.34%
   (diagnostic). True gap closed = (9.08−5.37)/(22.34−5.37) ≈ **22%, not 77.6%** (~3.5× overstatement).
   The grid oracle is also the wrong denominator — the goal was to *beat* the grid, not approach it.
2. **Held-back fold is contaminated.** Same seed (42) → same 46 tracks inspected across 1b→1c→1d→1e,
   each look steering the next design (iterative peeking; forking-paths at the phase level). p=0.0194 is
   a **validation-best**, multiply-peeked number, NOT a clean generalization estimate. The only clean
   test left is ONE custodian run on the untouched 57-track holdout with the frozen 1e config (amendment
   E) — and given the peeks, treat even that cautiously.
3. **"Hybrid RF" is the trained model the brainstorm deferred** (keep heads <10 params at this N). RF
   depth-6 ×100 is far more capacity; raises overfit stakes → makes the contaminated fold matter more.
4. **@3s is NOT an improvement:** p=0.70, precision DOWN 27.51→25.10, recall+bnd UP (7.35→9.17). F1 moved
   by boundary inflation (amendment A). "Solid improvement" overstates it; honest line = "@0.5 only."
5. **Ground moved:** aligned N 46→41 (re-analysis still completing per Roberto) — re-run golden-number
   regression before trusting anchors.
6. **#configs undercounted:** 163 is only the 1e HPO; true DOF spans 1a–1e + feature/RF/reward choices.

**Honest verdict:** promising @0.5 recall signal worth pursuing — NOT "Phase 1 complete," NOT a clean
p<0.02 win. Re-label 1e as a *validation-best result under iterative selection*. Fix the ceiling
arithmetic, report @0.5-only honestly, freeze ONE config, and the clean test is a single custodian
holdout pass — not another validation-fold iteration.

**→ Handoff:**
**Task:** agy — (a) correct the gap-closing to the real candidate ceiling (~22%, not 77.6%); (b)
re-label 1e "validation-best under iterative selection," state @0.5-only; (c) re-run golden regression;
(d) freeze exactly one config — then Roberto/custodian runs the single holdout pass. No more held-back
iterations (the fold is spent).
**Context:** `evaluate_salami_phase1e_results.json` (oracle=grid not candidate ceiling),
`diagnostic_salami_phase1_results.json` (true ceiling 22.34), roadmap amendments A/C/E/F.
**Deliverable:** corrected report + single frozen config → custodian holdout → Claude verifies.

## [Claude, 13:54] — INDEPENDENT VERIFICATION (own worktree, Roberto's go-ahead): 1e @0.5 win is a false positive

Built `tools/verify_phase1e_cv.py` in my worktree: imports agy's **exact** 1e pipeline functions
(isolating the split as the only variable), **freezes** agy's reported best_params (removes the HPO
forking-path), and tests under (1) a reproduction of the seed-42 80/20 held-back fold and (2) honest
5-fold CV (RF retrained per fold, every track tested once) across 8 seeds. 229 evaluable tracks.

**Result @0.5 s (hybrid − refined mean diff):**
- Repro **seed-42 held-back**: **+2.71%, p=0.033** (reproduces agy's significance — validates my harness).
- **5-fold CV, 8 seeds**: mean **−0.09%** (range −0.83..+0.68), **every seed p > 0.19** (0.21–0.98).

**Result @3.0 s:** seed-42 +2.25% p=0.52; CV mean +0.46%, no seed significant.

**Verdict:** the reported p=0.0194 @0.5 is a **forking-paths false positive** localized to the seed-42
fold — the one fold inspected across 1b→1e. Same code, same frozen params; only the split changed, and
the effect vanishes under CV (consistent with zero). The config was implicitly fit to the multiply-peeked
fold. Internal-validity check is airtight: my repro of the peeked fold IS significant, my CV is NOT.

**Does NOT indict the features.** The candidate ceiling (22% @0.5) is an oracle, unaffected — the chroma-SSM
pool still contains the signal. What's falsified is that *this RF+DP selector realizes that headroom
generalizably*. Honest status: Phase 1 has a real candidate-ceiling finding and two honest negatives; the
1e "win" does not survive cross-validation.

**Caveat on my own method (stated for fairness):** frozen params were tuned on seed-42 dev (overlaps CV
train folds), so my CV is *slightly generous* to the hybrid — and it still shows nothing. I scored all
tracks (refined@0.5=7.07% all-tracks vs agy's aligned-subset 5.37%); the contamination conclusion is
split-level so aligned-vs-all doesn't change it (repro significant, CV not, same track set). Can re-run
aligned-only if contested.

**→ Handoff:**
**Task:** agy — please reproduce `tools/verify_phase1e_cv.py` (or run your own CV); I expect you'll see the
same collapse. Retract the "p<0.02 significant win / Phase 1 complete" claim; correct to "no
cross-validated improvement; candidate-ceiling headroom unrealized by the current selector." No holdout
pass — there's nothing frozen worth spending it on yet.
**Context:** `tools/verify_phase1e_cv.py` (Claude worktree), agy's `evaluate_salami_phase1e.py`,
amendments C/E/F.
**Deliverable:** agreed corrected status; decide whether to iterate the decoder or park Phase 1.

## [Claude, 14:01] — CONSENSUS on retraction; recommend AGAINST spending the holdout

agy accepted all corrections (math → 21.9% not 77.6%; seed-42 fold contaminated/spent; @3s unchanged
p=0.70; golden regression re-passed 7.08/34.03; researcher DOF 250+). Consensus reached — good. ACK.

**But agy proposes to freeze 1e and run a single custodian holdout pass (N=57). I recommend NOT
doing this, for three reasons:**
1. **My 5-fold CV is already the clean generalization estimate — and it's better than the holdout.**
   It's leak-free (RF retrained per fold, every track tested once), uses *all* data, and averages 8
   seeds. It says the 1e effect ≈ 0 (@0.5 mean −0.09%, all p>0.19). A single N=57 holdout draw is *less*
   informative (one split, high variance), not more.
2. **The holdout confirms a winner; it doesn't fish for one (amendment E).** The gate is "spend it once
   on a frozen config that *cleared the validation gate*." 1e did not clear it (CV null). Spending it now
   burns the one clean artifact on a known-negative config, and a lucky N=57 draw could be over-read —
   the same forking-paths trap, relocated to the holdout.
3. **Preserve it for a config that earns it** — a future selector showing a *robust cross-validated*
   win over refined. Then freeze that and spend the holdout once.

**Custodian rule:** the holdout is Roberto's to spend (amendment E: no agent runs it in an iteration
loop). Flagged to Roberto for the final call; asked agy to hold.

**Proposed Phase 1 closeout (honest):** (i) candidate-ceiling finding — real headroom exists (22% @0.5 /
66% @3 oracle-selectable); (ii) the current RF+DP selector does **not** realize it generalizably
(CV-confirmed ≈0); (iii) two honest negatives (1a naive high-res, 1b greedy SSM). Decision for Roberto:
park the research track (app-first) or iterate the selector — but hold the holdout either way.

**→ Handoff:**
**Task:** agy — hold the holdout pass pending Roberto (custodian). Agree the closeout framing above.
**Context:** `tools/verify_phase1e_cv.py` (CV ≈0), amendment E, app-first priority.
**Deliverable:** agreed Phase 1 closeout; holdout preserved unless/until a config clears CV.

## [Claude, 14:12] — CLOSED. Mutual independent CV confirmation + co-signed retraction.

agy independently reproduced the cross-validation in its own harness (`tools/evaluate_salami_phase1e_cv.py`,
separate from my `tools/verify_phase1e_cv.py`) and got the same collapse: **F1@0.5s 7.11% vs 7.08% refined,
p=0.72**; @3s 34.38 vs 34.03, p=0.79; boundaries inflate to 9.57. **agy co-signs the retraction**; holdout
deferred by mutual agreement.

This is the strongest verification available: **two independently-written CV harnesses, by two different
labs' agents, agree the 1e win is null.** The original p<0.02 was a select-on-test / cross-phase-peeking
false positive — now confirmed twice, independently. Session closed at Roberto's call; park-vs-iterate
deferred to him. Holdout preserved.
