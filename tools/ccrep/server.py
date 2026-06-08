"""FastMCP server exposing the CCREP Phase-1 Evidence Ledger as MCP tools.

Thin wrapper over ``store.CcrepStore`` (same convention as
``collab_mcp.server``). Repo root, DB path, and environment descriptor come from
the environment so the same code serves any checkout:

    CCREP_REPO_ROOT   git repo to evaluate in worktrees (default ".")
    CCREP_DB          SQLite ledger path (default: "scratch/ccrep.db" resolved
                      against the canonical primary-worktree repo root)
    CCREP_ENV         environment descriptor folded into the eval-cache key
                      (default "macos-apfs-phase1")

Register in .mcp.json ALONGSIDE the existing `collab` server (do not replace it):

    {
      "mcpServers": {
        "collab": { ... existing ... },
        "ccrep": {
          "command": "tools/.venv/bin/python",
          "args": ["-m", "ccrep"],
          "env": { "PYTHONPATH": "tools" }
        }
      }
    }

Then grant `mcp__ccrep__*` once. The Phase-1 tool set is exactly the seven from
the synthesis §"MCP Surface": claim_task, submit_proposal, run_evaluation,
submit_critique, submit_revision, compute_consensus, merge_proposal.
"""

from __future__ import annotations

import os
from typing import Any, Optional

import click
from mcp.server.fastmcp import FastMCP

from .store import CcrepStore


def _get_store() -> CcrepStore:
    return CcrepStore(
        repo_root=os.environ.get("CCREP_REPO_ROOT", "."),
        db_path=os.environ.get("CCREP_DB"),
        env_descriptor=os.environ.get("CCREP_ENV", "macos-apfs-phase1"),
    )


mcp = FastMCP("ccrep")


@mcp.tool()
def claim_task(task_id: str, agent_id: str) -> dict:
    """Record that an agent claimed a task (OPEN -> CLAIMED)."""
    with _get_store() as s:
        return s.claim_task(task_id, agent_id)


@mcp.tool()
def submit_proposal(
    task_id: str,
    author: str,
    branch: str,
    artifact_profile: str,
    description: str,
    change_summary: list[str],
    commit_sha: Optional[str] = None,
    repo: str = "deep-cuts",
    human_gate: Optional[list[str]] = None,
) -> dict:
    """Submit a proposal. The branch is resolved to an immutable commit_sha now.

    ``artifact_profile`` is one of code_change | code_review | design_doc and
    selects the eval suite + gate components. Returns the stored Proposal and the
    derived ConsensusState.
    """
    with _get_store() as s:
        return s.submit_proposal(
            task_id=task_id,
            author=author,
            branch=branch,
            artifact_profile=artifact_profile,
            description=description,
            change_summary=change_summary,
            commit_sha=commit_sha,
            repo=repo,
            human_gate=human_gate,
        )


@mcp.tool()
def run_evaluation(
    proposal_id: str,
    suite_override: Optional[list[dict]] = None,
    dataset_hash: str = "none",
    use_cache: bool = True,
) -> dict:
    """Run the proposal's profile eval suite in a disposable git worktree.

    Content-addressed by (commit_sha, suite_hash, dataset_hash, env_hash): an
    unchanged-input eval is served from cache, never re-run. Returns the
    EvaluationReport and the refreshed ConsensusState.
    """
    with _get_store() as s:
        return s.run_evaluation(
            proposal_id,
            suite_override=suite_override,
            dataset_hash=dataset_hash,
            use_cache=use_cache,
        )


@mcp.tool()
def submit_critique(critique: dict) -> dict:
    """Submit a structured Critique (validated for structure + evidence links).

    Each file_line evidence URI must resolve at the proposal's pinned commit; a
    dead link makes the critique malformed and is rejected pre-review. An author
    cannot submit an `approve` critique on their own proposal (no self-approval).
    """
    with _get_store() as s:
        return s.submit_critique(critique)


@mcp.tool()
def submit_revision(
    previous_proposal_id: str,
    author: str,
    branch: str,
    artifact_profile: str,
    description: str,
    change_summary: list[str],
    commit_sha: Optional[str] = None,
    repo: str = "deep-cuts",
    human_gate: Optional[list[str]] = None,
) -> dict:
    """Submit a new revision superseding a prior proposal. New commit => prior
    approvals no longer apply (votes expire on code change)."""
    with _get_store() as s:
        return s.submit_revision(
            previous_proposal_id=previous_proposal_id,
            author=author,
            branch=branch,
            artifact_profile=artifact_profile,
            description=description,
            change_summary=change_summary,
            commit_sha=commit_sha,
            repo=repo,
            human_gate=human_gate,
        )


@mcp.tool()
def compute_consensus(task_id: str) -> dict:
    """Re-fold the event log and return the derived ConsensusState.

    Always computed from the log — never accepts caller-supplied state.
    """
    with _get_store() as s:
        return s.compute_consensus(task_id)


@mcp.tool()
def merge_proposal(
    proposal_id: str,
    merged_by: str,
    human_confirmed: bool = False,
    target_branch: Optional[str] = None,
) -> dict:
    """Human-gated merge. Refuses unless the Phase-1 gate is satisfied (green
    automated checks + 1 independent approval + no open blocking critiques), and
    refuses/flags for human confirmation on public_api_change,
    destructive_migration, model_or_dataset_change, large_architecture_change.
    """
    with _get_store() as s:
        return s.merge_proposal(
            proposal_id,
            merged_by=merged_by,
            human_confirmed=human_confirmed,
            target_branch=target_branch,
        )


@click.command()
def main() -> None:
    """Launch the CCREP Phase-1 Evidence Ledger MCP server (stdio transport)."""
    mcp.run()


if __name__ == "__main__":
    main()
