#!/usr/bin/env bash
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
TMP=$(mktemp -d)

# Section 3 starts daemons that record their PIDs in $TMP/*.pid.
cleanup() {
  local pidfile
  for pidfile in "$TMP"/*.pid; do
    [ -e "$pidfile" ] && kill "$(cat "$pidfile")" 2>/dev/null || true
  done
  rm -rf "$TMP"
}
trap cleanup EXIT

# --- 1. Command composition, against stubbed flock, taskset and nice ---------
#
# The stubs print what they are asked to do instead of doing it, so this
# asserts the exact sequence: take the turnstile, take the mutex, run the
# command, then unlock the mutex and the turnstile. The flock stub names the
# file behind the descriptor it is given.

mkdir "$TMP/bin"
cat >"$TMP/bin/flock" <<'EOF'
#!/usr/bin/env bash
echo "flock $1 $(readlink "/proc/self/fd/$2")"
EOF
cat >"$TMP/bin/taskset" <<'EOF'
#!/usr/bin/env bash
echo "$(basename "$0") $*"
EOF
cp "$TMP/bin/taskset" "$TMP/bin/nice"
chmod +x "$TMP/bin/flock" "$TMP/bin/taskset" "$TMP/bin/nice"
touch "$TMP/lock" "$TMP/lock.turnstile"
lock=$(readlink -f "$TMP/lock")

default=$(
  PATH="$TMP/bin:$PATH" GF2_CCX1_LOCK="$TMP/lock" \
    "$ROOT/dev/scripts/ccx1-bench-flock.sh" probe arg
)
full_host=$(
  PATH="$TMP/bin:$PATH" GF2_CCX1_LOCK="$TMP/lock" \
    "$ROOT/dev/scripts/ccx1-bench-flock.sh" --full-host probe arg
)

expected_default=$(printf '%s\n' "flock -x $lock.turnstile" "flock -x $lock" \
  "taskset -c 6-11 nice -n -5 probe arg" "flock -u $lock" "flock -u $lock.turnstile")
expected_full_host=$(printf '%s\n' "flock -x $lock.turnstile" "flock -x $lock" \
  "nice -n -5 probe arg" "flock -u $lock" "flock -u $lock.turnstile")

test "$default" = "$expected_default"
test "$full_host" = "$expected_full_host"

# The turnstile path is derived from the mutex path, so overriding the mutex in
# one place does not leave two runs sharing a stale turnstile.
custom=$(
  PATH="$TMP/bin:$PATH" GF2_CCX1_LOCK="$TMP/lock" GF2_CCX1_TURNSTILE="$TMP/ts" \
    "$ROOT/dev/scripts/ccx1-bench-flock.sh" --full-host probe
)
ts=$(readlink -f "$TMP/ts")
test "$custom" = "$(printf '%s\n' "flock -x $ts" "flock -x $lock" \
  "nice -n -5 probe" "flock -u $lock" "flock -u $ts")"

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
  echo "ccx1-bench-flock.test: flock unavailable, skipping the ordering and release tests" >&2
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

# --- 3. No lock outlives the wrapped command ---------------------------------
#
# What this guards: a daemon the wrapped command starts inherits its
# descriptors, and a flock lock lasts until every copy of its descriptor is
# closed or one of them unlocks it. A compiler-cache server started by a
# cargo-ci run under `ccx1-bench-flock.sh --full-host` is the case that arises
# in practice: without the wrapper's unlock it holds the mutex and the
# turnstile after the run, and every build on the host waits for it.
#
# The wrapped command records the locks as it sees them, then starts a daemon
# the way the sccache server starts: in its own session with its standard
# streams detached, so that it outlives the command. Under each wrapper every
# lock must be free once the wrapper returns, while the daemon still runs and
# holds no turnstile descriptor.
#
# cargo-budget.sh passes because it never gives the command a lock
# descriptor: it closes its CCX1 and slot descriptors in the child with the
# `{SLOT_FD}>&- {CCX1_FD}>&-` redirections and takes the test lock with
# `flock -o`. Under the benchmark wrapper the command and so the daemon do
# hold the mutex descriptor, because the benchmark runner refuses to measure
# without an inherited descriptor for the held lock (`host::inherited_lock`);
# the wrapper's unlock after the command must free the lock all the same.

# Prints the state of every lock, then which of them process $1 has open.
cat >"$TMP/observe" <<'EOF'
#!/usr/bin/env bash
set -eu
shopt -s nullglob
probe() { if flock -n "$@" true 2>/dev/null; then echo free; else echo held; fi; }
slot=free
for path in "$CARGO_CI_LOCK_DIR"/slot.*; do
  [ "$(probe -x "$path")" = free ] || slot=held
done
open=$(for fd in /proc/"$1"/fd/*; do readlink "$fd" || true; done)
has() { grep -qxF "$(readlink -f "$1")" <<<"$open"; }
descriptors=
if has "$GF2_CCX1_LOCK"; then descriptors+=mutex,; fi
if has "$GF2_CCX1_TURNSTILE"; then descriptors+=turnstile,; fi
if has "$CARGO_CI_BUILD_LOCK"; then descriptors+=test,; fi
for path in "$CARGO_CI_LOCK_DIR"/slot.*; do
  if has "$path"; then descriptors+=slot,; fi
done
descriptors=${descriptors%,}
echo "exclusive=$(probe -x "$GF2_CCX1_LOCK") shared=$(probe -s "$GF2_CCX1_LOCK")" \
  "turnstile=$(probe -x "$GF2_CCX1_TURNSTILE") slot=$slot" \
  "test=$(probe -x "$CARGO_CI_BUILD_LOCK") descriptors=${descriptors:-none}"
EOF

# The wrapped command: $1 receives its observation, $2 its daemon's PID.
cat >"$TMP/wrapped" <<'EOF'
#!/usr/bin/env bash
set -eu
"$(dirname "$0")/observe" $$ >"$1"
setsid bash -c 'echo $$ >"$1"; exec sleep 60' _ "$2" </dev/null >/dev/null 2>&1 &
for _ in $(seq 1 50); do
  [ -s "$2" ] && exit 0
  sleep 0.1
done
exit 1
EOF
chmod +x "$TMP/observe" "$TMP/wrapped"

# Runs one wrapper invocation, then checks the locks after it returned and as
# the command saw them while it ran.
check_release() {
  local name=$1 during=$2 after=$3 pid got
  shift 3
  "$@" "$TMP/wrapped" "$TMP/$name.during" "$TMP/$name.pid"
  pid=$(cat "$TMP/$name.pid")
  kill -0 "$pid" || { echo "$name: the daemon exited with the command" >&2; exit 1; }
  got=$("$TMP/observe" "$pid")
  test "$got" = "$after" || {
    echo "$name: after the command exited, expected '$after', got '$got'" >&2
    exit 1
  }
  got=$(cat "$TMP/$name.during")
  test "$got" = "$during" || {
    echo "$name: while the command ran, expected '$during', got '$got'" >&2
    exit 1
  }
  kill "$pid"
}

free="exclusive=free shared=free turnstile=free slot=free test=free"

check_release build \
  "exclusive=held shared=free turnstile=free slot=held test=free descriptors=none" \
  "$free descriptors=none" \
  "$ROOT/scripts/cargo-budget.sh"
check_release test \
  "exclusive=held shared=free turnstile=free slot=held test=held descriptors=none" \
  "$free descriptors=none" \
  "$ROOT/scripts/cargo-budget.sh" --test
check_release bench \
  "exclusive=held shared=held turnstile=held slot=free test=free descriptors=mutex" \
  "$free descriptors=mutex" \
  "$ROOT/dev/scripts/ccx1-bench-flock.sh" --full-host

echo "ccx1-bench-flock.test: ok"
