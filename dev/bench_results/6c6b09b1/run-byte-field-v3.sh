#!/usr/bin/env bash
# Protocol-v3 byte-field comparison campaigns (jit:6c6b09b1).
#
# Usage (from the repository root):
#   dev/bench_results/6c6b09b1/run-byte-field-v3.sh build
#   dev/bench_results/6c6b09b1/run-byte-field-v3.sh plan|run|finalize|window CAMPAIGN
#
# CAMPAIGN is one of region-axpy-pilot, matrix-product-pilot or
# pairwise-control-pilot.
#
# `build` stages the verified external prefix, compiles the C shim, the
# conformance and provenance binaries and the Rust arms, runs every
# correctness check (external conformance, FFI wrapper, gf2 adapters, the
# shared field-law suite over both gf2 element types) and records the build
# and arm provenance under dev/active/6c6b09b1/conformance-v3/, then builds
# the protocol runner and acceptance tool. Every step runs under the CPU
# budget and finishes before any timed work; any failed check stops it.
# `plan` projects the saved runner plan from the frozen family addendum once.
# `run` measures the plan as bounded checkpointed sessions under the
# canonical CCX1 mutex and resumes an interrupted campaign under the same
# identity without repeating completed cells. `finalize` assembles the
# receipt directory and evaluates it. `window` is `run` then `finalize`, the
# command the benchmark window executes: it resumes an unfinished campaign,
# only finalizes a complete one, and does nothing once the receipt carries
# its acceptance summary. `build` and `plan` run in a working session before
# a campaign is queued; `window` builds nothing.
#
# Every step that measures or builds takes the CCX1 mutex only through
# `dev/scripts/ccx1-bench-flock.sh --full-host` (timed sessions) and
# `scripts/cargo-budget.sh` (builds and correctness checks).
#
# Every numeric setting comes from the addendum and the protocol's frozen
# shared settings; this script fixes only campaign identities, seeds, the
# pilot pair count and the session budget.
set -euo pipefail
# The benchmark window starts this script with the login environment only.
[[ -d "$HOME/.cargo/bin" ]] && export PATH="$HOME/.cargo/bin:$PATH"
repo=$(git rev-parse --show-toplevel)
[[ "$PWD" == "$repo" ]] || { echo 'invoke from the repository root' >&2; exit 2; }
ISSUE=6c6b09b1
SURVEY=dev/active/$ISSUE/survey
EVIDENCE=dev/active/$ISSUE/conformance-v3
EXT=$repo/target/$ISSUE-ext
TARGET=$repo/target/$ISSUE-survey
CBUILD=$TARGET/c
STAGES=$repo/target/$ISSUE-campaigns
RUNNER=$repo/target/release/benchmark-ab-runner
ACCEPTANCE=$repo/target/release/benchmark-acceptance
LOCK=${GF2_CCX1_LOCK:-/tmp/gf2-ccx1.lock}
# The survey builds and measures with the repository's minimum supported
# toolchain; the arms, the runner and the acceptance tool all use it.
export RUSTUP_TOOLCHAIN=1.95
ARM_RUSTFLAGS="-C target-cpu=native"

ACTION=${1:?build, plan, run, finalize or window}
if [[ "$ACTION" == build ]]; then
  mkdir -p "$EVIDENCE"
  "$SURVEY/stage-externals.sh" >"$EVIDENCE/stage-externals.txt"
  ./scripts/cargo-budget.sh make -C "$SURVEY" -j4 EXT="$EXT" BUILD="$CBUILD"
  for manifest in gf2-side ext-side field-laws; do
    CARGO_TARGET_DIR="$TARGET" RUSTFLAGS="$ARM_RUSTFLAGS" \
      GF2_SURVEY_EXT="$EXT" GF2_SURVEY_BUILD="$CBUILD" \
      ./scripts/cargo-budget.sh cargo build --release --locked \
        --manifest-path "$SURVEY/$manifest/Cargo.toml"
  done
  ./scripts/cargo-budget.sh cargo build --release --locked -p tuning-campaign-support \
    --bin benchmark-ab-runner --bin benchmark-acceptance
  # Correctness precedes timing: each check exits non-zero on a mismatch.
  ./scripts/cargo-budget.sh "$CBUILD/byte_field_conformance" >"$EVIDENCE/externals.txt"
  ./scripts/cargo-budget.sh "$TARGET/release/ext-arm" --validate >"$EVIDENCE/ext-wrapper.txt"
  ./scripts/cargo-budget.sh "$TARGET/release/gf2-conformance" >"$EVIDENCE/gf2-side.txt"
  ./scripts/cargo-budget.sh "$TARGET/release/survey-field-laws" 2>/dev/null \
    >"$EVIDENCE/field-laws-wide.txt"
  ./scripts/cargo-budget.sh --test cargo nextest run -p gf2-core --all-features \
    --cargo-profile ci-test --profile ci \
    -E 'test(field::axiom_tests::test_gf2_8_field_axioms)' \
    >"$EVIDENCE/field-laws-element.txt" 2>&1
  GF2_SURVEY_EXT="$EXT" GF2_SURVEY_BUILD="$CBUILD" GF2_SURVEY_TARGET="$TARGET" \
    RUSTFLAGS="$ARM_RUSTFLAGS" \
    ./scripts/cargo-budget.sh "$SURVEY/arm-provenance.sh" >"$EVIDENCE/arm-provenance.txt"
  {
    echo "# Build record of the byte-field v3 campaigns (jit:6c6b09b1)"
    echo "# toolchain: $(rustc --version)"
    echo "# arm RUSTFLAGS: $ARM_RUSTFLAGS"
    echo "# c compiler: $(cc --version | head -1)"
    echo "# executables the plans name (sha256, path relative to the repository root):"
    for binary in "$TARGET/release/gf2-arm" "$TARGET/release/ext-arm" \
                  "$CBUILD/byte_field_conformance" "$CBUILD/backend_provenance" \
                  "$RUNNER" "$ACCEPTANCE"; do
      echo "$(sha256sum "$binary" | cut -d' ' -f1)  ${binary#"$repo"/}"
    done
  } >"$EVIDENCE/build-record.txt"
  exit 0
fi

CAMPAIGN=${2:?campaign}
case "$CAMPAIGN" in
  region-axpy-pilot)
    ADDENDUM=dev/active/$ISSUE/addendum-v3-region-axpy-pilot.json
    LABEL=pilot SEED=2026091101 MAX_CELLS=6 PILOT_PAIRS=12 ;;
  matrix-product-pilot)
    ADDENDUM=dev/active/$ISSUE/addendum-v3-matrix-product-pilot.json
    LABEL=pilot SEED=2026091102 MAX_CELLS=5 PILOT_PAIRS=12 ;;
  pairwise-control-pilot)
    ADDENDUM=dev/active/$ISSUE/addendum-v3-pairwise-control-pilot.json
    LABEL=pilot SEED=2026091103 MAX_CELLS=7 PILOT_PAIRS=12 ;;
  *) echo "unknown campaign $CAMPAIGN" >&2; exit 2 ;;
esac
ID=$ISSUE-v3-r1-$CAMPAIGN
STAGE=$STAGES/$ID
PLAN=$STAGE.plan.json
LAUNCH_LOG=$STAGE.launcher.log
OUT=dev/bench_results/$ISSUE/v3-r1-$CAMPAIGN

# The executables a plan names must be the ones the committed build record
# lists, and the libraries the external arm loads must be the verified
# prefix, so a rebuild or restage after the plan was projected cannot enter
# a campaign.
verify_build() {
  (cd "$repo" && sha256sum --quiet -c <(grep -v '^#' "$EVIDENCE/build-record.txt"))
  (cd "$EXT/prefix" && sha256sum --quiet -c "$repo/$SURVEY/ext-prefix.sha256")
}

run_sessions() {
  [[ -f "$PLAN" ]] || { echo "plan the campaign first: $PLAN" >&2; exit 2; }
  verify_build
  {
    echo "# command: $0 $*"
    echo "# started_utc: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
    echo "# gf2 revision (informational): $(git rev-parse HEAD)"
    echo "# addendum: $ADDENDUM sha256 $(sha256sum "$ADDENDUM" | cut -d' ' -f1)"
    echo "# plan: $PLAN sha256 $(sha256sum "$PLAN" | cut -d' ' -f1)"
    echo "# build record: $EVIDENCE/build-record.txt sha256 $(sha256sum "$EVIDENCE/build-record.txt" | cut -d' ' -f1)"
    echo "# toolchain: $(rustc --version)"
    echo "# load_avg_start: $(cut -d' ' -f1-3 /proc/loadavg)"
  } >>"$LAUNCH_LOG"
  # Exit 3 pauses at the session cell budget and releases the mutex so a
  # queued sibling gets the host; exit 0 completes the campaign.
  local session=0 code
  while :; do
    session=$((session + 1))
    echo "# session $session command: GF2_BENCH=1 CARGO_CI_NO_LOCK=1 dev/scripts/ccx1-bench-flock.sh --full-host $RUNNER run $STAGE $PLAN" >>"$LAUNCH_LOG"
    set +e
    GF2_BENCH=1 CARGO_CI_NO_LOCK=1 dev/scripts/ccx1-bench-flock.sh --full-host \
      "$RUNNER" run "$STAGE" "$PLAN" 2>&1 | tee -a "$LAUNCH_LOG"
    code=${PIPESTATUS[0]}
    set -e
    echo "# session $session exit: $code at $(date -u +%Y-%m-%dT%H:%M:%SZ) load $(cut -d' ' -f1-3 /proc/loadavg)" >>"$LAUNCH_LOG"
    case "$code" in
      0) break ;;
      3) ;;
      *) exit "$code" ;;
    esac
  done
}

finalize() {
  [[ ! -e "$OUT" ]] || { echo "receipt directory exists: $OUT" >&2; exit 2; }
  "$RUNNER" finalize "$STAGE" "$OUT" | tee -a "$LAUNCH_LOG"
  cp "$LAUNCH_LOG" "$OUT/launcher.log"
  "$ACCEPTANCE" "$OUT"
}

case "$ACTION" in
  plan)
    [[ ! -e "$PLAN" ]] || { echo "existing plan: $PLAN" >&2; exit 2; }
    # Publication precedes measurement: the frozen addendum and the build
    # record are committed and unmodified, so the receipt pins committed bytes.
    for frozen in "$ADDENDUM" "$EVIDENCE/build-record.txt"; do
      git ls-files --error-unmatch "$frozen" >/dev/null
      git diff --exit-code HEAD -- "$frozen" >/dev/null
    done
    verify_build
    mkdir -p "$STAGES"
    touch "$LOCK"
    python3 "$SURVEY/make-plan-v3.py" --addendum "$ADDENDUM" --label "$LABEL" \
      --campaign-id "$ID" --campaign-seed "$SEED" --lock "$(realpath "$LOCK")" \
      --target "$TARGET" --max-cells-per-session "$MAX_CELLS" \
      --pilot-pairs "$PILOT_PAIRS" --output "$PLAN"
    ;;
  run) run_sessions "$@" ;;
  finalize) finalize ;;
  window)
    if [[ -f "$OUT/acceptance-summary.json" ]]; then
      echo "campaign $ID is finalized and evaluated: $OUT"
      exit 0
    fi
    # A campaign whose log already ends in its `complete` terminal record is
    # only finalized; anything else resumes under its own identity.
    if ! { [[ -f "$STAGE/execution.log" ]] && \
           tail -n1 "$STAGE/execution.log" | grep -q '"event":"complete"'; }; then
      run_sessions "$@"
    fi
    finalize
    ;;
  *) echo 'action must be build, plan, run, finalize or window' >&2; exit 2 ;;
esac
