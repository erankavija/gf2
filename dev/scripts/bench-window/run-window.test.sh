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
  local name=$1 one=$2
  printf '%s 2.00 3.00 1/100 4242\n' "$one" >"$TMP/$name.loadavg"
  GF2_WINDOW_STATE="$TMP/$name" GF2_WINDOW_LOADAVG="$TMP/$name.loadavg" \
    GF2_WINDOW_LOAD_MAX=1.0 GF2_WINDOW_LOAD_WAIT_MAX_S=2 GF2_WINDOW_LOAD_POLL_S=1 \
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

printf '%s\n' "$quiet" "$loaded"
echo "run-window.test: ok"
