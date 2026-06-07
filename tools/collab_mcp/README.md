# collab_mcp — shared coordination adapter (v0)

Maildir-backed MCP server implementing the actor coordination protocol
([../../doc/collab/coordination-protocol.md](../../doc/collab/coordination-protocol.md)).
This started as Claude's side of the asymmetric design
([../../doc/collab/claude-mcp-adapter.md](../../doc/collab/claude-mcp-adapter.md)); the current
implementation is shared by any actor that can run the MCP server, parameterized by `COLLAB_ACTOR`
or by each tool call's optional `actor` argument.

## Modules
- `store.py` — `MailStore`, the pure-stdlib maildir backend (send/recv/try_recv/post/claim/complete/heartbeat/abandon/sweep/inbox). No MCP dependency, so it is unit-testable on its own.
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
`COLLAB_ACTOR` names this server's default mailbox (default `claude`); use `codex`, `claude`, `agy`,
or another session participant handle. `COLLAB_ROOT` is the shared root (default
`scratch/coordination`, gitignored). The checked-in project `.mcp.json` intentionally leaves
`COLLAB_ACTOR` unset so clients can share the registration; non-Claude clients should pass the
optional `actor` argument in tool calls.

## Tools
`send` · `recv` · `try_recv` · `post` · `claim` · `complete` · `heartbeat` · `abandon` · `sweep` ·
`inbox` — see `server.py` docstrings.

## v0 scope
Core ops + `inbox` tracing + the lease robustness layer (`heartbeat`, `abandon`, coordinator
`sweep`). Event-driven `recv` uses `watchfiles` when present, with a poll fallback. The Redis backend
is still deferred behind the same tool surface.
