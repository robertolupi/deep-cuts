#!/bin/sh
# Safely bring the current worktree up to date with main. The operator-guide
# companion script (doc/collab/PROTOCOL.md): merge-only, refuses to run in the
# shared main checkout, refuses without a per-worktree identity, refuses on a
# dirty tree. Safe to bind to an Obsidian Shell-commands hotkey.
#
# Usage: tools/sync-worktree.sh   (run from anywhere inside your worktree)
set -eu

die() { echo "sync-worktree: $*" >&2; exit 1; }

git_dir=$(git rev-parse --git-dir)
case "$git_dir" in
  */.git/worktrees/*) ;; # linked worktree: ok
  *) die "this is the shared main checkout — never sync here (see doc/collab/PROTOCOL.md). cd into your own worktree." ;;
esac

# Identity must come from this worktree's config, not the shared .git/config
# or a global includeIf that a repo-local entry could override.
git config --worktree user.name >/dev/null 2>&1 ||
  die "no per-worktree identity. Fix: tools/setup-worktree-identity.sh <actor>"

[ -z "$(git status --porcelain)" ] ||
  die "working tree is dirty — commit or stash first."

branch=$(git branch --show-current)
[ -n "$branch" ] || die "detached HEAD — check out your branch first."

echo "merging main into $branch (merge-only; main is never rebased)"
git merge main
git log --oneline -1
