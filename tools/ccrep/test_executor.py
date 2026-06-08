"""Executor unit tests — provenance numeric-claim detection (no git worktree)."""

from __future__ import annotations

import pytest

from .executor import _numeric_claims


@pytest.mark.parametrize(
    "line,expected",
    [
        # KNOWN_ISSUES #2: the regression case — unit-suffixed integer was missed.
        ("a run takes less than 15ms for a codebase of this size", ["15ms"]),
        # Multi-decimal (the original detector's only shape) still works.
        ("recall improved to 0.92 on the held-out set", ["0.92"]),
        ("accuracy was 99.27% across folds", ["99.27%"]),
        # Comparator / approximation prefixes.
        ("latency stays <15ms even under load", ["15ms", "<15"]),
        ("the model weights are ~6.3 GB on disk", ["6.3 GB", "~6.3"]),
        # Percentages and multipliers.
        ("throughput rose 92% after the change", ["92%"]),
        ("this is 3x faster than before", ["3x"]),
        # Time units beyond ms.
        ("the sweep finished in 2s", ["2s"]),
    ],
)
def test_flags_quantitative_claims(line, expected):
    # Superset: overlapping patterns may also emit a sub-span (e.g. "99.27" beside
    # "99.27%"); what matters is that every expected claim is detected.
    assert set(_numeric_claims(line)).issuperset(set(expected))


@pytest.mark.parametrize(
    "line",
    [
        "Refactored in commit on 2026-06-08 by the team",  # bare year/date
        "See section 4 for the rationale",  # plain ordinal
        "The CommandMap wires Tauri v2 handlers",  # version-ish, no unit
        "We have three reviewers and one author",  # no digits
        "0x1F is a hex literal, not a measurement",  # hex, no unit boundary
    ],
)
def test_does_not_flag_non_measurements(line):
    assert _numeric_claims(line) == []


def test_dedup_and_order_preserved():
    # Same claim twice on a line collapses; first-seen order kept.
    out = _numeric_claims("15ms then 15ms then 92%")
    assert out == ["15ms", "92%"]
