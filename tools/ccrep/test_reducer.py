"""Reducer invariant tests — the whole point of the package (no MCP, no git).

Each test builds an event list and folds it, asserting the ratchet invariants
from doc/proposals/ccrep-synthesis.md §"Invariants". Run from tools/:

    .venv/bin/python -m pytest ccrep/test_reducer.py -q
"""

from __future__ import annotations

import pytest

from . import ledger as L
from . import reducer as R
from .schemas import validate

SHA_A = "a" * 40
SHA_B = "b" * 40


def _ev(task_id, kind, payload, actor=None, seq=0):
    return {
        "seq": seq,
        "event_id": f"e{seq}",
        "task_id": task_id,
        "kind": kind,
        "actor": actor,
        "ts": float(seq),
        "payload": payload,
    }


def _proposal(pid, task, author, sha, profile="code_change", revision=0,
              supersedes=None, **extra):
    p = {
        "proposal_id": pid,
        "task_id": task,
        "revision": revision,
        "author": author,
        "git": {"repo": "deep-cuts", "branch": "feat/x", "commit_sha": sha},
        "created_at": "2026-06-08T12:00:00+00:00",
        "description": "d",
        "change_summary": ["s"],
        "artifact_profile": profile,
        "status": "submitted",
    }
    if supersedes:
        p["supersedes"] = supersedes
    p.update(extra)
    return p


def _report(pid, sha, passed=True, components=None):
    report = {
        "report_id": f"rep_{pid}",
        "proposal_id": pid,
        "commit_sha": sha,
        "suite_id": "code_change-default",
        "suite_hash": "h",
        "environment_hash": "e",
        "dataset_hash": "d",
        "started_at": "2026-06-08T12:00:00+00:00",
        "completed_at": "2026-06-08T12:01:00+00:00",
        "status": "passed" if passed else "failed",
        "hard_checks": [{"name": "cargo_test", "passed": passed}],
        "metrics": {},
    }
    if components is not None:
        report["_components"] = components
    return report


def _approve(pid, reviewer):
    return {
        "critique_id": f"appr_{reviewer}",
        "proposal_id": pid,
        "reviewer": reviewer,
        "created_at": "2026-06-08T12:00:00+00:00",
        "stance": "approve",
        "summary": "lgtm",
        "findings": [],
    }


def _blocking_critique(pid, reviewer):
    return {
        "critique_id": f"blk_{reviewer}",
        "proposal_id": pid,
        "reviewer": reviewer,
        "created_at": "2026-06-08T12:00:00+00:00",
        "stance": "request_changes",
        "summary": "broken",
        "findings": [
            {
                "finding_id": "f1",
                "severity": "blocking",
                "category": "correctness",
                "claim": "regression",
                "blocks_merge": True,
            }
        ],
    }


def _green_then_approve(task, author, reviewer, sha=SHA_A, pid="p1"):
    """Events: propose -> eval passes -> independent approve. Mergeable."""
    return [
        _ev(task, L.EVENT_PROPOSAL_SUBMITTED, {"proposal": _proposal(pid, task, author, sha)}, seq=1),
        _ev(task, L.EVENT_EVALUATION_COMPLETED, {"report": _report(pid, sha)}, seq=2),
        _ev(task, L.EVENT_CRITIQUE_SUBMITTED, {"critique": _approve(pid, reviewer)}, seq=3),
    ]


# --- happy path ------------------------------------------------------------
def test_phase1_gate_green_plus_independent_approval_is_mergeable():
    evs = _green_then_approve("t", "claude", "codex")
    red = R.reduce_task(evs)
    validate("ConsensusState", red.consensus)
    assert red.consensus["state"] == "consensus_ready"
    assert red.consensus["decision"]["mergeable"] is True


# --- invariant 4: no self-approval ----------------------------------------
def test_self_approval_does_not_satisfy_quorum():
    # author approves their own proposal => not mergeable
    evs = _green_then_approve("t", "claude", "claude")
    red = R.reduce_task(evs)
    assert red.consensus["decision"]["mergeable"] is False
    assert "independent approval" in red.consensus["decision"]["reason"]


def test_self_approval_not_tallied():
    evs = _green_then_approve("t", "claude", "claude")
    red = R.reduce_task(evs)
    # the self-approve vote is recorded with weight 0 and not tallied
    assert red.consensus["weighted_tallies"]["approve"] == 0.0


# --- invariant 3: votes expire on code change -----------------------------
def test_votes_expire_on_new_commit():
    task = "t"
    evs = [
        _ev(task, L.EVENT_PROPOSAL_SUBMITTED, {"proposal": _proposal("p1", task, "claude", SHA_A)}, seq=1),
        _ev(task, L.EVENT_EVALUATION_COMPLETED, {"report": _report("p1", SHA_A)}, seq=2),
        _ev(task, L.EVENT_CRITIQUE_SUBMITTED, {"critique": _approve("p1", "codex")}, seq=3),
        # revision on a NEW commit supersedes p1; the old approval must not carry
        _ev(task, L.EVENT_REVISION_SUBMITTED,
            {"proposal": _proposal("p2", task, "claude", SHA_B, revision=1, supersedes="p1")}, seq=4),
        _ev(task, L.EVENT_EVALUATION_COMPLETED, {"report": _report("p2", SHA_B)}, seq=5),
    ]
    red = R.reduce_task(evs)
    # head is p2 with no independent approval => not mergeable
    assert red.consensus["candidate_proposals"] == ["p2"]
    assert red.consensus["decision"]["mergeable"] is False
    assert "independent approval" in red.consensus["decision"]["reason"]


def test_new_commit_then_reapprove_is_mergeable():
    task = "t"
    evs = [
        _ev(task, L.EVENT_PROPOSAL_SUBMITTED, {"proposal": _proposal("p1", task, "claude", SHA_A)}, seq=1),
        _ev(task, L.EVENT_REVISION_SUBMITTED,
            {"proposal": _proposal("p2", task, "claude", SHA_B, revision=1, supersedes="p1")}, seq=2),
        _ev(task, L.EVENT_EVALUATION_COMPLETED, {"report": _report("p2", SHA_B)}, seq=3),
        _ev(task, L.EVENT_CRITIQUE_SUBMITTED, {"critique": _approve("p2", "codex")}, seq=4),
    ]
    red = R.reduce_task(evs)
    assert red.consensus["decision"]["mergeable"] is True


# --- invariant 5: derived state cannot be written -------------------------
def test_reducer_rejects_direct_consensus_write_event():
    task = "t"
    evs = [_ev(task, "consensus_state", {"state": "merged"}, seq=1)]
    with pytest.raises(R.ConsensusError):
        R.reduce_task(evs)


def test_reducer_rejects_consensus_state_in_payload():
    task = "t"
    evs = [
        _ev(task, L.EVENT_PROPOSAL_SUBMITTED,
            {"proposal": _proposal("p1", task, "claude", SHA_A),
             "consensus_state": {"mergeable": True}}, seq=1),
    ]
    with pytest.raises(R.ConsensusError):
        R.reduce_task(evs)


# --- invariant 1: report commit must match proposal commit ----------------
def test_report_commit_must_match_proposal_commit():
    task = "t"
    evs = [
        _ev(task, L.EVENT_PROPOSAL_SUBMITTED, {"proposal": _proposal("p1", task, "claude", SHA_A)}, seq=1),
        _ev(task, L.EVENT_EVALUATION_COMPLETED, {"report": _report("p1", SHA_B)}, seq=2),  # wrong sha
    ]
    with pytest.raises(R.ConsensusError):
        R.reduce_task(evs)


# --- blocking critiques ----------------------------------------------------
def test_open_blocking_critique_blocks_merge():
    task = "t"
    evs = [
        _ev(task, L.EVENT_PROPOSAL_SUBMITTED, {"proposal": _proposal("p1", task, "claude", SHA_A)}, seq=1),
        _ev(task, L.EVENT_EVALUATION_COMPLETED, {"report": _report("p1", SHA_A)}, seq=2),
        _ev(task, L.EVENT_CRITIQUE_SUBMITTED, {"critique": _approve("p1", "codex")}, seq=3),
        _ev(task, L.EVENT_CRITIQUE_SUBMITTED, {"critique": _blocking_critique("p1", "gemini")}, seq=4),
    ]
    red = R.reduce_task(evs)
    assert red.consensus["decision"]["mergeable"] is False
    assert red.consensus["open_blocking_findings"] == ["f1"]


# --- failing eval ----------------------------------------------------------
def test_failed_eval_blocks_merge():
    task = "t"
    evs = [
        _ev(task, L.EVENT_PROPOSAL_SUBMITTED, {"proposal": _proposal("p1", task, "claude", SHA_A)}, seq=1),
        _ev(task, L.EVENT_EVALUATION_COMPLETED, {"report": _report("p1", SHA_A, passed=False)}, seq=2),
        _ev(task, L.EVENT_CRITIQUE_SUBMITTED, {"critique": _approve("p1", "codex")}, seq=3),
    ]
    red = R.reduce_task(evs)
    assert red.consensus["decision"]["mergeable"] is False
    assert red.consensus["state"] == "revision_required"


# --- invariant 6: profile consistency -------------------------------------
def test_check_outside_profile_never_fires():
    # A design_doc proposal whose report carries a golden_metric component must
    # have that check dropped — it does not belong to the design_doc profile.
    task = "t"
    report = _report("p1", SHA_A, passed=True, components={"cargo_test": "golden_metric"})
    # add a failing golden-metric check tagged to a component design_doc lacks
    report["hard_checks"].append({"name": "golden_metric", "passed": False})
    report["_components"] = {"cargo_test": "build_test", "golden_metric": "golden_metric"}
    evs = [
        _ev(task, L.EVENT_PROPOSAL_SUBMITTED,
            {"proposal": _proposal("p1", task, "claude", SHA_A, profile="design_doc")}, seq=1),
        _ev(task, L.EVENT_EVALUATION_COMPLETED, {"report": report}, seq=2),
        _ev(task, L.EVENT_CRITIQUE_SUBMITTED, {"critique": _approve("p1", "codex")}, seq=3),
    ]
    red = R.reduce_task(evs)
    # golden_metric is not owned by design_doc => dropped => eval still passes
    head = red.proposals["p1"]
    names = [hc["name"] for hc in head.reports[-1]["hard_checks"]]
    assert "golden_metric" not in names, "out-of-profile check must not fire"
    assert red.consensus["decision"]["mergeable"] is True


# --- invariant 7: one-directional frontmatter status ----------------------
def test_frontmatter_accepted_without_approval_is_flagged():
    task = "t"
    prop = _proposal("p1", task, "claude", SHA_A, profile="design_doc",
                     frontmatter_status="accepted")
    evs = [
        _ev(task, L.EVENT_PROPOSAL_SUBMITTED, {"proposal": prop}, seq=1),
        # no eval, no approval => not APPROVED, yet doc claims accepted
    ]
    red = R.reduce_task(evs)
    actions = red.consensus["decision"].get("next_actions", [])
    assert any("invariant 7" in a for a in actions), "accepted-without-approval flagged"
    assert red.consensus["decision"]["mergeable"] is False


def test_frontmatter_accepted_with_approval_not_flagged():
    task = "t"
    prop = _proposal("p1", task, "claude", SHA_A, profile="design_doc",
                     frontmatter_status="accepted")
    evs = [
        _ev(task, L.EVENT_PROPOSAL_SUBMITTED, {"proposal": prop}, seq=1),
        _ev(task, L.EVENT_EVALUATION_COMPLETED, {"report": _report("p1", SHA_A)}, seq=2),
        _ev(task, L.EVENT_CRITIQUE_SUBMITTED, {"critique": _approve("p1", "codex")}, seq=3),
    ]
    red = R.reduce_task(evs)
    actions = red.consensus["decision"].get("next_actions", [])
    assert not any("invariant 7" in a for a in actions)
    assert red.consensus["decision"]["mergeable"] is True


# --- human gate ------------------------------------------------------------
def test_human_gate_blocks_auto_merge_even_when_green():
    task = "t"
    prop = _proposal("p1", task, "claude", SHA_A, human_gate=["public_api_change"])
    evs = [
        _ev(task, L.EVENT_PROPOSAL_SUBMITTED, {"proposal": prop}, seq=1),
        _ev(task, L.EVENT_EVALUATION_COMPLETED, {"report": _report("p1", SHA_A)}, seq=2),
        _ev(task, L.EVENT_CRITIQUE_SUBMITTED, {"critique": _approve("p1", "codex")}, seq=3),
    ]
    red = R.reduce_task(evs)
    assert red.consensus["state"] == "human_review_required"
    assert red.consensus["decision"]["mergeable"] is False
    assert "public_api_change" in red.consensus["decision"]["reason"]


# --- merge recorded --------------------------------------------------------
def test_merge_event_moves_state_to_merged():
    task = "t"
    evs = _green_then_approve(task, "claude", "codex")
    evs.append(_ev(task, L.EVENT_MERGE_RECORDED, {"proposal_id": "p1", "merged_by": "roberto"}, seq=4))
    red = R.reduce_task(evs)
    assert red.consensus["state"] == "merged"
    assert red.snapshot.merge_records and red.snapshot.merge_records[0]["proposal_id"] == "p1"
