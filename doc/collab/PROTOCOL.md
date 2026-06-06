# Multi-Agent Collaboration Protocol

This directory is the shared workspace for three-way sessions between Roberto, Gemini, and Claude.

---

## Participants

| Handle | Identity | Access model |
|---|---|---|
| **Roberto** | Human, project owner | Direct filesystem + git |
| **Claude** | Anthropic Claude (via Claude Code / FleetView) | Direct filesystem read/write, git commit |
| **Gemini** | Google Gemini (via Antigravity or similar interface) | Direct filesystem within project workspace |
| **Meta** | Meta AI | Reads via GitHub (public URLs); writes by generating markdown for Roberto to commit; can run Python/data experiments and attach results |

New participants can be added by appending a row here. The file format (`## [Handle, HH:MM]`) accommodates any handle without protocol changes.

---

## Session files

One markdown file per topic, named `YYYY-MM-DD-topic-slug.md`, in `doc/collab/sessions/`.

Each entry in a session file follows this format:

```markdown
## [Handle, HH:MM]
Content of the contribution — reasoning, findings, code snippets, decisions.

**→ Handoff:** One sentence describing what the next participant should do or answer.
```

The handoff line is optional if the session is just a log, not a turn-taking exchange.

---

## Turn-taking rules

1. An AI's turn ends when it appends its entry and writes a `**→ Handoff:**` line.
2. The human relays the handoff to the next participant by pasting:
   - The session file path
   - The handoff line verbatim
3. The receiving AI reads the session file, appends its response, and writes the next handoff.
4. Either AI can pass back to Roberto instead of the other AI when human judgement is needed.

---

## Handoff prompt template

When ending a turn, produce a fenced block the human can copy:

```
Check doc/collab/sessions/FILENAME.md.

Handoff: [one sentence summarising what was just decided/done]
Question for [Gemini|Claude]: [specific question or task]
```

---

## File locations

* **All sessions** (both project-specific and meta-discussions) are stored in `doc/collab/sessions/` within this repository.
* **Why**: This standardizes access for all participants and ensures zero-config compatibility with Gemini's workspace sandboxing constraints.

See the session file `2026-06-06-multi-agent-collab.md` for the full discussion.

---

## What goes in a session vs a doc

- Session files are **ephemeral working logs** — thinking out loud, proposals, back-and-forth
- Decisions that survive the session get promoted to a proper `doc/` file (design doc, SKILL.md, etc.)
- Session files are committed to the repo for traceability but are not maintained after the session closes
