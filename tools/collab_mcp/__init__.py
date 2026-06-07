"""Collab coordination adapter — maildir-backed message + task store and MCP server.

Implements the contract in doc/collab/coordination-protocol.md. `store.MailStore`
is pure stdlib (unit-testable standalone); `server` wraps it as MCP tools.
"""

from .store import MailStore

__all__ = ["MailStore"]
