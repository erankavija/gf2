#!/usr/bin/env bash
# Behavioral test for scripts/nextest-gpu-lock.sh. Every lock is a file under
# $TMP, so the host-wide GPU lock is never touched.
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/.." && pwd)
WRAP="$ROOT/scripts/nextest-gpu-lock.sh"
TMP=$(mktemp -d)
HOLDER=
cleanup() {
  [[ -n "$HOLDER" ]] && kill "$HOLDER" 2>/dev/null || true
  rm -rf "$TMP"
}
trap cleanup EXIT

export GF2_GPU_LOCK="$TMP/gpu.lock"
fail() { echo "FAIL: $*" >&2; exit 1; }

# A probe that reports whether the lock is free to a process outside the
# wrapper's descriptor.
# shellcheck disable=SC2016 # expanded by the probing shell
probe='if flock -n "$GF2_GPU_LOCK" true; then echo free; else echo held; fi'

# --- 1. A test in a locked group holds the lock while it runs --------------
out=$("$WRAP" bash -c "$probe")
[[ "$out" == held ]] || fail "locked-group test ran without the lock: $out"
out=$(bash -c "$probe")
[[ "$out" == free ]] || fail "lock not released after the test exited: $out"

# --- 2. Exit status passes through ------------------------------------------
rc=0
"$WRAP" bash -c 'exit 7' || rc=$?
[[ "$rc" == 7 ]] || fail "exit status 7 became $rc"

# --- 3. A contended test waits, then runs once the holder releases ---------
flock "$GF2_GPU_LOCK" sleep 1 &
HOLDER=$!
sleep 0.2
start=$SECONDS
out=$("$WRAP" bash -c "echo ran" 2>"$TMP/err4")
[[ "$out" == ran ]] || fail "contended test did not run after release: $out"
grep -qF "waiting for the host-wide GPU lock $GF2_GPU_LOCK" "$TMP/err4" ||
  fail "contended test did not name the lock it waits for: $(cat "$TMP/err4")"
wait "$HOLDER"
HOLDER=
(( SECONDS - start <= 5 )) || fail "contended test waited past the holder's release"

# --- 4. A test terminated while waiting fails naming the lock --------------
# nextest ends a test that exceeds its budget with SIGTERM to its process group.
flock "$GF2_GPU_LOCK" sleep 30 &
HOLDER=$!
sleep 0.2
# shellcheck disable=SC2016 # expanded by the setsid shell
setsid bash -c 'echo $$ >"$1/pgid"; exec "$2" bash -c "echo ran"' \
  _ "$TMP" "$WRAP" >"$TMP/out5" 2>"$TMP/err5" &
waiter=$!
sleep 0.5
kill -TERM -- "-$(cat "$TMP/pgid")"
rc=0
wait "$waiter" || rc=$?
[[ "$rc" != 0 ]] || fail "test terminated while waiting reported success"
[[ ! -s "$TMP/out5" ]] || fail "test ran without the lock: $(cat "$TMP/out5")"
grep -qF "timed out waiting for the host-wide GPU lock $GF2_GPU_LOCK" "$TMP/err5" ||
  fail "terminated waiter did not name the lock: $(cat "$TMP/err5")"
kill "$HOLDER" 2>/dev/null || true
HOLDER=

# --- 5. The filter check rejects a script filter that differs ---------------
CHECK="$ROOT/scripts/check-gpu-lock-filter.py"
python3 -B "$CHECK" >/dev/null || fail "filter check rejects the committed config"
cp "$ROOT/.config/nextest.toml" "$TMP/nextest.toml"
python3 - "$TMP/nextest.toml" <<'PY'
import sys
p = sys.argv[1]
s = open(p).read()
i = s.index("[[profile.default.scripts]]")
s = s[:i] + s[i:].replace("binary(gpu_dispatcher)", "binary(gpu_dispatcher_typo)", 1)
open(p, "w").write(s)
PY
if python3 -B "$CHECK" "$TMP/nextest.toml" 2>"$TMP/err6"; then
  fail "filter check accepted a script filter that differs from the gpu-serial override"
fi
grep -qF "differs from the 'gpu-serial' override filter" "$TMP/err6" ||
  fail "filter check failed for another reason: $(cat "$TMP/err6")"

echo "nextest-gpu-lock: all checks passed"
