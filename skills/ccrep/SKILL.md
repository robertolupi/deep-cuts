---
name: ccrep
description: Run the CCREP quality-ratchet loop — submit a code change, code review, or design doc as a proposal, evaluate it in a worktree, gather admissible peer critiques, and merge only when the consensus gate passes. Use when an artifact should be peer-evaluated and improved until provably better (not merely coordinated), or when the user says "CCREP", "quality ratchet", "submit a proposal", "run the evaluation", or asks for a gated multi-agent review/merge.
---

# CCREP — Quality-Ratchet Coordination

CCREP turns a proposed artifact into an immutable **value** that accumulates reproducible
evidence, independent approval, and unresolved-risk closure before it becomes the next accepted
state. The design and rationale live in
[doc/proposals/ccrep-synthesis.md](../../doc/proposals/ccrep-synthesis.md); the code is
[tools/ccrep/](../../tools/ccrep/) (see its `README.md`). **This skill is how an agent operates
the loop** — the judgment the server cannot enforce.

> **CCREP vs. the `collab` MCP.** They are different layers. `collab` (see
> [bot-collab](../bot-collab/SKILL.md)) is the *coordination transport* — mailboxes, handoffs, a
> task queue for turn-taking. **CCREP is the quality ratchet on one specific artifact**: eval +
> structured critique + a gated merge. Use `collab` to talk; use CCREP when an artifact must be
> *proven better*, not just agreed on. They compose — coordinate over `collab`, ratchet with CCREP.

## When to use it

Reach for CCREP when a change is worth more than one agent's say-so: a non-trivial code change, a
review of an existing/external diff, or a design doc that should not land on a single author's
judgment. For a throwaway edit, it is overkill — the per-change cost (full eval + peers reading the
diff + rounds) is real.

## Pick an artifact profile

`submit_proposal` carries `artifact_profile`, which selects the gate:

| Profile | Use for | Automated gate (eval suite) |
|---|---|---|
| `code_change` | independent development | build + test + lint + fmt; no golden-metric regression |
| `code_review` | reviewing an existing/external diff | build + test on the head; the deliverable is the critique set + verdict |
| `design_doc` | proposals & docs (no `cargo test`) | `lint_collab.py` + link-check + skill-index consistency + provenance **warnings**; no metric/AST gates |

## The loop

```
claim_task → submit_proposal → run_evaluation → submit_critique
                  │                                    │
                  └──────── submit_revision ◀──────────┘   (until clean)
                                   ↓
                        compute_consensus → merge_proposal
```

| Need | Tool |
|---|---|
| Take a task | `claim_task` |
| Propose changes on a branch (resolved to an immutable `commit_sha`) | `submit_proposal` (set `artifact_profile`) |
| Run the profile's eval suite in an isolated worktree → `EvaluationReport` | `run_evaluation` |
| File a structured finding against the exact commit | `submit_critique` |
| Address blocking findings on a new commit (invalidates prior approvals) | `submit_revision` |
| Read the derived gate state | `compute_consensus` |
| Merge (human-gated for sensitive categories) | `merge_proposal` |

## The gate (Phase 1)

A proposal merges only when **all** hold:

1. **Automated checks green** — the profile's eval suite passes; no golden-metric regression (`code_change`).
2. **One independent approval** — from a reviewer who is **not** the author, **preferably a
   different model family**. The author may explain or amend but can never satisfy the quorum.
3. **No open blocking critiques.**

Any new commit expires prior approvals. Consensus state is *derived* by the server from the event
log — never assert it; produce the evidence and let the reducer compute it.

## Writing an admissible critique (the judgment half)

The schema forces structure; **only you can supply substance.** A finding **blocks merge only if it
is specific + actionable + evidence-linked + severity-classified**:

- **Evidence-linked** — cite a `file:line` (or an eval metric). The server verifies the link
  *resolves at the proposed `commit_sha`*; a dead link is rejected as malformed before review.
- **Specific + actionable** — name what is wrong and what would resolve it. "This feels too
  complex" is **inadmissible**; "`reducer.py:212` double-counts a self-approve when the author also
  reviewed — skip `agent_id == author`" is admissible.
- **Severity-classified** — only `blocking` findings gate; lower severities inform.

Inadmissible critiques don't block — so don't rely on vibes to stop a merge, and don't file noise.

## Human-gated merges

`merge_proposal` refuses (pending explicit human confirmation) for: **public API change**,
**destructive migration**, **model-or-dataset change**, **large architecture change**. Surface these
to Roberto; do not self-merge them.

## Design-doc proposals: the provenance & status rules

For `artifact_profile: design_doc`, two **rules** carry what the linter only flags (it *warns*,
never auto-rejects — see [write-docs](../write-docs/SKILL.md)):

- **Provenance** — every numeric/empirical claim must be **sourced or explicitly marked
  speculative**. The linter lists unreferenced numbers; an admissible critique is what blocks a
  confabulated one.
- **Status ↔ consensus** — the frontmatter `status` *is* the consensus state: `need-human-review` ≈
  `APPROVED`-pending-human, `accepted` ≈ `MERGED`, `superseded` ≈ a merge that retired prior values.
  The invariant is **one-directional**: a doc may not sit at `status: accepted` without a reached
  `APPROVED` state, but the human flipping the status *is* the merge gesture and is never blocked.

## Setup / launch

Registered in `.mcp.json` as the `ccrep` server (console script `ccrep`, alongside `collab`).
Materialize the script once with `tools/.venv/bin/pip install -e tools/`, then grant
`mcp__ccrep__*`. Env: `CCREP_DB` (ledger path, default `scratch/ccrep.db` resolved against the
canonical primary-worktree root — the same file every linked worktree shares; **do not** point it
at a per-worktree `scratch/ccrep/ccrep.db`, that splits the ledger so reviewers see "unknown
proposal"), `CCREP_REPO_ROOT`, `CCREP_ENV`.

## Not built yet (Phases 2–4)

Do **not** expect: AST/line revision-budget gates, plateau/edit-war auto-`ESCALATED`, any voting
math (Kendall's W, Schulze/Condorcet, log-odds weighting), weighted quorum, or reviewer routing.
These are deferred in the design; the gate is exactly the three conditions above. If a loop is
churning without converging, escalate to Roberto by hand.
