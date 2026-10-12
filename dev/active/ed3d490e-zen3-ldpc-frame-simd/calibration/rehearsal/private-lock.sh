#!/usr/bin/env bash
# Rehearsal stand-in for the host mutex wrapper (jit:0a77ecfc).
#
# Usage: private-lock.sh [--full-host] <command> [args...]
#
# Holds the lock file GF2_CCX1_LOCK names across the command, as the wrapper it
# replaces in the rehearsal clone does, and refuses the host mutex path. It
# pins no CPU and raises no priority.
set -euo pipefail
if [[ ${1:-} == --full-host ]]; then
  shift
fi
lock=${GF2_CCX1_LOCK:?the rehearsal names its own lock file}
if [[ $(realpath "$lock") == /tmp/gf2-ccx1.lock ]]; then
  echo 'private-lock: the rehearsal never takes the host mutex' >&2
  exit 3
fi
exec {lock_fd}<"$lock"
flock -x "$lock_fd"
status=0
"$@" || status=$?
exit "$status"
