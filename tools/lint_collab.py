#!/usr/bin/env python3
"""Lint doc/collab/sessions/ for structural issues."""

from __future__ import annotations

import sys
from datetime import date, timedelta
from pathlib import Path

ROOT = Path(__file__).parent.parent
SESSIONS = ROOT / "doc" / "collab" / "sessions"
STALE_DAYS = 30


def main() -> int:
    errors: list[str] = []
    warnings: list[str] = []

    today = date.today()

    for entry in sorted(SESSIONS.iterdir()):
        if entry.name.startswith("."):
            continue

        # Flat .md files directly in sessions/ are not allowed
        if entry.is_file() and entry.suffix == ".md":
            errors.append(f"flat session file (should be a directory): {entry.relative_to(ROOT)}")
            continue

        if not entry.is_dir():
            continue

        # Every session directory must contain session.md
        session_md = entry / "session.md"
        if not session_md.exists():
            errors.append(f"session directory missing session.md: {entry.relative_to(ROOT)}/")
            continue

        # Warn on stale sessions (name starts with a date)
        name = entry.name
        if len(name) >= 10 and name[:10].replace("-", "").isdigit():
            try:
                session_date = date.fromisoformat(name[:10])
                age = (today - session_date).days
                if age > STALE_DAYS:
                    warnings.append(
                        f"stale session ({age}d old, consider closing): {entry.relative_to(ROOT)}/"
                    )
            except ValueError:
                pass

    if errors:
        print("ERRORS:")
        for e in errors:
            print(f"  ✗ {e}")
    if warnings:
        print("WARNINGS:")
        for w in warnings:
            print(f"  ⚠ {w}")
    if not errors and not warnings:
        print("✓ collab sessions look clean")

    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main())
