#!/usr/bin/env bash
# 98336ab4 GF(251)/n=4096 ISOLATED fgemm bench driver.
#
# Measures ONLY GF(251)/n=4096 in isolation (filter excludes all other
# primes so none runs first in the same process). This removes the L3
# cache contamination present in the consolidated 6-prime sweep, giving
# an apples-to-apples comparison against the fflas reference (which was
# itself measured one-config-per-process in isolation).
#
# Route A for GF(251)/n=4096 (select_f32_path, P==251 && n>=512) has an
# L3-budget heuristic tuned by 74ba1cdc to assume ~16 MB free L3. The
# consolidated sweep's 5 preceding n=4096 GEMMs violate that assumption;
# the isolated run does not.
#
# 5-trial CCX1-pinned (taskset -c 6-11 nice -n -5), sequential.
#
# Output:
#   dev/bench_results/2026-05-28-98336ab4-n4096-gf251-isolated/
#     iso_trial{1..5}.log    -- criterion display output
#     iso_trial{1..5}.json   -- estimates.json snapshot per trial
#
# Usage:
#   bash dev/bench_results/run_98336ab4_gf251_isolated_bench.sh

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
BENCH_BIN_NAME=fieldmatrix_gemm
TRIAL_DIR="$REPO_ROOT/dev/bench_results/2026-05-28-98336ab4-n4096-gf251-isolated"
NTRIALS=5

mkdir -p "$TRIAL_DIR"

build_bench() {
    echo "[gf251-iso] Building bench..." >&2
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
    echo "[gf251-iso] Bench binary: $bin" >&2
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
    local est="$REPO_ROOT/target/criterion/gemm_Fp_251/Fp_251/4096/new/estimates.json"
    if [[ -f "$est" ]]; then
        python3 -c "
import json
data = json.load(open('$est'))
print(json.dumps({'trial': $trial, 'prime': 251, 'n': 4096,
                  'route': 'isolated', 'estimates': data}))
" > "$snap_file"
    else
        echo "ERR: trial $trial: estimates.json not found at $est" >&2
        exit 1
    fi
}

BENCH_BIN="$(build_bench)"

# Filter: ONLY GF(251)/n=4096 -- no other prime runs in this process.
FILTER='gemm/Fp_251/Fp_251/4096$'

echo "[gf251-iso] === Isolated GF(251)/4096 dispatch (env vars unset) ===" >&2
for T in $(seq 1 "$NTRIALS"); do
    quiet_host_check "$T"
    echo "[gf251-iso] trial $T / $NTRIALS ..." >&2
    unset GF2_GF251_ROUTE_A GF2_GF251_ROUTE_C
    taskset -c 6-11 nice -n -5 "$BENCH_BIN" "$FILTER" --bench \
        > "$TRIAL_DIR/iso_trial${T}.log" 2>&1 \
        || { echo "trial $T failed (see $TRIAL_DIR/iso_trial${T}.log)" >&2; exit 1; }
    snapshot_trial "$TRIAL_DIR/iso_trial${T}.json" "$T"
done

echo "[gf251-iso] ALL TRIALS DONE" >&2
