#!/usr/bin/env bash
# Prepare or resume the protocol-v4 residual-route A/B campaign.
set -euo pipefail

repo=$(git rev-parse --show-toplevel)
cd "$repo"
export PATH="${HOME}/.cargo/bin:${PATH}"
export RAYON_NUM_THREADS=1 RUSTUP_TOOLCHAIN=1.95 CARGO_CI_NO_SCCACHE=1

original_argv=("$@")
action=${1:?prepare or window}
phase=${2:?pilot or confirmation}
case "$action:$phase" in
  prepare:pilot|window:pilot|prepare:confirmation|window:confirmation) ;;
  *) echo "usage: $0 prepare|window pilot|confirmation" >&2; exit 2 ;;
esac
if [[ "$action" == window && "${GF2_BENCH_WINDOW:-0}" != 1 ]]; then
  echo 'timed campaign requires the scheduled benchmark window' >&2
  exit 2
fi

active=dev/active/00dd43c3
survey="$active/survey"
build=target/00dd43c3
stage="$build/$phase-stage"
plan="$build/$phase-plan.json"
out="dev/bench_results/00dd43c3/v4-r1-$phase"
addendum="$active/$phase-addendum.json"
smoke="$active/$phase-smoke.json"
manifest="$survey/producing-inputs.json"
campaign="00dd43c3-residual-bmi2-$phase-v4-r1"
launcher_log="$build/$phase-launcher.log"
lock=${GF2_CCX1_LOCK:-/tmp/gf2-ccx1.lock}
case "$phase" in
  pilot) seed=2026092600 ;;
  confirmation) seed=2026092601 ;;
esac

if [[ "$action" == prepare ]]; then
  mkdir -p "$build"
  python3 -B "$survey/freeze-pilot.py" --check
  python3 -B "$survey/make-producing-inputs.py" "$build/producing-inputs.json"
  cmp "$manifest" "$build/producing-inputs.json"
  ./scripts/cargo-budget.sh cargo build --release -p tuning-campaign-support \
    --bin benchmark-ab-runner --bin benchmark-acceptance
  ./scripts/cargo-budget.sh cargo build --release -p gf2-core --bench shifts \
    --all-features --message-format=json >"$build/shifts-build.jsonl"
  arm=$(python3 -B dev/active/c04dd4ac-zen3-shifts-and-permutations/survey/find-shift-executable.py \
    "$build/shifts-build.jsonl")
  "$arm" --check-request-mirror
  "$arm" --verify >"$build/validation.json"
  touch "$lock"
  lock=$(realpath "$lock")
  export GF2_CCX1_LOCK="$lock"
  if [[ ! -f "$plan" ]]; then
    python3 -B "$survey/make-plan.py" "$plan" "$arm" "$lock" \
      --addendum "$addendum" --label "$phase" --campaign-id "$campaign" \
      --campaign-seed "$seed"
  fi
  "$arm" --check-plan "$plan"
  if [[ -f "$smoke" ]]; then
    target/release/benchmark-ab-runner smoke "$plan" --record "$build/$phase-smoke.json"
    cmp "$smoke" "$build/$phase-smoke.json"
  else
    target/release/benchmark-ab-runner smoke "$plan" --record "$smoke"
  fi
  python3 -B "$survey/verify-smoke.py" "$smoke" "$addendum" "$plan"
  echo "prepared $phase: plan=$plan, smoke=$smoke, arm=$arm"
  exit 0
fi

[[ -f "$plan" && -f "$smoke" ]] || {
  echo "run prepare $phase and commit its smoke before the window" >&2
  exit 2
}
mapfile -t frozen_inputs < <(
  python3 -c 'import json,sys; print("\n".join(json.load(open(sys.argv[1]))["build_inputs"]))' \
    "$manifest"
)
git ls-files --error-unmatch -- "$addendum" "$smoke" "$manifest" \
  dev/bench_results/00dd43c3/residual-shift-family-ledger.jsonl \
  "${frozen_inputs[@]}" >/dev/null
git diff --quiet HEAD -- "$addendum" "$smoke" "$manifest" "${frozen_inputs[@]}" || {
  echo 'campaign inputs differ from committed bytes' >&2
  exit 2
}
if [[ ! -f "$stage/execution.log" ]]; then
  git diff --quiet HEAD -- dev/bench_results/00dd43c3/residual-shift-family-ledger.jsonl || {
    echo 'family ledger differs from its committed genesis before the first run' >&2
    exit 2
  }
fi
python3 -B "$survey/verify-smoke.py" "$smoke" "$addendum" "$plan"
runner=$(realpath target/release/benchmark-ab-runner)
acceptance=$(realpath target/release/benchmark-acceptance)
if [[ -f "$out/receipt.json" ]]; then
  "$acceptance" "$out"
  exit 0
fi

echo "GF2_CAMPAIGN_EXECUTION_LOG=$stage/execution.log"
{
  printf '# launcher argv:'
  printf ' %q' "$0" "${original_argv[@]}"
  printf '\n'
  printf '# runner argv:'
  printf ' %q' "$runner" run "$stage" "$plan"
  printf '\n'
  printf '# wrapper argv: GF2_BENCH=1'
  printf ' %q' dev/scripts/ccx1-bench-flock.sh --full-host "$runner" run "$stage" "$plan"
  printf '\n'
  echo "# campaign: $campaign"
  echo "# plan: $plan"
  echo "# stage: $stage"
  echo "# source revision (informational): $(git rev-parse HEAD)"
  echo "# started_utc: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
} >>"$launcher_log"
while ! python3 -B dev/scripts/verify-campaign-log.py --log "$stage/execution.log" --stage-complete; do
  set +e
  GF2_BENCH=1 dev/scripts/ccx1-bench-flock.sh --full-host \
    "$runner" run "$stage" "$plan" | tee -a "$launcher_log"
  status=${PIPESTATUS[0]}
  set -e
  echo "# session exit: $status" >>"$launcher_log"
  case "$status" in
    0|3) ;;
    *) echo "campaign session exited $status" >&2; exit "$status" ;;
  esac
done

"$runner" finalize "$stage" "$out" | tee -a "$launcher_log"
cp "$launcher_log" "$out/launcher.log"
python3 -B dev/scripts/verify-campaign-log.py --log "$out/execution.log" \
  --receipt "$out/receipt.json" --plan "$out/plan.json"
"$acceptance" "$out" | tee -a "$out/launcher.log"
