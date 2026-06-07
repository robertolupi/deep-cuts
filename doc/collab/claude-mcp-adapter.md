# Claude's Collab Adapter — MCP Server over the Maildir Backend

Claude's implementation of the [coordination-protocol.md](coordination-protocol.md) contract. This
is **one of the asymmetric adapters**: Claude talks to the shared maildir backend through this MCP
server; agy brings its own adapter to the same directories. They share only the **backend layout**
and the **message envelope** — neither cares how the other connects.

Status: **design** (round 3 follow-up, 2026-06-07). v0 backend = maildir (Roberto's call); Redis is
the eventual swap behind the same tools.

## Why an MCP server (not shell commands)

- **Reactive `recv` with no polling tokens.** The tool call blocks server-side on a directory
  watcher and returns when a message lands → the agent is woken with the payload. No `cat`-on-a-FIFO
  rendezvous, no lost/duplicated handoff tokens (the failure mode that bit the FIFO).
- **Allowlisted once.** MCP tools are permission-granted by name (e.g. `mcp__collab__*`), so the
  whole adapter stops prompting after a single grant — unlike shell commands, where every compound
  variant re-prompts.
- **Transport hidden.** The agent calls `send`/`recv`/`claim`; whether that's maildir today or Redis
  later is invisible above the tool boundary.

## Runtime & location

- **Python**, using the official `mcp` SDK (FastMCP), run as a stdio MCP server.
- Lives in `tools/collab_mcp/` (reuses the existing `tools/.venv`). Event-driven `recv` via
  **`watchfiles`** (Rust-backed, pip-installable; falls back to a poll loop if absent).
- Actor identity is fixed by config/env at launch (`COLLAB_ACTOR=claude`), so the server knows whose
  mailbox it owns.

## Backend layout (shared contract with agy)

Per coordination-protocol.md, under the gitignored `scratch/`:
```
scratch/coordination/
  tmp/                     # write-staging (watchers ignore this)
  <actor>/new/             # delivered, unread   ← watched
  <actor>/cur/             # read/processed       (audit trail)
  tasks/open/              # postable work items
  tasks/claimed/<actor>/   # atomically claimed
  tasks/done/              # completed (result envelope inside)
```
Every message/task is a JSON file named `<ts>-<uuid>.json` holding the envelope
`{ id, from, to, type, payload, in_reply_to?, ts }`. Timestamp prefix gives per-mailbox ordering.

## Tools (map 1:1 to the contract)

| Tool | Contract op | Implementation |
|------|-------------|----------------|
| `send(to, type, payload, in_reply_to?)` | `send` | write `tmp/<f>.json` → atomic `rename` to `<to>/new/<f>.json`. Non-blocking. |
| `recv(pattern?, timeout_s?)` | `recv` | `await watchfiles.awatch(<me>/new/)` (or poll); take oldest match; atomic move `new/→cur/`; return envelope. Blocks (the one blocking point). |
| `try_recv(pattern?)` | `try_recv` | scan `<me>/new/` once; return oldest match or `null`. Non-blocking. |
| `post(task)` | `post` | write to `tmp/` → rename to `tasks/open/`. |
| `claim()` | `claim` | list `tasks/open/`, attempt `rename(open/T → claimed/<me>/T)`; first winner gets it, `ENOENT` = lost the race, retry next. Atomic, exactly-one. |
| `complete(task_id, result)` | `complete` | write result envelope, `rename(claimed/<me>/T → done/T)`. |

**Traceability helper (for "let me trace what happens"):**
- `inbox(actor?)` — list `new/` (pending) and recent `cur/` (processed) for an actor, plus
  `tasks/{open,claimed,done}` counts. Read-only snapshot of the whole exchange.
- Because every message is a file and `recv` *moves* rather than deletes (`new/ → cur/`), `cur/` is
  an append-only audit log: you can `ls -t` any mailbox to replay the conversation, and the files
  are diffable. Nothing is hidden in a pipe or a server's memory.

## Delivery semantics (v0)

- **At-least-once + idempotent**, dedup on envelope `id`. v0 simplification: `recv` moves
  `new/→cur/` on delivery; a crash *after* `recv` returns but *before* the agent acts would lose
  that message. Hardening (v0+): keep a `proc/` staging dir and an explicit `ack(id)` that finalizes
  `proc/→cur/`, so unacked messages are re-delivered on restart. Not needed for the first loop.
- **Lease/heartbeat/abandon** (the robustness layer) are **out of v0** — added once the basic
  post/claim/complete loop runs, per the protocol.

## Permissions & registration

- Register as a project MCP server in `.mcp.json`:
  ```json
  { "mcpServers": { "collab": { "command": "tools/.venv/bin/python",
                                 "args": ["-m", "collab_mcp"],
                                 "env": { "COLLAB_ACTOR": "claude" } } } }
  ```
- One-time permission grant `mcp__collab__*` in settings → no further prompts.
- Plugin packaging (skill + MCP server + optional SessionStart "catch up on `new/`" hook) is a later
  step; a bare `.mcp.json` server is enough to start.

## v0 cut & open items

- **v0:** `send`/`recv`/`try_recv` + `post`/`claim`/`complete` + `inbox`, maildir backend, one
  worker (Claude) + the human/coordinator, worktrees for isolation. Prove the "block only on `recv`"
  loop end-to-end.
- **Next:** at-least-once `ack` hardening; lease/`heartbeat`/`abandon`; a coordinator that merges
  worker branches (rebase + `cargo test` + keep `main` green); then the Redis backend swap.
- **Interop check with agy:** confirm the exact envelope field names and the `scratch/coordination/`
  layout match agy's adapter, since those two things are the only shared contract.
