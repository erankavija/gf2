#!/usr/bin/env bash
# Tests the window runner's pre-job load wait against a fake load source,
# through --load-wait-only, so no window opens and no job or lock is touched.
#
# Usage:
#   ./dev/scripts/bench-window/run-window.test.sh
set -euo pipefail

HERE=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

# Runs the wait with fake load $1 and prints the window log it wrote.
run_wait() {
  local name=$1 one=$2 bound=${3:-2} poll=${4:-1}
  printf '%s 2.00 3.00 1/100 4242\n' "$one" >"$TMP/$name.loadavg"
  GF2_WINDOW_STATE="$TMP/$name" GF2_WINDOW_LOADAVG="$TMP/$name.loadavg" \
    GF2_WINDOW_LOAD_MAX=1.0 GF2_WINDOW_LOAD_WAIT_MAX_S=$bound GF2_WINDOW_LOAD_POLL_S=$poll \
    "$HERE/run-window.sh" --load-wait-only
  cat "$TMP/$name/window.log"
}

reject() {
  local name=$1 pattern=$2 got=$3
  if grep -Eq "$pattern" <<<"$got"; then
    printf 'run-window.test: %s: unexpected line matching /%s/ in:\n%s\n' "$name" "$pattern" "$got" >&2
    exit 1
  fi
}

expect() {
  local name=$1 pattern=$2 got=$3
  grep -Eq "$pattern" <<<"$got" || {
    printf 'run-window.test: %s: no line matching /%s/ in:\n%s\n' "$name" "$pattern" "$got" >&2
    exit 1
  }
}

# A quiet host starts at once, and neither case opens the window.
quiet=$(run_wait quiet 0.40)
expect quiet 'load wait start max=1\.0 bound_s=2 load=0\.40 2\.00 3\.00$' "$quiet"
expect quiet 'load wait quiet after_s=0 load=0\.40 2\.00 3\.00$' "$quiet"
reject quiet 'bound passed|window start' "$quiet"

# A host that stays loaded is polled until the bound, then the job starts.
loaded=$(run_wait loaded 3.50)
expect loaded 'load wait start max=1\.0 bound_s=2 load=3\.50 2\.00 3\.00$' "$loaded"
expect loaded 'load wait bound passed after_s=[2-9] load=3\.50 2\.00 3\.00; starting anyway$' "$loaded"
reject loaded 'load wait quiet|window start' "$loaded"

# A poll interval that does not divide the bound stops at the bound.
capped=$(run_wait capped 3.50 1 5)
expect capped 'load wait bound passed after_s=1 load=3\.50 2\.00 3\.00; starting anyway$' "$capped"

# The estimate cap admits jobs up to its value; 0 lifts it.
cap_check() {
  local want=$1 cap=$2 minutes=$3 got=runs
  GF2_WINDOW_MAX_EST_MIN=$cap "$HERE/run-window.sh" --cap-check "$minutes" || got=deferred
  if [[ $got != "$want" ]]; then
    printf 'run-window.test: cap %s, estimate %s: %s, expected %s\n' "$cap" "$minutes" "$got" "$want" >&2
    exit 1
  fi
}
cap_check runs 5 5
cap_check deferred 5 6
cap_check runs 0 60
cap_check runs 60 60
cap_check deferred 20 60

# External inputs resolve under the window's repository root unless overridden,
# and no queue command finds them relative to its worktree's depth.
root=$("$HERE/run-window.sh" --print-root)
ext=$(env -u GF2_EXT_ROOT "$HERE/run-window.sh" --print-ext-root)
[[ $ext == "$root/.agents/ext" ]] || {
  printf 'run-window.test: default external-input root %s, expected %s\n' "$ext" "$root/.agents/ext" >&2
  exit 1
}
ext=$(GF2_EXT_ROOT="$TMP/ext" "$HERE/run-window.sh" --print-ext-root)
[[ $ext == "$TMP/ext" ]] || {
  printf 'run-window.test: GF2_EXT_ROOT override gave %s\n' "$ext" >&2
  exit 1
}
reject queue '^[^#].*\.\./' "$(cat "$HERE/queue.tsv")"

# The follower's status word comes from the runner's window.log alone.
status_of() {
  local name=$1 want=$2 got
  got=$(GF2_WINDOW_STATE="$TMP/$name" "$HERE/follow-window.sh" --print-status)
  if [[ $got != "$want" ]]; then
    printf 'run-window.test: follower status for %s: %s, expected %s\n' "$name" "$got" "$want" >&2
    exit 1
  fi
}
mkdir -p "$TMP/fw-none" "$TMP/fw-open" "$TMP/fw-ended" "$TMP/fw-second"
status_of fw-none idle
printf '2026-01-01T00:00:00Z window start host=h\n' >"$TMP/fw-open/window.log"
status_of fw-open open
printf '2026-01-01T00:00:00Z window start host=h\n2026-01-01T00:01:00Z window end\n' >"$TMP/fw-ended/window.log"
status_of fw-ended ended
printf '2026-01-01T00:00:00Z window start\n2026-01-01T00:01:00Z window end\n2026-01-02T00:00:00Z window start\n' >"$TMP/fw-second/window.log"
status_of fw-second open
# Command text in job records never counts as a lifecycle record.
mkdir -p "$TMP/fw-cmd-open" "$TMP/fw-cmd-ended"
printf '2026-01-01T00:00:00Z window start host=h\n2026-01-01T00:00:01Z job start issue=x key=k command=echo window end\n' >"$TMP/fw-cmd-open/window.log"
status_of fw-cmd-open open
printf '2026-01-01T00:00:00Z window start\n2026-01-01T00:01:00Z window end\n2026-01-01T00:01:01Z job start issue=x key=k command=echo window start\n' >"$TMP/fw-cmd-ended/window.log"
status_of fw-cmd-ended ended
# No default unit name remains in the follower.
reject follower 'systemctl|GF2_WINDOW_UNIT|gf2-bench-window' "$(cat "$HERE/follow-window.sh")"

printf '%s\n' "$quiet" "$loaded" "$capped"
echo "run-window.test: ok"
