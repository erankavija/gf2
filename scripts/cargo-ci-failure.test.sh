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
export CARGO_CI_LOG_DIR="$TMP/target/cargo-ci-logs/stamp"
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
  cmp -s "$2" "$CARGO_CI_LOG_DIR/$1.log" || fail "$1: kept log differs"
  printf '%s\n' "$out"
}
has() { grep -qE -- "$2" <<<"$1" || fail "missing /$2/ in: $1"; }

# nextest 0.9.133 (65e806bd5): errors.rs emits "\nCaused by:\n";
# confirmed in the installed cargo-nextest binary (strings), since fetching
# that source is unavailable. The metadata/JSON diagnostics below also occur
# in its strings. The setup lines use the literal formats in
# write_setup_script_status_line + short_status_str in the pinned source:
# https://raw.githubusercontent.com/nextest-rs/nextest/cargo-nextest-0.9.133/nextest-runner/src/reporter/displayer/imp.rs
# DisplayScriptInstance prints "id: command". Keep whitespace verbatim.
cat >"$TMP/chain.out" <<'LOG'
error: error parsing Cargo metadata

Caused by:
  EOF while parsing a value at line 1 column 0

  unrelated indented line
LOG
out=$(check chain "$TMP/chain.out" '^Caused by:$' 'tail|unrelated')
has "$out" '^  EOF while parsing a value at line 1 column 0$'
[[ "$out" == *$'error: error parsing Cargo metadata\n\nCaused by:\n  EOF'* ]] || fail "chain separator lost"

# Verbatim result/summary lines from a captured nextest run on fraktaali,
# 2026-11-15; its other results are omitted so Summary alone is also tested.
cat >"$TMP/summary.out" <<'LOG'
     Summary [  18.277s] 148 tests run: 147 passed, 1 timed out, 0 skipped
LOG
cat >"$TMP/failed-summary.out" <<'LOG'
     TIMEOUT [  15.023s] (147/148) tuning-calibration::core_calibration_harness tuning_calibration::campaign_owner::tests::accepted_complete_experiment_measures_defaults_and_reopens_only_its_measured_leaves
LOG
cat "$TMP/summary.out" >>"$TMP/failed-summary.out"
out=$(check failed-summary "$TMP/failed-summary.out" 'TIMEOUT .*accepted_complete_experiment' 'last 3 log lines')
[[ $(grep -cE "$NEXTEST_SUMMARY_RE" <<<"$out") == 1 ]] || fail "summary duplicated with failing test"
out=$(check summary "$TMP/summary.out" 'last 3 log lines')
[[ $(grep -cE "$NEXTEST_SUMMARY_RE" <<<"$out") == 1 ]] || fail "summary duplicated in fallback"
printf '%s\n' l1 l2 l3 l4 >>"$TMP/summary.out"
out=$(check summary-tail "$TMP/summary.out" 'last 3 log lines' '^l1$')
has "$out" '^l4$'
[[ $(grep -cE "$NEXTEST_SUMMARY_RE" <<<"$out") == 1 ]] || fail "summary outside tail lost"

cat >"$TMP/setup-success.out" <<'LOG'
  SETUP PASS [   0.100s] setup-pass: setup
  SETUP LEAK [   0.100s] setup-leak: setup
SETUP TMPASS [   0.100s] setup-timeout: setup
error: creating test list failed
LOG
out=$(check setup-success "$TMP/setup-success.out" 'error: creating test list failed' 'SETUP|last 3 log lines')

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

# Exercise the wrapper's actual failed-step path, without invoking cargo.
# Every nextest step preserves the entire log outside its disposable root.
(
  # shellcheck disable=SC1090 # load only these function definitions
  source <(sed -n '/^run_step() {/,/^}/p; /^summarize_fail() {/,/^}/p' "$ROOT/scripts/cargo-ci.sh")
  RUN_ROOT="$TMP/run"
  LOGDIR="$RUN_ROOT/logs"
  mkdir -p "$LOGDIR"
  # shellcheck disable=SC2034 # read by the loaded run_step function
  CARGO_CI_STEPS='' failed=0 skipped=0 summary=''
  # shellcheck disable=SC2329 # invoked by the loaded run_step function
  nextest_failure() { cat "$TMP/chain.out"; return 100; }
  while read -r name; do
    out=$(run_step "$name" nextest_failure)
    has "$out" "^full log: $CARGO_CI_LOG_DIR/$name.log$"
    cmp -s "$TMP/chain.out" "$CARGO_CI_LOG_DIR/$name.log" || fail "$name: full log differs"
  done < <(awk '/^run_step / && /cargo nextest|\$SCALAR_LANE/ { print $2 }' "$ROOT/scripts/cargo-ci.sh")
  rm -rf "$RUN_ROOT"
  cmp -s "$TMP/chain.out" "$CARGO_CI_LOG_DIR/test.log" || fail "log lost with run root"
)

# An unwritable log directory is reported and does not abort the caller.
touch "$TMP/file"
out=$(CARGO_CI_LOG_DIR="$TMP/file/sub" keep_failed_log x "$TMP/plain.out")
has "$out" '^full log not kept: cannot write '
echo "cargo-ci-failure: ok"
