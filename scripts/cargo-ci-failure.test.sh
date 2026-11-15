#!/usr/bin/env bash
# Behavioral test for scripts/cargo-ci-failure.sh on fixture logs; runs no cargo.
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/.." && pwd)
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT
fail() { echo "FAIL: $*" >&2; exit 1; }

plain() { sed -E 's/\x1b\[[0-9;]*[mK]//g' "$1"; }
# shellcheck source=scripts/cargo-ci-failure.sh
source "$ROOT/scripts/cargo-ci-failure.sh"
export CARGO_CI_LOG_DIR="$TMP/keep/stamp"
# shellcheck disable=SC2034 # read by print_failure_cause
CARGO_CI_FAIL_TAIL_LINES=3

printf '%s\n' '        PASS [   0.1s] a::ok' \
  $'        \x1b[31mFAIL\x1b[0m [   0.2s] a::broken' \
  '--- STDERR: a broken ---' 'assertion failed: x == y' >"$TMP/test.out"
printf '%s\n' '    Starting 3 tests' $'\x1b[31merror\x1b[0m: setup script failed' \
  '      SETUP FAIL [   0.0s] db-init' >"$TMP/setup.out"
printf '%s\n' l1 l2 l3 l4 l5 >"$TMP/plain.out"

check() { # name log pattern-present pattern-absent
  local out
  out=$(print_failure_cause "$2"; keep_failed_log "$1" "$2")
  grep -qE -- "$3" <<<"$out" || fail "$1: missing /$3/ in: $out"
  [ -z "${4:-}" ] || ! grep -qE -- "$4" <<<"$out" || fail "$1: unexpected /$4/"
  grep -qF "full log: $CARGO_CI_LOG_DIR/$1.log" <<<"$out" || fail "$1: no log path"
  [ -s "$CARGO_CI_LOG_DIR/$1.log" ] || fail "$1: log not kept"
  ! grep -q $'\x1b' "$CARGO_CI_LOG_DIR/$1.log" || fail "$1: kept log has colour"
  printf '%s\n' "$out"
}
has() { grep -qE -- "$2" <<<"$1" || fail "missing /$2/ in: $1"; }

out=$(check test "$TMP/test.out" 'FAIL .*a::broken' 'tail')
has "$out" 'assertion failed'
out=$(check setup "$TMP/setup.out" 'error: setup script failed' 'Starting 3')
has "$out" 'SETUP FAIL'
out=$(check plain "$TMP/plain.out" 'last 3 log lines' 'l2')
has "$out" '^l5$'

# Failure labels incl. compound and retry forms, an unknown future label, and
# success forms that stay unprinted, with a list error after the results.
printf '%s\n' '        XFAIL [   0.1s] m::x' '        SIGKILL [   0.2s] m::k' \
  '        SIGILL [   0.2s] m::i' '    TRY 2 TIMEOUT [  1.0s] m::t' \
  '        LKFAIL [   0.3s] m::l' '        PASS [   0.1s] m::ok' \
  '      FAIL + LEAK [   0.1s] m::fl' '    TRY 2 FL+LK [   0.1s] m::tfl' \
  '    TRY 2 TMT [   0.1s] m::tmt' '    TRY 2 KILL [   0.1s] m::tk' \
  '      NEWFAIL [   0.1s] m::future' '     FLAKY 2/3 [   0.1s] m::flaky' \
  '    TRY 1 PASS [   0.1s] m::tp' '        LEAK [   0.1s] m::leakok' \
  '        START [         ] m::st' \
  '     Summary [   1.000s] 9 tests run: 3 passed, 6 failed, 0 skipped' \
  'error: creating test list failed' '  Caused by:' '    archive is corrupt' \
  '    at byte 12' '' '  unrelated indented line' >"$TMP/mixed.out"
out=$(check mixed "$TMP/mixed.out" 'XFAIL .*m::x' 'tail|m::ok|unrelated|m::flaky|m::tp|m::leakok|m::st')
for p in 'SIGKILL .*m::k' 'SIGILL .*m::i' 'TIMEOUT .*m::t' 'LKFAIL .*m::l' \
  'FAIL \+ LEAK .*m::fl' 'FL\+LK .*m::tfl' 'TMT .*m::tmt' 'TRY 2 KILL .*m::tk' \
  'NEWFAIL .*m::future' 'Summary .*6 failed' \
  'error: creating test list failed' 'Caused by:' 'archive is corrupt' 'at byte 12'; do
  has "$out" "$p"
done

# An unwritable log directory is reported and does not abort the caller.
touch "$TMP/file"
out=$(CARGO_CI_LOG_DIR="$TMP/file/sub" keep_failed_log x "$TMP/plain.out")
has "$out" '^full log not kept: cannot write '
echo "cargo-ci-failure: ok"
