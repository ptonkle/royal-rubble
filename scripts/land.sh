#!/usr/bin/env bash
# Merge an agent's branch into main, keeping both sides of the append-only
# files, and stop on any real conflict: nothing is committed while one is left.
set -u
branch="$1"
worktree=".claude/worktrees/${branch#worktree-}"
if ! git merge --no-edit "$branch"; then
    python3 scripts/union_merge.py
    if [ -n "$(git diff --name-only --diff-filter=U)" ]; then
        echo "CONFLICT LEFT; resolve it by hand, nothing committed:" >&2
        git diff --name-only --diff-filter=U >&2
        exit 1
    fi
    if git grep -nE '^(<<<<<<<|>>>>>>>) ' -- . ':!scripts/union_merge.py' ':!scripts/land.sh'; then
        echo "CONFLICT MARKERS LEFT; nothing committed" >&2
        exit 1
    fi
    git commit --no-edit -q || exit 1
fi
git worktree unlock "$worktree" 2>/dev/null
git worktree remove --force "$worktree" && git branch -D "$branch"
