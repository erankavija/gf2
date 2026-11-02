#!/usr/bin/env bash
# Queue line for @/issue/67194d70: builds the validator, then runs the preregistered
# validation. Run from the repository root inside a benchmark window.
set -euo pipefail
export CARGO_CI_NO_LOCK=1
config=experiments/permanent-statistics/config/campaign.json
output=target/simulation_results/permanent-statistics/validation
./scripts/cargo-budget.sh cargo build --release -p permanent-statistics \
  --features hip --bin permanent_validation
exec target/release/permanent_validation --config "$config" --output "$output" \
  --workers "$(nproc)"
