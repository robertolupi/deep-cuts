# ccrep — CCREP Phase 1 Evidence Ledger

Implements **Phase 1 only** of the CCREP synthesis
([../../doc/proposals/ccrep-synthesis.md](../../doc/proposals/ccrep-synthesis.md)):
an append-only event ledger, a reducer that folds it into `ConsensusState` while
enforcing the seven ratchet invariants, a git-worktree eval executor, and the
seven-tool MCP surface. Phases 2-4 (AST/line revision gates, plateau/edit-war
detection, any voting math, weighted quorum, Condorcet) are explicitly **out of
scope** and not built.

Same layout convention as `collab_mcp`: a pure-stdlib, MCP-independent,
unit-testable core + a thin `FastMCP` wrapper.

## Modules
- `schemas.py` — the Draft 2020-12 JSON Schemas for `Proposal`,
  `EvaluationReport`, `Critique`, `ConsensusState` (verbatim from the Codex
  design, plus the additive `artifact_profile` field on `Proposal`) and
  `validate()` helpers. No MCP dependency.
- `profiles.py` — the three artifact profiles (`code_change`, `code_review`,
  `design_doc`): which gate components + default eval suite each selects. The
  eval-suite commands are config (overridable per task); only the dispatch is
  code.
- `ledger.py` — `Ledger`: the append-only `event_log` (source of truth), the
  content-addressed `eval_cache` keyed `(commit_sha, suite_hash, dataset_hash,
  env_hash)`, and the derived/materialized tables. Standard `sqlite3` + WAL.
- `reducer.py` — `reduce_task()`: folds one task's events into a derived
  `ConsensusState`, enforcing invariants 1-7. **This is the package's reason to
  exist** — the ratchet invariants live in code so an agent cannot satisfy them
  by asserting them.
- `executor.py` — `WorktreeExecutor`: `git worktree add --detach` → run the
  profile suite → `git worktree remove --force` (robust cleanup); design-doc
  linter checks (incl. provenance **warnings**); and critique evidence-link
  resolution (each `file:line` must resolve at the proposed commit).
- `store.py` — `CcrepStore`: the MCP-independent operation surface composing
  ledger + reducer + executor. Every mutating op appends an event then re-folds.
- `server.py` — thin `FastMCP` wrapper exposing the seven tools.
- `__main__.py` — `python -m ccrep` runs the stdio server.

## Invariants (enforced in code)
1. **Immutable proposal version** — a proposal pins a fixed `commit_sha`; a new
   commit is a new revision (`submit_revision` mints a superseding proposal).
2. **Content-addressed evaluations** — `eval_cache` keyed by the 4-tuple; an
   unchanged-input eval is served from cache, never re-run.
3. **Votes expire on code change** — approvals are bound to a proposal's pinned
   commit; a revision's vote set starts empty, so prior approvals don't carry.
4. **No self-approval** — the author's own `approve` never satisfies the peer
   quorum (rejected at submit time *and* zero-weighted in the reduction).
5. **Derived consensus state** — there is no event that writes `ConsensusState`;
   the reducer rejects any attempt to inject it. Derived tables are a pure
   function of the (immutable) log.
6. **Artifact-profile consistency** — a hard-check / metric tagged to a gate
   component the proposal's profile does not own is dropped (never fires).
7. **One-directional frontmatter-status sync** — `design_doc` status `accepted`
   without a reached APPROVED state is flagged/failed; the human's merge gesture
   is never blocked.

**Phase-1 gate** = green automated checks + one independent approval
(`reviewer != author`) + no open blocking critiques. `merge_proposal` is
human-gated: it refuses/flags for `public_api_change`, `destructive_migration`,
`model_or_dataset_change`, `large_architecture_change`.

## Test
```bash
cd tools && PYTHONPATH=. .venv/bin/python -m pytest ccrep/ -x -q
```
The reducer tests (`test_reducer.py`) need no git or MCP runtime; `test_store.py`
spins up throwaway git repos to exercise the worktree executor, eval cache, and
evidence-link resolution.

## Register as an MCP server
Added to `.mcp.json` **alongside** the existing `collab` server (not replacing
it). Grant `mcp__ccrep__*` once:
```json
{ "mcpServers": { "ccrep": {
    "command": "tools/.venv/bin/python",
    "args": ["-m", "ccrep"],
    "env": { "PYTHONPATH": "tools" } } } }
```
A `tools/run_ccrep_mcp.py` wrapper mirrors `tools/run_collab_mcp.py`.

Environment:
- `CCREP_REPO_ROOT` — git repo evaluated in worktrees (default `.`)
- `CCREP_DB` — ledger path (default `scratch/ccrep/ccrep.db`, gitignored)
- `CCREP_ENV` — environment descriptor folded into the eval-cache key

## Tools (Phase-1 subset)
`claim_task` · `submit_proposal` (carries `artifact_profile`) · `run_evaluation`
· `submit_critique` · `submit_revision` · `compute_consensus` · `merge_proposal`
(human-gated) — see `server.py` docstrings.
