# SALAMI Phase 1 — Next Steps & Proposals (Claude)

Saved 2026-06-08 as the session paused (Roberto returned to work). Captures where Phase 1 landed and
what to do when the research track is next picked up. App-first: this is opportunistic, not committed.

## Where Phase 1 actually landed (consensus: agy + Claude)

- **The signal exists.** Candidate-ceiling diagnostic (honest oracle subset-selection over the
  chroma-SSM candidate pool) reaches **22% F1@0.5s / 66% F1@3.0s** vs refined's 5% / 32% — far above the
  16-bin grid oracle. So the high-resolution features genuinely carry boundary information.
- **The current selector does not realize it generalizably.** The 1e Hybrid RF+DP first looked like a
  significant @0.5s win (p=0.0194), but that was a **forking-paths artifact**: the seed-42 held-back fold
  was inspected across phases 1b→1e. Independent 5-fold CV (`tools/verify_phase1e_cv.py`, agy's exact
  code, frozen params, 8 seeds) collapses the effect to **mean −0.09%, all p>0.19**. @3s never moved
  (p=0.70). agy accepted all of this.
- **Two honest negatives** along the way: 1a (naive high-res peak sources) and 1b (greedy SSM peak-pick).

Net: *we found where the treasure is and proved this shovel doesn't reach it.* That's a real, publishable
result set, not a failure.

## Open decision (Roberto's call)

1. **Holdout (57 tracks): HOLD — do not spend on 1e.** Recommendation stands. The 5-fold CV is already a
   cleaner generalization estimate than a single N=57 draw, and it's null; the holdout confirms a winner,
   it doesn't fish for one (amendment E). Preserve it for a config that earns it with a robust CV win.
2. **Park vs iterate the selector.** App-first says park is defensible. If iterating, see below.

## Proposals if/when we iterate (ordered by leverage)

There are two distinct levers — keep them separate:

### A. Realize the existing ceiling (better *selector*, ceiling fixed at 22%/66%)
- **Try the lean DP alone, no RF.** agy's optimal-partition DP (cumulative-sum reconstruction cost +
  log-normal duration prior + per-boundary penalty λ) with **far fewer DOF** than the 17-feature RF
  (which added 250+ researcher DOF and was still CV-null). Test whether a low-DOF selector generalizes
  where the high-capacity one didn't. Pre-register ≤4 params.
- **Bound the realizable gain first.** Current selector gets ~9% of the 22% ceiling @0.5; refined gets
  ~5%. Even a perfect realization caps at 22% @0.5 — modest. Decide if that ceiling is worth the effort
  *before* building more selector machinery.

### B. Raise the ceiling (better *candidate features*, harder)
- The 22% @0.5 ceiling means most GT boundaries have **no chroma-SSM candidate within 0.5s** — a feature
  limitation, not a selection one. Raising it needs denser/better candidates: onset-informed candidate
  generation, full-track dense features (off the cheap cached-window path), or **dense CLAP (roadmap
  Phase 5)**. This is the bigger, costlier lever and gated behind A showing the ceiling is worth chasing.

### C. Methodology hardening (do regardless — cheap, high value)
- **Make K-fold CV the default validation protocol**, not a single 80/20 split. Bake it into the eval
  harness. The single-split + same-seed pattern is exactly what enabled the cross-phase peeking.
- **Keep the candidate-ceiling diagnostic as a standard gate**: before building any selector, confirm the
  signal is reachable. It's what saved us from abandoning the features *and* what exposed the selector gap.
- **Report researcher DOF** (configs + feature/model/reward choices across phases), not just the final
  HPO trial count.
- **Re-run the golden-number regression after analysis completes.** Re-analysis was incomplete this
  session (aligned N drifted 46→41); anchors must be re-confirmed against the final DB state.

## Carry into the frozen roadmap (amendments)

Fold C above into `doc/collab/sessions/2026-06-07-salami-eval-followup/roadmap.md` when next unfrozen:
K-fold-CV-by-default, candidate-ceiling-as-gate, DOF reporting. These are the durable methodological wins.

## Adjacent / parked
- **SAX-transformer label-emission swap** (Phase-5 *labeling*, not boundaries) — separate thread, parked.
- **Blog + LinkedIn drafts** ready for the deep-analysis red-team pass:
  `doc/private/blog_draft_graded_own_homework_claude.md`, `linkedin_graded_own_homework_claude.md`,
  notes in `blog_notes_salami_phase1_claude_section.md` (merge with agy's `_agy_section`).
