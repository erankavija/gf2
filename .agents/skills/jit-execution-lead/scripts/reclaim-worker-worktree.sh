#!/usr/bin/env bash
# jit-execution-lead utility: reclaim per-issue worker worktrees after a wave
# closes, harvesting their build/dependency caches into the shared cache pool
# so the next wave's worktrees start warm instead of cold.
#
# Counterpart of dispatch-worker-worktree.sh, which seeds each new worktree
# from the same pool. Toolchain-agnostic: by default every top-level directory
# of the worktree that git ignores (build outputs and dependency caches are
# ignored by definition) is harvested into "$pool/<name>", newest file wins.
# Set LEAD_CACHE_DIRS to a space-separated list of names to harvest exactly
# those instead. The pool location is LEAD_CACHE_POOL
# (default: <repo>/.agents/cache-pool).
#
# Reference: references/worktree-dispatch-protocol.md (in this skill).
#
# Usage:
#   .agents/skills/jit-execution-lead/scripts/reclaim-worker-worktree.sh <short-id> [<short-id>...]
#
# Safety:
#   - Refuses to remove a worktree whose branch has commits unmerged into
#     main (salvage points). Override for a truly abandoned worktree with
#     LEAD_RECLAIM_FORCE=1.
#
# Exit codes:
#   0 — all named worktrees reclaimed (cache harvested, worktree removed)
#   1 — a worktree was skipped (unmerged commits) or removal failed
#   2 — bad invocation

set -euo pipefail

if [[ $# -lt 1 ]]; then
    echo "usage: $0 <short-id> [<short-id>...]" >&2
    exit 2
fi

repo_root="$(git rev-parse --show-toplevel)"
cd "$repo_root"

pool="${LEAD_CACHE_POOL:-$repo_root/.agents/cache-pool}"
status=0

# Cache directories for one worktree: LEAD_CACHE_DIRS when set, else every
# top-level directory git ignores in that worktree.
cache_dirs_for() {
    local wt="$1"
    if [[ -n "${LEAD_CACHE_DIRS:-}" ]]; then
        echo "$LEAD_CACHE_DIRS"
        return
    fi
    local entry names=""
    for entry in "$wt"/*/; do
        [[ -d "$entry" && ! -L "${entry%/}" ]] || continue
        local name
        name="$(basename "$entry")"
        if git -C "$wt" check-ignore -q "$name" 2>/dev/null; then
            names+="$name "
        fi
    done
    echo "$names"
}

for sid in "$@"; do
    wt_path=".agents/worktrees/agent-${sid}"
    wt_branch="worktree-agent-${sid}"

    if [[ ! -d "$wt_path" ]]; then
        echo "[skip] $wt_path does not exist" >&2
        status=1
        continue
    fi

    if [[ "${LEAD_RECLAIM_FORCE:-0}" != "1" ]] \
        && git rev-parse --verify --quiet "refs/heads/${wt_branch}" > /dev/null \
        && [[ -n "$(git log --oneline "main..${wt_branch}" -- 2>/dev/null)" ]]; then
        echo "[skip] ${wt_branch} has commits unmerged into main; not removing." >&2
        echo "       Merge it, or set LEAD_RECLAIM_FORCE=1 for an abandoned worktree." >&2
        status=1
        continue
    fi

    for name in $(cache_dirs_for "$wt_path"); do
        src="$wt_path/$name"
        [[ -d "$src" && ! -L "$src" ]] || continue
        mkdir -p "$pool/$name"
        # Newest file wins so a stale harvest never clobbers fresher artifacts.
        if command -v rsync > /dev/null; then
            rsync -a --update "$src/" "$pool/$name/"
        else
            cp -au "$src/." "$pool/$name/"
        fi
        echo "[ok] harvested $src -> $pool/$name"
    done

    git worktree remove --force "$wt_path"
    echo "[ok] removed $wt_path (branch ${wt_branch} kept)"
done

git worktree prune
exit "$status"
