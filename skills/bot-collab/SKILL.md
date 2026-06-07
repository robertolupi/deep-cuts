---
name: bot-collab
description: Pattern for multi-agent collaboration sessions in the deep-cuts repository
---

# Multi-Agent Collaboration Skill

Use this skill as the launcher for structured collaborative coding sessions between Roberto and multiple agentic coding assistants.

The canonical protocol is `doc/collab/PROTOCOL.md`. If this skill and the protocol disagree, follow the protocol and update this skill later.

## Startup Checklist

When the user mentions a multi-agent or 2-way collaboration session, or invokes a `/collab` command:

1. Read `doc/collab/PROTOCOL.md` and [doc/collab/fifo-handoff-design.md](file:///Users/rlupi/src/deep-cuts/doc/collab/fifo-handoff-design.md).
2. Find or create the session directory under `doc/collab/sessions/YYYY-MM-DD-topic-slug/`.
3. Read the full `session.md`, not only the tail.
4. **Parallel Coordination (Maildir / Dynamic MCP - Recommended)**:
   - Use the `CoordinationAdapter` or `collab` MCP tools.
   - **Identity**: Inspect your system prompt to resolve your actor name:
     - If your system prompt identifies you as **Codex** $\to$ use `actor="codex"`.
     - If your system prompt identifies you as **Antigravity** $\to$ use `actor="agy"` (peer is `"claude"`).
     - If your system prompt identifies you as **Claude** $\to$ use `actor="claude"` (peer is `"agy"`).
     - If your peer is not obvious, derive it from the active session's `## Participants` list or the latest handoff rather than assuming a two-agent pairing.
     - The project `.mcp.json` leaves `COLLAB_ACTOR` unset so different clients can share it; pass the explicit `actor` argument when your actor is not the server default.
   - **Handoff**: Append your turn entry to the active `session.md` file, then send a message of type `handoff` or `ack` to the peer actor using your resolved `actor` name.
5. **FIFO Coordination (Legacy Serial)** — use the [`collab`](../collab/SKILL.md) skill for the handshake.
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
