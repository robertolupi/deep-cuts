## [codex, 2026-06-08T08:42:45+02:00]

Roberto steering: Codex, Claude, and agy each proposed worktree-coordination changes after the
main-branch shared-working-tree setup caused coordination state to split. Roberto asked the agents
to use collab and CCREP to review each other's changes and reach a scheme that works.

Reviewed immutable branch heads:

- `bot/codex` at `b7e09a6` (`tooling(collab): share coordination across worktrees`)
- `bot/claude` at `86807c5` (`tooling(collab): support per-worktree coordination via DEEPCUTS_HOME`)
- `bot/agy` at `a4f192d` (`tooling(collab): configure and document worktree coordination`)

Findings:

- All three proposals correctly identify the root failure: relative MCP commands and relative
  `scratch/` state split collab mailboxes and CCREP ledgers by worktree.
- Codex's branch has the strongest split-log guidance and explicitly rejects live edits to generated
  `session.md`, but it hard-codes Roberto's path into committed `.mcp.json` and `.codex/config.toml`.
- Claude's branch has the clearest two-plane model and shared-session-log framing, but the proposed
  `${DEEPCUTS_HOME:-.}` strings depend on launcher expansion that is not guaranteed by JSON MCP
  clients.
- agy's branch captures the same shared-path problem, but its commit-before-every-handoff and
  merge-peer-branch-before-every-turn rule conflicts with the project instruction not to commit
  unless Roberto explicitly asks, and it would turn coordination into avoidable merge churn.

Implemented synthesis on `main` without committing:

- `.mcp.json` now launches `tools/run_collab_mcp.py` and `tools/run_ccrep_mcp.py`.
- The wrappers discover the canonical repo through `git rev-parse --path-format=absolute
  --git-common-dir`, then default `COLLAB_ROOT`, `CCREP_DB`, and `CCREP_REPO_ROOT` to the canonical
  repo's shared coordination plane.
- Added `doc/collab/worktree-coordination.md` as the durable scheme.
- Updated `doc/collab/PROTOCOL.md`, `doc/INDEX.md`, and `skills/bot-collab/SKILL.md` with the
  worktree-mode rules.

Consensus proposal:

- Isolated work plane: each bot edits deliverables in its own worktree and branch.
- Shared coordination plane: collab mailboxes, CCREP ledger, and live `session.<actor>.md` logs live
  in the canonical repo.
- collab MCP is for live handoffs, ACKs, task claims, and completions.
- CCREP is for immutable branch-proposal evaluation, peer critique, approval, and merge gating.
- Routine handoffs do not require commits; commit/merge happens only for deliverables and only under
  the project's normal commit rules.

CCREP-style state: `bot/codex`, `bot/claude`, and `bot/agy` were reviewed as immutable proposal
commits. Blocking critique against agy's default commit+merge handoff rule; blocking critique
against committed hard-coded absolute paths and unverified `${DEEPCUTS_HOME:-.}` launcher
interpolation. The synthesized wrapper approach addresses those blockers.

**→ Handoff:**
**Task:** Claude and agy should review the synthesized scheme and either ACK it or file a specific blocking critique.
**Context:** `doc/collab/worktree-coordination.md`, `.mcp.json`, `tools/run_collab_mcp.py`, `tools/run_ccrep_mcp.py`, `doc/collab/PROTOCOL.md`, and `skills/bot-collab/SKILL.md`.
**Deliverable:** ACK or actionable critique via collab MCP; if no peer response arrives, Roberto can treat this as Codex's proposed synthesis pending manual review.

## [codex, 2026-06-08T08:45:00+02:00]

Responding to agy's handoff:

> **Task:** Review the proposed hybrid scheme, write your turn entry to your private log file, and send the handoff back.
> **Context:** `doc/collab/worktree-coordination.md`, `skills/bot-collab/SKILL.md`
> **Deliverable:** `session.claude.md` (from Claude) / `session.codex.md` (from Codex)

ACK on the core two-plane scheme:

- Worktree branches are the isolated work plane for deliverables.
- collab MCP is the live coordination transport.
- Live split logs belong in the canonical main-tree session directory, with generated `session.md`
  compacted once at closeout.
- CCREP is the quality ratchet for concrete branch commits, not a replacement for collab messages.

Blocking critique on the config details in agy's entry:

- Committed `.mcp.json` should not depend on `${DEEPCUTS_HOME:-.}` expansion unless the MCP clients
  are verified to expand shell-style parameter syntax in JSON fields. The wrapper approach avoids
  this uncertainty and still remains portable across linked worktrees on Roberto's machine.
- `CCREP_REPO_ROOT=.` should not be the default in worktree mode. It makes the same proposal resolve
  against whichever worktree launched the server. The CCREP ledger and evaluator should default to
  the canonical repo discovered from Git's common directory; agents can still override
  `CCREP_REPO_ROOT` for a special evaluation.
- Commit-before-handoff remains rejected as a default. The handoff can include branch/commit details
  when a deliverable exists, but routine coordination must not require commits.

With those corrections, Codex approves the synthesized scheme now implemented in `.mcp.json`,
`tools/run_collab_mcp.py`, `tools/run_ccrep_mcp.py`, and
`doc/collab/worktree-coordination.md`.

**→ Handoff:**
**Task:** agy and Claude should review the wrapper-based correction and either ACK it or file one specific blocking critique.
**Context:** This turn accepts shared main-tree split logs and rejects only unverified env interpolation, relative CCREP repo roots, and commit-before-handoff as defaults.
**Deliverable:** ACK/critique in `session.agy.md` or `session.claude.md`, plus collab MCP message.

## [codex, 2026-06-08T08:48:00+02:00]

Consensus state after peer review:

- agy ACKed the wrapper-based scheme with no blocking critique in `session.agy.md`.
- Claude ACKed the wrapper-based scheme with no blocking critique in `session.claude.md`.
- Claude's two riders are non-blocking: commit/rebase is required before the scheme is live in
  every worktree, and the absolute venv shebang should be documented.
- Codex added the shebang comment to both wrappers.

Final consensus: 3/3 agents accept the wrapper-based two-plane scheme:

- `collab` and `ccrep` MCP servers launch through checked-in wrappers.
- Wrappers discover the canonical repo from Git's common directory.
- Shared collab state, CCREP ledger, and live session split logs live in the canonical repo.
- Bot worktrees remain isolated for deliverables.
- Routine handoffs do not require commits; CCREP gates immutable deliverable commits.

**→ Handoff:**
**Task:** Roberto should review the uncommitted synthesis and decide whether to authorize a commit/merge.
**Context:** Consensus is reached among codex, agy, and Claude. Agents must not commit without Roberto's explicit instruction.
**Deliverable:** Roberto approval to commit, or requested edits before commit.
