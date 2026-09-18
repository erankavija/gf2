#!/usr/bin/env bash
# Non-timed wire-contract smoke of the residual-shift arms (jit:85fc5ff4).
#
# Drives both arms of every frozen cell in the validation role with the runner's
# own request framing and child environment, so each arm performs one untimed
# dispatch and returns no timing window. The smoke opens no campaign: it takes
# no lock, reserves nothing in the family ledger, writes no stage and finalizes
# no receipt. Reading the arm's source does not establish the wire contract
# between the runner and a child.
#
# Usage (from the worker worktree root): smoke-shift-arms.sh [--check]
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
manifest="$survey/arm-smoke/Cargo.toml"
driver="$survey/arm-smoke/target/release/arm-smoke"
smoke=target/85fc5ff4-arm-smoke
rm -rf "$smoke"
mkdir -p "$smoke"

./scripts/cargo-budget.sh cargo build --release --manifest-path "$manifest" >/dev/null
./scripts/cargo-budget.sh --test cargo test --release --manifest-path "$manifest"

# The smoked executable is the queued executable: the same bench target the
# window job resolves, built from the current tree.
./scripts/cargo-budget.sh cargo build --release -p gf2-core --bench shifts \
  --all-features --message-format=json >"$smoke/shifts-build.jsonl"
arm=$(python3 "$survey/find-shift-executable.py" "$smoke/shifts-build.jsonl")

# The plan is the campaign's own projection of the frozen addendum over every
# frozen cell; the lock it names is never opened, because the smoke measures
# nothing.
python3 -B "$survey/make-shift-plan.py" "$smoke/plan.json" "$arm" \
  "$(realpath -m "$smoke/unused.lock")" --label smoke --campaign-id 85fc5ff4-arms-smoke
"$arm" --check-plan "$smoke/plan.json"

"$driver" --plan "$smoke/plan.json" --output "$smoke/observations.json"
python3 -B "$survey/summarize-arm-smoke.py" \
  --observations "$smoke/observations.json" --format json --output "$record" \
  --command "$survey/smoke-shift-arms.sh" ${mode:+--check}
