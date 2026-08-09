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

# Host-wide build serialization. Only one cargo-ci run executes the heavy
# build/test steps at a time. Concurrent gate runs (e.g. several agent sessions
# each calling `jit gate evaluate ... cargo-ci`) otherwise oversubscribe the CPU —
# every cargo build fans out to all cores, so K runs demand K×nproc — and
# multiply peak RAM into swap, making the host and any interactive shell laggy.
# That hurts especially here: the test step is a `--release` nextest run with
# GPU/SIMD features, one of the heaviest builds on the machine. We re-exec under
# a blocking flock so concurrent runs queue rather than fail; the lock is held
# for the whole run and released on exit. CARGO_CI_LOCKED guards against
# infinite re-exec; CARGO_CI_NO_LOCK=1 disables (e.g. an isolated CI container
# that already owns the machine); CARGO_CI_BUILD_LOCK overrides the lock path —
# set it to the same value in this and the jit repo to serialize host-wide.
if [ -z "${CARGO_CI_NO_LOCK:-}" ] && [ -z "${CARGO_CI_LOCKED:-}" ]; then
  BUILD_LOCK="${CARGO_CI_BUILD_LOCK:-${XDG_RUNTIME_DIR:-/tmp}/gf2-cargo-ci.lock}"
  if command -v flock >/dev/null 2>&1; then
    exec env CARGO_CI_LOCKED=1 flock "$BUILD_LOCK" "$0" "$@"
  fi
  echo "cargo-ci: flock not found; running without host-wide build lock" >&2
fi

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

# Compilation cache. sccache caches rustc invocations content-addressed, so a
# build reuses codegen produced by any other checkout on this host instead of
# starting from zero. That is the dominant cost here: every dispatched agent
# worktree begins with an empty target directory, and a cold run of this script
# was measured at 13+ minutes against ~35-90 s warm. Unlike a shared target
# directory it does not thrash when branches diverge, because each variant is
# cached under its own hash.
#
# Guarded exactly like the nice/ionice prefixes below: a host without sccache
# runs unchanged. An RUSTC_WRAPPER the caller already set is left alone.
# CARGO_CI_NO_SCCACHE=1 disables. Cache location and size come from sccache's
# own config (see ~/.config/sccache/config), not from this script, so the
# repository carries no host-specific path.
if [ -z "${CARGO_CI_NO_SCCACHE:-}" ] && [ -z "${RUSTC_WRAPPER:-}" ] &&
   command -v sccache >/dev/null 2>&1; then
  export RUSTC_WRAPPER=sccache
fi

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
      # Show failure details from nextest output.
      #
      # These patterns must not anchor at column zero: nextest indents its
      # per-test result lines ("        FAIL [   0.005s] ..."), so an `^FAIL`
      # anchor matches nothing and a failing gate records an exit code with no
      # diagnostic at all. Observed on a run whose seven TIMEOUT lines were
      # invisible in the gate record.
      echo "--- $name failures ---"
      grep -E "^[[:space:]]*(FAIL|TIMEOUT|SIGSEGV|SIGABRT|LEAK|×)" "$TMPDIR/$name.out" || true
      grep -A 20 -E "^[[:space:]]*--- (STDOUT|STDERR):" "$TMPDIR/$name.out" | head -60 || true
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

# Deprioritize the build/test work so an interactive shell preempts it under
# contention — this is what keeps the host responsive while the gate runs, not
# just the serialization above. nice -n 19 = lowest CPU priority; ionice -c2 -n7
# = best-effort lowest I/O priority (NOT the idle class -c3, which can be starved
# indefinitely). Both are best-effort: a missing binary degrades gracefully to
# running normally. CARGO_CI_NO_NICE=1 disables; CARGO_CI_NICE overrides it.
NICE_PREFIX=()
if [ -z "${CARGO_CI_NO_NICE:-}" ]; then
  command -v nice   >/dev/null 2>&1 && NICE_PREFIX+=(nice -n "${CARGO_CI_NICE:-19}")
  command -v ionice >/dev/null 2>&1 && NICE_PREFIX+=(ionice -c2 -n7)
fi

# Run all steps in order; continue through failures to report all of them.
run_step check  "${NICE_PREFIX[@]}" cargo check --workspace $FEAT_FLAGS
run_step test   "${NICE_PREFIX[@]}" cargo nextest run --workspace $FEAT_FLAGS --release --profile ci
run_step clippy "${NICE_PREFIX[@]}" cargo clippy --workspace --all-targets $FEAT_FLAGS -- -D warnings
run_step fmt    "${NICE_PREFIX[@]}" cargo fmt --all -- --check

echo "$summary"

if [ "$failed" -ne 0 ]; then
  exit 1
fi
