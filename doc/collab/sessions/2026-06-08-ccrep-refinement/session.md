# Session: CCREP Refinement — Implementation, Smoke Testing, and First Dogfood

## Participants
* Roberto (Human) — direction, decisions, restarts, mediation
* Claude (AI) — actor `claude` — implementation lead, verify + merge, reviewer
* Antigravity (AI) — actor `agy` — peer reviewer / proposer

## Overview

This session took CCREP from an accepted design to a **working, dogfooded Phase 1**. It builds on the
[implementation-split amendment](../2026-06-08-ccrep-implementation-split/session.md) of
[`doc/proposals/ccrep-synthesis.md`](../../../proposals/ccrep-synthesis.md). Across the session we:
implemented Phase 1, hardened it with a solo smoke test (which found and fixed two real bugs), ran a
live two-agent loop in both directions, and then put a *genuinely contested* change through the full
quality ratchet (block → revise → re-review → approve → merge). It ends with an open design
discussion about multi-agent repo topology.

The headline: **the ratchet works, and it earned its keep** — it stopped an eager agent from landing
a so-so change and produced a concrete, evidence-linked path to a better one.

## Phases

### 1. Phase 1 implementation
A worktree subagent implemented the CCREP Phase 1 "evidence ledger" as `tools/ccrep/` (Python,
mirroring `tools/collab_mcp/`): append-only SQLite `event_log` → reducer that folds it into
`ConsensusState` enforcing invariants 1–7 → git-worktree eval executor → 7-tool FastMCP server, with
three artifact profiles (`code_change` / `code_review` / `design_doc`). The main session verified
(independent `pytest`, invariant spot-checks), wired it to the repo's console-script convention
(`tools/pyproject.toml`, `.mcp.json`), and added the *judgment half* in skills:
`skills/ccrep/SKILL.md` (the operational loop + critique admissibility + reviewer independence), a
`bot-collab` pointer (coordination transport vs. quality ratchet), and the provenance + status↔
consensus rules in `write-docs`. Recorded as the "Implemented Outcome" in the proposal.

Landed: `069bc0d feat(ccrep): implement Phase 1 evidence ledger`,
`a1205d8 docs(ccrep): add operational skill and provenance/status rules`.

### 2. Merge mechanics + a hook fix
Main moved twice mid-session; each time the branch was **rebased onto main before a `--ff-only`
merge** ("rebase first, keep main green"). The repo's `pre-commit` hook used a *relative*
`tools/.venv/bin/python`, which silently no-op'd inside worktrees (they have no `.venv`). Fixed it to
resolve the interpreter from the main checkout via `GIT_COMMON_DIR`:
`ROOT=$(cd "$(git rev-parse --git-common-dir)/.." && pwd)`, with a `python3` fallback. (Local to
`.git/hooks/`; a tracked `core.hooksPath` version was deferred.)

### 3. Stage A — solo smoke test → two real bugs
Before involving agy, the store was driven directly against the *real* repo. The happy path,
no-self-approval, dead-evidence-link rejection, and vote-expiry all passed — but the run surfaced
**two integration bugs the unit tests missed** (they tested the reducer dict directly and used
distinct commits per test, so they never crossed these seams):

1. **Stale cached `proposal_id`.** A content-addressed eval cache hit returned a report still stamped
   with the proposal that first ran it; appending it under a second proposal on the same commit made
   `reduce_task` raise `evaluation for unknown proposal`. Fix: `run_evaluation` rebinds
   `proposal_id`/`report_id` to the requesting proposal (the cached *result* stays reusable).
2. **`ConsensusState` schema rejected `human_gate_categories`.** The reducer emits it (and
   `merge_proposal` reads it) but the schema was `additionalProperties:false` without it, so
   `compute_consensus` threw `SchemaError` for *any* human-gated proposal that reached approval —
   the entire sensitive-change → human-review path was broken at runtime. Fix: add the field to the
   schema.

Both fixed with regressions that exercise the previously-uncovered paths (`compute_consensus` on a
human-gated proposal; a cross-proposal cache hit). Landed:
`6f31374 fix(ccrep): rebind cached eval reports and add human_gate_categories to schema`,
`6af1d59 chore(ccrep): pin shared ledger path and ignore eval worktrees`.

### 4. Stage B — live two-agent loop, both directions
After pinning shared state (`CCREP_DB=scratch/ccrep.db`, `CCREP_REPO_ROOT=.`) and restarting both
sessions to load the `ccrep` MCP:
- **claude → agy** (task `stageb-readme`): claude proposed a `design_doc` edit, eval passed in a
  disposable worktree, agy gave the independent approval (weight 1.0, `reviewer != author`) → merged.
- **agy → claude** (task `throwaway-test-task`, role swap): agy proposed, claude *actually reviewed
  the diff* and approved **with an admissible advisory finding** (`status: proposed` is off the
  write-docs vocabulary) → merged.

A clean demonstration of the code/rule split: the automated eval (code) passed the doc, but the
*reviewer* (judgment) caught the lifecycle-vocab issue.

### 5. First real dogfood — agy's bot-collab change through the full ratchet
agy proposed adding a "Post-Handoff Monitoring" step to `skills/bot-collab/SKILL.md` telling agents
to spawn a background poller after handoff. Roberto and Claude agreed it identified a *real* problem
(agents idling after handoff) but prescribed the wrong fix, and chose to route it through CCREP
rather than edit it inline — explicitly to dogfood the revision loop.

Task `collab-skill-monitoring`:
- agy proposed (eager — it asked Claude to "approve and merge").
- Claude **declined to rubber-stamp** and filed an **admissible blocking critique**
  (`request_changes`, evidence-linked to `skills/bot-collab/SKILL.md:47`): the poller is redundant
  with the already-documented zero-cost blocking `collab/recv`, and reintroduces polling the
  maildir/doorbell design avoids. Gate → `revision_required`.
- agy **revised** to a harness-conditional version (blocking `recv` as default; poller demoted to an
  explicit "if your harness cannot block on a tool call" fallback). New commit expired the prior
  vote; eval re-ran green.
- Claude re-reviewed, the finding was resolved, **approved** (one non-blocking indentation nit) →
  `consensus_ready` → merged in the ledger.

This is the part Stage B skipped, and it is the strongest evidence for CCREP: an eager agent's
mediocre change was held by the gate and improved through a recorded, evidence-linked exchange.

### 6. Topology discussion (open)
Two recurring frictions motivated a design discussion:
- **agy repeatedly moved the shared `main` HEAD** by running `git checkout -b` in the shared working
  tree (Claude restored `main` each time).
- **`merge_proposal` records a MERGED value in the ledger; it does *not* git-merge to `main`.** So
  every dogfood proposal is "CCREP-merged" but still lives on its branch.

Roberto noted the harnesses can't be changed. Consensus direction (not yet executed): give each agent
its **own working tree** so the failure becomes *structurally* impossible — git refuses to check out
the same branch in two worktrees, so with `main` in the canonical tree neither agent's tree can be on
`main`, regardless of harness behavior. Requires making `CCREP_DB`/`CCREP_REPO_ROOT` **absolute
shared paths** (a relative `scratch/` path would give each tree a private ledger). Heavier hard-
enforcement (separate clones + a bare canonical repo with a `pre-receive` hook rejecting `main`
pushes) and `jj` (colocated; `jj undo` / auto-snapshot as a recovery net) were noted as alternatives.
## Antigravity (Gemini) Perspective & Execution Notes

From my end, driving the loop highlighted several operational realities of the two-agent CCREP system:

1. **Bypassing Dynamic MCP Limitations**: In my harness, registered MCP tools are sometimes not exposed as first-class tool declarations. To interact with the CCREP store and the collaboration inbox, I wrote Python scripts that called the `CcrepStore` APIs and invoked the `collab_mcp_cli.py` CLI wrapper directly. This demonstrates that CCREP is robust and remains operable even under varying agent harness capabilities.
2. **Git Worktree Isolation**: During the first pass, executing `git checkout -b` directly in the shared main repository tree moved the canonical `HEAD`, which interfered with Claude's workspace state. In the revision pass, using a dedicated worktree (`collab-skill-refinement-rev2` located in the sandboxed `<appDataDir>/worktrees/`) completely isolated my edits, ensuring the main branch remained clean while still allowing CCREP to build and evaluate the commit correctly.
3. **Consensus Validation**: The CCREP ledger successfully and strictly enforced peer review rules. For example, my attempt to propose the `collab-skill-monitoring` task initially stayed in a `reviewing` state (non-mergeable due to missing independent approval), and when Claude submitted the `request_changes` critique with a blocking finding, the ledger immediately updated the consensus state to `revision_required`. Submitting the revision correctly expired Claude's previous block, allowing a clean re-review.

## What CCREP proved in practice
- The full loop works live across two heterogeneous agents on a shared SQLite ledger: propose →
  worktree eval → independent critique → consensus gate → merge.
- The gate **blocks on substance**: a `request_changes` with one admissible blocking finding holds a
  merge, and an eager proposer cannot self-clear it.
- **Votes expire on revision**; a new commit forces fresh eval + fresh approval.
- The **code/rule split is real and useful**: automated checks (`lint_collab`, provenance,
  frontmatter) cannot judge prose quality — the value on doc/skill changes is the *structured,
  evidence-linked peer review*, not the green eval.

## Open items / next steps
- **Land the approved changes to git `main`** (CCREP-merged ≠ landed): `collab-skill-monitoring`
  (commit `6695092c`, branch `collab-skill-refinement`) — apply the 1-space dedent nit; and
  `ccrep-stageb` (README "Smoke testing" section) if wanted.
- **Multi-agent topology**: set up per-agent worktrees + absolute `CCREP_DB`/`CCREP_REPO_ROOT`;
  optionally pilot `jj` colocated and validate the executor resolves a `jj`-authored commit.
- **Sharpen the `design_doc` eval suite**: it is collab-session-centric (`lint_collab`); for
  `skills/**` changes it should run `generate_skill_index.py --check` + link-check. A good future
  CCREP `code_change`.
- **Bake "author proposals in a separate worktree" into `skills/ccrep/SKILL.md`** so it stops biting.
- Consider whether `merge_proposal` should optionally perform the git land, or whether landing stays
  a deliberate human gesture (current behavior; arguably correct for protected paths).

## Decision log
- **2026-06-08** — Phase 1 implemented, smoke-tested, and dogfooded. Two seam bugs found by the solo
  smoke test and fixed with regressions. Two-agent loop verified both directions; the revision loop
  verified on a contested `bot-collab` change. Topology (per-agent worktrees vs. clones vs. `jj`)
  discussed; decision pending. CCREP-merged proposals not yet landed on git `main`.
