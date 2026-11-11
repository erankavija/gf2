#!/usr/bin/env bash
# Queue line for @/issue/c62356d2: builds the route sweep, then runs or resumes
# it on the full host inside a benchmark window.
set -euo pipefail
repo=$(git -C "$(dirname "${BASH_SOURCE[0]}")" rev-parse --show-toplevel)
cd "$repo"
flock="$repo/$(python3 "$repo/$(git -C "$repo" ls-files -- ':(glob)**/repository_files.py')" file ccx1-bench-flock.sh)"
export CARGO_CI_NO_LOCK=1
output=target/aff3ct-ldpc-gaps/qc-route-sweep
./scripts/cargo-budget.sh cargo build --release -p aff3ct-ldpc-gaps --bin qc-route-sweep
exec "$flock" --full-host target/release/qc-route-sweep "$output"
