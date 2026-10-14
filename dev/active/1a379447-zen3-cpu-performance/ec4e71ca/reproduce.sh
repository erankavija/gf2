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
  # nextest's closing line reads "N/M stress run iterations: P passed, F failed";
  # a run set that did not complete every iteration is an error, not a count.
  local label=$1 log=$2 line completed failed
  line=$(grep -E '^ *Summary .* stress run iterations:' "$log" || true)
  completed=$(sed -nE 's/.* ([0-9]+)\/[0-9]+ stress run iterations:.*/\1/p' <<<"$line")
  failed=$(sed -nE 's/.* ([0-9]+) failed.*/\1/p' <<<"$line")
  if [ "${completed:-0}" -ne "$runs" ]; then
    printf '%s %s: %s of %d iterations completed; log:\n' "$binary" "$label" \
      "${completed:-0}" "$runs" >&2
    tail -n 20 "$log" >&2
    exit 1
  fi
  printf '%s %s: %d of %d runs failed\n' "$binary" "$label" "${failed:-0}" "$runs"
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
