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

## Session folders

Each session has a dedicated folder in `doc/collab/sessions/` named `YYYY-MM-DD-topic-slug/` containing:
- `session.md`: The markdown session log containing participant turns and handoffs.
- Optional session-specific artifacts, scripts, and sample datasets (e.g. `sample_tracks.json`, `dataset.py`).

Each entry in `session.md` follows this format:

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

### 5. Pre-flight verification (all AIs)
Before appending, each AI must:
1. `git pull` the repo to ensure they have the latest session file.
2. Read the full session file, not just the tail.
3. Quote the most recent `**→ Handoff:**` verbatim at the top of their response entry.
4. Verify their write succeeded by including the file-write tool output (Claude/Gemini) or by pasting the full proposed markdown block (Meta).

### 6. Error recovery
If the session file is missing or unreadable:
- Create it using the scaffold in `SKILL.md` §2.
- Log the creation in your entry as "Initialized missing session file".

### 7. Handoff structure (required)
Handoffs must contain three parts, each on its own line:
- **Task:** what to do
- **Context:** files, data, or prior decisions needed
- **Deliverable:** expected artifact (code, analysis, markdown)

This reduces ambiguity that causes Gemini to hallucinate next steps.

---


## Handoff prompt template

When ending a turn, produce a fenced block the human can copy:

```
Check doc/collab/sessions/YYYY-MM-DD-topic-slug/session.md.

Handoff: [one sentence summarising what was just decided/done]
Question for [Gemini|Claude|Meta]: [specific question or task]
```

---

## File locations

* **All sessions** are stored in dedicated subdirectories in `doc/collab/sessions/` within this repository.
* **Why**: This standardizes access for all participants, organizes all multi-file artifacts, and ensures compatibility with Gemini's workspace sandboxing constraints.

See the session file `2026-06-06-multi-agent-collab/session.md` for the full discussion.

---

## What goes in a session vs a doc

- Session files are **ephemeral working logs** — thinking out loud, proposals, back-and-forth
- Decisions that survive the session get promoted to a proper `doc/` file (design doc, SKILL.md, etc.)
- Session files are committed to the repo for traceability but are not maintained after the session closes
