#!/bin/bash
# Stable CCREP test runner. Allow-list once as `Bash(tools/ccrep_test.sh:*)`.
# Runs the worktree-local ccrep suite with the canonical venv. Extra args pass
# through to pytest, e.g.:
#   tools/ccrep_test.sh                       # whole suite
#   tools/ccrep_test.sh tools/ccrep/test_ledger.py
#   tools/ccrep_test.sh -k provenance -v
TOOLS="$(cd "$(dirname "$0")" && pwd)"
if [ "$#" -eq 0 ]; then set -- "$TOOLS/ccrep"; fi
exec env PYTHONPATH="$TOOLS" /Users/rlupi/src/deep-cuts/tools/.venv/bin/python -m pytest -q "$@"
