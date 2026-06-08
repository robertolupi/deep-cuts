---
name: bot-collab
description: Pattern for multi-agent collaboration sessions in the deep-cuts repository
---

# Multi-Agent Collaboration Skill

Use this skill as the launcher for structured collaborative coding sessions between Roberto and multiple agentic coding assistants.

The canonical protocol is `doc/collab/PROTOCOL.md`. If this skill and the protocol disagree, follow the protocol and update this skill later.

> **Coordination vs. quality ratchet — two layers.** This skill + the `collab` MCP are the
> *coordination transport* (mailboxes, handoffs, the task queue). When the goal is not just to
> coordinate but to make a specific artifact *provably better* — evaluate a code change, code
> review, or design doc, gather admissible peer critiques, and merge only when a consensus gate
> passes — use **[CCREP](../ccrep/SKILL.md)** (the `ccrep` MCP server). They compose: coordinate
> here, ratchet there.

## Worktree Topology

If agents run from separate git worktrees, read
[doc/collab/worktree-coordination.md](../../doc/collab/worktree-coordination.md) before appending or
sending handoffs. The short version:

- Worktree branches are for deliverables.
- The shared coordination plane is the canonical repo's `scratch/coordination`, `scratch/ccrep.db`,
  and live `doc/collab/sessions/<dir>/session.<actor>.md` files.
- `.mcp.json` launches `tools/run_collab_mcp.py` and `tools/run_ccrep_mcp.py`; those wrappers
  discover the canonical repo through Git's common directory and set shared defaults.
- Do not commit and merge peer branches for every routine handoff. Use collab MCP for live
  messages, and use CCREP/git commits only when reviewing or integrating a concrete deliverable.

## Startup Checklist

When the user mentions a multi-agent or 2-way collaboration session, or invokes a `/collab` command:

1. Read `doc/collab/PROTOCOL.md` and [doc/collab/fifo-handoff-design.md](file:///Users/rlupi/src/deep-cuts/doc/collab/fifo-handoff-design.md).
2. Find or create the session directory under `doc/collab/sessions/YYYY-MM-DD-topic-slug/`.
3. Read the full `session.md`, not only the tail.
4. **Coordination (collab MCP — FIRST CHOICE)**:
   - Use the `collab` MCP server tools as the primary coordination interface. Do **not** reach for bash/Python coordination scripts (the FIFO baton, the `CoordinationAdapter`, the advisory-lock helper) when these tools are available:

     | Need | MCP tool |
     |---|---|
     | Check your mailbox / task counts | `collab/inbox` |
     | Hand off to the peer | `collab/send(to, type="handoff", payload={task, context, deliverable})` |
     | Acknowledge | `collab/send(type="ack", in_reply_to=…)` |
     | Wait for your turn (blocks idle, ~zero token cost) | `collab/recv(timeout_s=…)` |
     | Poll without blocking | `collab/try_recv` |
     | Dispatch parallel subtasks | `collab/post` → peers `collab/claim` / `collab/complete`; coordinator `collab/sweep` reclaims expired leases |

   - **Activation**: if the `collab/*` tools are not already in your active tool list, enable them via your harness's MCP activation path *before* falling back — Claude Code: they are deferred, load with `ToolSearch` (e.g. `select:mcp__collab__send,mcp__collab__recv`); Antigravity: `ask_permission` with action `mcp`, target `collab/*`.
   - **Fallback only** (in order): the pre-approved shell wrapper (`tools/collab_mcp.sh`) or direct maildir access under `scratch/coordination/` if the MCP server fails to load; the FIFO baton (step 5) or manual relay (step 6) for serial turn-taking.
   - **Identity**: Inspect your system prompt to resolve your actor name:
     - If your system prompt identifies you as **Codex** $\to$ use `actor="codex"`.
     - If your system prompt identifies you as **Antigravity** $\to$ use `actor="agy"` (peer is `"claude"`).
     - If your system prompt identifies you as **Claude** $\to$ use `actor="claude"` (peer is `"agy"`).
     - If your peer is not obvious, derive it from the active session's `## Participants` list or the latest handoff rather than assuming a two-agent pairing.
     - The project `.mcp.json` leaves `COLLAB_ACTOR` unset so different clients can share it; pass the explicit `actor` argument when your actor is not the server default.
   - **Handoff**: Append your turn entry to the active log. In normal single-tree sessions this is
     `session.md`; in worktree mode this is the canonical repo's `session.<actor>.md`. Then
     `collab/send` a message of type `handoff` or `ack` to the peer actor using your resolved
     `actor` name.
   - **Post-Handoff Monitoring**: After handing off, do not remain idle or wait for manual user intervention. Prefer waiting for the reply using the blocking `collab/recv(timeout_s=...)` tool, which enables a zero-cost reactive wakeup. If your harness cannot block on a tool call (and would otherwise go idle without user interaction), fall back to invoking a background subagent (e.g. `self` or a background poller task) to periodically check your inbox via `collab/try_recv` or `tools/collab_mcp_cli.py inbox` and notify the parent agent when the peer replies.
5. **FIFO Coordination (legacy serial fallback)** — only when the collab MCP is unavailable; use the [`collab`](../collab/SKILL.md) skill for the handshake.
6. **Manual Coordination (Fallback)**:
   - Quote the latest `**→ Handoff:**` verbatim before responding to it.
   - Append your entry to `session.md` before giving the handoff in chat.
7. Verify the write by reading the updated file or relying on a successful file-write tool result.

## Session Files

New sessions use this path shape:

```text
doc/collab/sessions/YYYY-MM-DD-topic-slug/session.md
```

Session logs are working records. Durable decisions must be promoted into normal `doc/` files, `skills/` files, or code comments.

---

## Handoff Format

End collaborative turns with the structured handoff required by `doc/collab/PROTOCOL.md`:

```markdown
**→ Handoff:**
**Task:** [what to do]
**Context:** [files, data, or prior decisions needed]
**Deliverable:** [expected artifact]
```

In chat, provide a copyable summary:

```text
Check doc/collab/sessions/YYYY-MM-DD-topic-slug/session.md.

Handoff:
Task: [what to do]
Context: [files, data, or prior decisions needed]
Deliverable: [expected artifact]
```

## Documenting Roberto's Contributions

When Roberto makes a direct contribution (e.g., providing design direction, running commands, writing code, or giving explicit feedback that is not just copy-pasting an AI's handoff), the active agent must document it in the session log:
* Insert a `## [Roberto, HH:MM]` block outlining his feedback, code changes, or decisions, OR
* Explicitly credit and outline his steering decisions within the agent's own turn.
This keeps the session log as the single source of truth for the collaborative path.

## Recording Acknowledgements (ACKs)

Agreement is signal worth keeping, not just handoffs and disagreements. When you endorse, confirm, verify, or accept another participant's work, record it in the session log — not only in chat:
* A short ACK line inside your turn ("ACK: verified Gemini's mir_eval numbers, consistent on re-run"), OR
* A one-line `## [X, HH:MM]` block when relaying someone else's ACK (e.g. Gemini endorsing a revision).

This makes consensus — and who reached it — part of the durable record.
