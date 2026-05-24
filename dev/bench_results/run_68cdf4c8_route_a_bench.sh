#!/usr/bin/env bash
# 68cdf4c8 route-A (in-Rust GF(251) f32/FMA cascade rework) bench driver.
#
# Runs N=5 sequential criterion bench trials for the route-A reworked
# Candidate F path at GF(251)/n ∈ {256, 1024} on the CCX1-pinned Zen-3
# reference host. The route-A path is opted into via the launcher-
# convenience env var `GF2_GF251_ROUTE_A=1`, which the GF(251) bench
# function reads (safe `std::env::var`) and dispatches to the safe
# `gf2_core::gfp::simd_ops::set_route_a_gf251_enabled(true)` setter
# (jit:68cdf4c8 R1 commit `4bad2e72`; original env-var-driven toggle
# replaced with `AtomicBool` to satisfy SC#3 unsafe-isolation). All
# other primes / cells use the default Candidate C dispatch and share
# a single bench binary.
#
# Per-trial isolation:
#   * taskset -c 6-11 pins to CCX1 (cores 6-11). The parent shell lives
#     on CCX0 by default, avoiding cross-CCX cache traffic.
#   * nice -n -5 raises priority (non-privileged; falls back silently).
#   * Trials run sequentially, never in parallel.
#
# Output:
#   * dev/bench_results/2026-05-24-68cdf4c8-route-a/      — per-trial JSON snapshots
#   * dev/bench_results/2026-05-24-68cdf4c8-route-a-f32-cascade.csv
#
# Usage:
#   bash dev/bench_results/run_68cdf4c8_route_a_bench.sh

set -euo pipefail

WORKTREE_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
BENCH_BIN_NAME=fieldmatrix_gemm
TRIAL_DIR="$WORKTREE_ROOT/dev/bench_results/2026-05-24-68cdf4c8-route-a"
RESULTS_DIR="$WORKTREE_ROOT/dev/bench_results"
DATE_PREFIX="2026-05-24-68cdf4c8"
NTRIALS=5

mkdir -p "$TRIAL_DIR"

build_bench() {
    echo "[route-a] Building bench…" >&2
    cargo build --release -p gf2-core --bench "$BENCH_BIN_NAME" \
        --features rand,simd >&2
    local bin
    bin="$(find "$WORKTREE_ROOT/target/release/deps" \
        -maxdepth 1 -name "${BENCH_BIN_NAME}-*" -executable -type f \
        | xargs ls -t | head -1)"
    if [[ -z "$bin" || ! -x "$bin" ]]; then
        echo "ERR: failed to locate bench binary" >&2
        exit 1
    fi
    echo "[route-a] Bench binary: $bin" >&2
    echo "$bin"
}

snapshot_trial() {
    local snap_file="$1"
    local trial="$2"
    local route_label="$3"
    : > "$snap_file"
    # Capture GF(251) (route-A target) and non-regression controls
    # GF(7), GF(31), GF(127) at the criterion cells.
    for P in 7 31 127 251; do
        for N in 256 1024; do
            local est="$WORKTREE_ROOT/target/criterion/gemm_Fp_${P}/Fp_${P}/${N}/new/estimates.json"
            if [[ -f "$est" ]]; then
                python3 -c "
import json, sys
data = json.load(open('$est'))
print(json.dumps({'trial': $trial, 'prime': $P, 'n': $N,
                  'route': '$route_label', 'estimates': data}))
" >> "$snap_file"
            fi
        done
    done
    echo "[route-a]   trial $trial ($route_label): $(wc -l < "$snap_file") cells captured" >&2
}

BENCH_BIN="$(build_bench)"

# Filter pattern matching all four primes and both sizes.
FILTER='gemm/Fp_(7|31|127|251)/Fp_(7|31|127|251)/(256|1024)$'

# ─────────────────────────────────────────────────────────────────────────
# Phase 1: route-A enabled (GF(251) uses reworked Candidate F).
# Non-GF(251) cells still use Candidate C (env var is GF(251)-scoped).
# Skips trials whose .json snapshot already exists so the run can resume
# after a contamination abort.
# ─────────────────────────────────────────────────────────────────────────
echo "[route-a] === Phase 1: route-A enabled (GF2_GF251_ROUTE_A=1) ===" >&2
for T in $(seq 1 "$NTRIALS"); do
    if [[ -s "$TRIAL_DIR/RA_trial${T}.json" ]]; then
        echo "[route-a] route-A trial $T already snapshotted — skipping" >&2
        continue
    fi
    echo "[route-a] route-A trial $T / $NTRIALS …" >&2
    GF2_GF251_ROUTE_A=1 taskset -c 6-11 nice -n -5 "$BENCH_BIN" "$FILTER" --bench \
        > "$TRIAL_DIR/RA_trial${T}.log" 2>&1 \
        || { echo "route-A trial $T failed (see $TRIAL_DIR/RA_trial${T}.log)" >&2; exit 1; }
    snapshot_trial "$TRIAL_DIR/RA_trial${T}.json" "$T" "route_a"
done

# ─────────────────────────────────────────────────────────────────────────
# Phase 2: default (Candidate C for GF(251); same baseline for controls).
# Skips trials whose .json snapshot already exists so the run can resume
# after a contamination abort.
# ─────────────────────────────────────────────────────────────────────────
echo "[route-a] === Phase 2: default Candidate C (env var unset) ===" >&2
for T in $(seq 1 "$NTRIALS"); do
    if [[ -s "$TRIAL_DIR/C_trial${T}.json" ]]; then
        echo "[route-a] default trial $T already snapshotted — skipping" >&2
        continue
    fi
    echo "[route-a] default trial $T / $NTRIALS …" >&2
    unset GF2_GF251_ROUTE_A
    taskset -c 6-11 nice -n -5 "$BENCH_BIN" "$FILTER" --bench \
        > "$TRIAL_DIR/C_trial${T}.log" 2>&1 \
        || { echo "default trial $T failed (see $TRIAL_DIR/C_trial${T}.log)" >&2; exit 1; }
    snapshot_trial "$TRIAL_DIR/C_trial${T}.json" "$T" "default"
done

# ─────────────────────────────────────────────────────────────────────────
# Aggregate.
# ─────────────────────────────────────────────────────────────────────────
echo "[route-a] Aggregating $NTRIALS trials (route-A + default)…" >&2

python3 - "$TRIAL_DIR" "$RESULTS_DIR" "$NTRIALS" "$DATE_PREFIX" <<'PY'
import json, os, sys, csv, statistics, math

trial_dir = sys.argv[1]
out_dir   = sys.argv[2]
ntrials   = int(sys.argv[3])
pfx       = sys.argv[4]

records = []
for kpath in ("RA", "C"):
    for t in range(1, ntrials + 1):
        snap = os.path.join(trial_dir, f"{kpath}_trial{t}.json")
        if not os.path.exists(snap):
            continue
        with open(snap) as fh:
            for line in fh:
                line = line.strip()
                if line:
                    records.append(json.loads(line))

def gops(ns, n):
    return 2 * (n ** 3) / ns

buckets = {}
for r in records:
    key = (r["prime"], r["n"], r["route"])
    ns = r["estimates"]["median"]["point_estimate"]
    buckets.setdefault(key, []).append((r["trial"], gops(ns, r["n"])))

def agg(vals):
    s = sorted(v for _, v in vals)
    n = len(s)
    med = statistics.median(s)
    if n >= 4:
        q1 = s[1]; q3 = s[-2]
    else:
        q1 = s[0]; q3 = s[-1]
    return min(s), q1, med, q3, max(s), q3 - q1

# Raw CSV
raw_csv = os.path.join(out_dir, f"{pfx}-route-a-f32-cascade.csv")
with open(raw_csv, "w", newline="") as fh:
    w = csv.writer(fh)
    w.writerow(["prime", "n", "trial", "route", "gop_s"])
    for (prime, n, route), vals in sorted(buckets.items()):
        for trial, gop_s in sorted(vals):
            w.writerow([prime, n, trial, route, f"{gop_s:.4f}"])
print(f"WROTE {raw_csv}")

# Aggregate CSV
agg_csv = os.path.join(out_dir, f"{pfx}-route-a-f32-cascade-aggregate.csv")
with open(agg_csv, "w", newline="") as fh:
    w = csv.writer(fh)
    w.writerow(["prime", "n", "route", "median_gop_s", "q1", "q3", "iqr", "min", "max"])
    for (prime, n, route), vals in sorted(buckets.items()):
        mn, q1, med, q3, mx, iqr = agg(vals)
        w.writerow([prime, n, route,
                    f"{med:.3f}", f"{q1:.3f}", f"{q3:.3f}",
                    f"{iqr:.3f}", f"{mn:.3f}", f"{mx:.3f}"])
print(f"WROTE {agg_csv}")

# Console summary
print()
print(f"{'prime':>6} {'n':>5} {'route':>9} {'median':>8} {'IQR':>7}")
print("-" * 50)
for (prime, n, route), vals in sorted(buckets.items()):
    _, q1, med, q3, _, iqr = agg(vals)
    print(f"{prime:>6} {n:>5} {route:>9} {med:>8.2f} {iqr:>7.3f}")
PY

echo "[route-a] Done. Results in $RESULTS_DIR/${DATE_PREFIX}-route-a-f32-cascade*.csv" >&2
