#!/usr/bin/env python3
"""Programmatic CCREP sync helper for automated agents.

Usage:
  python tools/ccrep_sync.py claim <task_id> <actor>
  python tools/ccrep_sync.py propose <task_id> <actor> <branch> <desc>
  python tools/ccrep_sync.py approve <task_id> <actor> <proposal_id>
"""

import sys
import os
from pathlib import Path

# Insert the tools dir into python path so ccrep can be imported
tools_dir = Path(__file__).parent.resolve()
if str(tools_dir) not in sys.path:
    sys.path.insert(0, str(tools_dir))

from ccrep.store import CcrepStore

def main():
    if len(sys.argv) < 4:
        print("Usage: python tools/ccrep_sync.py [claim|propose|approve] ...")
        sys.exit(1)
        
    cmd = sys.argv[1]
    
    # Initialize store resolving Canonical CCREP DB
    repo_root = tools_dir.parent
    db_path = repo_root / "scratch" / "ccrep.db"
    
    with CcrepStore(repo_root=repo_root, db_path=db_path) as store:
        if cmd == "claim":
            task_id = sys.argv[2]
            actor = sys.argv[3]
            res = store.claim_task(task_id, actor)
            print(f"CLAIMED: {res}")
            
        elif cmd == "propose":
            if len(sys.argv) < 6:
                print("Usage: python tools/ccrep_sync.py propose <task_id> <actor> <branch> <desc>")
                sys.exit(1)
            task_id = sys.argv[2]
            actor = sys.argv[3]
            branch = sys.argv[4]
            desc = sys.argv[5]
            
            res = store.submit_proposal(
                task_id=task_id,
                author=actor,
                branch=branch,
                artifact_profile="design_doc", # We are editing comments/docs
                description=desc,
                change_summary=["Annotated codebase comments"],
            )
            print(f"PROPOSED: {res['proposal']['proposal_id']}")
            
        elif cmd == "approve":
            task_id = sys.argv[2]
            actor = sys.argv[3]
            proposal_id = sys.argv[4]
            
            # Submit an approval critique
            critique = {
                "proposal_id": proposal_id,
                "reviewer": actor,
                "stance": "approve",
                "findings": [],
                "created_at": None,
                "critique_id": None
            }
            res = store.submit_critique(critique)
            print(f"APPROVED: {proposal_id} by {actor}")
            
        else:
            print(f"Unknown command {cmd}")
            sys.exit(1)

if __name__ == "__main__":
    main()
