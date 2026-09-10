#!/usr/bin/env bash
# Benchmark window for epic 1a379447 (jit:1a379447).
#
# Invoker policy (2026-09-11): a timed run inside a working session lasts at
# most about as long as a cold cargo-ci run; longer campaigns run in separate
# time windows outside the session. This script is that window. It runs the
# jobs listed in queue.tsv beside it, one after another, each from its worker
# worktree root.
#
# queue.tsv: one job per line, tab-separated:
#   issue  worktree (repo-relative)  estimated minutes  command
# Lines starting with '#' and blank lines are ignored. The command runs under
# `bash -c` from the worktree root; it takes the CCX1 lock itself through the
# campaign launcher, so this script holds no lock.
#
# Runtime state lives in $GF2_WINDOW_STATE (default <repo>/.agents/bench-window):
# window.log (append-only orchestration log), <job-key>.out (job output) and
# <job-key>.done markers. Re-running the window skips completed jobs; an
# interrupted campaign resumes under its own identity without repeating cells.
# Each campaign's own execution log stays the authoritative record of its run.
set -uo pipefail

repo=/home/vkaskivuo/Projects/gf2
here="$repo/dev/active/1a379447-zen3-cpu-performance/bench-window"
state="${GF2_WINDOW_STATE:-$repo/.agents/bench-window}"
mkdir -p "$state"
log="$state/window.log"

say() { printf '%s %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$*" >> "$log"; }

say "window start host=$(hostname) load=$(cut -d' ' -f1-3 /proc/loadavg) queue_sha256=$(sha256sum "$here/queue.tsv" | cut -d' ' -f1)"
while IFS=$'\t' read -r issue worktree minutes command; do
    [[ -z "${issue// /}" || "$issue" == \#* ]] && continue
    key="$(printf '%s\t%s\t%s' "$issue" "$worktree" "$command" | sha256sum | cut -c1-16)"
    if [[ -e "$state/$key.done" ]]; then
        say "skip issue=$issue key=$key (done)"
        continue
    fi
    if [[ ! -d "$repo/$worktree" ]]; then
        say "missing issue=$issue key=$key worktree=$worktree"
        continue
    fi
    say "job start issue=$issue key=$key est_min=$minutes worktree=$worktree load=$(cut -d' ' -f1-3 /proc/loadavg) command=$command"
    (cd "$repo/$worktree" && bash -c "$command") >> "$state/$key.out" 2>&1 < /dev/null
    rc=$?
    say "job exit issue=$issue key=$key rc=$rc"
    if [[ $rc -eq 0 ]]; then
        : > "$state/$key.done"
    fi
done < "$here/queue.tsv"
say "window end"
