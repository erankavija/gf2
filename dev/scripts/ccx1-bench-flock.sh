#!/usr/bin/env bash
# CCX1 flock-guarded benchmark wrapper.
#
# Usage:
#   ./dev/scripts/ccx1-bench-flock.sh <args>
#
# Holds the /tmp/gf2-ccx1.lock mutex for the duration of the child
# command, then pins to CCX1 cores (6-11) with nice -n -5 (best-effort;
# may be denied for non-root). Used by the 74ba1cdc R1 dispatch to
# serialize benches across sibling workers.
set -euo pipefail

LOCK_FILE="${GF2_CCX1_LOCK:-/tmp/gf2-ccx1.lock}"
test -f "$LOCK_FILE" || touch "$LOCK_FILE"

exec flock -x "$LOCK_FILE" taskset -c 6-11 nice -n -5 "$@"
