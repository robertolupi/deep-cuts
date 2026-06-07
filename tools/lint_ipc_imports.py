#!/usr/bin/env python3
"""Lint src/ for direct @tauri-apps/api imports outside $lib/ipc.ts."""

from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).parent.parent
SRC = ROOT / "src"
ALLOWED = SRC / "lib" / "ipc.ts"

PATTERN = re.compile(r"""from\s+["']@tauri-apps/api""")


def main() -> int:
    errors: list[str] = []

    for ext in ("*.ts", "*.svelte", "*.js"):
        for path in sorted(SRC.rglob(ext)):
            if path.resolve() == ALLOWED.resolve():
                continue
            if path.name.endswith(".test.ts"):
                continue
            try:
                lines = path.read_text(encoding="utf-8").splitlines()
            except UnicodeDecodeError:
                continue
            for lineno, line in enumerate(lines, start=1):
                if PATTERN.search(line):
                    rel = path.relative_to(ROOT)
                    errors.append(f"direct Tauri import: {rel}:{lineno}")

    if errors:
        for e in errors:
            print(f"✗ {e}")
    else:
        print("✓ no direct Tauri imports outside $lib/ipc")

    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main())
