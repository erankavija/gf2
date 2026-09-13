#!/usr/bin/env bash
# Protocol-v4 byte-field comparison campaigns (jit:6c6b09b1).
#
# Usage (from the repository root):
#   dev/bench_results/6c6b09b1/run-byte-field-v4.sh build
#   dev/bench_results/6c6b09b1/run-byte-field-v4.sh plan|run|finalize|window CAMPAIGN
#
# CAMPAIGN is one of region-axpy-pilot, matrix-product-pilot,
# pairwise-control-pilot and the three -confirmation campaigns beside them.
#
# A pilot measures every cell its family declares on the exploratory pair
# count below. A confirmation measures the six cells its frozen confirmation
# addendum declares, on the protocol's confirmatory pair count, with a seed of
# its own: the samples are fresh, and the addendum's resolution, margins and
# pilot pin were frozen before this script could reach the host.
#
# `build` stages the verified external prefix, compiles the C shim, the
# conformance and provenance binaries and the Rust arms, runs every
# correctness check (external conformance, FFI wrapper, gf2 adapters, the
# shared field-law suite over both gf2 element types) and records the build
# and arm provenance under dev/active/6c6b09b1/conformance-v4/, then builds
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
# `run` and `finalize` check what the campaign wrote rather than trusting an
# exit code: a journal or receipt holding no cell with paired executions fails
# the job instead of passing as a clean run. Exit 4 marks a campaign that
# completed and still measured nothing, which is a plan that does not fit this
# host rather than a broken run; exit 1 marks a campaign that did not complete.
#
# Every step that measures or builds takes the CCX1 mutex only through
# `dev/scripts/ccx1-bench-flock.sh --full-host` (timed sessions) and
# `scripts/cargo-budget.sh` (builds and correctness checks).
#
# Every numeric setting comes from the addendum and the protocol's frozen
# shared settings; this script fixes only campaign identities, seeds, the
# pilot pair count and the session budget.
#
# `dev/active/6c6b09b1/survey/smoke-arms.sh` carries every arm these campaigns
# name through the real runner on a throwaway family before a campaign is
# queued, so a wire defect fails in a working session rather than in a window.
set -euo pipefail
# The benchmark window starts this script with the login environment only.
[[ -d "$HOME/.cargo/bin" ]] && export PATH="$HOME/.cargo/bin:$PATH"
repo=$(git rev-parse --show-toplevel)
[[ "$PWD" == "$repo" ]] || { echo 'invoke from the repository root' >&2; exit 2; }
ISSUE=6c6b09b1
SURVEY=dev/active/$ISSUE/survey
EVIDENCE=dev/active/$ISSUE/conformance-v4
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
    echo "# Build record of the byte-field v4 campaigns (jit:6c6b09b1)"
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
    ADDENDUM=dev/active/$ISSUE/addendum-v4-region-axpy-pilot.json
    LABEL=pilot SEED=2026091101 MAX_CELLS=6 PILOT_PAIRS=12 ;;
  matrix-product-pilot)
    ADDENDUM=dev/active/$ISSUE/addendum-v4-matrix-product-pilot.json
    LABEL=pilot SEED=2026091102 MAX_CELLS=5 PILOT_PAIRS=12 ;;
  pairwise-control-pilot)
    ADDENDUM=dev/active/$ISSUE/addendum-v4-pairwise-control-pilot.json
    LABEL=pilot SEED=2026091103 MAX_CELLS=7 PILOT_PAIRS=12 ;;
  region-axpy-confirmation)
    ADDENDUM=dev/active/$ISSUE/addendum-v4-region-axpy-confirmation.json
    LABEL=confirmation SEED=2026091201 MAX_CELLS=3 PILOT_PAIRS= ;;
  matrix-product-confirmation)
    ADDENDUM=dev/active/$ISSUE/addendum-v4-matrix-product-confirmation.json
    LABEL=confirmation SEED=2026091202 MAX_CELLS=3 PILOT_PAIRS= ;;
  pairwise-control-confirmation)
    ADDENDUM=dev/active/$ISSUE/addendum-v4-pairwise-control-confirmation.json
    LABEL=confirmation SEED=2026091203 MAX_CELLS=3 PILOT_PAIRS= ;;
  *) echo "unknown campaign $CAMPAIGN" >&2; exit 2 ;;
esac
ID=$ISSUE-v4-r1-$CAMPAIGN
STAGE=$STAGES/$ID
PLAN=$STAGE.plan.json
LAUNCH_LOG=$STAGE.launcher.log
OUT=dev/bench_results/$ISSUE/v4-r1-$CAMPAIGN

# The executables a plan names must be the ones the committed build record
# lists, and the libraries the external arm loads must be the verified
# prefix, so a rebuild or restage after the plan was projected cannot enter
# a campaign.
verify_build() {
  (cd "$repo" && sha256sum --quiet -c <(grep -v '^#' "$EVIDENCE/build-record.txt"))
  (cd "$EXT/prefix" && sha256sum --quiet -c "$repo/$SURVEY/ext-prefix.sha256")
}

# An exit code says a process ended, not that it produced data: a campaign
# whose every cell failed can still leave a session that exits cleanly. These
# two checks read what the campaign actually wrote and stop the job when it
# holds no usable cell, so a window slot spent on nothing fails loudly.
#
# They distinguish three outcomes, because a window operator has to act
# differently on each:
#
#   exit 0  at least one cell carries paired executions. Cells the host made
#           inapplicable are named and are not a failure: an unresolvable core
#           arm is a protocol outcome the receipt is meant to record.
#   exit 4  the campaign reached its own terminal `complete` record and still
#           holds no paired cell. Nothing is wrong with the run; every cell it
#           declared was inapplicable on this host, so the plan and the host
#           disagree and the plan is what needs changing.
#   exit 1  the campaign did not complete. The journal names the cause.
#
# `measured_cells` in the terminal record counts the cells the final session
# attempted, not the cells that hold data: the runner increments it for an
# inapplicable cell too, and a resumed session that finds every cell already
# checkpointed completes with zero. Neither check reads it as a data count.
# The `cell-complete` records do carry the outcome, and the journal keeps
# every session's, so they are counted over the whole log.

require_measured_journal() {
  python3 - "$STAGE/execution.log" <<'PY'
import json, sys
events = [json.loads(line) for line in open(sys.argv[1])]
last = events[-1]
if last["event"] != "complete":
    print(f"campaign ended in {last['event']!r}, not 'complete': {last['details']}",
          file=sys.stderr)
    sys.exit(1)
completed = [e["details"] for e in events if e["event"] == "cell-complete"]
paired = [d for d in completed if d.get("pairs")]
unpaired = len(completed) - len(paired)
if not paired:
    print(f"campaign completed holding no paired cell: {len(completed)} cells "
          f"completed, all without pairs; the session attempted "
          f"{last['details'].get('measured_cells')}", file=sys.stderr)
    sys.exit(4)
print(f"journal: {len(paired)} cells with pairs"
      + (f", {unpaired} without" if unpaired else ""))
PY
}

require_measured_receipt() {
  python3 - "$1/receipt.json" <<'PY'
import json, sys
cells = json.load(open(sys.argv[1]))["cells"]
paired = [c for c in cells if c.get("pairs")]
unpaired = [c["cell_id"] for c in cells if not c.get("pairs")]
if not paired:
    print(f"receipt carries {len(cells)} cells and no paired execution: "
          + ", ".join(unpaired), file=sys.stderr)
    sys.exit(4)
print(f"receipt: {len(paired)} paired cells of {len(cells)}"
      + (f"; no pairs: {', '.join(unpaired)}" if unpaired else ""))
PY
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
  require_measured_journal | tee -a "$LAUNCH_LOG"
}

finalize() {
  [[ ! -e "$OUT" ]] || { echo "receipt directory exists: $OUT" >&2; exit 2; }
  "$RUNNER" finalize "$STAGE" "$OUT" | tee -a "$LAUNCH_LOG"
  cp "$LAUNCH_LOG" "$OUT/launcher.log"
  "$ACCEPTANCE" "$OUT"
  require_measured_receipt "$OUT"
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
    # A confirmatory cell takes the protocol's frozen pair count, so only an
    # exploratory campaign names one.
    python3 "$SURVEY/make-plan-versioned.py" --addendum "$ADDENDUM" --label "$LABEL" \
      --campaign-id "$ID" --campaign-seed "$SEED" --lock "$(realpath "$LOCK")" \
      --target "$TARGET" --max-cells-per-session "$MAX_CELLS" \
      --producing-manifest "$SURVEY/producing-inputs-v4.json" \
      ${PILOT_PAIRS:+--pilot-pairs "$PILOT_PAIRS"} --output "$PLAN"
    ;;
  run) run_sessions "$@" ;;
  finalize) finalize ;;
  window)
    if [[ -f "$OUT/acceptance-summary.json" ]]; then
      echo "campaign $ID is finalized and evaluated: $OUT"
      require_measured_receipt "$OUT"
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
