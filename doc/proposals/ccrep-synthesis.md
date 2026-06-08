---
status: accepted
owner: Roberto
last_verified: 2026-06-08
implemented_by:
superseded_by:
related_code: tools/ccrep/
related_skills: ccrep, bot-collab, how-to-experiment, collab, write-docs
---

# CCREP Synthesis: A Quality-Ratchet Coordination Protocol

This is a synthesis of the four CCREP designs produced in the
[2026-06-08 multi-agent collaboration research session](../collab/sessions/2026-06-08-multi-agent-collaboration-research/session.md)
(Codex, Google DeepThink, Meta, and the Antigravity "Unified 2.1" merge). It folds the
strongest ideas from each into one buildable protocol and explicitly cuts the parts that
are over-engineered for our scale.

## Status

`accepted` (2026-06-08). Roberto read the four source designs and approved this synthesis as
the live CCREP proposal; the [implementation-split amendment](../collab/sessions/2026-06-08-ccrep-implementation-split/session.md)
(Claude + Antigravity) is folded in. This doc now supersedes the four source designs below —
they remain under the session folder as unchanged research records, consolidated here.

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
6. **Artifact-profile consistency** — every proposal declares an `artifact_profile`
   (`code_change` | `code_review` | `design_doc`); the gate components, revision gates, and
   eval suite that apply are exactly those the profile selects (see "Artifact Profiles &
   Usage Modes"). A check that does not belong to the declared profile never fires.
7. **Frontmatter-status sync (design docs), one-directional** — a doc may not sit at
   `status: accepted` unless its latest content reached `APPROVED` (green automated gate + ≥1
   independent approval + no open blocking critiques). The human flipping `status: accepted`
   and committing *is* the merge gesture — the server observes that commit and records the
   `MERGED` event against its `commit_sha`. The linter fails on `accepted`-without-approval
   (the real drift); it never blocks the human's merge gesture itself.

### 3. Anti-divergence: physical gates, not prompt temperature

The session's biggest correction (Unified 2.1): **agents cannot self-adjust inference
temperature, and clients ignore the sampling temperature.** Every "simulated annealing via
prompt" scheme in the Codex/Google/Meta drafts is therefore unenforceable and is dropped.

Replace it with **hard gates the server enforces on the diff**, parameterized by revision
round `n` and configurable per task — not the hardcoded "15 lines / 2 files" constants from
Unified 2.1. **These AST/line-budget gates apply only to the `code_change` artifact profile** —
they are meaningless on prose and are disabled for `design_doc` (forbidding "new function
defs" in a Markdown file is nonsense). See "Artifact Profiles & Usage Modes."

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

The **detection → escalation transition is code**: when a plateau or edit-war condition trips,
the server moves the proposal to `ESCALATED`, locks further commits on the branch, and posts a
task to a third agent (or flags Roberto). What stays a **skill** is only what the third agent
*does* with the frozen pair — produce the minimal-compromise diff. The reducer owns the state
transition; the skill owns the resolution.

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

## Implementation Split: Code vs. Skill/Rule

CCREP's **code surface is small, and it is exactly the set of invariants agents cannot be
trusted to self-police.** Everything that is judgment lives in the existing skills
(`bot-collab`, `how-to-experiment`, `write-docs`) and in rules — not in new services. State
this explicitly so nobody builds, e.g., an "admissibility classifier" microservice.

| Concern | Where it lives | Why |
|---|---|---|
| Append-only `event_log` + materialized views (proposals, critiques, votes, merge_records) | **Code** (MCP server) | Storage must be deterministic; this is the blackboard. |
| Reduce `ConsensusState`; *no self-approval*; *votes expire on new commit*; *N independent approvals* | **Code** (reducer) | The ratchet invariants. If an agent can satisfy them by asserting them, the ratchet is fake. |
| Resolve branch → immutable `commit_sha`; content-addressed eval cache `(commit_sha, eval_suite_hash, dataset_hash, env_hash)` | **Code** | Reproducibility primitive; pure mechanism. |
| Worktree lifecycle + run the eval suite → `EvaluationReport` | **Code** (executor) | *Which* suite runs is task/profile config, not server logic. |
| Physical revision gates (tree-sitter AST checks, `max_files` / `max_changed_lines`) | **Code** (`code_change` profile only) | "Physical gates on the diff" only mean anything if the server enforces them. |
| Critique **structure** (severity class, evidence link, `file:line` present) | **Code** (schema validation) | Presence of fields is mechanically checkable. |
| Critique **evidence-link validity** (the `file:line` resolves at the proposed `commit_sha`) | **Code** (executor) | A dead link is *malformed*; reject pre-review. Not judgment. |
| Critique **quality** / admissibility ("specific + actionable"; is the point substantive) | **Skill/Rule** (`bot-collab`) | Reviewer judgment. The schema demands a field; only a reviewer judges substance. |
| Plateau / edit-war **detection → `ESCALATED` transition** (lock commits, post to a third agent) | **Code** (reducer) | The state transition is mechanical and must be unbypassable. |
| Edit-war **resolution** (the minimal-compromise diff the third agent produces) | **Skill** | This is the judgment the escalation hands off. |
| Provenance: **detect** unreferenced numeric claims (warn, don't fail) | **Code** (linter, warning only) | A regex can flag `0.92` but cannot tell a real eval number from a confabulated `99.27%`. |
| Provenance: **adjudicate** flagged claims (sourced vs. speculative) | **Rule + admissible critique** | Judgment; a hard fail would train us to fake a source or disable the check. |
| Reviewer independence beyond `author != reviewer` (e.g. *different model family*) | **Code** enforces the checkable part; **Rule** for assignment | Model-family is a field; who-reviews-what is social. |
| Frontmatter `status` ↔ ledger state, one-directional | **Code** (linter) | Prevents `accepted`-without-approval drift; the human's merge gesture is never blocked. |
| Human-gate categories (public API, destructive migration, model/dataset, large arch) | **Code** enforces the block; **Rule** defines what counts | The block must be unbypassable; classification is judgment. |

Net buildable Phase 1: ~7 MCP tools + a reducer + a worktree executor. Everything softer is
*taught*, not coded.

**Two false-positive guardrails (load-bearing).** Both the provenance linter and the
frontmatter check are one regex away from a machine that annoys us into turning it off, so each
is split detect-vs-adjudicate:

- **Provenance linter WARNS, it does not FAIL.** It lists each unreferenced numeric claim and
  points a reviewer at it; the provenance *rule* + an admissible critique are what block. The
  fabricated numbers still get caught — by a reviewer the linter pointed at, not by auto-reject.
- **Frontmatter invariant is one-directional.** It fails only on `status: accepted` without a
  reached `APPROVED` state; it never blocks the human committing the flip that *is* the merge.

## Artifact Profiles & Usage Modes

The original draft's worked example (a boundary-threshold tweak with `cargo test` + golden
metrics) silently assumes **every proposal has a runnable eval suite.** A design doc has no
`cargo test`; you cannot run a golden-metric regression on Markdown. So CCREP is made
**artifact-type-aware**: the *server and ledger stay generic*, and each task declares an
`artifact_profile` that selects which gate components apply. This is the abstraction that lets
one protocol serve all three of Roberto's usage modes without three separate flows.

| Profile | Usage mode | Automated gate ("eval suite") | Peer gate | Human gate |
|---|---|---|---|---|
| `code_change` | **Independent development** | build + test + lint + fmt; no golden-metric regression; AST/line revision gates active | 1 implementation approval + 1 independent (different-family) approval; no open blocking critiques | public-API / destructive migration / model-or-dataset change / large arch |
| `code_review` | **Code reviews** (an existing/external diff) | build + test on the PR head (no metric gate unless the diff touches the pipeline) | ≥1 admissible structured critique + 1 independent approval | merge stays human — the value is a *verdict*, not auto-merge |
| `design_doc` | **Design docs** | `lint_collab.py` + link-check + skill-index consistency + provenance **warnings**; **no metric gate; AST/line gates disabled** | 1 independent approval + admissible critiques + the **provenance rule** | **always human** (`status: accepted` is the merge) |

Concretely, per mode:

- **Independent development** — CCREP is the eval+review harness, unchanged from Phase 1: propose
  on a branch → server evals in a worktree → a different-family agent critiques → gate = green
  checks + 1 independent approval.
- **Code reviews** — same machinery; the "eval suite" is build/test on the head and the
  deliverable is the admissible, evidence-linked `Critique` set plus an approve/block verdict.
  The ratchet value is the *structured critique*, not an automatic merge.
- **Design docs** — same machinery; the "eval suite" is the doc linters, and the gate rests on
  admissible critiques + provenance + human sign-off, with no metric math. `write-docs` already
  owns doc quality; CCREP adds the evidence ledger and the lifecycle reducer around it. The doc
  `status:` frontmatter *is* the consensus state: `need-human-review` ≈ `APPROVED`-pending-human,
  `accepted` ≈ `MERGED`, `superseded` ≈ a merge that retired prior values. (This very doc has
  drifted — `accepted` in frontmatter, `need-human-review` in the body — which is exactly the
  mismatch the one-directional invariant above is designed to fail on.)

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
above). `submit_proposal` carries an `artifact_profile` field (`code_change` | `code_review` |
`design_doc`) that selects the gate components, eval suite, and which revision gates apply.
Resources: `ccrep://tasks/{id}`, `ccrep://proposals/{id}/{diff,evaluation,critiques}`,
`ccrep://tasks/{id}/consensus`. Worktree lifecycle: `git worktree add --detach
.ccrep/worktrees/{proposal_id} {commit_sha}` → run suite → `git worktree remove --force`.

## Validation Plan

1. **Phase 1 — Evidence Ledger (only this is committed-to).** `submit_proposal` →
   worktree eval → `EvaluationReport` → structured `Critique` log, with the gate reduced to
   *green automated checks + one independent approval*. No voting math, no annealing, no
   weighting. Dogfood it on one real Deep Cuts task (e.g. a boundary-threshold tweak) and
   measure: did it catch a regression a solo agent missed, and what did it cost in tokens
   and wall-clock vs. just doing the change? Phase 1 also validates the **`design_doc`
   profile** — the doc-linter eval suite (incl. provenance warnings) and profile-switching —
   by dogfooding it on a real doc change (this amendment is a candidate), since the design-doc
   path needs no metric infrastructure and exercises the generic ledger end-to-end.
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

## Implemented Outcome

**Phase 1 (Evidence Ledger) is implemented** in [`tools/ccrep/`](../../tools/ccrep/) — a Python
package mirroring `tools/collab_mcp/`: a pure-stdlib, MCP-independent core + a thin `FastMCP`
wrapper. It maps to this design as written, with one deviation noted below.

- **Storage** (`ledger.py`): append-only `event_log` as the source of truth (SQLite + WAL,
  stdlib `sqlite3` — no WSL2/BTRFS machinery); content-addressed `eval_cache` keyed
  `(commit_sha, eval_suite_hash, dataset_hash, env_hash)`; derived tables written only by a
  `materialize()` step.
- **Reducer** (`reducer.py`): `reduce_task()` folds one task's log into a derived
  `ConsensusState`, enforcing invariants 1–7 in code (self-approval skipped from quorum,
  approvals bound to the pinned `commit_sha`, any attempt to inject `ConsensusState` rejected,
  out-of-profile checks dropped, one-directional frontmatter flag).
- **Executor** (`executor.py`): `git worktree add --detach` → run the profile suite →
  `git worktree remove --force` (cleanup in `finally`); design-doc linter incl. provenance
  **warnings**; critique evidence-link resolution (`file:line` must resolve at the proposed
  commit, via `git cat-file`).
- **Profiles** (`profiles.py`): `code_change` | `code_review` | `design_doc` select gate
  components + a default (overridable) eval suite.
- **MCP surface** (`server.py`): the seven Phase-1 tools; `submit_proposal` carries
  `artifact_profile`; `merge_proposal` is human-gated for the four sensitive categories.
- **Gate**: green automated checks + one independent approval (`reviewer != author`) + no open
  blocking critiques.
- **Deviation from spec**: a content-addressed cache *hit* re-records an identical `report_id`;
  the snapshot builder dedupes reports/critiques by id so a full re-fold doesn't violate the
  `UNIQUE` constraint. This dedup rule is the one decision not spelled out in the design.
- **Out of scope (Phases 2–4), not built**: AST/line revision gates, plateau/edit-war detection
  + `ESCALATED` transition, all voting math (Kendall's W, Friedman, Schulze/Condorcet, log-odds),
  weighted quorum, CIG routing, Pareto gating. `profiles.py` keeps an empty `revision_gates`
  slot as a forward-compatible placeholder only.
- **Launch**: console script `ccrep = "ccrep.server:main"` in `tools/pyproject.toml`, registered
  in `.mcp.json` alongside `collab`; run `tools/.venv/bin/pip install -e tools/` to materialize
  the script. Tests: `PYTHONPATH=tools tools/.venv/bin/python -m pytest tools/ccrep/` (37 passing).
- **The judgment half lives in skills**: [`skills/ccrep/SKILL.md`](../../skills/ccrep/SKILL.md)
  (the operational loop + critique admissibility + reviewer independence), with pointers from
  `bot-collab` and the provenance/status-lifecycle rules in `write-docs`.

## Decision Log

- **2026-06-08** — Synthesis authored (Claude). Codex chosen as spine; Unified 2.1 physical
  gates adopted over prompt-temperature; Kendall's W / Schulze / log-odds weighting deferred
  to Phase 4; left at `need-human-review` pending Roberto's read-through.
- **2026-06-08** — Implementation-split amendment (Claude + Antigravity collab,
  [session](../collab/sessions/2026-06-08-ccrep-implementation-split/session.md)). Added the
  *Implementation Split: Code vs. Skill/Rule* and *Artifact Profiles & Usage Modes* sections;
  added invariants 6 (artifact-profile consistency) and 7 (one-directional frontmatter-status
  sync); scoped AST/line revision gates to `code_change`; made plateau/edit-war escalation a
  code-owned state transition; made the provenance check a warning, not a hard fail. Consensus:
  code owns invariants, skills own judgment; one generic ledger serves all three usage modes
  via `artifact_profile`.
- **2026-06-08** — Phase 1 implemented in `tools/ccrep/` (worktree agent + Claude verify).
  Reducer enforces invariants 1–7; 37 tests passing. Operational `ccrep` skill added and
  `bot-collab` / `write-docs` updated to carry the judgment half. See "Implemented Outcome".

## Deferred Ideas (kept for when scale justifies them)

- **Condorcet/Schulze branch ranking** — only meaningful with ≥3 competing branches.
- **Kendall's W concordance + Friedman significance** — needs many rankers to be non-noise.
- **Bayes-optimal log-odds vote weighting** — needs a verdict-accuracy history; revisit once
  the event log has accumulated labeled outcomes.
- **Causal Interaction Graph reviewer routing** (Google) — a real optimization once review
  fan-out is large enough to matter.
- **Multi-Objective Optimization Vector / Pareto gating + elite cross-pollination** (Unified)
  — defer until single golden-metric gating proves insufficient.
