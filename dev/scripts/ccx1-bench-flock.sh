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
# The nesting below is the whole mechanism: the outer `flock` holds the
# turnstile and execs the inner `flock`, which holds the mutex and execs the
# command. Both descriptors survive `exec`, so both locks are held for the
# child's lifetime and released together when it exits. Deadlock is not
# reachable: a build holds the turnstile only while no measurement run does,
# and a measurement run holds both or neither.
#
# A run that already holds this mutex MUST set CARGO_CI_NO_LOCK=1 for its own
# cargo work. That was already required to avoid taking the shared side
# underneath its own exclusive side; it now also avoids blocking on the
# turnstile it is holding itself.
set -euo pipefail

LOCK_FILE="${GF2_CCX1_LOCK:-/tmp/gf2-ccx1.lock}"
TURNSTILE="${GF2_CCX1_TURNSTILE:-${LOCK_FILE}.turnstile}"
test -f "$LOCK_FILE" || touch "$LOCK_FILE"
test -f "$TURNSTILE" || touch "$TURNSTILE"

if [[ "${1:-}" == "--full-host" ]]; then
  shift
  test "$#" -gt 0 || {
    echo "usage: $0 [--full-host] <command> [args...]" >&2
    exit 2
  }
  exec flock -x "$TURNSTILE" flock -x "$LOCK_FILE" nice -n -5 "$@"
fi

exec flock -x "$TURNSTILE" flock -x "$LOCK_FILE" taskset -c 6-11 nice -n -5 "$@"
