# collab_mcp — Claude's coordination adapter (v0)

Maildir-backed MCP server implementing the actor coordination protocol
([../../doc/collab/coordination-protocol.md](../../doc/collab/coordination-protocol.md)).
This is Claude's side of the asymmetric design
([../../doc/collab/claude-mcp-adapter.md](../../doc/collab/claude-mcp-adapter.md)); agy connects
its own adapter to the same `scratch/coordination/` layout — they share only the directory layout
and the message envelope.

## Modules
- `store.py` — `MailStore`, the pure-stdlib maildir backend (send/recv/try_recv/post/claim/complete/inbox). No MCP dependency, so it is unit-testable on its own.
- `server.py` — thin FastMCP wrapper exposing the store methods as tools.
- `__main__.py` — `python -m collab_mcp` runs the stdio server.

## Test
```bash
cd tools && .venv/bin/python -m collab_mcp.test_store
```

## Register as an MCP server
Add to `.mcp.json` at the repo root, then grant `mcp__collab__*` once (no further prompts):
```json
{ "mcpServers": { "collab": {
    "command": "tools/.venv/bin/python",
    "args": ["-m", "collab_mcp"],
    "env": { "PYTHONPATH": "tools", "COLLAB_ACTOR": "claude" } } } }
```
`COLLAB_ACTOR` names this server's mailbox (default `claude`); `COLLAB_ROOT` is the shared root
(default `scratch/coordination`, gitignored).

## Tools
`send` · `recv` · `try_recv` · `post` · `claim` · `complete` · `inbox` — see `server.py` docstrings.

## v0 scope
Core ops + `inbox` tracing. Event-driven `recv` via `watchfiles` (poll fallback if absent). The
robustness layer (lease/TTL, `heartbeat`, `abandon`) and the Redis backend are deferred — both slot
in behind the same tool surface.
