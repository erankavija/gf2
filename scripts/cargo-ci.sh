#!/usr/bin/env bash
set -euo pipefail

# Cargo CI gate wrapper for jit.
#
# Runs the full Rust CI pipeline (check, test, clippy, fmt) and produces
# concise output: one-line summaries on success, full diagnostics on failure.
#
# Exit codes:
#   0 — all steps passed
#   1 — one or more steps failed
#   2 — environment problem: no real cargo available on PATH

# Resolve the real cargo binary. Some local setups place a debugging shim
# at ~/.cargo/bin/cargo (or its rustup proxy target) that exits 0 for every
# invocation; without this guard each cargo step below would silently
# succeed, recording a false-positive gate PASS in milliseconds. Detect a
# stub via the canonical `cargo X.Y.Z` version-probe pattern, then fall
# back to a rustup toolchain binary when needed. Fail loudly (exit 2) if
# no real cargo can be resolved — better an honest gate failure than a
# silent rubber-stamp.
#
# Discovered in jit issue 941d1528 (cargo-ci gate silently false-passes
# when cargo is a stub).
ensure_real_cargo() {
  local probe
  probe=$(cargo --version 2>&1 || true)
  if [[ "$probe" =~ ^cargo[[:space:]][0-9]+\.[0-9]+\.[0-9]+ ]]; then
    return 0
  fi
  for tc_dir in "$HOME/.rustup/toolchains"/stable-*; do
    [[ -d "$tc_dir" && -x "$tc_dir/bin/cargo" ]] || continue
    export PATH="$tc_dir/bin:$PATH"
    probe=$(cargo --version 2>&1 || true)
    if [[ "$probe" =~ ^cargo[[:space:]][0-9]+\.[0-9]+\.[0-9]+ ]]; then
      echo "cargo-ci: cargo on PATH was a stub; using $tc_dir/bin/cargo" >&2
      return 0
    fi
  done
  echo "ERROR: no real cargo available on PATH and no usable rustup stable toolchain found." >&2
  echo "       cargo --version output: $probe" >&2
  echo "       Restore ~/.cargo/bin/cargo or install a stable toolchain (rustup install stable)." >&2
  exit 2
}

ensure_real_cargo

TMPDIR=$(mktemp -d)
trap 'rm -rf "$TMPDIR"' EXIT

failed=0
summary=""

run_step() {
  local name="$1"
  shift

  if "$@" >"$TMPDIR/$name.out" 2>&1; then
    local detail
    detail=$(summarize_pass "$name")
    summary+="  ✓ $name: $detail"$'\n'
  else
    local rc=$?
    summary+="  ✗ $name: FAILED (exit $rc)"$'\n'
    summarize_fail "$name"
    failed=1
  fi
}

summarize_pass() {
  local name="$1"
  case "$name" in
    test)
      # Extract nextest summary line: "Summary [Xs] N tests run: P passed, F failed, S skipped"
      local summary_line
      summary_line=$(grep "^Summary" "$TMPDIR/$name.out" || true)
      if [ -n "$summary_line" ]; then
        local p f s
        p=$(echo "$summary_line" | grep -oP '\d+(?= passed)' || true)
        f=$(echo "$summary_line" | grep -oP '\d+(?= failed)' || true)
        s=$(echo "$summary_line" | grep -oP '\d+(?= skipped)' || true)
        echo "${p:-0} passed, ${f:-0} failed, ${s:-0} skipped"
      else
        echo "ok"
      fi
      ;;
    *)
      echo "ok"
      ;;
  esac
}

summarize_fail() {
  local name="$1"
  case "$name" in
    test)
      # Show failure details from nextest output
      echo "--- $name failures ---"
      grep -E "^(FAIL|TIMEOUT|  ×)" "$TMPDIR/$name.out" || true
      grep -A 20 "^--- STDOUT:" "$TMPDIR/$name.out" | head -60 || true
      ;;
    clippy)
      echo "--- $name diagnostics ---"
      # Show warning/error lines with context
      grep -E "^(warning|error)" "$TMPDIR/$name.out" || true
      ;;
    fmt)
      echo "--- $name diffs ---"
      cat "$TMPDIR/$name.out"
      ;;
    *)
      echo "--- $name output ---"
      tail -20 "$TMPDIR/$name.out"
      ;;
  esac
}

# Determine feature flags: include 'hip' only when hipcc is available.
if command -v hipcc &>/dev/null || [ -x /opt/rocm/bin/hipcc ]; then
  FEAT_FLAGS="--all-features"
else
  FEAT_FLAGS="--features simd,parallel,visualization,llr-f64"
fi

# Run all steps in order; continue through failures to report all of them.
run_step claude-md ./scripts/check-claude-md.sh
run_step check  cargo check --workspace $FEAT_FLAGS
run_step test   cargo nextest run --workspace $FEAT_FLAGS --release --profile ci
run_step clippy cargo clippy --workspace --all-targets $FEAT_FLAGS -- -D warnings
run_step fmt    cargo fmt --all -- --check

echo "$summary"

if [ "$failed" -ne 0 ]; then
  exit 1
fi
