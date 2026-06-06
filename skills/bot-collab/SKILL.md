---
name: bot-collab
description: Pattern for multi-agent collaboration sessions in the deep-cuts repository
---

# Multi-Agent Collaboration Skill

This skill is the trigger and quick-start checklist for structured collaborative coding sessions between Roberto and multiple agentic coding assistants. The canonical protocol lives in `doc/collab/PROTOCOL.md`; if this skill and the protocol disagree, follow `PROTOCOL.md` and update this skill later.

All coordination documents reside under `doc/collab/`:

- `doc/collab/PROTOCOL.md`: source of truth for turn-taking, handoff structure, and error recovery.
- `doc/collab/sessions/`: topic-specific session directories. New sessions should use `YYYY-MM-DD-topic-slug/session.md`.

---

## Agent Usage Instructions

When the user mentions a multi-agent or 3-way session, or invokes a `/collab` command, first read `doc/collab/PROTOCOL.md`, then use this checklist.

### ⚠️ Verification rule (applies to every action)
**Never describe writing a file — actually write it.** Confirm the write succeeded before
producing the handoff block. This rule exists because an AI hallucinating a file write will
silently corrupt the shared context for the other agent.

What "confirmation" looks like per participant:
- **Claude / Gemini**: use your file-write tool; the tool output confirms the write.
- **Meta**: you can't write directly — generate the exact markdown block for Roberto to
  commit, and include the full content in your response so Roberto can verify before pasting.

### Gemini-specific checklist
Gemini runs in a sandboxed workspace that resets between turns. To prevent errors:
1. At the start of every turn, re-read `doc/collab/PROTOCOL.md` and `skills/bot-collab/SKILL.md`.
2. After writing to the session file, verify the write succeeded by executing a read tool on the file (e.g. read the last 20 lines) and include that output in your chat response as proof.
3. Never assume file state from memory – always read from disk.
4. If you encounter a write failure, hand back to Roberto with a `**→ Handoff:**` describing the error; do not retry silently.

### 1. Checking the Active Session (`/collab check`)
1. Look for the latest session directory under `doc/collab/sessions/` (usually the YYYY-MM-DD prefix).
2. Read `session.md` in that directory to get context on what has already been discussed, what decisions were made, and what the latest handoff is.
3. **Prominently quote the most recent `→ Handoff:` line** in a blockquote at the very start of your response, e.g.:
   > **→ Handoff:** [exact text]
4. Acknowledge the handoff query and proceed with your analysis or code modifications.


### 2. Creating a Session (`/collab new`)
If starting a new topic:
1. Create a directory under `doc/collab/sessions/YYYY-MM-DD-topic-slug/`.
2. Initialize `session.md` inside it with:
   ```markdown
   # Session: [Topic Title]
   **Date:** YYYY-MM-DD  
   **Participants:** Roberto, [Agent 1], [Agent 2]  
   **Goal:** [Summary of what we want to solve]

   ---

   ## [Roberto, ~HH:MM]
   [Initial user request details...]
   ```

### 3. Appending Your Contribution & Handoff (`/collab handoff`)
When you finish your work/reasoning and want to hand the turn to the other agent or the user:
1. **CRITICAL**: Do NOT just write in the chat that you are writing the file. You must actually write to the session `session.md` file.
2. Format your entry as a header: `## [AgentName, HH:MM]`.
3. Write your reasoning, changes, and findings.
4. End your entry with the structured handoff required by `PROTOCOL.md`:
   ```markdown
   **→ Handoff:**
   **Task:** [what to do]
   **Context:** [files, data, or prior decisions needed]
   **Deliverable:** [expected artifact]
   ```
5. Output a copyable handoff box in your final chat response:
   ```
   Check doc/collab/sessions/YYYY-MM-DD-topic-slug/session.md.

   Handoff:
   Task: [what to do]
   Context: [files, data, or prior decisions needed]
   Deliverable: [expected artifact]
   ```
