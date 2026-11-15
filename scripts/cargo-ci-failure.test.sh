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
  out=$(report_failed_step "$1" "$2")
  grep -qE -- "$3" <<<"$out" || fail "$1: missing /$3/ in: $out"
  [ -z "${4:-}" ] || ! grep -qE -- "$4" <<<"$out" || fail "$1: unexpected /$4/"
  grep -qF "full log: $CARGO_CI_LOG_DIR/$1.log" <<<"$out" || fail "$1: no log path"
  [ -s "$CARGO_CI_LOG_DIR/$1.log" ] || fail "$1: log not kept"
  ! grep -q $'\x1b' "$CARGO_CI_LOG_DIR/$1.log" || fail "$1: kept log has colour"
}

check test "$TMP/test.out" 'FAIL .*a::broken' 'tail'
grep -q 'assertion failed' <(report_failed_step test "$TMP/test.out") || fail "stderr section"
check setup "$TMP/setup.out" 'error: setup script failed' 'Starting 3'
grep -q 'SETUP FAIL' <(report_failed_step setup "$TMP/setup.out") || fail "SETUP FAIL line"
check plain "$TMP/plain.out" 'last 3 log lines' 'l2'
grep -qx l5 <(report_failed_step plain "$TMP/plain.out") || fail "tail content"
echo "cargo-ci-failure: ok"
