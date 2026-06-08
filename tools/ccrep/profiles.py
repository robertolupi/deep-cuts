"""Artifact profiles — the code-side dispatch table (synthesis §"Artifact Profiles").

The server and ledger stay generic; a proposal declares an ``artifact_profile``
and that selection is what picks the eval suite (a list of shell commands) plus
which *gate components* apply. The eval-suite commands are config, not logic, so
they are overridable per task; only the component dispatch lives in code.

Invariant 6 (artifact-profile consistency) is enforced here and in the reducer:
a check that does not belong to the declared profile must never fire. Each
profile lists the gate components it owns, and the reducer/executor consult that
list rather than running every possible check.

Phase 1 deliberately does NOT implement the AST / line-budget revision gates
(``new_function_defs``, ``max_files``, ``max_changed_lines``). Those are Phase 2
and are listed here only as the (empty in Phase 1) ``revision_gates`` slot so the
profile shape is forward-compatible.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Mapping

# Gate-component identifiers. A component is "owned" by a profile iff it appears
# in that profile's ``gate_components``; the reducer rejects any report/finding
# referencing a component the proposal's profile does not own (invariant 6).
GATE_BUILD_TEST = "build_test"
GATE_LINT_FMT = "lint_fmt"
GATE_GOLDEN_METRIC = "golden_metric"
GATE_DOC_LINT = "doc_lint"
GATE_PROVENANCE_WARN = "provenance_warn"
GATE_FRONTMATTER_STATUS = "frontmatter_status"

# Human-gate categories that force ``merge_proposal`` to refuse/flag for a human
# (synthesis §"Consensus gate", invariant: the block is unbypassable in code).
HUMAN_GATE_CATEGORIES = (
    "public_api_change",
    "destructive_migration",
    "model_or_dataset_change",
    "large_architecture_change",
)


@dataclass(frozen=True)
class EvalCommand:
    """One command in a profile's eval suite.

    ``name`` is the stable hard-check name recorded in the EvaluationReport.
    ``argv`` is the shell command run inside the worktree. ``required`` flags a
    hard check whose non-zero exit fails the gate; non-required commands are
    advisory (their failure is recorded but does not flip the gate).
    """

    name: str
    argv: list[str]
    required: bool = True


@dataclass(frozen=True)
class ArtifactProfile:
    """Static description of one artifact profile's gate."""

    name: str
    gate_components: frozenset[str]
    default_eval_suite: tuple[EvalCommand, ...]
    # Phase 2 slot; empty in Phase 1 (AST/line budgets are explicitly out of scope).
    revision_gates: tuple[str, ...] = field(default_factory=tuple)

    def owns(self, component: str) -> bool:
        return component in self.gate_components


# --- code_change -----------------------------------------------------------
# build + test + lint + fmt; no golden-metric regression. Commands default to
# this repo's Rust toolchain but are overridable per task (see resolve_suite).
_CODE_CHANGE = ArtifactProfile(
    name="code_change",
    gate_components=frozenset(
        {GATE_BUILD_TEST, GATE_LINT_FMT, GATE_GOLDEN_METRIC}
    ),
    default_eval_suite=(
        EvalCommand(
            "cargo_test",
            ["cargo", "test", "--manifest-path", "src-tauri/Cargo.toml"],
        ),
        EvalCommand(
            "cargo_fmt_check",
            ["cargo", "fmt", "--manifest-path", "src-tauri/Cargo.toml", "--check"],
        ),
        EvalCommand(
            "cargo_clippy",
            [
                "cargo",
                "clippy",
                "--manifest-path",
                "src-tauri/Cargo.toml",
                "--",
                "-D",
                "warnings",
            ],
        ),
    ),
)

# --- code_review -----------------------------------------------------------
# build + test on the head; deliverable is the critique set + verdict, so there
# is no golden-metric gate by default.
_CODE_REVIEW = ArtifactProfile(
    name="code_review",
    gate_components=frozenset({GATE_BUILD_TEST}),
    default_eval_suite=(
        EvalCommand(
            "cargo_test",
            ["cargo", "test", "--manifest-path", "src-tauri/Cargo.toml"],
        ),
    ),
)

# --- design_doc ------------------------------------------------------------
# doc linters + link-check + skill-index consistency + provenance WARNINGS +
# the one-directional frontmatter-status check (invariant 7). No metric gate;
# AST/line gates disabled.
_DESIGN_DOC = ArtifactProfile(
    name="design_doc",
    gate_components=frozenset(
        {GATE_DOC_LINT, GATE_PROVENANCE_WARN, GATE_FRONTMATTER_STATUS}
    ),
    default_eval_suite=(
        EvalCommand("lint_collab", ["python3", "tools/lint_collab.py"]),
    ),
)


PROFILES: Mapping[str, ArtifactProfile] = {
    p.name: p for p in (_CODE_CHANGE, _CODE_REVIEW, _DESIGN_DOC)
}


def get_profile(name: str) -> ArtifactProfile:
    try:
        return PROFILES[name]
    except KeyError:
        raise ValueError(
            f"unknown artifact_profile {name!r}; expected one of {sorted(PROFILES)}"
        ) from None


def resolve_suite(
    profile_name: str, override: list[dict] | None = None
) -> list[EvalCommand]:
    """Return the eval suite for a profile, applying an optional per-task override.

    An override is a list of ``{"name", "argv", "required"?}`` dicts. Keeping the
    commands as data (not code) is the synthesis's "which suite runs is config"
    rule: the executor runs whatever this returns.
    """
    if override:
        return [
            EvalCommand(
                name=c["name"],
                argv=list(c["argv"]),
                required=bool(c.get("required", True)),
            )
            for c in override
        ]
    return list(get_profile(profile_name).default_eval_suite)
