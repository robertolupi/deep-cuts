"""The CCREP reducer — folds the append-only event log into ConsensusState.

This is the whole point of the package: the ratchet invariants live here, in
code, because an agent that could satisfy them by asserting them would make the
ratchet fake (synthesis §"Implementation Split"). The reducer is a pure function
of an immutable event list; it never writes to the log.

Invariants enforced (synthesis §"Invariants" 1-7):

  1. Immutable proposal version — a proposal pins a fixed ``commit_sha``; a new
     commit is a new revision (handled by ``submit_revision`` minting a new
     proposal; the reducer treats each as distinct and keys votes by commit).
  2. Content-addressed evaluations — the executor caches by the 4-tuple; the
     reducer consumes the report and attributes it to (proposal, commit).
  3. Votes expire on code change — an approval is only counted for the LATEST
     commit of its proposal lineage; any newer commit invalidates it.
  4. No self-approval — the proposal author's own approval never satisfies the
     independent-approval quorum.
  5. Derived consensus state — there is no event that writes ConsensusState; it
     is only ever produced here by folding the log. ``reduce_task`` rejects any
     event that purports to carry consensus state directly.
  6. Artifact-profile consistency — a hard-check / metric / finding that names a
     gate component the proposal's profile does not own is dropped (never fires).
  7. One-directional frontmatter-status sync (design_doc) — surfaced as a flag in
     the decision; the linter (executor side) fails on ``accepted``-without-
     APPROVED, never on the human's merge gesture.

Phase 1 gate = green automated checks + one independent approval (reviewer !=
author) + no open blocking critiques. No voting math (deferred to Phases 3-4).
"""

from __future__ import annotations

from dataclasses import dataclass, field
from datetime import datetime, timezone
from typing import Optional

from . import ledger as ledger_mod
from .ledger import MaterializedSnapshot
from .profiles import (
    HUMAN_GATE_CATEGORIES,
    get_profile,
)

GATE_POLICY_VERSION = "ccrep-phase1-v1"

# Severities that block merge when a finding is marked blocking (synthesis: the
# *structure* — severity class + blocks_merge flag — is mechanically checkable).
_BLOCKING_SEVERITIES = {"blocking", "critical"}


def _now_iso() -> str:
    return datetime.now(timezone.utc).isoformat()


class ConsensusError(ValueError):
    """Raised when an event violates a reducer invariant at fold time."""


@dataclass
class _ProposalState:
    proposal: dict
    revision: int
    commit_sha: str
    author: str
    profile: str
    supersedes: Optional[str]
    reports: list[dict] = field(default_factory=list)
    critiques: list[dict] = field(default_factory=list)
    votes: dict[str, dict] = field(default_factory=dict)  # agent_id -> vote record
    merged: Optional[dict] = None


@dataclass
class TaskReduction:
    """Result of folding one task's events."""

    task_id: str
    proposals: dict[str, _ProposalState]
    consensus: dict
    snapshot: MaterializedSnapshot


def _latest_proposal(proposals: dict[str, _ProposalState]) -> Optional[_ProposalState]:
    """The head of the proposal lineage = highest revision (latest commit)."""
    if not proposals:
        return None
    return max(proposals.values(), key=lambda ps: ps.revision)


def _filter_report_to_profile(report: dict, profile_name: str) -> dict:
    """Invariant 6: drop hard-checks/metrics whose component isn't owned by the profile.

    The EvaluationReport records each hard-check by name; the executor tags which
    gate component produced it via the report's ``_components`` map (an internal,
    non-schema annotation). Anything tagged to a component the profile does not
    own is removed so it can never fire in the gate.
    """
    profile = get_profile(profile_name)
    components = report.get("_components", {})
    if not components:
        return report  # nothing tagged => nothing to filter (e.g. test fixtures)
    kept_checks = [
        hc
        for hc in report.get("hard_checks", [])
        if profile.owns(components.get(hc["name"], hc["name"]))
        or components.get(hc["name"]) is None
    ]
    kept_metrics = {
        name: m
        for name, m in report.get("metrics", {}).items()
        if profile.owns(components.get(name, name))
        or components.get(name) is None
    }
    out = dict(report)
    out["hard_checks"] = kept_checks
    out["metrics"] = kept_metrics
    return out


def reduce_task(events: list[dict]) -> TaskReduction:
    """Fold one task's events into proposal state + a derived ConsensusState.

    ``events`` must all share a task_id (the caller slices the log per task).
    """
    if not events:
        raise ConsensusError("reduce_task requires at least one event")
    task_id = events[0]["task_id"]

    proposals: dict[str, _ProposalState] = {}
    claimed_by: Optional[str] = None

    for ev in events:
        if ev["task_id"] != task_id:
            raise ConsensusError(
                f"reduce_task got mixed task_ids: {task_id} vs {ev['task_id']}"
            )
        kind = ev["kind"]
        payload = ev["payload"]

        # Invariant 5: there is no event kind that writes ConsensusState; reject
        # any attempt to inject derived state directly into the log.
        if kind in ("consensus_state", "set_consensus") or "consensus_state" in payload:
            raise ConsensusError(
                "invariant 5: ConsensusState is derived, never written by an event"
            )

        if kind == ledger_mod.EVENT_TASK_CLAIMED:
            claimed_by = payload.get("agent_id") or ev.get("actor")

        elif kind in (
            ledger_mod.EVENT_PROPOSAL_SUBMITTED,
            ledger_mod.EVENT_REVISION_SUBMITTED,
        ):
            prop = payload["proposal"]
            ps = _ProposalState(
                proposal=prop,
                revision=int(prop["revision"]),
                commit_sha=prop["git"]["commit_sha"],
                author=prop["author"],
                profile=prop["artifact_profile"],
                supersedes=prop.get("supersedes"),
            )
            proposals[prop["proposal_id"]] = ps
            # A revision supersedes its parent => parent is no longer a candidate.
            if ps.supersedes and ps.supersedes in proposals:
                proposals[ps.supersedes].proposal = {
                    **proposals[ps.supersedes].proposal,
                    "status": "superseded",
                }

        elif kind == ledger_mod.EVENT_EVALUATION_COMPLETED:
            report = payload["report"]
            ps = proposals.get(report["proposal_id"])
            if ps is None:
                raise ConsensusError(
                    f"evaluation for unknown proposal {report['proposal_id']}"
                )
            # Invariant 1/2: a report is bound to the proposal's pinned commit.
            if report["commit_sha"] != ps.commit_sha:
                raise ConsensusError(
                    "invariant 1: evaluation commit_sha does not match the "
                    "proposal's pinned commit"
                )
            ps.reports.append(_filter_report_to_profile(report, ps.profile))

        elif kind == ledger_mod.EVENT_CRITIQUE_SUBMITTED:
            critique = payload["critique"]
            ps = proposals.get(critique["proposal_id"])
            if ps is None:
                raise ConsensusError(
                    f"critique for unknown proposal {critique['proposal_id']}"
                )
            ps.critiques.append(critique)
            # A critique carries a stance; an `approve` stance counts as a vote
            # against the exact commit it was cast on.
            stance = critique.get("stance")
            if stance in ("approve", "request_changes", "veto", "abstain"):
                ps.votes[critique["reviewer"]] = {
                    "agent_id": critique["reviewer"],
                    "proposal_id": ps.proposal["proposal_id"],
                    "task_id": task_id,
                    "commit_sha": ps.commit_sha,
                    "vote": stance,
                }

        elif kind == ledger_mod.EVENT_MERGE_RECORDED:
            pid = payload["proposal_id"]
            ps = proposals.get(pid)
            if ps is None:
                raise ConsensusError(f"merge for unknown proposal {pid}")
            ps.merged = {
                "proposal_id": pid,
                "task_id": task_id,
                "commit_sha": ps.commit_sha,
                "merged_by": payload.get("merged_by"),
            }

        else:
            raise ConsensusError(f"unknown event kind {kind!r}")

    consensus = _compute_consensus(task_id, proposals, claimed_by)
    snapshot = _build_snapshot(proposals)
    return TaskReduction(
        task_id=task_id,
        proposals=proposals,
        consensus=consensus,
        snapshot=snapshot,
    )


def _open_blocking_findings(ps: _ProposalState) -> list[str]:
    """finding_ids of unresolved blocking findings on this proposal's commit.

    Invariant 3 (votes expire on code change) applies to critiques too: only
    critiques cast against the proposal's CURRENT commit count. Since each
    _ProposalState is a single immutable commit, its own critiques are all
    on-commit; carry-over from a superseded revision is the caller's concern.
    """
    blocking: list[str] = []
    for cr in ps.critiques:
        for f in cr.get("findings", []):
            is_blocking = f.get("blocks_merge") or (
                f.get("severity") in _BLOCKING_SEVERITIES
            )
            if is_blocking:
                blocking.append(f["finding_id"])
    return blocking


def _evaluation_passed(ps: _ProposalState) -> Optional[bool]:
    """True/False if the latest report decides the gate; None if no report yet."""
    if not ps.reports:
        return None
    latest = ps.reports[-1]
    if latest.get("status") != "passed":
        return False
    # Every hard check must pass (golden-metric checks ride in here too).
    return all(hc.get("passed") for hc in latest.get("hard_checks", []))


def _independent_approval(ps: _ProposalState) -> bool:
    """Invariant 4: at least one approval from a reviewer who is NOT the author.

    Invariant 3: votes are keyed to ps.commit_sha, so an approval that predates
    the current commit simply isn't in this proposal's vote set.
    """
    for agent_id, v in ps.votes.items():
        if v["vote"] != "approve":
            continue
        if agent_id == ps.author:
            continue  # self-approval cannot satisfy peer quorum
        if v["commit_sha"] != ps.commit_sha:
            continue  # vote on a stale commit (defensive; should not happen here)
        return True
    return False


def _human_gate_required(ps: _ProposalState) -> list[str]:
    """Human-gate categories the proposal declares it touches (invariant block).

    The proposal may carry a ``human_gate`` list in its change metadata; the
    block is unbypassable in code (merge_proposal refuses), classification of
    what counts is a rule.
    """
    declared = ps.proposal.get("human_gate") or ps.proposal.get(
        "human_gate_categories", []
    )
    return [c for c in declared if c in HUMAN_GATE_CATEGORIES]


def _frontmatter_flag(ps: _ProposalState, approved: bool) -> Optional[str]:
    """Invariant 7 surfacing for design_doc proposals (one-directional).

    Returns a warning string if the doc claims ``status: accepted`` without
    having reached APPROVED. Never blocks the human's merge gesture — it is a
    decision-level flag the linter (executor) turns into a failed check.
    """
    if ps.profile != "design_doc":
        return None
    claimed_status = ps.proposal.get("frontmatter_status")
    if claimed_status == "accepted" and not approved:
        return (
            "invariant 7: design_doc declares status: accepted without a reached "
            "APPROVED state (green checks + independent approval + no blocking "
            "critiques)"
        )
    return None


def _compute_consensus(
    task_id: str,
    proposals: dict[str, _ProposalState],
    claimed_by: Optional[str],
) -> dict:
    """Reduce to a schema-valid ConsensusState (Phase-1 single-approval gate)."""
    head = _latest_proposal(proposals)
    candidate_ids = [head.proposal["proposal_id"]] if head else []

    votes_out: list[dict] = []
    tallies = {"approve": 0.0, "request_changes": 0.0, "abstain": 0.0, "veto": 0.0}
    open_blocking: list[str] = []
    next_actions: list[str] = []

    if head is None:
        state = "collecting_proposals" if claimed_by else "collecting_proposals"
        decision = {
            "mergeable": False,
            "selected_proposal_id": None,
            "reason": "no proposal submitted yet",
            "next_actions": ["submit_proposal"],
        }
        return _consensus_state(
            task_id, state, candidate_ids, votes_out, tallies, open_blocking, decision
        )

    for agent_id, v in head.votes.items():
        # Phase 1 has no weighting; record weight 1.0 for schema validity, but
        # NEVER tally a self-approval into the peer quorum (invariant 4).
        counted = not (v["vote"] == "approve" and agent_id == head.author)
        votes_out.append(
            {
                "agent_id": agent_id,
                "proposal_id": v["proposal_id"],
                "vote": v["vote"],
                "weight": 1.0 if counted else 0.0,
                "domains": {},
            }
        )
        if counted:
            tallies[v["vote"]] = tallies.get(v["vote"], 0.0) + 1.0

    open_blocking = _open_blocking_findings(head)

    eval_passed = _evaluation_passed(head)
    has_independent = _independent_approval(head)
    human_cats = _human_gate_required(head)
    frontmatter_warn = _frontmatter_flag(head, approved=(
        eval_passed and has_independent and not open_blocking
    ))

    # ---- Phase-1 gate ----------------------------------------------------
    reasons: list[str] = []
    if eval_passed is None:
        reasons.append("evaluation not yet run")
        next_actions.append("run_evaluation")
    elif eval_passed is False:
        reasons.append("automated checks did not pass")
        next_actions.append("submit_revision")
    if not has_independent:
        reasons.append("no independent approval (reviewer != author)")
        next_actions.append("submit_critique:approve")
    if open_blocking:
        reasons.append(f"{len(open_blocking)} open blocking critique(s)")
        next_actions.append("submit_revision")

    approved = (eval_passed is True) and has_independent and not open_blocking

    if head.merged is not None:
        state = "merged"
        decision = {
            "mergeable": True,
            "selected_proposal_id": head.proposal["proposal_id"],
            "reason": "merged",
            "next_actions": [],
        }
    elif approved and human_cats:
        state = "human_review_required"
        decision = {
            "mergeable": False,
            "selected_proposal_id": head.proposal["proposal_id"],
            "reason": "gate satisfied but human review required for: "
            + ", ".join(human_cats),
            "next_actions": ["merge_proposal (human-gated)"],
        }
    elif approved:
        state = "consensus_ready"
        decision = {
            "mergeable": True,
            "selected_proposal_id": head.proposal["proposal_id"],
            "reason": "green automated checks + 1 independent approval + no open "
            "blocking critiques",
            "next_actions": ["merge_proposal"],
        }
    elif eval_passed is False or open_blocking:
        state = "revision_required"
        decision = {
            "mergeable": False,
            "selected_proposal_id": None,
            "reason": "; ".join(reasons),
            "next_actions": sorted(set(next_actions)),
        }
    elif eval_passed is None:
        state = "evaluating"
        decision = {
            "mergeable": False,
            "selected_proposal_id": None,
            "reason": "; ".join(reasons),
            "next_actions": sorted(set(next_actions)),
        }
    else:
        state = "reviewing"
        decision = {
            "mergeable": False,
            "selected_proposal_id": None,
            "reason": "; ".join(reasons),
            "next_actions": sorted(set(next_actions)),
        }

    cs = _consensus_state(
        task_id, state, candidate_ids, votes_out, tallies, open_blocking, decision
    )
    if frontmatter_warn:
        cs["decision"].setdefault("next_actions", []).append(frontmatter_warn)
    if human_cats:
        cs["human_gate_categories"] = human_cats  # consumed by merge_proposal's human gate
    return cs


def _consensus_state(
    task_id: str,
    state: str,
    candidate_ids: list[str],
    votes: list[dict],
    tallies: dict,
    open_blocking: list[str],
    decision: dict,
) -> dict:
    return {
        "task_id": task_id,
        "state": state,
        "computed_at": _now_iso(),
        "gate_policy_version": GATE_POLICY_VERSION,
        "candidate_proposals": candidate_ids,
        "votes": votes,
        "weighted_tallies": tallies,
        "open_blocking_findings": open_blocking,
        "decision": decision,
    }


def _build_snapshot(proposals: dict[str, _ProposalState]) -> MaterializedSnapshot:
    """Project the folded proposal state into the materialized-view rows.

    Reports and critiques are deduped by their primary id: a content-addressed
    cache hit re-records an identical report under the same ``report_id``, and a
    full re-fold must collapse those to a single derived row rather than violate
    the table's UNIQUE constraint.
    """
    prop_rows: list[dict] = []
    report_rows: list[dict] = []
    critique_rows: list[dict] = []
    vote_rows: list[dict] = []
    merge_rows: list[dict] = []
    seen_reports: set[str] = set()
    seen_critiques: set[str] = set()

    for pid, ps in proposals.items():
        prop_rows.append(ps.proposal)
        for r in ps.reports:
            row = {k: v for k, v in r.items() if not k.startswith("_")}
            if row["report_id"] in seen_reports:
                continue
            seen_reports.add(row["report_id"])
            report_rows.append(row)
        for cr in ps.critiques:
            if cr["critique_id"] in seen_critiques:
                continue
            seen_critiques.add(cr["critique_id"])
            critique_rows.append(cr)
        for agent_id, v in ps.votes.items():
            vote_rows.append(
                {
                    "vote_id": f"{pid}:{agent_id}:{v['commit_sha'][:12]}",
                    "task_id": v["task_id"],
                    "proposal_id": pid,
                    "agent_id": agent_id,
                    "commit_sha": v["commit_sha"],
                    "vote": v["vote"],
                }
            )
        if ps.merged is not None:
            merge_rows.append(ps.merged)

    return MaterializedSnapshot(
        proposals=prop_rows,
        evaluation_reports=report_rows,
        critiques=critique_rows,
        votes=vote_rows,
        merge_records=merge_rows,
    )


def reduce_all(events: list[dict]) -> dict[str, TaskReduction]:
    """Group the full log by task_id and reduce each task independently."""
    by_task: dict[str, list[dict]] = {}
    for ev in events:
        by_task.setdefault(ev["task_id"], []).append(ev)
    return {tid: reduce_task(evs) for tid, evs in by_task.items()}
