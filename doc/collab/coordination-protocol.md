# Minimum Shared Coordination Protocol (Actor Model)

The transport-agnostic contract that lets independent agents coordinate **parallel** work. This is
the parallel counterpart to the serial [fifo-handoff-design.md](fifo-handoff-design.md): the FIFO
baton is for turn-taking on one artifact; this protocol is for N agents doing useful work at once.

Status: **agreed** (round 3, 2026-06-07) — Claude + agy reached consensus on the actor model, the core/optional operations, at-least-once + idempotent delivery, and the coordinator role. **v0 backend: maildir** (Roberto, 2026-06-07); **Redis is the eventual target**. Next: Claude's MCP-adapter implementation — see [claude-mcp-adapter.md](claude-mcp-adapter.md).

## Model

- **Actors, share-nothing.** Each agent is an actor / active object. Its private state is its **own
  git worktree/branch**. No shared working files ⇒ no data races ⇒ no locks on work. (Erlang's
  guarantee.)
- **Async message passing.** Agents never touch each other's state; they post messages to
  mailboxes. **The only routine blocking point is `recv`** — you block *only* when you need to
  receive.
- **Protected objects for the little shared state.** The work queue and the integration point are
  passive resources with guarded, mutually-exclusive operations (Ada protected objects / monitors).
  Callers invoke the guarded op; serialization happens inside it.
- **Asymmetric adapters.** The *contract* below is shared; each agent implements it however its
  harness prefers. Claude → an MCP server (tools, allowlisted once, server-side blocking `recv`);
  agy → its own adapter. They interoperate by sharing the backend + the envelope format.

## The contract

### Message envelope
```
{ id, from, to, type, payload, in_reply_to?, ts }
```
- `id` unique (used for dedupe). `in_reply_to` correlates a reply to a request (futures).
- Per-recipient mailbox is **ordered**; delivery is **at-least-once**, so handlers must be
  idempotent (dedupe on `id`).

### Operations

The irreducible **minimum** is two mailbox ops + three queue ops; everything under "robustness layer" is optional.

**Core — mailbox (actor messaging)**
- `send(to, msg)` — **non-blocking**; append to recipient's mailbox.
- `recv(pattern?, timeout?)` — block until a matching message is available (selective receive, Erlang-style: take the first match, leave the rest). The one blocking point.
- `try_recv(pattern?)` — non-blocking peek; check mail *between* work units without blocking.

**Core — task store (protected object / guarded queue)**
- `post(task)` — enqueue work.
- `claim() -> task | none` — **atomic** dequeue of one open task (exactly one claimer).
- `complete(task_id, result)` — mark done; result is typically "branch X ready to merge."

**Robustness layer (optional — crash-recovery hardening; from agy's round-3 input)**
- **lease/TTL** — a claimed task auto-returns if the claimer dies or stops heartbeating before `complete`.
- `heartbeat(task_id)` — extend the lease for a long-running task, so a healthy-but-slow worker isn't preempted by the coordinator.
- `abandon(task_id, reason)` — explicitly release a claimed task back to the queue with diagnostics (fatal error, bad environment) instead of waiting for the TTL.

Drop the whole robustness layer and the core still runs the happy path; add it once the basic loop works.

### Roles
- **Workers** — actors executing tasks in their own worktrees.
- **Coordinator** — owns the single integration barrier: `recv` "branch ready" → rebase → `cargo test` → merge, keeping `main` green. This is the one intrinsic serialization (a protected `merge` entry), not ceremony. The coordinator also runs the periodic background supervisor loop to check lease TTL expirations and re-enqueue orphaned tasks.

### Behavioral law
1. Work in your own worktree/branch; never edit shared files or another actor's state directly.
2. Block **only** at `recv`/`claim`; otherwise run free. (This is where the parallelism lives.)
3. Every cross-actor effect goes through `send`/`recv` or a protected op — no back channels.
4. The **durable record stays in git** (session log / committed task log), regardless of transport.

### Derived primitives
Message passing is the primitive; the classic synchronization tools are sugar over it:
- **lock / mutex** = a 1-token mailbox (hold the token = hold the lock).
- **semaphore(N)** = N tokens.
- **barrier** = collect N "done" messages before proceeding.

So the minimum the substrate must provide is **a mailbox + an atomic claim** — nothing else.

## Transport requirements (what any adapter/backend must provide)
- **Buffered async `send`** — a queue, *not* a rendezvous. (This rules out a raw FIFO, whose `send` blocks until a reader opens — that would violate "block only on `recv`.")
- **Per-recipient ordered queue.**
- **Atomic claim** for the task store.
- **Blocking `recv` with timeout** → reactive wakeup.
- (Optional) **lease/TTL** for crash recovery.

### Candidate backend A (agy's proposal): Write-then-Rename Filesystem (Maildir)
The backend sits behind the core ops, so it is swappable. agy proposes a maildir-style filesystem transport to get zero-poll event-driven notifications while preserving sandbox safety — note this is one of two candidates (see "v0 backend decision"):

1. **Mailbox Directory Structure**:
   ```
   scratch/coordination/
     tmp/                  # Write-staging (ignored by watchers)
     <actor>/new/          # Watched directory for incoming messages
     <actor>/cur/          # Processed/ACKed messages
   ```
2. **Atomic Send Operation (`send`)**:
   - Write the JSON envelope to `scratch/coordination/tmp/msg_UUID.json`.
   - Close the file.
   - Perform an atomic OS `rename()` (or `mv`) to `scratch/coordination/<recipient>/new/msg_UUID.json`. Because the rename system call is atomic at the kernel level, it triggers exactly one filesystem event and ensures the recipient never reads a partial write.
3. **Event-Driven Block (`recv`)**:
   - The recipient blocks on a directory watcher (e.g. `fswatch` on macOS, `inotifywait` on Linux, or Python's `watchfiles`) targeting `scratch/coordination/<recipient>/new/`.
   - On the `Created` / `MovedTo` event, the watcher wakes up, processes the message, and atomically moves the file to `scratch/coordination/<recipient>/cur/` (ACK).

---

## Failure model
- **Worker crash** — leased task auto-returns after TTL; coordinator reassigns. Work-in-progress is recoverable from the branch/commits.
- **Message loss / duplication** — at-least-once delivery + idempotent handlers (dedupe on `id`).
- **No deadlock** — no shared lock is held across a `recv`; share-nothing removes the cycle.

## Minimal viable subset (v0)
Two workers + one coordinator, operations `post`/`claim`/`complete` + `send`/`recv`, worktrees for isolation, no leases yet (add once the basic loop works). The backend is **pluggable behind the 5 core ops**, so it is a cheap, reversible decision — and Redis remains the upgrade path if coordination must span machines, needs self-expiring leases, or wants a pub/sub chat hub.

### v0 backend decision (open — Roberto to confirm)
Two candidates, both satisfying the transport requirements:
- **A. Maildir (write-then-rename + dir watcher)** — agy's proposal. *Pros:* truly non-blocking `send` (write+`rename`), atomic `claim` (rename, exactly one winner), zero-poll notification via `watchfiles`/`fswatch`/`kqueue`, daemonless, sandbox-safe, state = plain inspectable files. *Cons:* zero-poll wants a watcher lib (or fall back to polling); ordering is by `ts`/seq, not native.
- **B. SQLite (WAL) task+mailbox tables** — Roberto's earlier lean. *Pros:* already a project dependency, ACID transactional `claim` (`UPDATE … RETURNING`), native ordering, durable. *Cons:* no built-in push — notification is still poll or fs-watch on the `.db` file.
Both are daemonless and local; neither needs new infra beyond an optional watcher. Recommendation: either is fine behind the pluggable interface — lean **maildir** if we want event-driven + share-nothing purity (and the co-builder prefers it), **SQLite** if "already a dep, ACID, zero new libs" wins. Roberto decides the v0 default.

**Decision (Roberto, 2026-06-07): maildir for v0** — chosen to make the message flow easy to *trace* (every message is an inspectable file; `new/` vs `cur/` shows pending vs processed). **Redis is the eventual target** (cross-machine, self-expiring leases, pub/sub); SQLite is not being pursued. Because the backend stays behind the 5 core ops, maildir → Redis is a swap, not a rewrite.

## Adapters (asymmetric, interoperable)
- **Claude:** an MCP server (shipped as a Claude Code plugin) exposing `send`/`recv`/`try_recv` and `post`/`claim`/`complete` as tools. `recv` blocks server-side using the file-watcher and returns on a new message → reactive wakeup; tools are allowlisted once (no per-command shell prompts).
- **agy:** its own adapter to the same backend (Antigravity-native).
- **Interop:** they share only the backend + the envelope format. Neither cares how the other connects.
