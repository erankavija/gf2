#!/usr/bin/env bash
# Record the steady-state allocation counter's own run (jit:07ca8585, REQ-07).
#
# Runs `crates/gf2-coding/tests/ldpc_decode_allocations.rs` in one thread, so
# the records it prints arrive in a fixed order, and keeps the JSON lines it
# emits from its counted sections. The test asserts the zero steady-state
# relation, so a run that does not hold it fails here instead of publishing.
# The counts are exact and deterministic for a fixed decoder and input; nothing
# here is a timing, so this runs outside the benchmark mutex.
#
# Usage (from the worktree root): record-allocation-counter.sh
set -euo pipefail
repo=$(git rev-parse --show-toplevel)
[[ "$PWD" == "$repo" ]] || { echo 'invoke from the worktree root' >&2; exit 2; }
export PATH="$HOME/.cargo/bin:$PATH"
OUT=dev/bench_results/07ca8585/preparation/allocation-counter.jsonl
mkdir -p "$(dirname "$OUT")"
./scripts/cargo-budget.sh cargo +1.95 test -p gf2-coding --offline --profile ci-test \
  --test ldpc_decode_allocations -- --nocapture --test-threads 1 \
  | grep '^{"schema":"ldpc-decode-allocation-census-v1"' > "$OUT"
echo "$OUT $(wc -l < "$OUT") records"
