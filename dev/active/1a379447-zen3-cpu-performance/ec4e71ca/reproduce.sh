#!/usr/bin/env bash
# Runs one allocation-witness test binary `runs` times under the delayed-spawn
# shim at each delay, then once more beside the package's full suite, and
# prints one summary line per run set.
#
# Usage: reproduce.sh <checkout> <package> <test-binary> [runs] [delays-us...]
#
# The shim (`spawn-delay.c`) busy-waits in the creating thread for
# SPAWN_DELAY_US microseconds after `pthread_create` returns, emulating a
# libtest main thread descheduled right after it spawns the test thread.
set -euo pipefail

checkout=$(cd "$1" && pwd)
package=$2
binary=$3
runs=${4:-200}
shift $(( $# < 4 ? $# : 4 ))
delays=("$@")
[ "${#delays[@]}" -gt 0 ] || delays=(150 200 250 300)

here=$(cd "$(dirname "$0")" && pwd)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
cc -shared -fPIC -O2 -o "$work/spawn-delay.so" "$here/spawn-delay.c" -ldl

cd "$checkout"
budget=./scripts/cargo-budget.sh
nextest=(cargo nextest run -p "$package" --cargo-profile ci-test --profile ci)
"$budget" "${nextest[@]}" --test "$binary" --no-run >/dev/null 2>&1

summary() {
  # nextest closes each stress iteration with one "Stress test" line that
  # names the failed tests of that iteration, if any.
  local label=$1 log=$2 failed
  failed=$(grep -c '^ *Stress test \[.*[0-9] failed' "$log" || true)
  printf '%s %s: %d of %d runs failed\n' "$binary" "$label" "$failed" "$runs"
}

for delay in "${delays[@]}"; do
  log="$work/delay-$delay.log"
  LD_PRELOAD="$work/spawn-delay.so" SPAWN_DELAY_US=$delay \
    "$budget" --test "${nextest[@]}" --test "$binary" --stress-count "$runs" \
    --no-fail-fast >"$log" 2>&1 || true
  summary "shim ${delay}us" "$log"
done

# Beside the package suite: the suite runs unlocked so both hold the host.
"$budget" "${nextest[@]}" --no-run >/dev/null 2>&1
CARGO_CI_NO_LOCK=1 "$budget" --test "${nextest[@]}" --no-fail-fast \
  >"$work/package.log" 2>&1 &
suite=$!
log="$work/beside.log"
"$budget" --test "${nextest[@]}" --test "$binary" --stress-count "$runs" \
  --no-fail-fast >"$log" 2>&1 || true
wait "$suite" || true
summary "beside the $package suite" "$log"
