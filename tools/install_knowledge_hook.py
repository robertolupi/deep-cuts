#!/usr/bin/env python3
import os
import re
import sys
import subprocess
from pathlib import Path

def main():
    try:
        res = subprocess.run(
            ["git", "rev-parse", "--git-common-dir"],
            capture_output=True,
            text=True,
            check=True
        )
        git_common = Path(res.stdout.strip()).resolve()
    except Exception as e:
        print(f"Error: Not a Git repository or Git not found: {e}")
        sys.exit(1)

    hooks_dir = git_common / "hooks"
    hooks_dir.mkdir(parents=True, exist_ok=True)
    hook_file = hooks_dir / "pre-commit"

    hook_block = (
        "# [KNOWLEDGE_MGR_HOOK_START]\n"
        "# Validate codebase knowledge using Solo Mode linter\n"
        'echo "Running Codebase Knowledge Manager Linter..."\n'
        'tools/.venv/bin/python tools/knowledge_mgr.py lint --root "$PWD" || exit 1\n'
        "# [KNOWLEDGE_MGR_HOOK_END]\n"
    )

    if hook_file.exists():
        content = hook_file.read_text(encoding="utf-8")
        
        # Remove any existing hook block to clean up first
        pattern = re.compile(r'# \[KNOWLEDGE_MGR_HOOK_START\].*?# \[KNOWLEDGE_MGR_HOOK_END\]\n?', re.DOTALL)
        content = pattern.sub("", content)
        
        # Strip trailing exit 0 and whitespace
        content = content.rstrip()
        if content.endswith("exit 0"):
            content = content[:-6].rstrip()
            
        # Append our block and then exit 0
        new_content = content + "\n\n" + hook_block + "\nexit 0\n"
    else:
        new_content = "#!/bin/sh\n\n" + hook_block + "\nexit 0\n"

    hook_file.write_text(new_content, encoding="utf-8")
    
    # Make hook executable on POSIX systems
    try:
        os.chmod(hook_file, 0o755)
    except OSError:
        pass
        
    print(f"✓ Pre-commit hook successfully configured at: {hook_file}")

if __name__ == "__main__":
    main()
