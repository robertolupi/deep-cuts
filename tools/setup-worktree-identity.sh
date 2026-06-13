#!/bin/sh
# Set this worktree's git identity (per-worktree config, immune to user.*
# entries in the shared .git/config — the 2026-06-12 misattribution bug).
# See doc/collab/PROTOCOL.md §4.
#
# Usage: tools/setup-worktree-identity.sh <actor>   e.g. rlupi, claude, agy
set -eu

[ "$#" -eq 1 ] || { echo "usage: tools/setup-worktree-identity.sh <actor>" >&2; exit 1; }
actor=$1

git_dir=$(git rev-parse --git-dir)
case "$git_dir" in
  */.git/worktrees/*) ;;
  *) echo "setup-worktree-identity: run inside a linked worktree, not the main checkout." >&2; exit 1 ;;
esac

git config extensions.worktreeConfig true
git config --worktree user.name "$actor"
if [ "$actor" = "rlupi" ]; then
  git config --worktree user.email "roberto.lupi@gmail.com"
else
  git config --worktree user.email "roberto.lupi+$actor@gmail.com"
fi

echo "identity for this worktree:"
git var GIT_AUTHOR_IDENT
