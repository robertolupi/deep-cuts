---
name: collab
description: Launch or join a two-party FIFO collaboration session (the /collab handshake). Use when the user says "collab", "/collab", or asks you to collaborate turn-by-turn with another agent on shared files via the scratch/fifo-handoff named pipe.
---

# /collab — FIFO collaboration launcher

The concrete launch + turn-taking handshake for a two-participant session. The design and
rationale live in [doc/collab/fifo-handoff-design.md](../../doc/collab/fifo-handoff-design.md);
the conduct rules — session-log format, handoff structure, ACKs, documenting Roberto's input,
archiving — live in [bot-collab](../bot-collab/SKILL.md) and
[doc/collab/PROTOCOL.md](../../doc/collab/PROTOCOL.md). **This skill is only the handshake.**

Fixed by convention (so the commands are whitelist-able and never need configuring per session):

- **Pipe:** `scratch/fifo-handoff` (gitignored)
- **Handoff token:** `echo NEXT > scratch/fifo-handoff` — the payload is meaningless; the real
  handoff (Task / Context / Deliverable) goes in `session.md`.

## Connection handshake (TCP SYN-ACK style)

Run exactly this to start **or** join, then branch on the exit code. Do **not** test-then-create
(that races into a double-wait deadlock):

```bash
mkfifo scratch/fifo-handoff
```

- **Exit 0 (you created it) → you are FIRST → WAIT.** You have nothing to hand off yet. Block on
  the pipe in the background: `cat scratch/fifo-handoff`. When it returns, take your turn.
- **Non-zero ("File exists") → you are SECOND → GO FIRST.** The peer is already waiting. Make the
  first edits, log your turn in `session.md`, then hand off with `echo NEXT > scratch/fifo-handoff`.
  Then `cat` to wait for the return.

`mkfifo` is an atomic create-or-fail, so exactly one participant is ever the creator — the roles
never collide.

## Each turn

1. **Wait:** `cat scratch/fifo-handoff` (background command → reactive wakeup when the peer writes).
2. **Verify:** read the changed files and the latest `## [Handle, HH:MM]` block in `session.md`.
3. **Work:** make your edits.
4. **Log:** append your `## [Handle, HH:MM]` turn with a `**→ Handoff:**` (Task/Context/Deliverable)
   to `session.md`. Record ACKs and Roberto's direct input per `PROTOCOL.md`.
5. **Hand off:** `echo NEXT > scratch/fifo-handoff` (background; completion = turn received).

## Session files

Find or create `doc/collab/sessions/YYYY-MM-DD-topic-slug/session.md` with a `## Participants`
section (see [bot-collab](../bot-collab/SKILL.md) / `PROTOCOL.md`). Run
`python3 tools/lint_collab.py` after structural edits.

## Close

At consensus: write a closeout summary in `session.md` → `rm scratch/fifo-handoff` → add an
`ARCHIVED` file to the session directory.

## Recovery

- **Startup `echo` hangs with no peer** → stale pipe from a dead session:
  `rm -f scratch/fifo-handoff && mkfifo scratch/fifo-handoff` (you become first, then wait).
- **`cat` never wakes** → peer not started or crashed; fall back to the manual `**→ Handoff:**`
  relay in `PROTOCOL.md`.
- Edits and the `session.md` log always land before the `echo`, so a crash loses only the handoff
  signal, not the work — resume from the last logged turn.

## Turn discipline — chat is not a turn

Only the participant **holding the turn** edits shared files. Thinking aloud, answering the human's
questions, or discussing while blocked on `cat` is **not** a turn: do not modify any shared file
(including `session.md`) during chat. Capture the point and apply it on your next turn.

## When NOT to use /collab

This protocol is **serial by design** — the baton blocks the idle participant, so two agents are
slower than one. It is right only for *inherently serial* work (co-editing one artifact, turn-by-turn
review), where the payoff is cross-model review quality, not speed. For **parallel throughput**, do
not use the FIFO baton — use per-agent **git worktrees** + a lock-free **claim store** (atomic-rename
queue or a SQLite WAL task table) + **fs-watch/poll** notification, with one coordinator owning
merge. See "Beyond FIFO" in [doc/collab/fifo-handoff-design.md](../../doc/collab/fifo-handoff-design.md).
