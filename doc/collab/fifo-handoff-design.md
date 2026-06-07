# Simplified FIFO-Based Collaboration Protocol

A low-overhead protocol for two participants (two agents, or a human and an agent) to take turns editing shared files, coordinated by a single Unix named pipe (FIFO). It is the mechanism behind the [`/collab`](../../skills/collab/SKILL.md) skill.

## TL;DR

- **One fixed pipe:** `scratch/fifo-handoff` (gitignored, never committed).
- **One fixed handoff token:** `echo NEXT > scratch/fifo-handoff`. The payload is meaningless — the real handoff (task / context / deliverable) lives in `session.md`. A constant command on a constant path means the FIFO commands can be whitelisted once and never prompt again.
- **Connection setup is a TCP-style handshake** that uses the pipe's *existence* as the synchronization signal, so the two sides never have to agree out-of-band on who goes first.
- **Serial by design.** Turn-taking blocks the idle participant, so this protocol buys *quality* (cross-model review), not *speed* — two agents are slower than one. For parallel throughput it is the wrong tool; see [Scope and limits](#scope-and-limits--serial-by-design) and [Beyond FIFO](#beyond-fifo--coordinating-parallel-work).

---

## The Connection Handshake (TCP SYN-ACK Analogy)

We map the named pipe turn-taking directly to a simplified TCP state machine to break symmetry:

| TCP State / Concept | FIFO Collaboration Equivalent | Action & Meaning |
|---|---|---|
| **Active Open (SYN)** | `mkfifo scratch/fifo-handoff` | Attempting to create the pipe. |
| **LISTEN / SYN-SENT** | `mkfifo` succeeds (Exit 0) | You are **FIRST** (Initiator). You have nothing to hand off yet. Immediately run `cat scratch/fifo-handoff` in the background and wait. |
| **ESTABLISHED (SYN-ACK)** | `mkfifo` fails with `File exists` | You are **SECOND** (Responder). The peer is already waiting. You go first: make edits, log your turn, then run `echo NEXT > scratch/fifo-handoff`. |
| **FIN (Teardown)** | final wake-write, **then** `rm` | Consensus reached. The closing party must first release the peer blocked on `cat` with a wake-write (`echo NEXT`; even a 0-byte EOF works), *then* `rm scratch/fifo-handoff`. **`rm` alone does not wake a blocked reader.** |

```
    First Arrival (Exit 0)                        Second Arrival (Exit 1)
   (Initiator: Create & Wait)                     (Responder: Work First)
   ──────────────────────────                     ──────────────────────────
      1. mkfifo (succeeds)
      2. cat scratch/fifo-handoff
         (blocks on read) ◄────────────────┐
                                           │  1. mkfifo (fails, file exists)
                                           │  2. Performs first turn work
                                           │  3. Logs turn in session.md
                                           │  4. Passes turn baton:
                                           └─── `echo NEXT > scratch/fifo-handoff`
                                                (unblocks reader)
      3. Wakes up reactively
      4. Performs work...
```

---

## Step-by-step Execution

### 1. Start or Join (The Handshake)
Attempt to create the pipe (do **not** test-then-create to avoid races):
```bash
mkfifo scratch/fifo-handoff
```
- **Exit 0**: You are **FIRST** (Initiator) -> Block on read immediately (Step 2).
- **Non-zero ("File exists")**: You are **SECOND** (Responder) -> Take your turn first (Step 3).

### 2. Wait for your turn
```bash
cat scratch/fifo-handoff
```
Run this as a **background** command using whatever async execution mechanism your agent harness provides. It blocks at the OS level with zero CPU/token cost. When the peer writes `NEXT`, `cat` exits and you are reactively woken.

### 3. Take your turn
1. Verify the peer's handoff — read the changed files and the latest `## [Handle, HH:MM]` block in `session.md`.
2. Make your edits to the target file(s).
3. Append your turn to `doc/collab/sessions/<session-dir>/session.md`:
   ```markdown
   ## [YourHandle, HH:MM]
   Description of changes …

   **→ Handoff:** Task / Context / Deliverable for the next participant.
   ```
   *FIFO turn-taking already serializes edits — only the participant holding the turn writes — so concurrent-write locking is unnecessary. If you must edit a shared file outside the turn discipline, follow the advisory file-locking rules in `PROTOCOL.md`.*

   **Chat is not a turn.** Only the turn-holder edits shared files. Thinking aloud, answering the human's questions, or discussing while blocked on `cat` is **not** a turn — do not modify any shared file (including `session.md`) during chat. Capture the point in your reply and apply it on your next turn. This keeps the logs a faithful record and prevents edits that race the turn-holder.

### 4. Hand off
```bash
echo NEXT > scratch/fifo-handoff
```
Run this in the **background** too. Writing to a FIFO blocks until the peer opens it for reading, so the `echo` completing is your confirmation the turn was received. Then return to Step 2 to wait for the next turn.

*Payload note: the token is semantically irrelevant — the reader only needs the writer to open and close the pipe, so the true minimum to wake it is a 0-byte EOF (`: > scratch/fifo-handoff`). We keep the readable `NEXT` anyway: the byte count is a non-issue, and a fixed, greppable token keeps the command whitelist-able and legible in logs.*

### 5. Close
At consensus the closing party is the active turn-holder and the peer is blocked on `cat`, so you must release it before cleaning up:
1. Write a closeout summary in `session.md`.
2. **Wake the waiting peer** with a final write: `echo NEXT > scratch/fifo-handoff` (any write works — even a 0-byte EOF via `: > scratch/fifo-handoff`). This is mandatory, not optional: **`rm` alone does not wake a reader blocked in `cat`**, so skipping the wake-write strands the peer.
3. Remove the pipe: `rm scratch/fifo-handoff`.
4. Archive the session directory by adding an `ARCHIVED` file.

> Verified empirically: a reader blocked in `cat` is released only by a writer opening+closing the pipe (EOF), **not** by `rm` (unlinking leaves the blocked reader hanging). Once woken, the peer sees the closeout in `session.md` and exits its loop instead of re-waiting.

---

## Error recovery & deadlocks

- **Both waiting (read–read deadlock).** Cannot happen at setup as long as everyone branches on `mkfifo` (only one creator exists). If it arises mid-session, whoever logged the most recent turn in `session.md` breaks the tie by echoing `NEXT`.
- **Handoff write hangs (no reader).** Your `echo` never returns because the peer never ran `cat` (crashed, or never started). Cancel it, post the handoff manually for the human to relay, and fall back to the manual `**→ Handoff:**` path in `PROTOCOL.md`.
- **Stale pipe from a dead session.** A leftover `scratch/fifo-handoff` makes a genuine first-arriver branch as "second" and echo into a pipe nobody is reading, causing the handoff to hang. If your startup `echo NEXT` hangs and no peer is expected, the pipe is stale: run `rm -f scratch/fifo-handoff && mkfifo scratch/fifo-handoff` to become the first-arriver and wait.
- **Lost work on crash.** Edits and the `session.md` turn log land *before* the handoff `echo`, so a crash mid-turn loses only the handoff signal, not the work. The recovering participant reads `session.md` and resumes from the last logged turn.

The FIFO is an optimization, not a requirement — when in doubt, fall back to the manual `**→ Handoff:**` relay described in `PROTOCOL.md`.

---

## Scope and limits — serial by design

A FIFO is a rendezvous/mutex primitive: exactly one participant works while the other blocks on
`cat`. Turn-taking is therefore **strictly sequential** — total wall-clock ≈ the sum of every turn
plus handoff overhead, so *two agents are slower than one*. The only things this protocol buys are:

1. **Collision-free edits** to a single shared file (the baton is a mutex).
2. **Zero token cost while idle** (blocking `cat` consumes nothing).

What it does **not** buy is throughput. Its real value is **quality** — cross-model review catches
what one model misses. (This very document is the evidence: one participant contributed the TCP
state-machine framing; the other caught the `rm`-teardown bug.) That gain would survive even if the
turns were taken with no IPC at all.

**Use this protocol only when the work is inherently serial** — two participants co-editing one
artifact, turn by turn. The moment you want parallel speedup, a single baton is the wrong primitive.

## Beyond FIFO — coordinating parallel work

For actual parallelism, stop coordinating with a *baton* (which blocks everyone into one thread)
and coordinate with **isolate + claim + notify**, serializing only at integration:

- **Isolation** — each agent works in its own **git worktree**, so there are no shared-file
  collisions at all (see the worktree-agent workflow: subagents work in isolation, the main session
  owns verify + merge, rebase first, keep `main` green).
- **Claim store (lock-free)** — hand out disjoint subtasks via an **atomic-rename queue**
  (`mv`/`rename(2)`) or a **SQLite (WAL) task table** with a transactional claim
  (`UPDATE … WHERE status='open' … RETURNING`). Either gives safe concurrent claiming with no global
  lock.
- **Notification** — react to **`fswatch`/`kqueue`/`inotify`** events or poll the task table, instead
  of blocking on a baton, so an agent can do its own work and still be woken.
- **Integration** — one coordinator owns merge/verify.

Daemonless options (advisory locks, atomic rename, SQLite, fs-watch) preserve the "no servers"
property; a Unix-domain-socket / message bus is more flexible but needs a broker.

These are **complementary regimes**, not competitors: the FIFO answers *"how do we not stomp on each
other in one file"*; the parallel model answers *"how do N agents do useful work at once."*
