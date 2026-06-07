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

def _get_store(actor: Optional[str] = None) -> MailStore:
    actor_name = actor or os.environ.get("COLLAB_ACTOR", "claude")
    return MailStore(
        os.environ.get("COLLAB_ROOT", "scratch/coordination"),
        actor_name,
    )

mcp = FastMCP("collab")


@mcp.tool()
def send(to: str, type: str, payload: Any, in_reply_to: Optional[str] = None, actor: Optional[str] = None) -> dict:
    """Send a message to another actor's mailbox. Non-blocking (write-then-rename)."""
    return _get_store(actor).send(to, type, payload, in_reply_to)


@mcp.tool()
def recv(match_type: Optional[str] = None, timeout_s: float = 120, actor: Optional[str] = None) -> Optional[dict]:
    """Block until a (matching) message arrives or timeout elapses; return the envelope or null.

    This is the one blocking point of the actor loop. Pass match_type for selective receive.
    """
    return _get_store(actor).recv(match_type, timeout_s)


@mcp.tool()
def try_recv(match_type: Optional[str] = None, actor: Optional[str] = None) -> Optional[dict]:
    """Non-blocking: return the oldest (matching) message or null."""
    return _get_store(actor).try_recv(match_type)


@mcp.tool()
def post(payload: Any, type: str = "task", actor: Optional[str] = None) -> dict:
    """Enqueue a task into the shared open queue."""
    return _get_store(actor).post(payload, type)


@mcp.tool()
def claim(lease_ttl: float = 120.0, actor: Optional[str] = None) -> Optional[dict]:
    """Atomically claim one open task (exactly one claimer wins) or null.

    The claim carries a lease: if you don't complete or heartbeat within lease_ttl
    seconds, a coordinator sweep() may return the task to the open queue.
    """
    return _get_store(actor).claim(lease_ttl)


@mcp.tool()
def complete(task_id: str, result: Any, actor: Optional[str] = None) -> Optional[dict]:
    """Mark a task this actor has claimed as done, recording its result."""
    return _get_store(actor).complete(task_id, result)


@mcp.tool()
def heartbeat(task_id: str, lease_ttl: float = 120.0, actor: Optional[str] = None) -> Optional[dict]:
    """Extend the lease on a task you hold so a sweep won't reclaim it. Null if not held."""
    return _get_store(actor).heartbeat(task_id, lease_ttl)


@mcp.tool()
def abandon(task_id: str, reason: str, actor: Optional[str] = None) -> Optional[dict]:
    """Release a task you hold back to the open queue with a diagnostic reason. Null if not held."""
    return _get_store(actor).abandon(task_id, reason)


@mcp.tool()
def sweep(actor: Optional[str] = None) -> list[dict]:
    """Coordinator op: return any expired-lease tasks (across all actors) to the open queue."""
    return _get_store(actor).sweep()


@mcp.tool()
def inbox(actor: Optional[str] = None) -> dict:
    """Read-only snapshot of a mailbox (new/ pending, cur/ recent) and task counts — for tracing."""
    return _get_store(actor).inbox(actor)


def main() -> None:
    mcp.run()  # stdio transport


if __name__ == "__main__":
    main()
