#!/usr/bin/env bash
set -euo pipefail

# Cargo CI gate wrapper for jit.
#
# Runs the full Rust CI pipeline (check, test, clippy, fmt, doc, doctest) and
# produces concise output: one-line summaries on success, full diagnostics on
# failure.
#
# Exit codes:
#   0 — all steps passed
#   1 — one or more steps failed
#   2 — environment problem: no real cargo available on PATH

# Every step runs its command directly. The only host lock a step takes is the
# GPU lock that nextest holds for each `gpu-serial` test
# (`scripts/nextest-gpu-lock.sh`). Compile
# parallelism is cargo's own: its default job count, or the jobserver it
# inherits. `RAYON_NUM_THREADS` from `.cargo/config.toml` bounds each test
# process's pool and pairs with `threads-required` in `.config/nextest.toml`.
# Benchmark runs serialize among themselves under
# `dev/scripts/ccx1-bench-flock.sh`, which this script never takes.
SCALAR_LANE="$(dirname "$0")/scalar-lane-tests.sh"

# BUSY_HOST_OVERRIDE=1 selects the `ci-busy` nextest profile, whose per-test
# kill absorbs host contention beyond what the `ci` tier budget is sized for.
NEXTEST_CI_PROFILE=ci
[ -n "${BUSY_HOST_OVERRIDE:-}" ] && NEXTEST_CI_PROFILE=ci-busy

# CARGO_CI_STEPS is a PCRE over step names; only the matching steps run, so
# `CARGO_CI_STEPS='^doc$' ./scripts/cargo-ci.sh` re-runs the doc step. A
# filtered run is a focused re-run: its summary states the filter and the
# skipped steps, and it is not the CI contract's verdict. The summary lists
# every step with its status, so `CARGO_CI_STEPS='^$'`, which matches no step,
# prints the step names without running any.

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

# One private root per run. `LOGDIR` holds the per-step output this script
# summarizes; `TMPDIR` is exported so every test process puts its scratch trees
# under `SCRATCH` instead of the shared system temp dir, and the trap removes
# both however the run ends.
SYSTEM_TMPDIR=${TMPDIR:-/tmp}
RUN_ROOT=$(mktemp -d)
trap 'rm -rf "$RUN_ROOT"' EXIT
LOGDIR="$RUN_ROOT/logs"
SCRATCH="$RUN_ROOT/scratch"
mkdir -p "$LOGDIR" "$SCRATCH"
export TMPDIR="$SCRATCH"
if [ -z "${CARGO_CI_NO_SCCACHE:-}" ] && [ -z "${RUSTC_WRAPPER:-}" ] &&
   command -v sccache >/dev/null 2>&1; then
  export CARGO_CI_SYSTEM_TMPDIR="$SYSTEM_TMPDIR"
  RUSTC_WRAPPER="$(cd "$(dirname "$0")" && pwd)/sccache-system-tmp.sh"
  export RUSTC_WRAPPER
fi

# Directory-name prefixes the suites give their scratch trees. A run that ends
# with more of these than it started with leaked a fixture: `/tmp` is a tmpfs
# here and an exhausted inode table freezes every build and shell on the host.
#
# The private scratch root takes the wide pattern set; the shared system temp
# dir takes only the names a suite can still create there (the campaign stage
# the launcher resolves as exactly `/tmp/<campaign-id>`, plus the historical
# fixture names), so an unrelated process on this host cannot trip the check.
scratch_fixtures() {
  [ -d "$SCRATCH" ] || return 0
  find "$SCRATCH" -mindepth 1 -maxdepth 1 -type d \
    \( -name 'gf2-*' -o -name 'gf2sim-*' -o -name 'tuning-campaign-support-*' \
       -o -name 'campaign-*' -o -name 'session-publication-*' \
       -o -name 'active-log-repair-*' \) 2>/dev/null
}

system_fixtures() {
  [ -d "$SYSTEM_TMPDIR" ] || return 0
  find "$SYSTEM_TMPDIR" -mindepth 1 -maxdepth 1 -type d \
    \( -name 'gf2-a83583e0-*' -o -name 'gf2-dbd8787d-*' -o -name 'gf2-f547c394-*' \
       -o -name 'gf2sim-*' \
       -o -name 'gf2-coordinator-fixture-*' -o -name 'gf2-driver-*' \
       -o -name 'tuning-campaign-support-*' -o -name 'gf2-launcher-*' \) 2>/dev/null
}

fixture_inventory() {
  {
    system_fixtures
    scratch_fixtures
  } | sort
}

fixtures_before=$(fixture_inventory)

failed=0
skipped=0
summary=""

run_step() {
  local name="$1"
  shift

  if [ -n "${CARGO_CI_STEPS:-}" ] && ! grep -qP -- "$CARGO_CI_STEPS" <<<"$name"; then
    skipped=$((skipped + 1))
    summary+="  - $name: skipped by CARGO_CI_STEPS"$'\n'
    return 0
  fi

  local started=$SECONDS

  if "$@" >"$LOGDIR/$name.out" 2>&1; then
    local detail
    detail=$(summarize_pass "$name")
    summary+="  ✓ $name: $detail ($((SECONDS - started))s)"$'\n'
  else
    local rc=$?
    summary+="  ✗ $name: FAILED (exit $rc, $((SECONDS - started))s)"$'\n'
    summarize_fail "$name"
    failed=1
  fi
}

# nextest colorizes its output even when it is redirected to a file, and
# NO_COLOR does not suppress it, so every pattern below must see plain text.
# Both anchors here were defeated by it: a passing gate reported "ok" instead of
# its test counts, and a failing gate recorded an exit code with no diagnostic
# at all. nextest also indents both its summary and its per-test result lines,
# so no pattern may anchor at column zero either.
plain() {
  sed -E 's/\x1b\[[0-9;]*[mK]//g' "$1"
}

summarize_pass() {
  local name="$1"
  case "$name" in
    test|scalar-lane-nextest)
      # Extract nextest summary line: "Summary [Xs] N tests run: P passed, F failed, S skipped"
      local summary_line
      summary_line=$(plain "$LOGDIR/$name.out" | grep -E "^[[:space:]]*Summary" || true)
      if [ -n "$summary_line" ]; then
        local p f s
        p=$(echo "$summary_line" | grep -oP '\d+(?= passed)' || true)
        f=$(echo "$summary_line" | grep -oP '\d+(?= failed)' || true)
        s=$(echo "$summary_line" | grep -oP '\d+(?= skipped)' || true)
        echo -n "${p:-0} passed, ${f:-0} failed, ${s:-0} skipped"
      else
        echo -n "ok"
      fi
      # The resolved feature lists the scalar-lane script prints.
      plain "$LOGDIR/$name.out" | sed -n 's/^scalar lane: /; /p' | tr -d '\n'
      echo
      ;;
    *)
      echo "ok"
      ;;
  esac
}

summarize_fail() {
  local name="$1"
  case "$name" in
    test|scalar-lane-nextest)
      # Show failure details from nextest output.
      #
      # These patterns must not anchor at column zero: nextest indents its
      # per-test result lines ("        FAIL [   0.005s] ..."), so an `^FAIL`
      # anchor matches nothing and a failing gate records an exit code with no
      # diagnostic at all. Observed on a run whose seven TIMEOUT lines were
      # invisible in the gate record.
      echo "--- $name failures ---"
      plain "$LOGDIR/$name.out" | grep -E "^[[:space:]]*(FAIL|TIMEOUT|SIGSEGV|SIGABRT|LEAK|×)" || true
      plain "$LOGDIR/$name.out" | grep -A 20 -E "^[[:space:]]*--- (STDOUT|STDERR):" | head -60 || true
      ;;
    clippy|doc|doc-coding)
      echo "--- $name diagnostics ---"
      # Show warning/error lines with context
      plain "$LOGDIR/$name.out" | grep -E "^(warning|error)" || true
      ;;
    fmt)
      echo "--- $name diffs ---"
      plain "$LOGDIR/$name.out"
      ;;
    *)
      echo "--- $name output ---"
      plain "$LOGDIR/$name.out" | tail -20
      ;;
  esac
}

# The host's workspace feature selection: `hip` only where hipcc exists.
CARGO_FEATURES="$(dirname "$0")/cargo-features.sh"
FEAT_FLAGS=$("$CARGO_FEATURES")

# Run all steps in order; continue through failures to report all of them.
run_step check  cargo check --workspace $FEAT_FLAGS

# Typed selector sections and active accessors are always built; JSON codecs
# are separately opt-in. Keep the no-default and codec-only surfaces explicit
# so optional dependency unification cannot make this layout pass accidentally.
run_step tuning-core-no-default cargo check -p gf2-core --no-default-features
run_step tuning-core-codec-only cargo check -p gf2-core --no-default-features --features tuning-profile
run_step tuning-algebra-no-default cargo check -p gf2-algebra --no-default-features
run_step tuning-algebra-codec-only cargo check -p gf2-algebra --no-default-features --features tuning-profile
run_step tuning-coding-no-default cargo check -p gf2-coding --no-default-features
run_step tuning-coding-codec-only cargo check -p gf2-coding --no-default-features --features tuning-profile

# One package configuration without --all-features: default features, no
# `simd`, the build a bare `cargo nextest run -p <package>` selects. The check
# compiles every target, so a test or bench that needs a feature it does not
# name fails here. The execution step runs the target of the kernel-dispatch
# fallback contract (`gf2_core::dispatch_contract`) in the build that has no
# kernel route to select.
run_step default-features-core-check cargo check -p gf2-core --all-targets --profile ci-test
run_step default-features-core-build cargo nextest run -p gf2-core --cargo-profile ci-test --profile "$NEXTEST_CI_PROFILE" --test dispatch_fallback --no-run
run_step default-features-core-nextest cargo nextest run -p gf2-core --cargo-profile ci-test --profile "$NEXTEST_CI_PROFILE" --test dispatch_fallback

# The scalar lane of the two packages whose default features select `simd`:
# their suites in a build that resolves no `simd` feature, which the script
# verifies and whose resolved feature lists the summary carries.
run_step scalar-lane-build "$SCALAR_LANE" --cargo-profile ci-test --profile "$NEXTEST_CI_PROFILE" --no-run
run_step scalar-lane-nextest "$SCALAR_LANE" --cargo-profile ci-test --profile "$NEXTEST_CI_PROFILE"

# The build is its own step, so the summary times the workspace compile apart
# from test execution.
#
# `ci-test` rather than `release`: see the profile's rationale in Cargo.toml.
# Benchmarks and receipts stay on `release`.
run_step test-build cargo nextest run --workspace $FEAT_FLAGS --cargo-profile ci-test --profile "$NEXTEST_CI_PROFILE" --no-run
run_step test   cargo nextest run --workspace $FEAT_FLAGS --cargo-profile ci-test --profile "$NEXTEST_CI_PROFILE"

# The ordinary non-HIP feature set intentionally omits profile I/O, so keep the
# format-2 authority, process lifecycle, and calibration producer unit surface
# explicitly reachable in the fast tier. These are ordinary fast-tier tests: no
# ignored test or benchmark/calibration action is selected.
run_step tuning-profile-build cargo nextest run -p gf2-core --cargo-profile ci-test --profile "$NEXTEST_CI_PROFILE" --features parallel,simd,test-support,tuning-profile --test tuning_envelope_v2 --test tuning_process_lifecycle --test tuning_conservative_cfg --test tuning_extent_runtime --test tuning_extent_candidates --test tuning_observation_quiet --test field_vec_dot_chunk --test tuning_profile_prime_k_chunk_install --test tuning_profile_sparse_block_rows_install --no-run
run_step tuning-profile-nextest cargo nextest run -p gf2-core --cargo-profile ci-test --profile "$NEXTEST_CI_PROFILE" --features parallel,simd,test-support,tuning-profile --test tuning_envelope_v2 --test tuning_process_lifecycle --test tuning_conservative_cfg --test tuning_extent_runtime --test tuning_extent_candidates --test tuning_observation_quiet --test field_vec_dot_chunk --test tuning_profile_prime_k_chunk_install --test tuning_profile_sparse_block_rows_install
run_step tuning-core-calibration-build cargo nextest run -p tuning-calibration --cargo-profile ci-test --profile "$NEXTEST_CI_PROFILE" --features parallel,simd,test-support,tuning-profile --test core_calibration_harness --no-run
run_step tuning-core-calibration-nextest cargo nextest run -p tuning-calibration --cargo-profile ci-test --profile "$NEXTEST_CI_PROFILE" --features parallel,simd,test-support,tuning-profile --test core_calibration_harness
run_step tuning-algebra-calibration-build cargo nextest run -p tuning-calibration --cargo-profile ci-test --profile "$NEXTEST_CI_PROFILE" --features parallel,simd,test-support,tuning-profile --test algebra_calibration_harness --no-run
run_step tuning-algebra-calibration-nextest cargo nextest run -p tuning-calibration --cargo-profile ci-test --profile "$NEXTEST_CI_PROFILE" --features parallel,simd,test-support,tuning-profile --test algebra_calibration_harness
run_step tuning-coding-calibration-build cargo nextest run -p tuning-calibration --cargo-profile ci-test --profile "$NEXTEST_CI_PROFILE" --features parallel,simd,test-support,tuning-profile --test coding_calibration_harness --no-run
run_step tuning-coding-calibration-nextest cargo nextest run -p tuning-calibration --cargo-profile ci-test --profile "$NEXTEST_CI_PROFILE" --features parallel,simd,test-support,tuning-profile --test coding_calibration_harness
run_step tuning-campaign-support-build cargo nextest run -p tuning-campaign-support --cargo-profile ci-test --profile "$NEXTEST_CI_PROFILE" --no-run
run_step tuning-campaign-support-nextest cargo nextest run -p tuning-campaign-support --cargo-profile ci-test --profile "$NEXTEST_CI_PROFILE"
run_step tuning-campaign-validator python3 dev/scripts/validate-tuning-extent-campaign.py --self-test
run_step aff3ct-ldpc-gaps-python python3 -m pytest -q experiments/aff3ct-ldpc-gaps
run_step repository-files-self-test python3 dev/scripts/repository_files_test.py
run_step commit-subject-self-test python3 -B scripts/check-commit-subject.py --self-test
run_step reference-check-self-test python3 -B scripts/check-references.py --self-test
run_step reference-check python3 -B scripts/check-references.py
run_step tracked-budget-self-test python3 -B scripts/check-tracked-budget.py --self-test
run_step tracked-budget python3 -B scripts/check-tracked-budget.py
run_step docs-mechanical-self-test python3 contrib/gates/tests/docs-mechanical-test.py
run_step tuning-campaign-launcher-syntax bash -n dev/scripts/tuning-extent-campaign.sh
run_step gpu-lock-wrapper bash scripts/nextest-gpu-lock.test.sh
run_step gpu-lock-filter python3 -B scripts/check-gpu-lock-filter.py .config/nextest.toml crates/gf2-kernels-hip/.config/nextest.toml
run_step tuning-lifecycle-cargo cargo test -p gf2-core --profile ci-test --no-default-features --test tuning_process_lifecycle

# Add profile I/O to the same host-appropriate feature selection used by the
# workspace lint. On ordinary hosts FEAT_FLAGS is the explicit non-HIP set, so
# this reaches both owner codecs without pulling in the excluded ROCm crate.
run_step clippy cargo clippy --workspace --all-targets $FEAT_FLAGS --features tuning-profile -- -D warnings
run_step fmt    cargo fmt --all -- --check

# Fixed rather than host-dependent: matches the doc and doctest steps of
# .github/workflows/ci.yml.
DOC_FEAT_FLAGS="--features simd,parallel,visualization,io,rand"
run_step doc     env RUSTDOCFLAGS="-D warnings" cargo doc --workspace --release --no-deps $DOC_FEAT_FLAGS
run_step doctest cargo test --doc --workspace --release $DOC_FEAT_FLAGS

# Documents every gf2-coding item the workspace feature selection above leaves
# out. The `hip` feature compiles only where hipcc exists, so hosts without it
# document the remaining features.
CODING_DOC_FEAT_FLAGS=$("$CARGO_FEATURES" simd,parallel,visualization,bench-csv,sim-observability,tuning-profile,test-support)
run_step doc-coding env RUSTDOCFLAGS="-D warnings" cargo doc -p gf2-coding --release --no-deps $CODING_DOC_FEAT_FLAGS

# The gf2_tuning_baked cfg is not a Cargo feature, so --all-features never
# builds it; this scoped step executes the
# baked routing witnesses and drift tests. Each baked family names its witness
# target here. The frozen selector non-regression harness is excluded
# deliberately: its self-tests bracket the default configuration's threshold
# and are expected to report a re-pinning need under the baked cfg.
run_step baked-core env RUSTFLAGS="--cfg gf2_tuning_baked" cargo test -p gf2-core --profile ci-test --features simd,test-support,tuning-profile --lib --test backend_selection_baked --test matrix_selection_baked --test gemm_tiles_baked --test prime_route_baked --test field_vec_baked --test tuning_conservative_cfg --test backend_selection --test backend_selection_profile --test backend_selection_tunable --test tuning_profile_prime_k_chunk_install --test tuning_profile_sparse_block_rows_install

# Format-2 artifacts are opt-in I/O surfaces rather than ordinary feature
# defaults. Validate each explicit owner and the mechanically composed complete
# repository envelope without filesystem discovery.
run_step tuning-core-artifact cargo test -p gf2-core --profile ci-test --features tuning-profile --test tuning_profile_committed --test tuning_profile_prime_k_chunk_install --test tuning_profile_sparse_block_rows_install
run_step tuning-algebra-artifacts cargo test -p gf2-algebra --profile ci-test --features parallel,tuning-profile --test tuning_section --test tuning_repository_envelopes --test tuning_profile_permanent_install --test tuning_profile_permanent_install_large_chunk
run_step tuning-coding-codec cargo test -p gf2-coding --profile ci-test --features parallel,tuning-profile --test bch_encode_dispatch_profile --test bch_encode_dispatch_allocation
run_step tuning-composer cargo test --release --manifest-path dev/tools/tuning-profile-compose/Cargo.toml

fixtures_after=$(fixture_inventory)
before_count=$(printf '%s\n' "$fixtures_before" | grep -c . || true)
after_count=$(printf '%s\n' "$fixtures_after" | grep -c . || true)
if [ "$after_count" -gt "$before_count" ]; then
  summary+="  ✗ fixture-leak: FAILED ($before_count before, $after_count after)"$'\n'
  echo "--- leaked test fixture directories ---"
  comm -13 <(printf '%s\n' "$fixtures_before") <(printf '%s\n' "$fixtures_after")
  failed=1
else
  summary+="  ✓ fixture-leak: none ($before_count before, $after_count after)"$'\n'
fi

echo "$summary"

if [ -n "${CARGO_CI_STEPS:-}" ]; then
  echo "FILTERED RUN: CARGO_CI_STEPS='$CARGO_CI_STEPS' skipped $skipped steps; this is not the CI contract's verdict."
fi

if [ "$failed" -ne 0 ]; then
  exit 1
fi
