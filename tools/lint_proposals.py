#!/usr/bin/env python3
"""Lint doc/proposals/ and doc/research/ for missing lifecycle frontmatter."""

from __future__ import annotations

import sys
from pathlib import Path

ROOT = Path(__file__).parent.parent
DIRS = [ROOT / "doc" / "proposals", ROOT / "doc" / "research"]


def has_frontmatter(path: Path) -> bool:
    """Return True if the file starts with --- and contains status: and owner:."""
    try:
        text = path.read_text(encoding="utf-8")
    except OSError:
        return False

    if not text.startswith("---"):
        return False

    # Find the closing ---
    rest = text[3:]
    end = rest.find("\n---")
    if end == -1:
        return False

    block = rest[:end]
    return "status:" in block and "owner:" in block


def main() -> int:
    warnings: list[str] = []

    for directory in DIRS:
        if not directory.exists():
            continue
        for md_file in sorted(directory.rglob("*.md")):
            if not has_frontmatter(md_file):
                warnings.append(f"missing frontmatter: {md_file.relative_to(ROOT)}")

    if warnings:
        print("WARNINGS:")
        for w in warnings:
            print(f"  ⚠ {w}")
    else:
        print("✓ all proposal/research docs have frontmatter")

    return 0


if __name__ == "__main__":
    sys.exit(main())
