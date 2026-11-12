#!/usr/bin/env bash
# Benchmark window.
#
# Invoker policy (2026-09-13): the host is never locked during a working
# session; every stated result runs in a benchmark window outside the session,
# and indications run unlocked at any time.
# This script is that window. It exports GF2_BENCH_WINDOW=1, which is what
# lets dev/scripts/ccx1-bench-flock.sh take the host mutex and makes
# scripts/cargo-budget.sh apply its lock discipline. Outside the window,
# ccx1-bench-flock.sh refuses execution (exit 3), while cargo-budget.sh runs
# unlocked and unbudgeted. It runs the
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
#
# Host-local external inputs (comparator trees, install prefixes, recorded
# bundles) live under $GF2_EXT_ROOT (default <repo>/.agents/ext), which the
# window exports to every job; a queue command names an input as
# "$GF2_EXT_ROOT/<id>/...", independent of its worktree's depth.
#
# Before its first job the window waits until the one-minute load average is
# below LOAD_MAX, polling every LOAD_POLL_S seconds for at most LOAD_WAIT_MAX_S
# seconds. When the bound passes with the host still loaded, it logs that and
# starts the job anyway, so a window is never lost to a busy host; the job's
# own start line records the load it ran under. GF2_WINDOW_LOAD_MAX,
# GF2_WINDOW_LOAD_WAIT_MAX_S, GF2_WINDOW_LOAD_POLL_S and GF2_WINDOW_LOADAVG (a
# file in /proc/loadavg format) override these for testing.
#
# A window runs the jobs whose estimated minutes are at most MAX_EST_MIN and
# logs every longer job as deferred. The default, 5, is the longest pause a
# working session's builds take for a window; the night window sets
# GF2_WINDOW_MAX_EST_MIN=0, which lifts the cap.
#
# --print-root prints the resolved repository root and --print-ext-root the
# external-input root; each exits before the window opens. --load-wait-only runs the load wait into the window log and exits
# before the window opens, and --cap-check <minutes> exits 0 when a job of
# that estimate runs under the cap and 1 when it is deferred;
# run-window.test.sh drives both.
set -uo pipefail

LOAD_MAX="${GF2_WINDOW_LOAD_MAX:-4.0}"
LOAD_WAIT_MAX_S="${GF2_WINDOW_LOAD_WAIT_MAX_S:-1800}"
LOAD_POLL_S="${GF2_WINDOW_LOAD_POLL_S:-30}"
LOADAVG="${GF2_WINDOW_LOADAVG:-/proc/loadavg}"
MAX_EST_MIN="${GF2_WINDOW_MAX_EST_MIN:-5}"

under_cap() { ((MAX_EST_MIN == 0 || $1 <= MAX_EST_MIN)); }
if [[ "${1:-}" == --cap-check ]]; then
    under_cap "$2"
    exit
fi

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo=$(git -C "$here" rev-parse --show-toplevel) || exit 1
state="${GF2_WINDOW_STATE:-$repo/.agents/bench-window}"
export GF2_EXT_ROOT="${GF2_EXT_ROOT:-$repo/.agents/ext}"
if [[ "${1:-}" == --print-root ]]; then
    printf '%s\n' "$repo"
    exit 0
fi
if [[ "${1:-}" == --print-ext-root ]]; then
    printf '%s\n' "$GF2_EXT_ROOT"
    exit 0
fi
mkdir -p "$state"
log="$state/window.log"

say() { printf '%s %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$*" >> "$log"; }
load() { cut -d' ' -f1-3 "$LOADAVG"; }

wait_for_quiet_host() {
    local start=$SECONDS one left
    say "load wait start max=$LOAD_MAX bound_s=$LOAD_WAIT_MAX_S load=$(load)"
    while true; do
        one=$(cut -d' ' -f1 "$LOADAVG")
        if awk -v l="$one" -v m="$LOAD_MAX" 'BEGIN { exit !(l < m) }'; then
            say "load wait quiet after_s=$((SECONDS - start)) load=$(load)"
            return
        fi
        if ((SECONDS - start >= LOAD_WAIT_MAX_S)); then
            say "load wait bound passed after_s=$((SECONDS - start)) load=$(load); starting anyway"
            return
        fi
        left=$((LOAD_WAIT_MAX_S - (SECONDS - start)))
        sleep "$((left < LOAD_POLL_S ? left : LOAD_POLL_S))"
    done
}

if [[ "${1:-}" == --load-wait-only ]]; then
    wait_for_quiet_host
    exit 0
fi
export GF2_BENCH_WINDOW=1

say "window start host=$(hostname) load=$(load) queue_sha256=$(sha256sum "$here/queue.tsv" | cut -d' ' -f1)"

# The lock wrappers on main release the CCX1 locks when the wrapped command
# exits (jit:a387825e), so a compiler-cache server they spawned no longer keeps
# the locks held. A worktree that has not merged that fix still runs the old
# wrapper, under which such a server inherits the lock descriptors and blocks
# every later acquirer until it exits; and a server that idles out mid-window is
# respawned by the next cargo run, possibly under a lock. Replace any server
# that holds a lock descriptor or can idle out with one started here, outside
# every lock, that never idles out.
if command -v sccache > /dev/null; then
    for pid in $(pgrep -x sccache); do
        holds_lock=0
        for fd in "/proc/$pid/fd/"*; do
            case "$(readlink "$fd" 2> /dev/null)" in
                /tmp/gf2-ccx1.lock*) holds_lock=1 ;;
            esac
        done
        if [[ $holds_lock -eq 1 ]] \
            || ! tr '\0' '\n' < "/proc/$pid/environ" 2> /dev/null | grep -qx 'SCCACHE_IDLE_TIMEOUT=0'; then
            say "replacing sccache server pid=$pid (lock descriptor held or idle timeout set)"
            sccache --stop-server > /dev/null 2>&1 || kill "$pid"
        fi
    done
    SCCACHE_IDLE_TIMEOUT=0 sccache --start-server > /dev/null 2>&1 || true
    say "sccache servers: $(pgrep -x sccache | tr '\n' ' ')"
fi
waited=0
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
    if ! under_cap "$minutes"; then
        say "defer issue=$issue key=$key est_min=$minutes (cap $MAX_EST_MIN)"
        continue
    fi
    if [[ $waited -eq 0 ]]; then
        wait_for_quiet_host
        waited=1
    fi
    say "job start issue=$issue key=$key est_min=$minutes worktree=$worktree load=$(load) command=$command"
    (cd "$repo/$worktree" && bash -c "$command") >> "$state/$key.out" 2>&1 < /dev/null
    rc=$?
    say "job exit issue=$issue key=$key rc=$rc"
    if [[ $rc -eq 0 ]]; then
        : > "$state/$key.done"
    fi
done < "$here/queue.tsv"
say "window end"
