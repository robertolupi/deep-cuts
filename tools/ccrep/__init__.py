"""CCREP Phase 1 — the Evidence Ledger.

Implements Phase 1 of the CCREP synthesis (doc/proposals/ccrep-synthesis.md):
an append-only event ledger, a reducer that folds it into ConsensusState while
enforcing the seven ratchet invariants, a git-worktree eval executor, and the
seven-tool MCP surface. Phases 2-4 (AST/line revision gates, plateau/edit-war
detection, any voting math, weighted quorum, Condorcet) are explicitly out of
scope.

``store.CcrepStore`` is the MCP-independent operation surface (unit-testable
standalone, same convention as ``collab_mcp.store``); ``server`` wraps it as MCP
tools.
"""

from .store import CcrepStore

__all__ = ["CcrepStore"]
