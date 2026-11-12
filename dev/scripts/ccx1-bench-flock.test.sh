#!/usr/bin/env bash
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
TMP=$(mktemp -d)

# Section 2 starts a daemon that records its PID in $TMP/*.pid.
cleanup() {
  local pidfile
  for pidfile in "$TMP"/*.pid; do
    [ -e "$pidfile" ] && kill "$(cat "$pidfile")" 2>/dev/null || true
  done
  rm -rf "$TMP"
}
trap cleanup EXIT

# The wrapper runs only inside a benchmark window. Every lock below is a file
# under $TMP, so the host mutex is never touched.
export GF2_BENCH_WINDOW=1
# The default-list cases below run without a caller's CPU list.
unset GF2_CCX1_CPUS

# --- 1. Command composition, against stubbed flock, taskset and nice ---------
#
# The stubs print what they are asked to do instead of doing it, so this
# asserts the exact sequence: take the mutex, run the command, then unlock the
# mutex. The flock stub names the file behind the descriptor it is given.

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
touch "$TMP/lock"
lock=$(readlink -f "$TMP/lock")

default=$(
  PATH="$TMP/bin:$PATH" GF2_CCX1_LOCK="$TMP/lock" \
    "$ROOT/dev/scripts/ccx1-bench-flock.sh" probe arg
)
full_host=$(
  PATH="$TMP/bin:$PATH" GF2_CCX1_LOCK="$TMP/lock" \
    "$ROOT/dev/scripts/ccx1-bench-flock.sh" --full-host probe arg
)

expected_default=$(printf '%s\n' "flock -x $lock" \
  "taskset -c 6-11 nice -n -5 probe arg" "flock -u $lock")
expected_full_host=$(printf '%s\n' "flock -x $lock" \
  "nice -n -5 probe arg" "flock -u $lock")

test "$default" = "$expected_default"
test "$full_host" = "$expected_full_host"

# GF2_CCX1_CPUS replaces the pinned CPU list and nothing else.
pinned=$(
  PATH="$TMP/bin:$PATH" GF2_CCX1_LOCK="$TMP/lock" GF2_CCX1_CPUS=0-3 \
    "$ROOT/dev/scripts/ccx1-bench-flock.sh" probe arg
)
test "$pinned" = "${expected_default/taskset -c 6-11/taskset -c 0-3}"

if ! command -v flock >/dev/null 2>&1; then
  echo "ccx1-bench-flock.test: flock unavailable, skipping the release test" >&2
  exit 0
fi

export GF2_CCX1_LOCK="$TMP/rt.lock"
touch "$GF2_CCX1_LOCK"

# --- 2. No lock outlives the wrapped command ---------------------------------
#
# What this guards: a daemon the wrapped command starts inherits its
# descriptors, and a flock lock lasts until every copy of its descriptor is
# closed or one of them unlocks it. Under `ccx1-bench-flock.sh --full-host` the
# command holds the mutex descriptor by design, because the benchmark runner
# refuses to measure without an inherited descriptor for the held lock
# (`host::inherited_lock`). An inherited copy in a daemon the command leaves
# running — a compiler-cache server is the case that arises in practice —
# would hold the mutex past the run and every acquirer on the host would wait
# for it. The wrapper's unlock is what prevents that, and this test is what
# holds it to it.
#
# The wrapped command records the lock as it sees it, then starts a daemon
# the way the sccache server starts: in its own session with its standard
# streams detached, so that it outlives the command. The lock must be free
# once the wrapper returns, while the daemon still runs and holds its
# descriptor.

# Prints the state of the lock, then whether process $1 has it open.
cat >"$TMP/observe" <<'EOF'
#!/usr/bin/env bash
set -eu
probe() { if flock -n "$@" true 2>/dev/null; then echo free; else echo held; fi; }
open=$(for fd in /proc/"$1"/fd/*; do readlink "$fd" || true; done)
has() { grep -qxF "$(readlink -f "$1")" <<<"$open"; }
descriptors=none
if has "$GF2_CCX1_LOCK"; then descriptors=mutex; fi
echo "exclusive=$(probe -x "$GF2_CCX1_LOCK") shared=$(probe -s "$GF2_CCX1_LOCK")" \
  "descriptors=$descriptors"
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

# Runs one wrapper invocation, then checks the lock after it returned and as
# the command saw it while it ran.
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

check_release bench \
  "exclusive=held shared=held descriptors=mutex" \
  "exclusive=free shared=free descriptors=mutex" \
  "$ROOT/dev/scripts/ccx1-bench-flock.sh" --full-host

echo "ccx1-bench-flock.test: ok"
