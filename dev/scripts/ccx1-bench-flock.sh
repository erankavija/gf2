#!/usr/bin/env bash
# CCX1 flock-guarded benchmark wrapper.
#
# Usage:
#   ./dev/scripts/ccx1-bench-flock.sh <args>
#   ./dev/scripts/ccx1-bench-flock.sh --full-host <args>
#
# Holds the /tmp/gf2-ccx1.lock mutex for the duration of the child
# command, then pins to the GF2_CCX1_CPUS list with nice -n -5 (best-effort;
# may be denied for non-root). Used by the 74ba1cdc R1 dispatch to
# serialize benches across sibling workers.
#
# GF2_CCX1_CPUS is a `taskset -c` CPU list and the only pinning configuration
# point. Its default, 6-11, is the CCX1 cores of the reference host; a host
# without those CPUs sets it to one core complex of its own.
#
# --full-host holds the same canonical mutex but deliberately omits taskset.
# Use it for a benchmark whose named configuration requires the full processor;
# it does not create a second lock domain or weaken serialization.
#
# ## Lock lifetime
#
# The lock is released when the command exits, even if it left a daemon
# running.
#
# The command inherits the mutex descriptor, and so does every process it
# starts: the benchmark runner and the calibration harness refuse to measure
# without an inherited descriptor for the held lock (`host::inherited_lock` in
# dev/tools/tuning-campaign-support, `observed_lock_file` in
# experiments/tuning-calibration/benches/core.rs). A flock lock is released by
# an unlock or once every copy of its descriptor is closed, so an inherited
# copy in a daemon the command leaves running — a compiler-cache server is the
# case that arises in practice — would hold the mutex for as long as that
# daemon lived, and every acquirer on the host would wait for it. The wrapper
# closes that path by unlocking the mutex once the command exits, which
# releases it whatever copies remain.
#
# HUP, INT and TERM are caught so that the wrapper outlives the command and
# the unlock runs; the command still receives any signal sent to it or to its
# process group. SIGKILL cannot be caught: if the wrapper is killed with it,
# the unlock cannot run and the mutex stays held until the command and every
# process holding a copy of its descriptor, a daemon included, have exited.
set -euo pipefail

# Invoker policy (2026-09-13): the host is never locked during a working
# session; every stated result runs in the benchmark window, which
# exports GF2_BENCH_WINDOW=1. Outside it this wrapper refuses to run.
if [[ -z "${GF2_BENCH_WINDOW:-}" ]]; then
  echo "ccx1-bench-flock: refusing to take the host mutex outside a benchmark window (GF2_BENCH_WINDOW is unset); queue the run in dev/scripts/bench-window/queue.tsv" >&2
  exit 3
fi

LOCK_FILE="${GF2_CCX1_LOCK:-/tmp/gf2-ccx1.lock}"
test -f "$LOCK_FILE" || touch "$LOCK_FILE"

PIN=(taskset -c "${GF2_CCX1_CPUS:-6-11}")
if [[ "${1:-}" == "--full-host" ]]; then
  shift
  test "$#" -gt 0 || {
    echo "usage: $0 [--full-host] <command> [args...]" >&2
    exit 2
  }
  PIN=()
fi

exec {LOCK_FD}<"$LOCK_FILE"
flock -x "$LOCK_FD"

trap : HUP INT TERM
rc=0
"${PIN[@]}" nice -n -5 "$@" || rc=$?
flock -u "$LOCK_FD"
exit "$rc"
