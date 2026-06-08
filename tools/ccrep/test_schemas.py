"""Schema-validation tests for the CCREP core objects (no MCP needed).

Run from tools/:  .venv/bin/python -m pytest ccrep/test_schemas.py -q
"""

from __future__ import annotations

import pytest

from .schemas import ARTIFACT_PROFILES, SchemaError, validate, validation_errors


def _valid_proposal() -> dict:
    return {
        "proposal_id": "prop_1",
        "task_id": "task_1",
        "revision": 0,
        "author": "claude",
        "git": {"repo": "deep-cuts", "branch": "feat/x", "commit_sha": "a" * 40},
        "created_at": "2026-06-08T12:00:00+00:00",
        "description": "tweak boundary prior",
        "change_summary": ["adjust threshold"],
        "artifact_profile": "code_change",
        "status": "submitted",
    }


def test_valid_proposal_passes():
    validate("Proposal", _valid_proposal())


def test_proposal_requires_artifact_profile():
    p = _valid_proposal()
    del p["artifact_profile"]
    errs = validation_errors("Proposal", p)
    assert errs, "missing artifact_profile must be rejected"
    with pytest.raises(SchemaError):
        validate("Proposal", p)


def test_proposal_rejects_unknown_profile():
    p = _valid_proposal()
    p["artifact_profile"] = "nonsense"
    assert validation_errors("Proposal", p)


def test_proposal_rejects_short_commit_sha():
    p = _valid_proposal()
    p["git"]["commit_sha"] = "abc"
    assert validation_errors("Proposal", p)


def test_proposal_rejects_additional_properties():
    p = _valid_proposal()
    p["mystery"] = 1
    assert validation_errors("Proposal", p)


def test_all_three_profiles_validate():
    for prof in ARTIFACT_PROFILES:
        p = _valid_proposal()
        p["artifact_profile"] = prof
        validate("Proposal", p)


def test_valid_evaluation_report():
    report = {
        "report_id": "r1",
        "proposal_id": "prop_1",
        "commit_sha": "a" * 40,
        "suite_id": "code_change-default",
        "suite_hash": "h",
        "environment_hash": "e",
        "dataset_hash": "d",
        "started_at": "2026-06-08T12:00:00+00:00",
        "completed_at": "2026-06-08T12:01:00+00:00",
        "status": "passed",
        "hard_checks": [{"name": "cargo_test", "passed": True}],
        "metrics": {},
    }
    validate("EvaluationReport", report)


def test_valid_critique():
    critique = {
        "critique_id": "c1",
        "proposal_id": "prop_1",
        "reviewer": "codex",
        "created_at": "2026-06-08T12:00:00+00:00",
        "stance": "request_changes",
        "summary": "duration prior too aggressive",
        "findings": [
            {
                "finding_id": "f1",
                "severity": "blocking",
                "category": "correctness",
                "claim": "suppresses valid short boundaries",
                "evidence": [
                    {"kind": "file_line", "uri": "src/x.rs:42", "details": "here"}
                ],
                "blocks_merge": True,
            }
        ],
    }
    validate("Critique", critique)


def test_valid_consensus_state():
    cs = {
        "task_id": "task_1",
        "state": "consensus_ready",
        "computed_at": "2026-06-08T12:00:00+00:00",
        "gate_policy_version": "ccrep-phase1-v1",
        "candidate_proposals": ["prop_1"],
        "votes": [],
        "weighted_tallies": {"approve": 1.0},
        "open_blocking_findings": [],
        "decision": {"mergeable": True, "reason": "ok"},
    }
    validate("ConsensusState", cs)
