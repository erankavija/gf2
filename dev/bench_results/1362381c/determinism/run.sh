#!/usr/bin/env bash
# Untimed determinism suites on the integrated tree (REQ-11). Run from the worktree root.
set -uo pipefail
out=$(dirname "${BASH_SOURCE[0]}")
filter='binary(ldpc_batch_contract) | binary(batch_decode_worker_determinism) | binary(pooled_batch_decoder) | binary(checkpoint_compat) | binary(core_dispatch_fallback) | binary(ldpc_simd_validation) | binary(hybrid_resume) | binary(executor_oom_fallback_run)'
{
  rustc --version
  git rev-parse HEAD
  printf 'filter: %s\n' "$filter"
} > "$out/run-identity.txt"
CARGO_TERM_COLOR=never ./scripts/cargo-budget.sh --test cargo nextest run --workspace --all-features \
  --cargo-profile ci-test --profile ci --no-fail-fast --status-level all --final-status-level none -E "$filter" \
  > "$out/nextest.log" 2>&1
status=$?
echo "exit: $status" >> "$out/run-identity.txt"
exit "$status"
