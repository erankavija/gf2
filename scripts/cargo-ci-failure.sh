# shellcheck shell=bash
# Failure reporting for scripts/cargo-ci.sh, sourced by it and by
# scripts/cargo-ci-failure.test.sh. Defines functions only.
#
# `report_failed_step <name> <log>` prints a failed step's cause and keeps its
# full log under `$CARGO_CI_LOG_DIR/<name>.log`; the caller sets
# `CARGO_CI_LOG_DIR` (a directory outside the tracked tree) and `plain`.

# Lines of the log tail printed when no test result or error line is found.
CARGO_CI_FAIL_TAIL_LINES=40

# nextest status and error lines outside a test result: `error:`/`Error:`
# (setup script, wrapper script, list and archive failures, exit-code
# reports), `SETUP FAIL`-style setup-script statuses, and cause chains.
NEXTEST_ERROR_RE='^[[:space:]]*(error|Error|ERROR)\b|^[[:space:]]*SETUP[[:space:]]+(FAIL|LKFAIL|LEAK|TMO|EXFAIL|TIMEOUT)|^[[:space:]]*(Caused by|caused by)'

# Prints the per-test result lines and captured output, else the nextest error
# lines, else the log tail.
print_failure_cause() {
  local log="$1" found
  found=$(plain "$log" | grep -E "^[[:space:]]*(FAIL|TIMEOUT|SIGSEGV|SIGABRT|LEAK|×)" || true)
  if [ -n "$found" ]; then
    printf '%s\n' "$found"
    plain "$log" | grep -A 20 -E "^[[:space:]]*--- (STDOUT|STDERR):" | head -60 || true
    return 0
  fi
  found=$(plain "$log" | grep -E "$NEXTEST_ERROR_RE" || true)
  if [ -n "$found" ]; then
    printf '%s\n' "$found"
    return 0
  fi
  echo "(no test result or error line; last $CARGO_CI_FAIL_TAIL_LINES log lines)"
  plain "$log" | tail -n "$CARGO_CI_FAIL_TAIL_LINES"
}

keep_failed_log() {
  mkdir -p "$CARGO_CI_LOG_DIR"
  plain "$2" >"$CARGO_CI_LOG_DIR/$1.log"
  echo "full log: $CARGO_CI_LOG_DIR/$1.log"
}

report_failed_step() {
  echo "--- $1 failures ---"
  print_failure_cause "$2"
  keep_failed_log "$1" "$2"
}
