#!/usr/bin/env bash
# Build or execute the protocol-v4 residual BitVec shift profile (jit:85fc5ff4).
#
# `build` performs only non-timed compilation and semantic/source/plan checks.
# `window` is the bounded resumable measurement and refuses to start unless
# the scheduled-window runner exported GF2_BENCH_WINDOW=1.

set -euo pipefail
repo=$(git rev-parse --show-toplevel)
cd "$repo"
export PATH="/home/vkaskivuo/.cargo/bin:$PATH"
export RAYON_NUM_THREADS=1 RUSTUP_TOOLCHAIN=1.95 CARGO_CI_NO_SCCACHE=1

mode=${1:-}
case "$mode" in
  build|window) ;;
  *) echo "usage: $0 build|window" >&2; exit 2 ;;
esac
if [[ "$mode" == window && "${GF2_BENCH_WINDOW:-0}" != 1 ]]; then
  echo "timed residual-shift work requires the scheduled benchmark window" >&2
  exit 2
fi

active=dev/active/c04dd4ac-zen3-shifts-and-permutations
survey="$active/survey"
addendum="$active/shift-profile-addendum.json"
validation="$active/shift-profile-validation.json"
producing="$survey/shift-profile-producing-inputs.json"
out=dev/bench_results/c04dd4ac/residual-shift-profile
stage=target/bench-stage/85fc5ff4-residual-shift-profile
plan="$stage.plan.json"
build_dir=target/85fc5ff4-build
mkdir -p "$build_dir" "$(dirname "$stage")"

python3 "$survey/freeze-shift-addendum.py" --check
python3 "$survey/inspect-shift-consumers.py" --check
python3 "$survey/make-shift-producing-inputs.py" "$build_dir/producing-inputs.json"
cmp "$producing" "$build_dir/producing-inputs.json"

./scripts/cargo-budget.sh cargo build --release -p tuning-campaign-support \
  --bin benchmark-ab-runner --bin benchmark-acceptance
./scripts/cargo-budget.sh cargo build --release -p gf2-core --bench shifts \
  --all-features --message-format=json >"$build_dir/shifts-build.jsonl"
arm=$(python3 "$survey/find-shift-executable.py" "$build_dir/shifts-build.jsonl")
runner=$(realpath target/release/benchmark-ab-runner)
acceptance=$(realpath target/release/benchmark-acceptance)

"$arm" --verify >"$build_dir/validation.json"
cmp "$validation" "$build_dir/validation.json"
lock=${GF2_CCX1_LOCK:-/tmp/gf2-ccx1.lock}
touch "$lock"
lock=$(realpath "$lock")
export GF2_CCX1_LOCK="$lock"

if [[ -f "$plan" ]]; then
  echo "# preserving staged plan for resumability: $plan" >&2
else
  python3 "$survey/make-shift-plan.py" "$plan" "$arm" "$lock"
fi
"$arm" --check-plan "$plan"

if [[ "$mode" == build ]]; then
  echo "# non-timed preparation complete" >&2
  echo "# arm: $arm" >&2
  echo "# runner: $runner" >&2
  echo "# plan: $plan" >&2
  exit 0
fi
if [[ -f "$out/receipt.json" ]]; then
  "$acceptance" "$out"
  echo "# accepted receipt already exists: $out" >&2
  exit 0
fi

launcher_log="$stage.launcher.log"
{
  echo "# command: $0 $*"
  echo "# started_utc: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo "# source revision (informational): $(git rev-parse HEAD)"
  echo "# addendum: $addendum"
  echo "# plan: $plan"
  echo "# stage: $stage"
  echo "# output: $out"
  echo "# arm: $arm"
  echo "# arm build identity: $("$arm" --build-identity)"
  echo "# load_avg_start: $(uptime)"
} >>"$launcher_log"

session=0
while true; do
  session=$((session + 1))
  set +e
  GF2_BENCH=1 dev/scripts/ccx1-bench-flock.sh --full-host \
    "$runner" run "$stage" "$plan" | tee -a "$launcher_log"
  status=${PIPESTATUS[0]}
  set -e
  echo "# session $session exit: $status (0 = complete, 3 = paused)" >>"$launcher_log"
  case "$status" in
    0) break ;;
    3) continue ;;
    *) echo "session $session exited $status" >&2; exit "$status" ;;
  esac
done

"$runner" finalize "$stage" "$out" | tee -a "$launcher_log"
cp "$launcher_log" "$out/launcher.log"
set +e
"$acceptance" "$out" | tee -a "$out/launcher.log"
verdict=${PIPESTATUS[0]}
set -e
{
  echo "# acceptance exit: $verdict"
  echo "# load_avg_end: $(uptime)"
  echo "# finished_utc: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
} >>"$out/launcher.log"
exit "$verdict"
