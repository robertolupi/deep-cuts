"""CCREP orchestration store — the MCP-independent operation surface.

Same convention as ``collab_mcp.store``: all logic lives here so it is
unit-testable standalone; ``server.py`` is a thin FastMCP wrapper that just
forwards to a ``CcrepStore``.

A ``CcrepStore`` composes the three code layers:
  * ``Ledger``   — append-only event log + eval cache + derived-view storage,
  * ``reducer``  — fold the log into ConsensusState (the invariants),
  * ``WorktreeExecutor`` — run eval suites + resolve critique evidence links.

Every mutating op appends an event, then re-folds and re-materializes so the
derived tables stay a pure function of the log (invariant 5). Reads come from the
reduction, never from agent-supplied state.
"""

from __future__ import annotations

import uuid
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Optional

from . import ledger as ledger_mod
from . import reducer as reducer_mod
from . import schemas
from .executor import WorktreeExecutor
from .ledger import Ledger
from .profiles import HUMAN_GATE_CATEGORIES
from .schemas import ARTIFACT_PROFILES


def _now_iso() -> str:
    return datetime.now(timezone.utc).isoformat()


class CcrepError(ValueError):
    """Raised on a rejected operation (malformed input or invariant violation)."""


class CcrepStore:
    def __init__(
        self,
        repo_root: str | Path,
        db_path: str | Path | None = None,
        env_descriptor: str = "macos-apfs-phase1",
    ) -> None:
        self.repo_root = Path(repo_root).resolve()
        self.ledger = Ledger(db_path)
        self.executor = WorktreeExecutor(self.repo_root, self.ledger, env_descriptor)

    def close(self) -> None:
        self.ledger.close()

    def __enter__(self) -> "CcrepStore":
        return self

    def __exit__(self, *exc: Any) -> None:
        self.close()

    # -- internal: re-fold + re-materialize one task ----------------------
    def _refresh(self, task_id: str) -> dict:
        events = self.ledger.events(task_id)
        reduction = reducer_mod.reduce_task(events)
        self.ledger.materialize(reduction.snapshot)
        return reduction.consensus

    # -- tools ------------------------------------------------------------
    def claim_task(self, task_id: str, agent_id: str) -> dict:
        """Record that ``agent_id`` claimed ``task_id`` (OPEN -> CLAIMED)."""
        self.ledger.append(
            task_id,
            ledger_mod.EVENT_TASK_CLAIMED,
            {"agent_id": agent_id},
            actor=agent_id,
        )
        return {"task_id": task_id, "claimed_by": agent_id}

    def submit_proposal(
        self,
        task_id: str,
        author: str,
        branch: str,
        artifact_profile: str,
        description: str,
        change_summary: list[str],
        commit_sha: Optional[str] = None,
        repo: str = "deep-cuts",
        revision: int = 0,
        supersedes: Optional[str] = None,
        human_gate: Optional[list[str]] = None,
        extra: Optional[dict] = None,
    ) -> dict:
        """Submit a proposal: resolve branch -> immutable commit, validate, append.

        Invariant 1: the branch is resolved to a fixed commit_sha now; later
        branch movement does not change this proposal.
        """
        if artifact_profile not in ARTIFACT_PROFILES:
            raise CcrepError(
                f"unknown artifact_profile {artifact_profile!r}; "
                f"expected one of {list(ARTIFACT_PROFILES)}"
            )
        sha = commit_sha or self.executor.resolve_commit(branch)
        proposal = {
            "proposal_id": uuid.uuid4().hex,
            "task_id": task_id,
            "revision": revision,
            "author": author,
            "git": {"repo": repo, "branch": branch, "commit_sha": sha},
            "created_at": _now_iso(),
            "description": description,
            "change_summary": list(change_summary),
            "artifact_profile": artifact_profile,
            "status": "submitted",
        }
        if supersedes:
            proposal["supersedes"] = supersedes
        schemas.validate("Proposal", proposal)
        # Non-schema metadata the reducer/executor consult (kept off the schema
        # object, attached to the event payload around it).
        meta = {"proposal": proposal}
        if human_gate:
            bad = [c for c in human_gate if c not in HUMAN_GATE_CATEGORIES]
            if bad:
                raise CcrepError(f"unknown human_gate categories: {bad}")
            proposal["human_gate"] = human_gate
        if extra:
            proposal.update(extra)
        self.ledger.append(
            task_id,
            ledger_mod.EVENT_PROPOSAL_SUBMITTED
            if revision == 0
            else ledger_mod.EVENT_REVISION_SUBMITTED,
            meta,
            actor=author,
        )
        consensus = self._refresh(task_id)
        return {"proposal": proposal, "consensus": consensus}

    def submit_revision(
        self,
        previous_proposal_id: str,
        author: str,
        branch: str,
        artifact_profile: str,
        description: str,
        change_summary: list[str],
        commit_sha: Optional[str] = None,
        repo: str = "deep-cuts",
        human_gate: Optional[list[str]] = None,
        extra: Optional[dict] = None,
    ) -> dict:
        """Submit a new revision superseding ``previous_proposal_id``.

        Invariant 3: a new commit invalidates prior approvals — the revision is a
        distinct proposal with its own (empty) vote set, so old approvals simply
        do not apply to it.
        """
        prev = self._find_proposal(previous_proposal_id)
        if prev is None:
            raise CcrepError(f"unknown previous proposal {previous_proposal_id}")
        return self.submit_proposal(
            task_id=prev["task_id"],
            author=author,
            branch=branch,
            artifact_profile=artifact_profile,
            description=description,
            change_summary=change_summary,
            commit_sha=commit_sha,
            repo=repo,
            revision=int(prev["revision"]) + 1,
            supersedes=previous_proposal_id,
            human_gate=human_gate,
            extra=extra,
        )

    def run_evaluation(
        self,
        proposal_id: str,
        suite_override: Optional[list[dict]] = None,
        dataset_hash: str = "none",
        use_cache: bool = True,
        timeout_s: int = 1800,
    ) -> dict:
        """Run the proposal's profile eval suite in a worktree; store the report."""
        proposal = self._find_proposal(proposal_id)
        if proposal is None:
            raise CcrepError(f"unknown proposal {proposal_id}")
        report = self.executor.run_evaluation(
            proposal,
            suite_override=suite_override,
            dataset_hash=dataset_hash,
            use_cache=use_cache,
            timeout_s=timeout_s,
        )
        # Append the report (strip internal annotations are kept on payload so the
        # reducer can enforce invariant 6).
        self.ledger.append(
            proposal["task_id"],
            ledger_mod.EVENT_EVALUATION_COMPLETED,
            {"report": report},
            actor="executor",
        )
        consensus = self._refresh(proposal["task_id"])
        return {"report": report, "consensus": consensus}

    def submit_critique(self, critique: dict) -> dict:
        """Submit a structured critique.

        Two mechanical gates run pre-review (synthesis §"Implementation Split"):
          * schema-structure validation (severity class, evidence link, fields),
          * evidence-link resolution — each ``file_line`` must resolve at the
            proposal's pinned commit; a dead link makes the critique malformed.
        Quality/admissibility ("specific + actionable") is reviewer judgment and
        is NOT checked here.
        """
        # Mint ids if the caller omitted them, so well-formed-but-id-less critiques
        # from agents still validate.
        critique = dict(critique)
        critique.setdefault("critique_id", uuid.uuid4().hex)
        critique.setdefault("created_at", _now_iso())
        for f in critique.get("findings", []):
            f.setdefault("finding_id", uuid.uuid4().hex)

        schemas.validate("Critique", critique)

        proposal = self._find_proposal(critique["proposal_id"])
        if proposal is None:
            raise CcrepError(f"critique for unknown proposal {critique['proposal_id']}")

        dead = self.executor.resolve_evidence_links(
            critique, proposal["git"]["commit_sha"]
        )
        if dead:
            raise CcrepError(
                "critique rejected: evidence link(s) do not resolve at the "
                "proposed commit (malformed pre-review): " + "; ".join(dead)
            )

        # Invariant 4 enforced at the source too: an author cannot self-approve.
        if (
            critique["reviewer"] == proposal["author"]
            and critique.get("stance") == "approve"
        ):
            raise CcrepError(
                "invariant 4: author cannot submit an `approve` critique on their "
                "own proposal (no self-approval)"
            )

        self.ledger.append(
            proposal["task_id"],
            ledger_mod.EVENT_CRITIQUE_SUBMITTED,
            {"critique": critique},
            actor=critique["reviewer"],
        )
        consensus = self._refresh(proposal["task_id"])
        return {"critique": critique, "consensus": consensus}

    def compute_consensus(self, task_id: str) -> dict:
        """Re-fold the log and return the derived ConsensusState (invariant 5).

        This never accepts a caller-supplied state; it is always computed.
        """
        consensus = self._refresh(task_id)
        schemas.validate("ConsensusState", consensus)
        return consensus

    def merge_proposal(
        self,
        proposal_id: str,
        merged_by: str,
        human_confirmed: bool = False,
        target_branch: Optional[str] = None,
    ) -> dict:
        """Human-gated merge. Refuses unless the Phase-1 gate is satisfied.

        Refuses / flags for human confirmation when the proposal declares any
        human-gate category (public_api_change, destructive_migration,
        model_or_dataset_change, large_architecture_change). The block is
        unbypassable in code unless ``human_confirmed`` is explicitly passed.
        """
        proposal = self._find_proposal(proposal_id)
        if proposal is None:
            raise CcrepError(f"unknown proposal {proposal_id}")
        consensus = self._refresh(proposal["task_id"])

        decision = consensus["decision"]
        state = consensus["state"]

        if state == "human_review_required" and not human_confirmed:
            cats = consensus.get("human_gate_categories", [])
            return {
                "merged": False,
                "requires_human": True,
                "reason": f"human confirmation required for: {', '.join(cats)}",
                "consensus": consensus,
            }

        if not decision["mergeable"] and not (
            state == "human_review_required" and human_confirmed
        ):
            return {
                "merged": False,
                "requires_human": False,
                "reason": decision["reason"],
                "consensus": consensus,
            }

        self.ledger.append(
            proposal["task_id"],
            ledger_mod.EVENT_MERGE_RECORDED,
            {
                "proposal_id": proposal_id,
                "merged_by": merged_by,
                "target_branch": target_branch,
                "human_confirmed": human_confirmed,
            },
            actor=merged_by,
        )
        consensus = self._refresh(proposal["task_id"])
        return {"merged": True, "reason": "gate satisfied", "consensus": consensus}

    # -- read helpers -----------------------------------------------------
    def _find_proposal(self, proposal_id: str) -> Optional[dict]:
        """Look up a proposal from the materialized view (refreshed lazily)."""
        cur = self.ledger._conn.execute(
            "SELECT snapshot FROM proposals WHERE proposal_id = ?", (proposal_id,)
        )
        row = cur.fetchone()
        if row is not None:
            import json

            return json.loads(row["snapshot"])
        # Fall back to folding from the log (covers the just-appended case where
        # materialization hasn't run yet for this task).
        for ev in self.ledger.events():
            payload = ev["payload"]
            prop = payload.get("proposal")
            if prop and prop.get("proposal_id") == proposal_id:
                return prop
        return None

    def get_consensus(self, task_id: str) -> dict:
        return self.compute_consensus(task_id)
