---
status: need-human-review
owner: Roberto
last_verified: 2026-06-08
implemented_by:
superseded_by:
related_code:
related_skills: bot-collab, how-to-experiment, collab
---

# CCREP Synthesis: A Quality-Ratchet Coordination Protocol

This is a synthesis of the four CCREP designs produced in the
[2026-06-08 multi-agent collaboration research session](../collab/sessions/2026-06-08-multi-agent-collaboration-research/session.md)
(Codex, Google DeepThink, Meta, and the Antigravity "Unified 2.1" merge). It folds the
strongest ideas from each into one buildable protocol and explicitly cuts the parts that
are over-engineered for our scale.

## Status

`need-human-review`. Roberto asked for an `accepted` doc, but also said he would decide
tomorrow after reading the source designs with a clear head — so marking it `accepted`
now would invent a decision that hasn't been made. **If Roberto approves as-is, flip
`status: accepted` and set the four session designs as superseded-by this doc.** Until
then this supersedes nothing.

Source designs this consolidates (all under the session folder, unchanged):

| Source | Strongest contribution | Kept? |
|---|---|---|
| [codex-ccrep-design.md](../collab/sessions/2026-06-08-multi-agent-collaboration-research/codex-ccrep-design.md) | Tuple-as-value, event-log reducer, content-addressed eval cache, critique admissibility, independence rule, phased rollout | **Spine** |
| [unified-ccrep-design.md](../collab/sessions/2026-06-08-multi-agent-collaboration-research/unified-ccrep-design.md) | Physical AST/line-diff gates replacing prompt-temperature; git worktree isolation | **Gate mechanism** (budgets made configurable) |
| [meta-ccrep-design.md](../collab/sessions/2026-06-08-multi-agent-collaboration-research/meta-ccrep-design.md) | Fixed domain-weight table; scribe linearizes `session.md` on merge | **Weights v1 + scribe** |
| [google-ccrep-design.md](../collab/sessions/2026-06-08-multi-agent-collaboration-research/google-ccrep-design.md) | Causal Interaction Graph for targeted reviewer wake-up | **Deferred** (kept as an idea, not v1) |

## Problem

Two problems were conflated in the session, and only the second is worth solving:

1. **Write contention on `session.md`** (the original complaint). This has a small, boring
   answer: agents never edit `session.md` directly; a single scribe appends a linearized
   entry on merge. At our concurrency (≤ ~4 agents, mostly turn-taking) this is sufficient —
   the session log itself was co-authored by five participants without the failure recurring.
2. **Turning multi-agent collaboration into a quality ratchet** (Roberto's actual goal):
   one agent's output is evaluated and improved by peers until the merged result is
   provably better than any single agent would produce, not merely coordinated.

This proposal targets (2) and treats (1) as a one-line consequence of the design.

## Core Principle

The protocol does not ask *"Do the agents agree?"* It asks:

> *"Has this exact commit accumulated enough reproducible evidence, domain-appropriate
> independent approval, and unresolved-risk closure to become the next accepted state?"*

That is the difference between a chat loop and a quality ratchet. Consensus is reached on a
**value**, defined (per Codex) as an immutable tuple:

```
(task_id, commit_sha, evaluation_report, critique_set, vote_set, gate_policy_version)
```

The MCP server is the coordinator/blackboard; agents are proposers, reviewers, and amenders.
Paxos/Raft are metaphors for roles and invariants only — we have a central coordinator, so
we do not implement replication consensus.

## Proposed Design

### 1. The loop

```
OPEN → CLAIMED → PROPOSED → EVALUATING → REVIEWING → (AMENDING ⤴ revision loop) → APPROVED → MERGED
                                   │                                      ▲
                                   └── EVALUATION_FAILED ─────────────────┘
```

- **Propose**: agent claims a task, makes changes on a branch, calls `submit_proposal`.
  The branch is resolved to an immutable `commit_sha`.
- **Evaluate**: the server checks the commit out in an isolated **git worktree**, runs the
  task's eval suite, and stores a content-addressed `EvaluationReport`.
- **Review**: one or more independent agents submit a structured `Critique` against the
  exact commit.
- **Amend**: the proposer (or another agent) addresses blocking findings on a new commit.
  Any new commit invalidates prior approvals; prior critiques carry over until resolved.
- **Merge**: only when the consensus gate passes.

### 2. Invariants (from Codex — non-negotiable)

1. **Immutable proposal version** — a proposal points to a fixed `commit_sha`; if the branch
   moves, that's a new revision.
2. **Content-addressed evaluations** — cache by `(commit_sha, eval_suite_hash, dataset_hash,
   env_hash)`. No re-running an eval whose inputs are unchanged.
3. **Votes expire on code change** — any new commit invalidates previous approvals.
4. **No self-approval** — the author may explain or amend but cannot satisfy peer quorum.
5. **Derived consensus state** — agents never write `ConsensusState`; the server reduces it
   from an append-only event log.

### 3. Anti-divergence: physical gates, not prompt temperature

The session's biggest correction (Unified 2.1): **agents cannot self-adjust inference
temperature, and clients ignore the sampling temperature.** Every "simulated annealing via
prompt" scheme in the Codex/Google/Meta drafts is therefore unenforceable and is dropped.

Replace it with **hard gates the server enforces on the diff**, parameterized by revision
round `n` and configurable per task — not the hardcoded "15 lines / 2 files" constants from
Unified 2.1:

```yaml
revision_gate_policy:           # defaults; overridable per task
  - round: 0                    # exploratory
    scope: unrestricted
  - round: [1, 2]               # refinement
    forbid: [new_function_defs, signature_changes, new_files]   # AST-checked (tree-sitter)
    allow: [function_body_edits]
  - round: [3, 4]               # surgical
    max_files: 3
    max_changed_lines: 40
  - round: 5                    # terminal
    allow: [linter_autofix_only]   # rustc --fix / eslint --fix diffs only
  max_revisions: 5
```

Plus the cheaper, source-agnostic anti-churn levers from Codex, which do more work than any
schedule:

- **Critique admissibility**: a finding blocks merge only if it is
  *specific + actionable + evidence-linked + severity-classified*. "This feels too complex"
  is inadmissible; a finding citing an eval metric or a file:line is admissible.
- **Plateau stop**: halt if no metric clears its `min_metric_delta` for `patience_rounds`
  consecutive revisions (e.g. `boundary_f1_at_3s: 0.002`).
- **Edit-war stop**: if normalized Levenshtein(diff_n, diff_{n-2}) < 0.05 and authors
  alternate, freeze the two and hand a minimal-compromise diff to a third agent.

### 4. Consensus gate (simplified for v1)

Cut from v1: Kendall's W, Friedman χ², Schulze/Condorcet, and Bayes-optimal log-odds
weighting. Those assume many voters ranking many candidates; we have ~3 agents and almost
always a single proposal, and log-odds weighting needs a verdict-accuracy history that does
not exist yet (cold-start unaddressed in every draft). They are **deferred**, not deleted —
see "Deferred ideas."

v1 gate:

```yaml
consensus_gate:
  automated:
    require_all_hard_checks_pass: true        # build, tests, lint, fmt
    forbid_golden_metric_regression: true     # beyond per-metric tolerance
  peer:
    require_one_implementation_approval: true
    require_one_independent_approval: true    # reviewer != author, different model family
    forbid_open_blocking_critiques: true
  human:
    required_for: [public_api_change, destructive_migration, model_or_dataset_change,
                   large_architecture_change]
```

Domain weighting, when it earns its place, starts from Meta's honest fixed table
(Claude 0.6 implementation, Gemini 0.6 architecture, Codex 0.6 verification, Roberto = veto)
rather than a formula fed by data we don't have.

### 5. session.md contention — solved as a side effect

Agents write turns to a spool (`spool/<agent>/new/`); the server is the only writer of
`session.md` and appends a linearized entry when a proposal merges. No locks on the log, no
write-write races, clean linear git history.

## Data Model / Schema Impact

Adopt Codex's JSON Schema (Draft 2020-12) for `Proposal`, `EvaluationReport`, `Critique`,
`ConsensusState` verbatim — it is the most complete and the only one that models evidence
links, content-addressed eval keys, severity classes, and derived consensus. The Google and
Meta schemas are strict subsets and add nothing v1 needs.

Storage: append-only `event_log` table + materialized views in SQLite (`proposals`,
`evaluation_reports`, `critiques`, `votes`, `merge_records`). Standard WAL — we are on
macOS/APFS, so the WSL2/BTRFS fallback machinery from the Courier and Unified docs is **not
needed** and should not be built.

## MCP Surface (Phase 1 subset)

Tools: `claim_task`, `submit_proposal`, `run_evaluation`, `submit_critique`,
`submit_revision`, `compute_consensus`, `merge_proposal` (human-gated for the categories
above). Resources: `ccrep://tasks/{id}`, `ccrep://proposals/{id}/{diff,evaluation,critiques}`,
`ccrep://tasks/{id}/consensus`. Worktree lifecycle: `git worktree add --detach
.ccrep/worktrees/{proposal_id} {commit_sha}` → run suite → `git worktree remove --force`.

## Validation Plan

1. **Phase 1 — Evidence Ledger (only this is committed-to).** `submit_proposal` →
   worktree eval → `EvaluationReport` → structured `Critique` log, with the gate reduced to
   *green automated checks + one independent approval*. No voting math, no annealing, no
   weighting. Dogfood it on one real Deep Cuts task (e.g. a boundary-threshold tweak) and
   measure: did it catch a regression a solo agent missed, and what did it cost in tokens
   and wall-clock vs. just doing the change?
2. **Phase 2 — Physical revision gates + plateau/edit-war stops.** Add only if Phase 1's
   loop actually churns.
3. **Phase 3 — Weighted quorum (fixed table).** Add only when single-approval proves too
   coarse.
4. **Phase 4 — Competing-branch ranking (Condorcet/Schulze) + CIG reviewer routing.** Add
   only when a real multi-branch bake-off appears. May never be needed.

Each phase ships independently and is justified by a problem observed in the previous phase,
not by the design's elegance.

## Product-Fit Notes

- Standing priority is **app-first, not coordination research**. CCREP ships zero app
  features; the per-change cost (full eval suite + multiple agents reading diffs + rounds)
  is real. Phase 1 is cheap and genuinely useful as a reproducible-eval harness even if the
  consensus layers are never built — that is the strongest reason to start there and stop
  early if the value isn't visible.
- **Provenance hygiene**: the Courier and Google docs present confabulated benchmarks
  (DPBench rates, an RL reward table, two-decimal model deadlock percentages) as fact. If
  these sessions feed real decisions, the collab protocol should adopt a rule mirroring the
  critique-admissibility gate: *claims must be sourced or explicitly marked speculative.*
- Recommend stripping or flagging the fabricated numbers and the irrelevant WSL2/BTRFS
  sections from the Courier doc before it's cited anywhere.

## Decision Log

- **2026-06-08** — Synthesis authored (Claude). Codex chosen as spine; Unified 2.1 physical
  gates adopted over prompt-temperature; Kendall's W / Schulze / log-odds weighting deferred
  to Phase 4; left at `need-human-review` pending Roberto's read-through.

## Deferred Ideas (kept for when scale justifies them)

- **Condorcet/Schulze branch ranking** — only meaningful with ≥3 competing branches.
- **Kendall's W concordance + Friedman significance** — needs many rankers to be non-noise.
- **Bayes-optimal log-odds vote weighting** — needs a verdict-accuracy history; revisit once
  the event log has accumulated labeled outcomes.
- **Causal Interaction Graph reviewer routing** (Google) — a real optimization once review
  fan-out is large enough to matter.
- **Multi-Objective Optimization Vector / Pareto gating + elite cross-pollination** (Unified)
  — defer until single golden-metric gating proves insufficient.
