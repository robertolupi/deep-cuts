"""FastMCP server exposing the maildir MailStore as collab tools.

Thin wrapper over store.MailStore. Actor identity and backend root come from
the environment so the same code serves any actor:

    COLLAB_ACTOR   this server's mailbox name (default "claude")
    COLLAB_ROOT    shared coordination root  (default "scratch/coordination")

Register in .mcp.json (run from the repo root, with tools/ on PYTHONPATH):

    {
      "mcpServers": {
        "collab": {
          "command": "tools/.venv/bin/python",
          "args": ["-m", "collab_mcp"],
          "env": { "PYTHONPATH": "tools", "COLLAB_ACTOR": "claude" }
        }
      }
    }

Then grant `mcp__collab__*` once — no further permission prompts.
"""

from __future__ import annotations

import os
from typing import Any, Optional

from mcp.server.fastmcp import FastMCP

from .store import MailStore

_store = MailStore(
    os.environ.get("COLLAB_ROOT", "scratch/coordination"),
    os.environ.get("COLLAB_ACTOR", "claude"),
)

mcp = FastMCP("collab")


@mcp.tool()
def send(to: str, type: str, payload: Any, in_reply_to: Optional[str] = None) -> dict:
    """Send a message to another actor's mailbox. Non-blocking (write-then-rename)."""
    return _store.send(to, type, payload, in_reply_to)


@mcp.tool()
def recv(match_type: Optional[str] = None, timeout_s: float = 120) -> Optional[dict]:
    """Block until a (matching) message arrives or timeout elapses; return the envelope or null.

    This is the one blocking point of the actor loop. Pass match_type for selective receive.
    """
    return _store.recv(match_type, timeout_s)


@mcp.tool()
def try_recv(match_type: Optional[str] = None) -> Optional[dict]:
    """Non-blocking: return the oldest (matching) message or null."""
    return _store.try_recv(match_type)


@mcp.tool()
def post(payload: Any, type: str = "task") -> dict:
    """Enqueue a task into the shared open queue."""
    return _store.post(payload, type)


@mcp.tool()
def claim() -> Optional[dict]:
    """Atomically claim one open task (exactly one claimer wins) or null."""
    return _store.claim()


@mcp.tool()
def complete(task_id: str, result: Any) -> Optional[dict]:
    """Mark a task this actor has claimed as done, recording its result."""
    return _store.complete(task_id, result)


@mcp.tool()
def inbox(actor: Optional[str] = None) -> dict:
    """Read-only snapshot of a mailbox (new/ pending, cur/ recent) and task counts — for tracing."""
    return _store.inbox(actor)


def main() -> None:
    mcp.run()  # stdio transport


if __name__ == "__main__":
    main()
