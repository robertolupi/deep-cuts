#!/usr/bin/env python3
"""Scan src-tauri/src/ for filter_map(ok) patterns that silently swallow errors."""

from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).parent.parent
SRC = ROOT / "src-tauri" / "src"

# Matches filter_map(Result::ok) and filter_map(|<binding>| <expr>.ok())
PATTERN = re.compile(r"filter_map\s*\(\s*(?:Result::ok|\|[^|]*\|\s*[^)]*\.ok\(\s*\))\s*\)")


def main() -> int:
    warnings: list[str] = []

    for rs_file in sorted(SRC.rglob("*.rs")):
        for lineno, line in enumerate(rs_file.read_text(encoding="utf-8").splitlines(), start=1):
            if PATTERN.search(line):
                rel = rs_file.relative_to(ROOT)
                warnings.append(f"  ⚠ silent error swallow: {rel}:{lineno}")

    if warnings:
        print("WARNINGS:")
        for w in warnings:
            print(w)
    else:
        print("✓ no filter_map(ok) found")

    return 0


if __name__ == "__main__":
    sys.exit(main())
