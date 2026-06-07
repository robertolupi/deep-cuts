# Claude's Collab Adapter — MCP Server over the Maildir Backend

Claude's original implementation of the [coordination-protocol.md](coordination-protocol.md)
contract. It now serves as the shared MCP implementation for any actor that can run the server
(`COLLAB_ACTOR=claude`, `agy`, `codex`, etc.), while preserving the same maildir backend layout and
message envelope.

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
- Actor identity is fixed by config/env at launch (`COLLAB_ACTOR=claude`) or supplied per call via
  the optional `actor` argument, so one project registration can serve Claude, agy, Codex, or another
  participant handle.

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
| `complete(task_id, result)` | `complete` | write result envelope, move `claimed/<me>/T → done/T`; task lookup matches `envelope.id`, not filename. |
| `heartbeat(task_id)` | `heartbeat` | extend the lease on a claimed task. |
| `abandon(task_id, reason)` | `abandon` | return a claimed task to `open/` with diagnostics. |
| `sweep()` | `sweep` | coordinator operation that returns expired claimed tasks to `open/`. |

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
- **Lease/heartbeat/abandon/sweep** are implemented for crash recovery. A coordinator can sweep
  expired claims back to `open/`.

## Permissions & registration

- Register as a project MCP server in `.mcp.json`:
  ```json
  { "mcpServers": { "collab": { "command": "tools/.venv/bin/python",
                                 "args": ["-m", "collab_mcp"],
                                 "env": { "PYTHONPATH": "tools", "COLLAB_ACTOR": "claude" } } } }
  ```
- One-time permission grant `mcp__collab__*` in settings → no further prompts.
- Plugin packaging (skill + MCP server + optional SessionStart "catch up on `new/`" hook) is a later
  step; a bare `.mcp.json` server is enough to start.

## v0 cut & open items

- **v0:** `send`/`recv`/`try_recv` + `post`/`claim`/`complete` + lease operations + `inbox`, maildir
  backend, workers + the human/coordinator, worktrees for isolation. Prove the "block only on
  `recv`" loop end-to-end.
- **Next:** at-least-once `ack` hardening; a coordinator that merges worker branches (rebase +
  `cargo test` + keep `main` green); then the Redis backend swap.
- **Interop check with agy:** confirm the exact envelope field names and the `scratch/coordination/`
  layout match agy's adapter, since those two things are the only shared contract.
