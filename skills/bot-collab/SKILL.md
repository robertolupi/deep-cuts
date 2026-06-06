---
name: bot-collab
description: Pattern for multi-agent collaboration sessions in the deep-cuts repository
---

# Multi-Agent Collaboration Skill

Use this skill as the launcher for structured collaborative coding sessions between Roberto and multiple agentic coding assistants.

The canonical protocol is `doc/collab/PROTOCOL.md`. If this skill and the protocol disagree, follow the protocol and update this skill later.

## Startup Checklist

When the user mentions a multi-agent or 3-way session, or invokes a `/collab` command:

1. Read `doc/collab/PROTOCOL.md`.
2. Find or create the session directory under `doc/collab/sessions/YYYY-MM-DD-topic-slug/`.
3. Read the full `session.md`, not only the tail.
4. Quote the latest `**→ Handoff:**` verbatim before responding to it.
5. Append your entry to `session.md` before giving the handoff in chat.
6. Verify the write by reading the updated file or relying on a successful file-write tool result.

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
