#!/Users/rlupi/src/fams/deep-cuts/main/tools/.venv/bin/python
# The absolute venv shebang is intentional: linked worktrees do not carry tools/.venv.
import os
import subprocess
import sys
from pathlib import Path


def canonical_repo_root() -> Path:
    """Return the primary worktree root shared by linked git worktrees."""
    try:
        common_dir = subprocess.check_output(
            ["git", "rev-parse", "--path-format=absolute", "--git-common-dir"],
            text=True,
            stderr=subprocess.DEVNULL,
        ).strip()
    except (OSError, subprocess.CalledProcessError):
        return Path(__file__).resolve().parents[1]

    git_dir = Path(common_dir)
    if git_dir.name == ".git":
        return git_dir.parent
    return Path(__file__).resolve().parents[1]


repo_root = canonical_repo_root()
tools_dir = str(Path(__file__).resolve().parent)
os.environ.setdefault("COLLAB_ROOT", str(repo_root / "scratch" / "coordination"))

# Add tools_dir to sys.path so we can import the collab_mcp package
if tools_dir not in sys.path:
    sys.path.insert(0, tools_dir)

# Import the main function from the collab_mcp package
from collab_mcp.server import main

if __name__ == '__main__':
    main()
