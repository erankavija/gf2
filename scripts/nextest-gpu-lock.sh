#!/usr/bin/env bash
# nextest wrapper script: a test in one of the named groups holds the
# host-wide GPU lock while it runs.
#
#   scripts/nextest-gpu-lock.sh <group>... -- <command> [args...]
#
# A test whose NEXTEST_TEST_GROUP is a named group takes an exclusive flock on
# GF2_GPU_LOCK (default /tmp/gf2-gpu.lock; $TMPDIR differs between sessions)
# and execs the command, whose inherited descriptor holds the lock until it and
# its children exit. Concurrent HIP processes oversubscribe the device runlist
# and the driver then faults a queue (@/issue/f21b25ac).
#
# Waiting counts toward the test's budget. On expiry nextest sends SIGTERM to
# the process group, and a test still waiting fails naming the lock.
set -euo pipefail

groups=()
while (($#)) && [[ "$1" != -- ]]; do
  groups+=("$1")
  shift
done
if (($# < 2)); then
  echo "usage: $0 <group>... -- <command> [args...]" >&2
  exit 2
fi
shift

locked=
for g in "${groups[@]}"; do
  [[ "${NEXTEST_TEST_GROUP:-}" == "$g" ]] && locked=1
done
[[ -n "$locked" ]] || exec "$@"

LOCK_FILE="${GF2_GPU_LOCK:-/tmp/gf2-gpu.lock}"
exec {LOCK_FD}>>"$LOCK_FILE" || {
  echo "nextest-gpu-lock: cannot open the host-wide GPU lock $LOCK_FILE (set GF2_GPU_LOCK)" >&2
  exit 1
}

if ! flock -n -x "$LOCK_FD"; then
  echo "nextest-gpu-lock: waiting for the host-wide GPU lock $LOCK_FILE, held by another test run" >&2
  # `wait` returns as soon as a trapped signal arrives; a foreground flock
  # would defer the trap until it returned.
  trap 'echo "nextest-gpu-lock: timed out waiting for the host-wide GPU lock $LOCK_FILE, held by another test run" >&2; exit 1' HUP INT TERM
  flock -x "$LOCK_FD" &
  wait "$!"
  trap - HUP INT TERM
fi

exec "$@"
