#!/usr/bin/env bash
# Non-timed smoke of the residual-shift arms (jit:85fc5ff4).
#
# Usage (from the worker worktree root): smoke-shift-arms.sh [--check]
# Contract: `benchmark-ab-runner smoke`, stated at
# `tuning_campaign_support::arm::smoke`.
set -euo pipefail
repo=$(git rev-parse --show-toplevel)
[[ "$(pwd -P)" == "$(cd "$repo" && pwd -P)" ]] || {
  echo 'invoke from the worker worktree root' >&2
  exit 2
}
export PATH="$HOME/.cargo/bin:$PATH"
export RAYON_NUM_THREADS=1 RUSTUP_TOOLCHAIN=1.95 CARGO_CI_NO_SCCACHE=1

mode=${1:-}
case "$mode" in
  ""|--check) ;;
  *) echo "usage: $0 [--check]" >&2; exit 2 ;;
esac

active=dev/active/c04dd4ac-zen3-shifts-and-permutations
survey="$active/survey"
record="$active/shift-profile-smoke.json"
smoke=target/85fc5ff4-arm-smoke
rm -rf "$smoke"
mkdir -p "$smoke"

./scripts/cargo-budget.sh cargo build --release -p tuning-campaign-support \
  --bin benchmark-ab-runner
runner=$(realpath target/release/benchmark-ab-runner)

# The smoked executable is the queued executable: the same bench target the
# window job resolves, built from the current tree.
./scripts/cargo-budget.sh cargo build --release -p gf2-core --bench shifts \
  --all-features --message-format=json >"$smoke/shifts-build.jsonl"
arm=$(python3 "$survey/find-shift-executable.py" "$smoke/shifts-build.jsonl")

# The plan is the campaign's own projection of the frozen addendum over every
# frozen cell; the lock it names is never opened.
python3 -B "$survey/make-shift-plan.py" "$smoke/plan.json" "$arm" \
  "$(realpath -m "$smoke/unused.lock")" --label smoke --campaign-id 85fc5ff4-arms-smoke
"$arm" --check-plan "$smoke/plan.json"

if [[ "$mode" == --check ]]; then
  "$runner" smoke "$smoke/plan.json" --record "$smoke/smoke.json"
  cmp "$smoke/smoke.json" "$record"
  echo "$record: unchanged by this smoke"
else
  "$runner" smoke "$smoke/plan.json" --record "$record"
fi
