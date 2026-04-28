#!/usr/bin/env bash
# perf stat capture for C1 (GF(2^m) batch multiply via VPCLMULQDQ).
#
# Runs the V0 (single-shot loop) and V3+V4 (batched VPCLMULQDQ-YMM) paths
# back-to-back at m ∈ {8, 16, 32} via the gf2m_mul_strategies bench
# entrypoint, capturing IPC, branch-misses, and cycles for both. Saves the
# combined stat output to dev/bench_results/2026-04-28-c1-perf-stat.txt.
#
# Usage:
#   ./dev/scripts/perf-stat-c1.sh

set -euo pipefail

OUT="${OUT:-dev/bench_results/2026-04-28-c1-perf-stat.txt}"
mkdir -p "$(dirname "$OUT")"

PATH="$HOME/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin:$PATH" \
    cargo bench -p gf2-core --bench gf2m_mul_strategies --no-run 2>&1 | tail -3

# Resolve the bench binary (criterion compiles to target/release/deps/<bench>-<hash>).
BIN=$(find target/release/deps -maxdepth 1 -name 'gf2m_mul_strategies-*' \
    -executable -type f -printf '%T@ %p\n' \
    | sort -nr | head -1 | awk '{print $2}')

if [[ -z "$BIN" ]]; then
    echo "ERROR: could not find gf2m_mul_strategies binary under target/release/deps/" >&2
    exit 1
fi

echo "Using binary: $BIN"
echo

# Wrap with perf stat, restricting events to the ones the PPC-spiral
# protocol asks for: cycles, instructions (→ IPC), branches/branch-misses,
# L1d-misses (cache-misses).
PERF_EVENTS="cycles,instructions,branches,branch-misses,L1-dcache-loads,L1-dcache-load-misses"

{
    echo "# C1 perf stat capture (kernel: GF(2^m) batch multiply via VPCLMULQDQ)"
    echo "# date    : $(date -u +%Y-%m-%dT%H:%M:%SZ)"
    echo "# host    : $(uname -a)"
    echo "# binary  : $BIN"
    echo "# events  : $PERF_EVENTS"
    echo
    echo "Bench binary records, with -r 10 repeats, both V0 (pclmulqdq_barrett_loop_v0)"
    echo "and V3+V4 (gf2m_batch_unroll4) leaves at m ∈ {8, 16, 32}."
    echo
    echo "============================================================"
    echo " V0 BASELINE: pclmulqdq_barrett_loop_v0 (single-shot loop)"
    echo "============================================================"
    perf stat -r 10 -e "$PERF_EVENTS" \
        "$BIN" --bench --profile-time 1 \
        '^gf2m_mul_crossover/pclmulqdq_barrett_loop_v0/(8|16|32)$' \
        2>&1 || true

    echo
    echo "============================================================"
    echo " V3+V4 KERNEL: gf2m_batch_unroll4 (VPCLMULQDQ-YMM)"
    echo "============================================================"
    perf stat -r 10 -e "$PERF_EVENTS" \
        "$BIN" --bench --profile-time 1 \
        '^gf2m_mul_crossover/gf2m_batch_unroll4/(8|16|32)$' \
        2>&1 || true
} > "$OUT" 2>&1

echo "perf stat output written to $OUT"
