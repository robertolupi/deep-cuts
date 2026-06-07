# Multi-Agent Collaboration Protocol

This directory is the shared workspace for structured collaboration sessions between Roberto and multiple AI coding/research assistants.

---

## Participants

The table below is the **roster of known participants**. Not all are available in every session — token limitations or interface constraints may apply. When starting a new session, list only the participants who will actually take part in the session's `session.md` header.

| Handle | Identity | Access model |
|---|---|---|
| **Roberto** | Human, project owner | Direct filesystem + git |
| **Codex** | OpenAI Codex coding agent | Direct filesystem read/write in the project workspace, command execution, git commits when explicitly requested |
| **Claude** | Anthropic Claude (via Claude Code / FleetView) | Direct filesystem read/write, git commit |
| **Gemini** | Google Gemini (via Antigravity or similar interface) | Direct filesystem within project workspace |
| **Meta** | Meta AI | Reads via GitHub (public URLs); writes by generating markdown for Roberto to commit; acts in design & architecture advisory role |

New participants can be added by appending a row here. The file format (`## [Handle, HH:MM]`) accommodates any handle without protocol changes.

---

## Meta AI – Role Definition (updated 2026-06-06)

**Primary focus:** big-picture architecture, research synthesis, and design review for Approach B (Neural Sequence Classifier + Viterbi).

**Do:**
- Review training results, accuracy curves, and prediction patterns (e.g., the 99.27% on 740 tracks, the chorus/verse flips in O Fortuna)
- Propose model choices (GRU vs tiny Transformer for 16×3 inputs), feature design (energy/rep_score/position), and alignment strategies for Genius tags
- Design Viterbi priors at a conceptual level – transition logic, duration modeling, handling "unknown" states
- Compare Approach A (DTW) vs Approach B tradeoffs, and suggest integration points for Tauri/ONNX at a high level
- Summarize findings in concise handoff notes

**Don't:**
- Generate full source files (Rust, Python training scripts, ONNX export code) – delegate to Claude/Gemini
- Provide deployment-ready code blocks or step-by-step build instructions

**Why:** Meta's guardrails often block end-to-end code generation, but excel at synthesis and architectural feedback. Keep Meta on design review; use other models for implementation.


---

## Session folders

Each session is a **directory** in `doc/collab/sessions/` named `YYYY-MM-DD-topic-slug/`. Flat `.md` files directly in `sessions/` are not allowed — `tools/lint_collab.py` flags them as errors.

Each session directory contains:
- `session.md`: The markdown session log containing participant turns and handoffs.
- A `## Participants` section near the top of `session.md` listing only the participants active in that session (chosen from the roster above).
- Optional session-specific artifacts, scripts, and sample datasets (e.g. `sample_tracks.json`, `dataset.py`).

**Creating a new session:**
1. Choose participants from the roster who are actually available for this session.
2. Run `mkdir doc/collab/sessions/YYYY-MM-DD-topic-slug/`.
3. Create `session.md` with a `## Participants` section listing selected participants.
4. Run `python3 tools/lint_collab.py` to verify the structure is valid.

Each entry in `session.md` follows this format:

```markdown
## [Handle, HH:MM]
Content of the contribution — reasoning, findings, code snippets, decisions.

**→ Handoff:** One sentence describing what the next participant should do or answer.
```

The handoff line is optional if the session is just a log, not a turn-taking exchange.

---

## Turn-taking rules

1. **Sequential FIFO Handoff (Recommended for 2-participant sessions)** — automated by the [`/collab`](file:///Users/rlupi/src/deep-cuts/skills/collab/SKILL.md) skill; see [fifo-handoff-design.md](file:///Users/rlupi/src/deep-cuts/doc/collab/fifo-handoff-design.md) for the full design:
   - Participants coordinate turns via a single fixed named pipe at `scratch/fifo-handoff`.
   - **Handshake (who goes first):** run `mkfifo scratch/fifo-handoff` and branch on the result. If it **succeeds**, you are first — *wait* (`cat scratch/fifo-handoff`). If it **fails** because the pipe already exists, the peer is already waiting — *you go first*: edit, log your turn, then hand off. The atomic create-or-fail removes any cold-start ambiguity.
   - To wait for a turn: Run `cat scratch/fifo-handoff` (as a background command). This blocks at the OS level until the other participant writes, triggering a reactive wakeup when the command completes.
   - To hand off a turn: Edit files, append your entry to `session.md`, and then run `echo NEXT > scratch/fifo-handoff` (background). The fixed token keeps the command whitelist-able; the real handoff content lives in `session.md`.
2. **Manual Handoff (Relayed by Roberto)**:
   - An AI's turn ends when it appends its entry and writes a `**→ Handoff:**` line.
   - The human relays the handoff to the next participant by pasting:
     - The session file path
     - The handoff line verbatim
3. **Documenting Roberto's Active Feedback**: When Roberto provides active direction, feedback, or manually runs commands/code (instead of just copy-pasting handoffs), the acting agent must document his contribution in the session log. This can be done by adding a dedicated `## [Roberto, HH:MM]` block or explicitly detailing his input in the agent's turn to ensure the log is a complete history.
4. **Recording acknowledgements (ACKs)**: Agreement is signal, not noise — log it. When a participant endorses, confirms, verifies, or accepts another participant's work, record it in the session log, not only in chat. A short ACK line in the acknowledging participant's turn (or a one-line `## [X, HH:MM]` block for a relayed ACK) is enough. This makes consensus — and who reached it — part of the durable record.

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


## Handoff Prompt Template

When ending a turn, produce a fenced block the human can copy:

```
Check doc/collab/sessions/YYYY-MM-DD-topic-slug/session.md.

Handoff:
Task: [what to do]
Context: [files, data, or prior decisions needed]
Deliverable: [expected artifact]
```

---

## File locations

* **All sessions** are stored in dedicated subdirectories in `doc/collab/sessions/` within this repository.
* **Why**: This standardizes access for all participants, organizes all multi-file artifacts, and ensures compatibility with Gemini's workspace sandboxing constraints.

See the session file `2026-06-06-multi-agent-collab/session.md` for the full discussion.

---

## Session archiving (tombstones)

Roberto controls which sessions are *active*. A session directory containing a file named
`ARCHIVED` is **archived**: agents must not actively work on it, and tools (the Collab Hub)
exclude it from the active list and never auto-select it.

- **Archive:** create an `ARCHIVED` file in the session dir (any contents — a date is nice).
- **Unarchive:** delete the `ARCHIVED` file.
- The marker is **committed to git**, so archive state is shared across all participants.
- Picking "the active session" means: among non-archived session dirs, the most recently
  modified. Never resume or append to an archived session unless Roberto unarchives it first.

## File locking ("I temporarily own this file")

When concurrent agents (Claude, Gemini) and Roberto can all edit the same file, take an
**advisory lock** before editing a shared file — `session.md`, `chat_log.jsonl`, `tasks.md`,
`PROTOCOL.md`, or any shared doc.

The advisory-lock helper lives in the standalone collaboration tooling (the
`multi-agent-ops` project): acquire a `<path>.lock` sidecar (`{owner, pid, ts}`; stale locks
reclaimed after ~120 s so a crashed agent never wedges a file) before editing, and release it
after.

Rules:
- It is **advisory** — it only works if every writer checks it first. Always acquire before
  editing a shared file; always release after (the context manager does this automatically).
- If `acquire` reports `LOCKED`, do **not** edit — wait, or coordinate via the handoff.
- Append-only logs (`chat_log.jsonl`) may instead use atomic `O_APPEND` single-line writes.
- Never commit `.lock` files — they are transient (add to `.gitignore` if needed).

### KNOWN ISSUE — `session.md` write-races (TODO, 2026-06-07)

Observed in `sessions/2026-06-07-salami-eval-followup/`: with three agents (Codex, Gemini/agy,
Claude) editing one `session.md` concurrently, the advisory lock above was **not actually enforced**
by the live tooling, so edits repeatedly collided. Symptom: a writer's edit fails with "file has been
modified since read" / silently loses the race; one agent (Claude) lost its turn entirely and a
Roberto steering turn had to be back-filled out of order. The append-log survived (no data loss) but
turn ordering and authorship got muddled.

We need to re-establish a working mechanism before the next multi-agent session. Candidates, roughly
in order of effort:

1. **Re-establish the advisory lockfile** described above, but *actually wired into every agent's
   edit path* (the `multi-agent-ops` helper exists but wasn't loaded). Cheapest if the helper just
   needs registering/whitelisting.
2. **Spool-and-reassemble (preferred if locking stays unreliable):** each agent appends its turns to
   its **own** per-actor file (e.g. `turns/<actor>.md`, append-only — no cross-agent contention), and
   a small script reassembles them into the canonical `session.md` ordered by the per-turn timestamp.
   This removes the shared-write hot spot entirely; the reassembler is the only writer of `session.md`.
   Mirrors the maildir pattern the collab MCP already uses for messages (per-actor dirs, no shared
   mutable file).
3. **Single-writer coordinator:** agents emit turns only as MCP messages; one coordinator drains the
   queue and is the sole writer of `session.md`.

Action: pick (1) or (2), implement, and update this section + the `collab`/`bot-collab` skills.

---

## What goes in a session vs a doc

- Session files are **ephemeral working logs** — thinking out loud, proposals, back-and-forth
- Decisions that survive the session get promoted to a proper `doc/` file (design doc, SKILL.md, etc.)
- Session files are committed to the repo for traceability but are not maintained after the session closes

## Closing a Session

When a collaboration session reaches a stable conclusion:

1. Append a short closeout entry summarizing accepted decisions.
2. List rejected alternatives if they are likely to be proposed again.
3. Link any implementation commits, PRs, or follow-up docs.
4. Promote durable instructions into a normal `doc/` file, a `skills/` file, or code comments.
5. Mark the final handoff as complete or explicitly hand back remaining work to Roberto.
6. Mark the session archived by appending a `## [Closed, YYYY-MM-DD]` entry. This signals to any future reader that the session reached a conclusion and requires no further action.
