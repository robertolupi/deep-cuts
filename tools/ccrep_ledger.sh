#!/bin/bash
# Stable read-only CCREP ledger inspector. Allow-list once as
# `Bash(tools/ccrep_ledger.sh:*)`. Canonical venv (linked worktrees lack one);
# the CLI script is resolved next to this wrapper so each worktree runs its own
# ccrep package. Usage: tools/ccrep_ledger.sh {db|tasks|proposals|consensus|critiques|events} [args] [--json]
TOOLS="$(cd "$(dirname "$0")" && pwd)"
exec /Users/rlupi/src/deep-cuts/tools/.venv/bin/python "$TOOLS/ccrep_ledger_cli.py" "$@"
