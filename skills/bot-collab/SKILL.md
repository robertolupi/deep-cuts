---
name: bot-collab
description: Pattern for multi-agent collaboration sessions in the deep-cuts repository
---

# Multi-Agent Collaboration Protocol

This skill outlines the process for conducting structured collaborative coding sessions between human developers and multiple agentic coding assistants (e.g. Gemini/Antigravity and Claude).

All coordination documents reside under the [doc/collab/](file:///Users/rlupi/src/deep-cuts/doc/collab/) directory:
* **[PROTOCOL.md](file:///Users/rlupi/src/deep-cuts/doc/collab/PROTOCOL.md)**: Rules and structures for turn-taking.
* **[sessions/](file:///Users/rlupi/src/deep-cuts/doc/collab/sessions/)**: Topic-specific markdown log files.

---

## Agent Usage Instructions

When the user mentions that they are running a multi-agent or 3-way session (or when they invoke the `/collab` command), follow these steps:

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
1. Look for the latest session file in `doc/collab/sessions/` (usually the one with today's date).
2. Read the file to get context on what has already been discussed, what decisions were made, and what the latest handoff is.
3. **Prominently quote the most recent `→ Handoff:` line** in a blockquote at the very start of your response, e.g.:
   > **→ Handoff:** [exact text]
4. Acknowledge the handoff query and proceed with your analysis or code modifications.


### 2. Creating a Session (`/collab new`)
If starting a new topic:
1. Create a file under `doc/collab/sessions/YYYY-MM-DD-topic-slug.md`.
2. Initialize it with:
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
1. **CRITICAL**: Do NOT just write in the chat that you are writing the file. You must actually write to the session markdown file.
2. Format your entry as a header: `## [AgentName, HH:MM]`.
3. Write your reasoning, changes, and findings.
4. End your entry with: `**→ Handoff:** [Specific question or next step description]`.
5. Output a copyable handoff box in your final chat response:
   ```
   Check doc/collab/sessions/YYYY-MM-DD-topic-slug.md.

   Handoff: [one-sentence summary of what you did]
   Question for [Claude/Gemini/Roberto]: [specific question or task]
   ```


