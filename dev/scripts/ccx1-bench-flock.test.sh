#!/usr/bin/env bash
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

# --- 1. Command composition, against a stubbed flock -------------------------
#
# The stub prints its arguments instead of locking, so this asserts the exact
# nesting: the outer flock takes the turnstile and would exec the inner flock,
# which takes the mutex and execs the command.

mkdir "$TMP/bin"
printf '#!/usr/bin/env bash\nprintf "%%s\\n" "$@"\n' >"$TMP/bin/flock"
chmod +x "$TMP/bin/flock"
touch "$TMP/lock" "$TMP/lock.turnstile"

default=$(
  PATH="$TMP/bin:$PATH" GF2_CCX1_LOCK="$TMP/lock" \
    "$ROOT/dev/scripts/ccx1-bench-flock.sh" probe arg
)
full_host=$(
  PATH="$TMP/bin:$PATH" GF2_CCX1_LOCK="$TMP/lock" \
    "$ROOT/dev/scripts/ccx1-bench-flock.sh" --full-host probe arg
)

expected_default=$(printf '%s\n' \
  -x "$TMP/lock.turnstile" flock -x "$TMP/lock" taskset -c 6-11 nice -n -5 probe arg)
expected_full_host=$(printf '%s\n' \
  -x "$TMP/lock.turnstile" flock -x "$TMP/lock" nice -n -5 probe arg)

test "$default" = "$expected_default"
test "$full_host" = "$expected_full_host"

# The turnstile path is derived from the mutex path, so overriding the mutex in
# one place does not leave two runs sharing a stale turnstile.
custom=$(
  PATH="$TMP/bin:$PATH" GF2_CCX1_LOCK="$TMP/lock" GF2_CCX1_TURNSTILE="$TMP/ts" \
    "$ROOT/dev/scripts/ccx1-bench-flock.sh" --full-host probe
)
test "$custom" = "$(printf '%s\n' -x "$TMP/ts" flock -x "$TMP/lock" nice -n -5 probe)"

# --- 2. Writer preference, against the real flock ----------------------------
#
# The regression this guards: a pending `flock -x` does not block a new
# `flock -s` on Linux, so before the turnstile a stream of builds could keep
# the shared side permanently occupied and a measurement run was never granted.
# Observed 2026-09-08 with four runners queued 10-29 minutes.
#
# Timeline: a build takes the shared side and holds it for HOLD seconds; a
# measurement run queues behind it; a second build starts after that. The
# second build must land after the measurement run, not slip in ahead of it.

if ! command -v flock >/dev/null 2>&1; then
  echo "ccx1-bench-flock.test: flock unavailable, skipping the ordering test" >&2
  exit 0
fi

HOLD=4
events="$TMP/events"
: >"$events"

export GF2_CCX1_LOCK="$TMP/rt.lock"
export GF2_CCX1_TURNSTILE="$TMP/rt.lock.turnstile"
export CARGO_CI_LOCK_DIR="$TMP/slots"
export CARGO_CI_BUILD_LOCK="$TMP/build.lock"
touch "$GF2_CCX1_LOCK" "$GF2_CCX1_TURNSTILE"

"$ROOT/scripts/cargo-budget.sh" \
  sh -c "echo build-1 >>'$events'; sleep $HOLD" &
build1=$!

# Wait until build 1 actually holds the shared side.
for _ in $(seq 1 100); do
  grep -q build-1 "$events" && break
  sleep 0.1
done
grep -q build-1 "$events" || { echo "build 1 never started" >&2; exit 1; }

"$ROOT/dev/scripts/ccx1-bench-flock.sh" --full-host \
  sh -c "echo bench >>'$events'" &
bench=$!

sleep 1  # let the bench run reach the turnstile and queue for the mutex

"$ROOT/scripts/cargo-budget.sh" sh -c "echo build-2 >>'$events'" &
build2=$!

wait "$build1" "$bench" "$build2"

got=$(tr '\n' ' ' <"$events")
test "$got" = "build-1 bench build-2 " || {
  echo "expected 'build-1 bench build-2 ', got '$got'" >&2
  exit 1
}

echo "ccx1-bench-flock.test: ok"
