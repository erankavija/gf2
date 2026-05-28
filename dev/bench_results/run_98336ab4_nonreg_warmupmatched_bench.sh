#!/usr/bin/env bash
# 98336ab4 warmup-matched non-regression bench driver (SC#3).
#
# Re-measures the non-regression control cells under measurement
# conditions IDENTICAL to the 41096af5 post-wire-in baseline:
#   filter = gemm/Fp_(7|31|127|241|251|65521)/.../(64|256|1024)$
#
# Crucially the filter INCLUDES n=64, so the Candidate-C small-prime
# path's instruction cache is warm before the n=256 cell runs --
# exactly the ordering the 41096af5 baseline used (filter ...(64|256|1024)$).
# The consolidated 6-prime n=4096 sweep started at n=256 (no n=64
# warmup), which left GF(7)/256 with a cold i-cache (44.3 vs 70.7).
#
# This run is the apples-to-apples gf2-now vs gf2-41096af5 comparison
# required by SC#3 ("No regression on existing (prime,n) cells PASSing
# post-41096af5, delta <= 5%").
#
# 5-trial CCX1-pinned (taskset -c 6-11 nice -n -5), sequential.
#
# Output:
#   dev/bench_results/2026-05-28-98336ab4-nonreg-warmupmatched/
#     nr_trial{1..5}.log    -- criterion display output
#     nr_trial{1..5}.json   -- estimates.json snapshots (one obj per cell)
#
# Usage:
#   bash dev/bench_results/run_98336ab4_nonreg_warmupmatched_bench.sh

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
BENCH_BIN_NAME=fieldmatrix_gemm
TRIAL_DIR="$REPO_ROOT/dev/bench_results/2026-05-28-98336ab4-nonreg-warmupmatched"
NTRIALS=5

mkdir -p "$TRIAL_DIR"

build_bench() {
    echo "[nonreg-wm] Building bench..." >&2
    cargo build --release -p gf2-core --bench "$BENCH_BIN_NAME" \
        --features rand,simd >&2
    local bin
    bin="$(find "$REPO_ROOT/target/release/deps" \
        -maxdepth 1 -name "${BENCH_BIN_NAME}-*" -executable -type f \
        | xargs ls -t | head -1)"
    if [[ -z "$bin" || ! -x "$bin" ]]; then
        echo "ERR: failed to locate bench binary" >&2
        exit 1
    fi
    echo "[nonreg-wm] Bench binary: $bin" >&2
    echo "$bin"
}

quiet_host_check() {
    local trial="$1"
    if pgrep -af 'cargo|rustc|criterion' \
        | grep -v 'jit-server' \
        | grep -v "fieldmatrix_gemm" \
        | grep -v "pgrep" \
        | grep -qv "^[[:space:]]*$" 2>/dev/null; then
        echo "ERR: trial $trial: competing cargo/rustc/criterion process detected -- aborting" >&2
        pgrep -af 'cargo|rustc|criterion' \
            | grep -v 'jit-server' \
            | grep -v "fieldmatrix_gemm" >&2 || true
        exit 1
    fi
}

snapshot_trial() {
    local snap_file="$1"
    local trial="$2"
    : > "$snap_file"
    for P in 7 31 127 241 251 65521; do
        for N in 64 256 1024; do
            local est
            est="$REPO_ROOT/target/criterion/gemm_Fp_${P}/Fp_${P}/${N}/new/estimates.json"
            if [[ -f "$est" ]]; then
                python3 -c "
import json
data = json.load(open('$est'))
print(json.dumps({'trial': $trial, 'prime': $P, 'n': $N,
                  'route': 'warmup_matched', 'estimates': data}))
" >> "$snap_file"
            fi
        done
    done
}

BENCH_BIN="$(build_bench)"

# Filter matches the 41096af5 baseline ordering exactly: starts at n=64
# so the small-prime Candidate-C i-cache is warm before n=256.
FILTER='gemm/Fp_(7|31|127|241|251|65521)/Fp_(7|31|127|241|251|65521)/(64|256|1024)$'

echo "[nonreg-wm] === Warmup-matched non-regression dispatch (env vars unset) ===" >&2
for T in $(seq 1 "$NTRIALS"); do
    quiet_host_check "$T"
    echo "[nonreg-wm] trial $T / $NTRIALS ..." >&2
    unset GF2_GF251_ROUTE_A GF2_GF251_ROUTE_C
    taskset -c 6-11 nice -n -5 "$BENCH_BIN" "$FILTER" --bench \
        > "$TRIAL_DIR/nr_trial${T}.log" 2>&1 \
        || { echo "trial $T failed (see $TRIAL_DIR/nr_trial${T}.log)" >&2; exit 1; }
    snapshot_trial "$TRIAL_DIR/nr_trial${T}.json" "$T"
done

echo "[nonreg-wm] ALL TRIALS DONE" >&2
