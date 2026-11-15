# shellcheck shell=bash
# Failure reporting for scripts/cargo-ci.sh, sourced by it and by
# scripts/cargo-ci-failure.test.sh. Defines functions only.
#
# `print_failure_cause <log>` prints a failed nextest step's cause.
# `keep_failed_log <name> <log>` copies the full log to
# `$CARGO_CI_LOG_DIR/<name>.log` and prints its path, or one line saying it
# could not. The caller defines `plain` and `CARGO_CI_LOG_DIR` (a directory
# outside the tracked tree).

# Lines of the log tail printed when no failing result or error is found.
CARGO_CI_FAIL_TAIL_LINES=40

# nextest result lines (reporter/displayer/imp.rs): a right-aligned label, then
# the bracketed duration `[   0.005s]`, then the test or script name. Every
# such line is a failure unless its label is a success or progress status, so
# a label nextest adds later is printed by default.
NEXTEST_RESULT_RE='^[[:space:]]*[A-Za-z][A-Za-z0-9 +/-]*[[:space:]]\[[ 0-9.]+s\]'
NEXTEST_OK_LABEL_RE='^[[:space:]]*(SETUP )?(PASS|SKIP|SLOW|SLOW \+ LEAK|SLOW\+TMPASS|LEAK|TMPASS|TIMEOUT-PASS|START|TERMINATING|FLAKY [0-9]+/[0-9]+|TRY [0-9]+ (PASS|SLOW|START|TRMNTG))[[:space:]]+\['
NEXTEST_SUMMARY_RE='^[[:space:]]*Summary[[:space:]]+\['

# A nextest diagnostic outside a test result: an `error:`/`Error:` line with
# its indented cause chain.
print_nextest_errors() {
  plain "$1" | awk '
    /^[[:space:]]*(error|Error|ERROR)\>/ { on = 1; blanks = ""; print; next }
    on && /^[[:space:]]*$/ { blanks = blanks $0 "\n"; next }
    # Only a cause heading can continue a diagnostic across blank lines.
    on && /^[[:space:]]*(Caused by|caused by):/ {
      printf "%s", blanks; blanks = ""; print; next
    }
    on && blanks == "" && /^[[:space:]]+[^[:space:]]/ { print; next }
    { on = 0; blanks = "" }'
}

# Per-test failures with captured output, then nextest error blocks; the log
# tail only when both are absent.
print_failure_cause() {
  local log="$1" tests errors
  tests=$(plain "$log" | grep -E "$NEXTEST_RESULT_RE" | grep -vE "$NEXTEST_OK_LABEL_RE|$NEXTEST_SUMMARY_RE" || true)
  errors=$(print_nextest_errors "$log")
  if [ -n "$tests" ]; then
    printf '%s\n' "$tests"
    plain "$log" | grep -A 20 -E "^[[:space:]]*--- (STDOUT|STDERR):" | head -60 || true
  fi
  [ -z "$errors" ] || printf '%s\n' "$errors"
  plain "$log" | grep -E "$NEXTEST_SUMMARY_RE" || true
  if [ -z "$tests$errors" ]; then
    echo "(no failing test result or error line; last $CARGO_CI_FAIL_TAIL_LINES log lines)"
    plain "$log" | tail -n "$CARGO_CI_FAIL_TAIL_LINES" | grep -vE "$NEXTEST_SUMMARY_RE" || true
  fi
  return 0
}

keep_failed_log() {
  local dest="$CARGO_CI_LOG_DIR/$1.log"
  if { mkdir -p "$CARGO_CI_LOG_DIR" && cp "$2" "$dest"; } 2>/dev/null; then
    echo "full log: $dest"
  else
    echo "full log not kept: cannot write $dest"
  fi
  return 0
}
