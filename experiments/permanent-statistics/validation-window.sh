#!/usr/bin/env bash
# Queue line for @/issue/67194d70: builds the validator, then runs the preregistered
# validation. Run from the repository root inside a benchmark window.
#
# The validation is one-shot with no redraw, so its record lives outside every
# target/ directory and outside the tracked tree: under the main checkout's
# gitignored .agents/campaign-evidence/, resolved from git's common directory
# so a linked worktree writes to the same place. GF2_VALIDATION_OUTPUT overrides
# it. A rerun resumes the record there.
set -euo pipefail
export CARGO_CI_NO_LOCK=1
config=experiments/permanent-statistics/config/campaign.json
root="$(dirname "$(git rev-parse --path-format=absolute --git-common-dir)")"
output="${GF2_VALIDATION_OUTPUT:-$root/.agents/campaign-evidence/67194d70/validation}"
mkdir -p "$(dirname "$output")"
echo "validation output $output"
./scripts/cargo-budget.sh cargo build --release -p permanent-statistics \
  --features hip --bin permanent_validation
exec target/release/permanent_validation --config "$config" --output "$output" \
  --workers "$(nproc)"
