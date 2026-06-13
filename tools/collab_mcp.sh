#!/bin/bash
TOOLS="$(cd "$(dirname "$0")" && pwd)"
exec /Users/rlupi/src/fams/deep-cuts/main/tools/.venv/bin/python "$TOOLS/collab_mcp_cli.py" "$@"
