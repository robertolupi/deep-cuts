"""CCREP core-object JSON Schemas (Draft 2020-12).

Copied verbatim from the authoritative Codex design
(doc/collab/sessions/2026-06-08-multi-agent-collaboration-research/codex-ccrep-design.md,
§6) for ``Proposal``, ``EvaluationReport``, ``Critique``, and ``ConsensusState``.
These are the only schemas Phase 1 commits to. Nothing here depends on MCP, so
the validation helpers are unit-testable standalone (same convention as
``collab_mcp.store``).

Phase-1 note: the synthesis (doc/proposals/ccrep-synthesis.md) adds an
``artifact_profile`` field to ``Proposal`` (``code_change`` | ``code_review`` |
``design_doc``). The base Codex schema sets ``additionalProperties: false`` on
``Proposal``, so the field is declared here explicitly rather than smuggled in.
"""

from __future__ import annotations

from typing import Any

from jsonschema import Draft202012Validator

# Artifact profiles selectable by a proposal (synthesis §"Artifact Profiles").
ARTIFACT_PROFILES = ("code_change", "code_review", "design_doc")

# ---------------------------------------------------------------------------
# Core objects — Draft 2020-12, verbatim from Codex §6 with the one additive
# `artifact_profile` field the synthesis requires on Proposal.
# ---------------------------------------------------------------------------

CCREP_SCHEMA: dict[str, Any] = {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "$id": "https://deep-cuts.local/schemas/ccrep.schema.json",
    "title": "CCREP Core Objects",
    "oneOf": [
        {"$ref": "#/$defs/Proposal"},
        {"$ref": "#/$defs/EvaluationReport"},
        {"$ref": "#/$defs/Critique"},
        {"$ref": "#/$defs/ConsensusState"},
    ],
    "$defs": {
        "AgentId": {
            "type": "string",
            "pattern": "^[A-Za-z0-9_.-]+$",
        },
        "IsoTime": {
            "type": "string",
            "format": "date-time",
        },
        "GitRef": {
            "type": "object",
            "required": ["repo", "commit_sha"],
            "properties": {
                "repo": {"type": "string"},
                "branch": {"type": "string"},
                "commit_sha": {
                    "type": "string",
                    "pattern": "^[a-f0-9]{40,64}$",
                },
                "base_commit_sha": {
                    "type": "string",
                    "pattern": "^[a-f0-9]{40,64}$",
                },
            },
            "additionalProperties": False,
        },
        "Proposal": {
            "type": "object",
            "required": [
                "proposal_id",
                "task_id",
                "revision",
                "author",
                "git",
                "created_at",
                "description",
                "change_summary",
                "status",
                "artifact_profile",
            ],
            "properties": {
                "proposal_id": {"type": "string"},
                "task_id": {"type": "string"},
                "revision": {"type": "integer", "minimum": 0},
                "supersedes": {"type": ["string", "null"]},
                "author": {"$ref": "#/$defs/AgentId"},
                "git": {"$ref": "#/$defs/GitRef"},
                "created_at": {"$ref": "#/$defs/IsoTime"},
                "description": {"type": "string"},
                "change_summary": {
                    "type": "array",
                    "items": {"type": "string"},
                },
                "artifact_profile": {"enum": list(ARTIFACT_PROFILES)},
                "claimed_domains": {
                    "type": "object",
                    "additionalProperties": {
                        "type": "number",
                        "minimum": 0,
                        "maximum": 1,
                    },
                },
                "expected_eval_suites": {
                    "type": "array",
                    "items": {"type": "string"},
                },
                "status": {
                    "enum": [
                        "submitted",
                        "evaluating",
                        "evaluation_failed",
                        "reviewing",
                        "revision_requested",
                        "approved",
                        "consensus_candidate",
                        "merged",
                        "rejected",
                        "abandoned",
                        "superseded",
                    ]
                },
            },
            "additionalProperties": False,
        },
        "EvaluationReport": {
            "type": "object",
            "required": [
                "report_id",
                "proposal_id",
                "commit_sha",
                "suite_id",
                "suite_hash",
                "environment_hash",
                "dataset_hash",
                "started_at",
                "completed_at",
                "status",
                "hard_checks",
                "metrics",
            ],
            "properties": {
                "report_id": {"type": "string"},
                "proposal_id": {"type": "string"},
                "commit_sha": {
                    "type": "string",
                    "pattern": "^[a-f0-9]{40,64}$",
                },
                "suite_id": {"type": "string"},
                "suite_hash": {"type": "string"},
                "environment_hash": {"type": "string"},
                "dataset_hash": {"type": "string"},
                "started_at": {"$ref": "#/$defs/IsoTime"},
                "completed_at": {"$ref": "#/$defs/IsoTime"},
                "status": {
                    "enum": ["passed", "failed", "error", "timeout", "cancelled"]
                },
                "hard_checks": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "required": ["name", "passed"],
                        "properties": {
                            "name": {"type": "string"},
                            "passed": {"type": "boolean"},
                            "details": {"type": "string"},
                            "log_uri": {"type": "string"},
                        },
                        "additionalProperties": False,
                    },
                },
                "metrics": {
                    "type": "object",
                    "additionalProperties": {
                        "type": "object",
                        "required": ["value", "direction"],
                        "properties": {
                            "value": {"type": "number"},
                            "baseline_value": {"type": ["number", "null"]},
                            "delta": {"type": ["number", "null"]},
                            "threshold": {"type": ["number", "null"]},
                            "direction": {
                                "enum": [
                                    "higher_is_better",
                                    "lower_is_better",
                                    "target",
                                ]
                            },
                            "passed": {"type": ["boolean", "null"]},
                            "p_value": {
                                "type": ["number", "null"],
                                "minimum": 0,
                                "maximum": 1,
                            },
                        },
                        "additionalProperties": False,
                    },
                },
                "artifacts": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "required": ["kind", "uri"],
                        "properties": {
                            "kind": {"type": "string"},
                            "uri": {"type": "string"},
                            "mime_type": {"type": "string"},
                        },
                        "additionalProperties": False,
                    },
                },
            },
            "additionalProperties": False,
        },
        "Critique": {
            "type": "object",
            "required": [
                "critique_id",
                "proposal_id",
                "reviewer",
                "created_at",
                "stance",
                "summary",
                "findings",
            ],
            "properties": {
                "critique_id": {"type": "string"},
                "proposal_id": {"type": "string"},
                "reviewer": {"$ref": "#/$defs/AgentId"},
                "created_at": {"$ref": "#/$defs/IsoTime"},
                "stance": {
                    "enum": ["approve", "request_changes", "abstain", "veto"]
                },
                "summary": {"type": "string"},
                "findings": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "required": ["finding_id", "severity", "category", "claim"],
                        "properties": {
                            "finding_id": {"type": "string"},
                            "severity": {
                                "enum": [
                                    "advisory",
                                    "minor",
                                    "major",
                                    "blocking",
                                    "critical",
                                ]
                            },
                            "category": {
                                "enum": [
                                    "correctness",
                                    "performance",
                                    "architecture",
                                    "security",
                                    "testing",
                                    "maintainability",
                                    "style",
                                    "research_assumption",
                                    "metric_regression",
                                ]
                            },
                            "claim": {"type": "string"},
                            "evidence": {
                                "type": "array",
                                "items": {
                                    "type": "object",
                                    "properties": {
                                        "kind": {
                                            "enum": [
                                                "file_line",
                                                "eval_metric",
                                                "test_log",
                                                "benchmark",
                                                "reasoning",
                                            ]
                                        },
                                        "uri": {"type": "string"},
                                        "details": {"type": "string"},
                                    },
                                    "additionalProperties": False,
                                },
                            },
                            "suggested_patch": {
                                "type": ["object", "null"],
                                "properties": {
                                    "format": {
                                        "enum": [
                                            "unified_diff",
                                            "parameter_change",
                                            "natural_language",
                                        ]
                                    },
                                    "content": {"type": "string"},
                                },
                                "additionalProperties": False,
                            },
                            "blocks_merge": {"type": "boolean"},
                        },
                        "additionalProperties": False,
                    },
                },
            },
            "additionalProperties": False,
        },
        "ConsensusState": {
            "type": "object",
            "required": [
                "task_id",
                "state",
                "computed_at",
                "gate_policy_version",
                "candidate_proposals",
                "votes",
                "weighted_tallies",
                "open_blocking_findings",
                "decision",
            ],
            "properties": {
                "task_id": {"type": "string"},
                "state": {
                    "enum": [
                        "collecting_proposals",
                        "evaluating",
                        "reviewing",
                        "revision_required",
                        "candidate_selection",
                        "consensus_ready",
                        "human_review_required",
                        "merged",
                        "parked",
                        "rejected",
                    ]
                },
                "computed_at": {"$ref": "#/$defs/IsoTime"},
                "gate_policy_version": {"type": "string"},
                "candidate_proposals": {
                    "type": "array",
                    "items": {"type": "string"},
                },
                "votes": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "required": [
                            "agent_id",
                            "proposal_id",
                            "vote",
                            "weight",
                            "domains",
                        ],
                        "properties": {
                            "agent_id": {"$ref": "#/$defs/AgentId"},
                            "proposal_id": {"type": "string"},
                            "vote": {
                                "enum": [
                                    "approve",
                                    "request_changes",
                                    "abstain",
                                    "veto",
                                ]
                            },
                            "weight": {
                                "type": "number",
                                "minimum": 0,
                                "maximum": 1,
                            },
                            "domains": {
                                "type": "object",
                                "additionalProperties": {
                                    "type": "number",
                                    "minimum": 0,
                                    "maximum": 1,
                                },
                            },
                            "confidence": {
                                "type": "number",
                                "minimum": 0,
                                "maximum": 1,
                            },
                        },
                        "additionalProperties": False,
                    },
                },
                "weighted_tallies": {
                    "type": "object",
                    "properties": {
                        "approve": {"type": "number"},
                        "request_changes": {"type": "number"},
                        "abstain": {"type": "number"},
                        "veto": {"type": "number"},
                    },
                    "additionalProperties": False,
                },
                "domain_quorum_status": {
                    "type": "object",
                    "additionalProperties": {
                        "type": "object",
                        "properties": {
                            "required": {"type": "number"},
                            "current": {"type": "number"},
                            "satisfied": {"type": "boolean"},
                        },
                        "additionalProperties": False,
                    },
                },
                "open_blocking_findings": {
                    "type": "array",
                    "items": {"type": "string"},
                },
                "decision": {
                    "type": "object",
                    "required": ["mergeable", "reason"],
                    "properties": {
                        "mergeable": {"type": "boolean"},
                        "selected_proposal_id": {"type": ["string", "null"]},
                        "reason": {"type": "string"},
                        "next_actions": {
                            "type": "array",
                            "items": {"type": "string"},
                        },
                    },
                    "additionalProperties": False,
                },
            },
            "additionalProperties": False,
        },
    },
}


# ---------------------------------------------------------------------------
# Validation helpers — one validator per core object, by $ref into $defs.
# ---------------------------------------------------------------------------

_DEFS = CCREP_SCHEMA["$defs"]


def _validator_for(def_name: str) -> Draft202012Validator:
    """A Draft 2020-12 validator scoped to one $def, resolving sibling $refs."""
    schema = {
        "$schema": CCREP_SCHEMA["$schema"],
        "$id": CCREP_SCHEMA["$id"],
        "$ref": f"#/$defs/{def_name}",
        "$defs": _DEFS,
    }
    return Draft202012Validator(schema)


_VALIDATORS = {
    name: _validator_for(name)
    for name in ("Proposal", "EvaluationReport", "Critique", "ConsensusState")
}


class SchemaError(ValueError):
    """Raised when a CCREP object fails schema validation."""


def validation_errors(def_name: str, obj: Any) -> list[str]:
    """Return human-readable validation errors for ``obj`` against a $def (empty = valid)."""
    validator = _VALIDATORS[def_name]
    out: list[str] = []
    for err in sorted(validator.iter_errors(obj), key=lambda e: list(e.path)):
        loc = "/".join(str(p) for p in err.path) or "<root>"
        out.append(f"{loc}: {err.message}")
    return out


def validate(def_name: str, obj: Any) -> None:
    """Raise :class:`SchemaError` if ``obj`` is not a valid instance of the named $def."""
    errs = validation_errors(def_name, obj)
    if errs:
        raise SchemaError(f"{def_name} invalid: " + "; ".join(errs))
