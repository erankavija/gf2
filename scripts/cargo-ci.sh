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

# CPU budget and serialization live in `scripts/cargo-budget.sh`, which wraps
# every step below. Build and lint steps share the machine by budget; `--test`
# marks the steps whose per-test wall-clock kills need the exclusive lock. The
# same wrapper is what direct agent invocations use, so one mechanism governs
# both — see the commands section of AGENTS.md.
#
# It is not the only mutex on this host. Benchmark evidence runs serialize
# separately under `dev/scripts/ccx1-bench-flock.sh`; the two are independent by
# design and neither subsumes the other. Do not double-wrap.
BUDGET="$(dirname "$0")/cargo-budget.sh"

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

  local started=$SECONDS

  if "$@" >"$TMPDIR/$name.out" 2>&1; then
    local detail
    detail=$(summarize_pass "$name")
    summary+="  ✓ $name: $detail ($((SECONDS - started))s)"$'\n'
  else
    local rc=$?
    if [ "$rc" -eq 75 ]; then
      # Exit 75 from the budget wrapper: this step lost the queue for a host
      # lock — the exclusive test lock, or the CCX1 mutex held by a --full-host
      # bench run. It is the only status here that says nothing about the tree
      # under evaluation, so it must not read as a test failure.
      summary+="  ⏳ $name: QUEUED OUT on a host lock ($((SECONDS - started))s)"$'\n'
    else
      summary+="  ✗ $name: FAILED (exit $rc, $((SECONDS - started))s)"$'\n'
      summarize_fail "$name"
    fi
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

# Run all steps in order; continue through failures to report all of them.
run_step check  "$BUDGET" cargo check --workspace $FEAT_FLAGS

# Typed selector sections and active accessors are always built; JSON codecs
# are separately opt-in. Keep the no-default and codec-only surfaces explicit
# so optional dependency unification cannot make this layout pass accidentally.
run_step tuning-core-no-default "$BUDGET" cargo check -p gf2-core --no-default-features
run_step tuning-core-codec-only "$BUDGET" cargo check -p gf2-core --no-default-features --features tuning-profile
run_step tuning-algebra-no-default "$BUDGET" cargo check -p gf2-algebra --no-default-features
run_step tuning-algebra-codec-only "$BUDGET" cargo check -p gf2-algebra --no-default-features --features tuning-profile

# Build outside the exclusive lock, then execute inside it. The release compile
# with GPU/SIMD features is the heaviest work here, and holding the whole host
# for it would serialize the part that has no wall-clock assertion.
run_step test-build "$BUDGET" cargo nextest run --workspace $FEAT_FLAGS --release --profile ci --no-run
run_step test   "$BUDGET" --test cargo nextest run --workspace $FEAT_FLAGS --release --profile ci

# The ordinary non-HIP feature set intentionally omits profile I/O, so keep the
# format-2 authority, process lifecycle, and calibration producer unit surface
# explicitly reachable in the fast tier. These are ordinary release tests: no
# ignored test or benchmark/calibration action is selected.
run_step tuning-profile-build "$BUDGET" cargo nextest run -p gf2-core --release --profile ci --features tuning-profile --test tuning_envelope_v2 --test tuning_process_lifecycle --test tuning_calibration_harness --no-run
run_step tuning-profile-nextest "$BUDGET" --test cargo nextest run -p gf2-core --release --profile ci --features tuning-profile --test tuning_envelope_v2 --test tuning_process_lifecycle --test tuning_calibration_harness
run_step tuning-lifecycle-cargo "$BUDGET" cargo test -p gf2-core --release --no-default-features --test tuning_process_lifecycle

# Add profile I/O to the same host-appropriate feature selection used by the
# workspace lint. On ordinary hosts FEAT_FLAGS is the explicit non-HIP set, so
# this reaches both owner codecs without pulling in the excluded ROCm crate.
run_step clippy "$BUDGET" cargo clippy --workspace --all-targets $FEAT_FLAGS --features tuning-profile -- -D warnings
run_step fmt    "$BUDGET" cargo fmt --all -- --check
# Baked selector fields (DEC-G, and the follow-on families of
# dev/active/7d824b2f/design.md §2.2): the gf2_tuning_baked cfg is not a Cargo
# feature, so --all-features never builds it; this scoped step executes the
# baked routing witnesses and drift tests. Each baked family names its witness
# target here. The frozen selector non-regression harness is excluded
# deliberately: its self-tests bracket the default configuration's threshold
# and are expected to report a re-pinning need under the baked cfg.
run_step baked-core env RUSTFLAGS="--cfg gf2_tuning_baked" "$BUDGET" cargo test -p gf2-core --features simd,tuning-profile --lib --test backend_selection_baked --test matrix_selection_baked --test gemm_tiles_baked --test prime_route_baked --test field_vec_baked --test backend_selection --test backend_selection_profile --test backend_selection_tunable

# Format-2 artifacts are opt-in I/O surfaces rather than ordinary feature
# defaults. Validate each explicit owner and the mechanically composed complete
# repository envelope without filesystem discovery.
run_step tuning-core-artifact "$BUDGET" cargo test -p gf2-core --release --features tuning-profile --test tuning_profile_committed
run_step tuning-algebra-artifacts "$BUDGET" cargo test -p gf2-algebra --release --features parallel,tuning-profile --test tuning_section --test tuning_repository_envelopes --test tuning_profile_permanent_install --test tuning_profile_permanent_install_large_chunk
run_step tuning-composer "$BUDGET" cargo test --release --manifest-path dev/tools/tuning-profile-compose/Cargo.toml

echo "$summary"

if [ "$failed" -ne 0 ]; then
  exit 1
fi
