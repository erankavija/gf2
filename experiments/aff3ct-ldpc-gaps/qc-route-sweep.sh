#!/usr/bin/env bash
# Queue line for @/issue/c62356d2: builds the route sweep, then runs or resumes
# it on the full host. Run from the repository root inside a benchmark window.
set -euo pipefail
export CARGO_CI_NO_LOCK=1
output=target/aff3ct-ldpc-gaps/qc-route-sweep
./scripts/cargo-budget.sh cargo build --release -p aff3ct-ldpc-gaps --bin qc-route-sweep
exec dev/scripts/ccx1-bench-flock.sh --full-host target/release/qc-route-sweep "$output"
