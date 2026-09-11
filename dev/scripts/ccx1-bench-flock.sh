#!/usr/bin/env bash
# CCX1 flock-guarded benchmark wrapper.
#
# Usage:
#   ./dev/scripts/ccx1-bench-flock.sh <args>
#   ./dev/scripts/ccx1-bench-flock.sh --full-host <args>
#
# Holds the /tmp/gf2-ccx1.lock mutex for the duration of the child
# command, then pins to CCX1 cores (6-11) with nice -n -5 (best-effort;
# may be denied for non-root). Used by the 74ba1cdc R1 dispatch to
# serialize benches across sibling workers.
#
# --full-host holds the same canonical mutex but deliberately omits taskset.
# Use it for a benchmark whose named configuration requires the full processor;
# it does not create a second lock domain or weaken serialization.
#
# ## Writer preference
#
# `scripts/cargo-budget.sh` takes the same mutex SHARED, so builds run together
# and a measurement run excludes them. On Linux a pending `flock -x` does not
# block a new `flock -s`, so with several sibling workers building there is
# never an instant with zero shared holders and a measurement run is never
# granted. Observed 2026-09-08: four benchmark runners queued 10-29 minutes
# behind seven shared holders that kept being replenished, each needing under
# five minutes of lock time.
#
# The fix is a turnstile that every acquirer passes through first. A
# measurement run takes it exclusively and holds it for its whole run, so
# builds arriving afterwards block on the turnstile instead of joining the
# shared side ahead of it. Builds take it exclusively too, but release it the
# moment they hold the shared side, so they never serialize against each other
# for longer than one lock acquisition.
#
# This wrapper takes the turnstile, then the mutex, and holds both until the
# command exits. Deadlock is not reachable: an acquirer waits for the
# turnstile holding no lock, and for the mutex holding only the turnstile,
# which no holder of the mutex waits for.
#
# ## Lock lifetime
#
# Both locks end when the command exits, even if it left a daemon running.
#
# The command inherits the mutex descriptor, and so does every process it
# starts: the benchmark runner and the calibration harness refuse to measure
# without an inherited descriptor for the held lock (`host::inherited_lock` in
# dev/tools/tuning-campaign-support, `observed_lock_file` in
# crates/gf2-core/benches/tuning_calibration.rs). A flock lock is released by
# an unlock or once every copy of its descriptor is closed, so a daemon the
# command starts would otherwise keep the mutex. Observed 2026-09-10: an
# sccache server that a cargo-ci run under this wrapper started held the mutex
# and the turnstile after the run, and every build on the host waited until
# the server was stopped. The wrapper therefore unlocks the mutex once the
# command exits, which releases it whatever copies remain. The turnstile
# descriptor is closed in the command, so no descendant holds one.
#
# HUP, INT and TERM are caught so that the wrapper outlives the command and
# the unlock runs; the command still receives any signal sent to it or to its
# process group. A SIGKILL of the wrapper alone skips the unlock.
#
# A run that already holds this mutex MUST set CARGO_CI_NO_LOCK=1 for its own
# cargo work. That was already required to avoid taking the shared side
# underneath its own exclusive side; it now also avoids blocking on the
# turnstile it is holding itself.
#
# The turnstile binds only acquirers that pass through it. A script taking
# `flock -s` on this mutex directly — rather than through
# `scripts/cargo-budget.sh` — skips it and can still enter ahead of a queued
# measurement run. Observed in two survey launchers, wrapping a harness build.
# One such holder delays a measurement run by the length of its build, which is
# bounded; a stream of them restores the starvation this turnstile removes.
# Take the shared side through `cargo-budget.sh`, which is the only supported
# shared acquirer.
set -euo pipefail

LOCK_FILE="${GF2_CCX1_LOCK:-/tmp/gf2-ccx1.lock}"
TURNSTILE="${GF2_CCX1_TURNSTILE:-${LOCK_FILE}.turnstile}"
test -f "$LOCK_FILE" || touch "$LOCK_FILE"
test -f "$TURNSTILE" || touch "$TURNSTILE"

PIN=(taskset -c 6-11)
if [[ "${1:-}" == "--full-host" ]]; then
  shift
  test "$#" -gt 0 || {
    echo "usage: $0 [--full-host] <command> [args...]" >&2
    exit 2
  }
  PIN=()
fi

exec {TURNSTILE_FD}<"$TURNSTILE"
flock -x "$TURNSTILE_FD"
exec {LOCK_FD}<"$LOCK_FILE"
flock -x "$LOCK_FD"

trap : HUP INT TERM
rc=0
"${PIN[@]}" nice -n -5 "$@" {TURNSTILE_FD}<&- || rc=$?
flock -u "$LOCK_FD"
flock -u "$TURNSTILE_FD"
exit "$rc"
