#!/usr/bin/env bash
# Bounded protocol-v3 rerun of the preserved YMM raw-batch comparison.
# Usage: run-clmul-v3.sh prepare|session|finalize pilot|pilot-r2|confirmation
set -euo pipefail

ACTION=${1:?prepare|session|finalize}
MODE=${2:?pilot|pilot-r2|confirmation}
case "$MODE" in pilot|pilot-r2|confirmation) ;; *) exit 2 ;; esac

export RUSTUP_TOOLCHAIN=1.95.0
export CARGO_CI_NO_SCCACHE=1
repo=$(git rev-parse --show-toplevel)
cd "$repo"

ISSUE=1d0da41f
FAMILY=ymm-clmul-dispatch
OUT="dev/bench_results/$ISSUE/v3-$MODE"
STAGE="/tmp/gf2-$ISSUE-v3-$MODE"
PLAN="dev/active/$ISSUE/plan-ymm-clmul-v3-$MODE.json"
ADDENDUM="dev/active/$ISSUE/addendum-ymm-clmul-v3-$MODE.json"
LEDGER="dev/bench_results/$ISSUE/v3-family-ledger.jsonl"
LAUNCH_LOG="dev/bench_results/$ISSUE/v3-$MODE-launcher.log"
V1_RECEIPT="dev/bench_results/$ISSUE/2026-09-08-$ISSUE-clmul-dispatch-confirmation"
SOURCE_SNAPSHOT="$repo/$V1_RECEIPT/inputs/producing"
ADAPTER="$SOURCE_SNAPSHOT/dev/active/$ISSUE/survey/ymm-clmul-arm"
ARM_STAGE="/tmp/gf2-$ISSUE-v3-arms"
BASELINE_TREE="$ARM_STAGE/baseline-source"
BASELINE_PROJECT="$ARM_STAGE/arm-baseline"
CANDIDATE_TARGET="$ARM_STAGE/target-candidate"
BASELINE_TARGET="$ARM_STAGE/target-baseline"
TOOL_TARGET=${CARGO_TARGET_DIR:-target}
RUNNER="$TOOL_TARGET/release/benchmark-ab-runner"
ACCEPTANCE="$TOOL_TARGET/release/benchmark-acceptance"

verify_current_runner() {
  RUNNER=$(realpath "$RUNNER")
  ACCEPTANCE=$(realpath "$ACCEPTANCE")
  if ! grep -aFq 'zen3-benchmark-addendum-v3' "$RUNNER"; then
    echo "runner does not contain the protocol-v3 addendum identity: $RUNNER" >&2
    exit 2
  fi
  printf 'GF2_BENCHMARK_RUNNER=%s\n' "$RUNNER"
  printf 'GF2_BENCHMARK_RUNNER_SHA256=%s\n' "$(sha256sum "$RUNNER" | cut -d' ' -f1)"
}

build_arms() {
  test -f "$SOURCE_SNAPSHOT/producing-snapshot.json"
  test -f "$ADAPTER/Cargo.toml"
  mkdir -p "$ARM_STAGE"
  if [[ ! -f "$BASELINE_TREE/crates/gf2-core/Cargo.toml" ]]; then
    if [[ -e "$BASELINE_TREE" ]]; then
      echo "incomplete baseline staging exists: $BASELINE_TREE" >&2
      exit 2
    fi
    mkdir "$BASELINE_TREE"
    tar -xzf "$SOURCE_SNAPSHOT/dev/active/$ISSUE/survey/baseline-source.tar.gz" \
      -C "$BASELINE_TREE"
  fi
  if [[ -e "$BASELINE_PROJECT" ]]; then
    echo "baseline project staging exists: $BASELINE_PROJECT" >&2
    exit 2
  fi
  mkdir "$BASELINE_PROJECT"
  cp -r "$ADAPTER/src" "$BASELINE_PROJECT/src"
  cp "$ADAPTER/Cargo.lock" "$BASELINE_PROJECT/Cargo.lock"
  sed \
    -e "s#path = \"../../../../../crates/gf2-core\"#path = \"$BASELINE_TREE/crates/gf2-core\"#" \
    -e "s#path = \"../../../../../crates/gf2-kernels-simd\"#path = \"$BASELINE_TREE/crates/gf2-kernels-simd\"#" \
    -e "s#path = \"../../../../tools/tuning-campaign-support\"#path = \"$SOURCE_SNAPSHOT/dev/tools/tuning-campaign-support\"#" \
    "$ADAPTER/Cargo.toml" > "$BASELINE_PROJECT/Cargo.toml"

  ./scripts/cargo-budget.sh cargo build --locked --release \
    --manifest-path "$ADAPTER/Cargo.toml" --target-dir "$CANDIDATE_TARGET"
  ./scripts/cargo-budget.sh cargo build --locked --release \
    --manifest-path "$BASELINE_PROJECT/Cargo.toml" --features frozen-baseline \
    --target-dir "$BASELINE_TARGET"
}

case "$ACTION" in
prepare)
  if [[ -e "$STAGE" || -e "$OUT" || -e "$PLAN" || -e "$ADDENDUM" || -e "$LAUNCH_LOG" ]]; then
    echo "campaign exists; resume it instead of overwriting frozen inputs" >&2
    exit 2
  fi
  test -s "$LEDGER"
  ./scripts/cargo-budget.sh cargo build --release -p tuning-campaign-support \
    --bin benchmark-ab-runner --bin benchmark-acceptance
  if [[ ! -x "$BASELINE_TARGET/release/ymm-clmul-arm" || \
        ! -x "$CANDIDATE_TARGET/release/ymm-clmul-arm" ]]; then
    build_arms
  fi
  BASELINE=$(realpath "$BASELINE_TARGET/release/ymm-clmul-arm")
  CANDIDATE=$(realpath "$CANDIDATE_TARGET/release/ymm-clmul-arm")
  python3 - "$MODE" "$PLAN" "$ADDENDUM" "$LEDGER" "$BASELINE" "$CANDIDATE" <<'PY'
import datetime, hashlib, json, pathlib, sys

mode, planpath, addendumpath, ledger, baseline, candidate = sys.argv[1:]
issue = '1d0da41f'
root = pathlib.Path('.')
source = root / 'dev/active' / issue / (
    'addendum-ymm-clmul-dispatch-pilot.json' if mode != 'confirmation'
    else 'addendum-ymm-clmul-dispatch.json')
addendum = json.loads(source.read_bytes())
addendum['schema'] = 'zen3-benchmark-addendum-v3'
addendum['protocol']['version'] = 3
addendum['family']['id'] = 'ymm-clmul-dispatch'
addendum['family']['description'] = (
    'Protocol-v3 rerun of the preserved raw carry-less batch comparison. '
    'Both arms rebuild from the immutable source closure of the accepted v1 '
    'confirmation: the baseline requires AVX512VL and selects sequential '
    'PCLMULQDQ on Zen 3; the candidate selects the repaired YMM '
    'VPCLMULQDQ lane. The v1 evidence remains immutable and superseded.')
addendum['frozen']['frozen_utc'] = datetime.datetime.now(
    datetime.timezone.utc).strftime('%Y-%m-%dT%H:%M:%SZ')
addendum['family_wise']['prior_confirmatory_trials'] = 0
addendum['family_wise']['prior_trials'] = []
addendum['family_wise']['ledger_path'] = ledger

if mode != 'confirmation':
    addendum['effect']['measurement_resolution'] = None
    addendum['effect']['resolution_evidence'] = None
    if mode == 'pilot-r2':
        addendum['search_budget']['max_pilot_trials_per_cell'] = 2
        addendum['cells'] = [
            cell for cell in addendum['cells']
            if cell['cell_id'] == 'raw-batch-odd-tail-65-1core'
        ]
else:
    pilot = root / 'dev/bench_results' / issue / 'v3-pilot-r2' / 'receipt.json'
    summary_path = pilot.with_name('acceptance-summary.json')
    summary = json.loads(summary_path.read_bytes())
    if summary['verdict'] != 'accepted':
        raise SystemExit('v3 pilot is not accepted')
    widths = [
        max(cell['interval']['estimate'] - cell['interval']['lower'],
            cell['interval']['upper'] - cell['interval']['estimate'])
        / cell['interval']['estimate']
        for cell in summary['cells'] if cell.get('interval')
    ]
    resolution = max(widths) * 1.01
    if not 0 < resolution < 0.05:
        raise SystemExit(
            f'pilot resolution {resolution} cannot support the frozen 1.05 margin')
    addendum['effect']['measurement_resolution'] = resolution
    addendum['effect']['resolution_evidence'] = {
        'receipt': str(pilot),
        'sha256': hashlib.sha256(pilot.read_bytes()).hexdigest(),
    }
    addendum['effect']['rationale'] = (
        'The original one-tenth worthwhile threshold strictly exceeds one '
        'plus the protocol-v3 pilot resolution.')
    addendum['effect']['equivalence_rationale'] = (
        'The original five-percent consumer non-regression margin strictly '
        'exceeds one plus the protocol-v3 pilot resolution.')

def arm(executable, description):
    return {
        'build': 'conservative-portable', 'description': description,
        'executable': executable, 'arguments': [], 'environment': {},
        'rustflags': None, 'tuning_profile': None,
    }

pilot_pairs = 24 if mode == 'pilot-r2' else (6 if mode == 'pilot' else None)
cells = [
    ('raw-batch-small-8-1core', 'raw-batch', 8, 64, 101),
    ('raw-batch-odd-tail-65-1core', 'raw-batch', 65, 64, 102),
    ('raw-batch-l1-512-1core', 'raw-batch', 512, 64, 103),
    ('fieldvec-dot-1024-1core', 'fieldvec-dot', 1024, 8, 104),
]
if mode == 'pilot-r2':
    cells = [cells[1]]
if mode == 'confirmation':
    cells.append(('raw-batch-l1-512-streaming-6core', 'raw-batch', 512, 64, 105))
plan = {
    'schema': 'zen3-benchmark-plan-v1',
    'campaign_id': f'{issue}-v3-{mode}', 'issue': issue,
    'label': 'pilot' if mode != 'confirmation' else 'confirmation',
    'campaign_seed': 20260907, 'addendum': addendumpath,
    'producing_manifest': 'dev/active/1d0da41f/producing-inputs.json',
    'lock_path': '/tmp/gf2-ccx1.lock',
    'wrapper': 'dev/scripts/ccx1-bench-flock.sh', 'timing_override': None,
    'arms': {
        'baseline': arm(baseline,
            'immutable v1 baseline source: AVX512VL-gated dispatch'),
        'candidate': arm(candidate,
            'immutable v1 candidate source: AVX2+VPCLMULQDQ YMM dispatch'),
    },
    'cells': [
        {'cell_id': cell_id, 'baseline_arm': 'baseline',
         'candidate_arm': 'candidate',
         'case': {'degree': degree, 'elements': elements, 'seed': seed,
                  'workload': workload}, 'pilot_pairs': pilot_pairs}
        for cell_id, workload, elements, degree, seed in cells
    ],
    'max_cells_per_session': 2,
}
for path, value in [(pathlib.Path(addendumpath), addendum),
                    (pathlib.Path(planpath), plan)]:
    with path.open('x') as output:
        json.dump(value, output, indent=2)
        output.write('\n')
PY
  ;;
session)
  test -f "$PLAN"
  test -f "$ADDENDUM"
  test -x "$BASELINE_TARGET/release/ymm-clmul-arm"
  test -x "$CANDIDATE_TARGET/release/ymm-clmul-arm"
  verify_current_runner
  printf 'GF2_BENCHMARK_STAGE=%s\n' "$STAGE"
  printf 'GF2_BENCHMARK_LOG=%s/execution.log\n' "$STAGE"
  printf 'GF2_BENCHMARK_FINAL_RECEIPT=%s/receipt.json\n' "$OUT"
  {
    printf '# session start UTC: %s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
    printf '# runner: %s\n' "$RUNNER"
    printf '# runner sha256: %s\n' "$(sha256sum "$RUNNER" | cut -d' ' -f1)"
    printf '# command: GF2_BENCH=1 CARGO_CI_NO_LOCK=1 dev/scripts/ccx1-bench-flock.sh --full-host %s run %s %s\n' "$RUNNER" "$STAGE" "$PLAN"
  } >> "$LAUNCH_LOG"
  set +e
  GF2_BENCH=1 CARGO_CI_NO_LOCK=1 dev/scripts/ccx1-bench-flock.sh --full-host \
    "$RUNNER" run "$STAGE" "$PLAN" 2>&1 | tee -a "$LAUNCH_LOG"
  code=${PIPESTATUS[0]}
  set -e
  printf '# session exit: %s; end UTC: %s\n' "$code" \
    "$(date -u +%Y-%m-%dT%H:%M:%SZ)" >> "$LAUNCH_LOG"
  [[ "$code" == 0 || "$code" == 3 ]]
  ;;
finalize)
  test ! -e "$OUT"
  verify_current_runner
  "$RUNNER" finalize "$STAGE" "$OUT"
  cp "$LAUNCH_LOG" "$OUT/launcher.log"
  "$ACCEPTANCE" "$OUT"
  ;;
*)
  echo 'usage: run-clmul-v3.sh prepare|session|finalize pilot|pilot-r2|confirmation' >&2
  exit 2
  ;;
esac
