## [Claude, 2026-06-08T11:45:00+02:00]

# CCREP Review — knowledge-manager-design-review (Reviewer 1: Claude)

Role: **Reviewer 1** (actor `claude`, Anthropic). Proposer: `codex`. Reviewer 2: `agy`.
Targets: `doc/proposals/codebase-knowledge-manager.md`, `doc/proposals/bot-knowledge-discovery.md`.

## Blocker found before review could start: CCREP ledger DB mismatch

- Codex's proposal `20f5748d39674222b36095a8c4115dc1` was written to
  `/Users/rlupi/src/deep-cuts/scratch/ccrep/ccrep.db`.
- The canonical launcher `tools/run_ccrep_mcp.py` sets `CCREP_DB` to
  `<git-common-dir parent>/scratch/ccrep.db` (i.e. `/Users/rlupi/src/deep-cuts/scratch/ccrep.db`).
- My ccrep MCP server reads the canonical DB, so `run_evaluation` / `compute_consensus`
  return "unknown proposal". Verified: `collab-skill-monitoring` (canonical DB) IS visible.
- Likely root cause: `skills/ccrep/SKILL.md` documents the old default
  `CCREP_DB=scratch/ccrep/ccrep.db`; codex used that override instead of the launcher.
- **Action:** asked codex (collab `blocked` msg) to re-submit against the canonical ledger and
  resend the new `proposal_id`. Commit `417529e` is fine — both targets resolve there.

## Prepared critique set (to file once the proposal is on the canonical ledger)

Verified against the repo at `417529e`:

- **C1 (blocking) — the two docs specify mutually contradictory embedding/runtime stacks.**
  `codebase-knowledge-manager.md` uses all-MiniLM-L6-v2 ONNX + `sqlite-vec` (`:119`);
  `bot-knowledge-discovery.md` uses Ollama @ `localhost:11434` (`:97`, `:136`). Same system,
  two runtimes — implementer can't tell which is canonical. The repo already ships a local
  all-MiniLM ONNX path (`src-tauri/src/embeddings.rs`, `tools/export_sentence_onnx.py`), so
  Ollama adds a new daemon duplicating existing capability. Resolution: pick one backend across
  both docs; if Ollama, justify the new dependency vs. the existing ONNX pipeline.

- **C2 (blocking) — new Go toolchain introduced without justification, contradicting the
  companion doc.** `bot-knowledge-discovery.md:99,:117` proposes a Go binary
  (`dc-knowledge-mgr`, `mark3labs/mcp-go`). No Go exists in the repo; `tools/collab_mcp` and
  `tools/ccrep` are Python MCP servers already wired into `.mcp.json`. The companion
  `codebase-knowledge-manager.md` uses Python (`tools/*.py`). Resolution: justify Go over
  extending the existing Python MCP stack, or align both docs on one language.

- **C3 (advisory) — invalid sqlite-vec DDL.** `codebase-knowledge-manager.md:122` declares
  `embedding_vector F32_VEC(384)` in a plain `CREATE TABLE`. Repo convention is
  `CREATE VIRTUAL TABLE ... USING vec0(...)` (`src-tauri/migrations/05_audio_embeddings.sql:1`,
  `11_description_embeddings.sql:1`). `F32_VEC` is not valid. Use the `vec0` form.

- **C4 (advisory) — Mangle/Datalog unjustified vs. plain SQL.** `:212-219` rejects DDlog/Neo4j
  but never compares Mangle to plain SQLite anti-joins, which the repo already uses. Rules 1-3
  (`:138-175`) are non-recursive and expressible as SQL `NOT EXISTS`. Mangle adds a Go runtime +
  a second query language. Resolution: add a "why not SQL" comparison or drop Mangle.

- **C5 (advisory) — pre-commit gate assumes absent infra + worktree interaction unspecified.**
  Both docs hinge on a pre-commit linter (`codebase-knowledge-manager.md:56`,
  `bot-knowledge-discovery.md:123`). No hook exists today (no `.git/hooks/pre-commit`, no
  `.pre-commit-config.yaml`, no `core.hooksPath`). Git hooks live in the shared common dir, so a
  hook applies across all worktrees — interacts with the Solo/Parallel design (`:179-194`).
  Specify install mechanism + worktree behavior.

- **C6 (advisory, provenance) — unsourced perf claim.**
  `codebase-knowledge-manager.md:215` "a simple interpreter run takes less than 15ms for a
  codebase of this size" — no benchmark; it's the load-bearing rationale for rejecting DDlog.
  Mark speculative or cite a measurement.

Domain split agreed with agy: I own architecture/feasibility + provenance (C1-C4, C6); agy owns
integration (CCREP/collab/MCP) + ops/security (C5 and beyond).

## [Claude, 2026-06-08T11:50:00+02:00] — Round 1 critique filed

- Re-submitted proposal `acae48be590248abb4e02f636e55564f` (canonical ledger). `run_evaluation`
  **passed** (lint_collab ✓, provenance ✓, frontmatter ✓).
- Filed one `request_changes` critique (`claude-kmgr-rev1`) with 6 findings: **C1 + C2 blocking**,
  C3-C6 advisory (as prepared above).
- Consensus → **`revision_required`**; `open_blocking_findings = [C1-stack-contradiction,
  C2-go-toolchain-unjustified]`; `next_actions = [submit_revision]`.
- Note: **agy had already cast `approve`** before I filed. Their approval is now outvoted by the
  open blockers; it will also auto-expire on codex's next revision. Flagged agy to re-review the
  revision rather than rely on the stale approval.
- Handed back to codex (collab `handoff`) to `submit_revision` resolving C1/C2 (and ideally
  C3-C6). Will re-evaluate + approve once both blockers are closed.

## [Claude, 2026-06-08T11:57:00+02:00] — Round 2: revision verified, APPROVED

- Codex submitted revision 1: proposal `847b5884d4164db5a70beb6808cb9632`, commit `778eab8`.
- `run_evaluation` **passed** (lint_collab ✓, provenance ✓, frontmatter ✓).
- Verified the actual diff `417529e..778eab8` (not just codex's claim) — **all six findings
  genuinely resolved**:
  - C1 → both docs align on Python + ONNX all-MiniLM + sqlite-vec; Ollama removed, explicitly
    reuses `src-tauri/src/embeddings.rs`.
  - C2 → Go binary replaced by `tools/knowledge_mgr.py` following the `collab_mcp`/`ccrep`
    Python MCP pattern; `mark3labs/mcp-go` removed.
  - C3 → `CREATE VIRTUAL TABLE node_embeddings USING vec0(... FLOAT[384])` per migrations 05/11.
  - C4 → Phase 1 = SQL anti-joins; Mangle demoted to optional follow-up; Rejected Alt #4 added.
  - C5 → new "Hook Installation and Worktree Isolation" section (opt-in hook, `--root "$PWD"`,
    Solo-mode worktree isolation). [agy's lane — agy also approved it.]
  - C6 → unsourced 15ms figure removed, replaced with a verify-before-requirement caveat.
- Filed `approve` critique (`claude-kmgr-approve-r1`). Consensus → **`consensus_ready`**:
  2 independent approvals (agy + claude), green checks, `open_blocking_findings = []`,
  `next_actions = [merge_proposal]`.
- Handed off to codex to run `merge_proposal` (proposer owns merge). My review is complete.
