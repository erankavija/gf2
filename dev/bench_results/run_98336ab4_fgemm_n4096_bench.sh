#!/usr/bin/env bash
# 98336ab4 consolidated fgemm n=4096 bench driver — production-default dispatch.
#
# Measures GF(7), GF(31), GF(127), GF(241), GF(251), GF(65521) at n=4096
# (all 6 target primes) plus re-measures non-regression control cells at
# n ∈ {256, 1024} for the same primes.
#
# GF(251)/n=4096 routes through Route A via select_f32_path (n >= 512, P ==
# 251 → select_f32_path returns true; env var GF2_GF251_ROUTE_A unset).
# GF(65521)/n=4096 routes through the f64 cascade (select_f64_path, issue
# 0749dbad). All other cells use Candidate C (small primes).
#
# Predecessor kernels on main at run time:
#   74ba1cdc — Route A L3-budget tuning → GF(251)/n=4096 1.466× PASS
#   0749dbad — f64 GEMM cascade for fp_medium → GF(65521)/n=4096 1.283× PASS
#
# 5-trial CCX1-pinned (taskset -c 6-11 nice -n -5), sequential.
#
# Output:
#   * dev/bench_results/2026-05-28-98336ab4-n4096/   — per-trial snapshots
#   * dev/bench_results/2026-05-28-98336ab4-fgemm-n4096.csv          (raw)
#   * dev/bench_results/2026-05-28-98336ab4-fgemm-n4096-aggregate.csv (median/IQR)
#
# Usage:
#   bash dev/bench_results/run_98336ab4_fgemm_n4096_bench.sh

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
BENCH_BIN_NAME=fieldmatrix_gemm
TRIAL_DIR="$REPO_ROOT/dev/bench_results/2026-05-28-98336ab4-n4096"
RESULTS_DIR="$REPO_ROOT/dev/bench_results"
DATE_PREFIX="2026-05-28-98336ab4"
NTRIALS=5

mkdir -p "$TRIAL_DIR"

build_bench() {
    echo "[98336ab4] Building bench..." >&2
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
    echo "[98336ab4] Bench binary: $bin" >&2
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
    local route_label="$3"
    : > "$snap_file"
    for P in 7 31 127 241 251 65521; do
        for N in 256 1024 4096; do
            local est
            est="$REPO_ROOT/target/criterion/gemm_Fp_${P}/Fp_${P}/${N}/new/estimates.json"
            if [[ -f "$est" ]]; then
                python3 -c "
import json
data = json.load(open('$est'))
print(json.dumps({'trial': $trial, 'prime': $P, 'n': $N,
                  'route': '$route_label', 'estimates': data}))
" >> "$snap_file"
            fi
        done
    done
    local cells
    cells="$(wc -l < "$snap_file")"
    echo "[98336ab4]   trial $trial ($route_label): $cells cells captured" >&2
}

BENCH_BIN="$(build_bench)"

# Filter: all 6 target primes at n ∈ {256, 1024, 4096}.
FILTER='gemm/Fp_(7|31|127|241|251|65521)/Fp_(7|31|127|241|251|65521)/(256|1024|4096)$'

echo "[98336ab4] === Production-default dispatch (env vars unset) ===" >&2
for T in $(seq 1 "$NTRIALS"); do
    if [[ -s "$TRIAL_DIR/PD_trial${T}.json" ]]; then
        echo "[98336ab4] trial $T already snapshotted -- skipping" >&2
        continue
    fi
    quiet_host_check "$T"
    echo "[98336ab4] trial $T / $NTRIALS ..." >&2
    unset GF2_GF251_ROUTE_A GF2_GF251_ROUTE_C
    taskset -c 6-11 nice -n -5 "$BENCH_BIN" "$FILTER" --bench \
        > "$TRIAL_DIR/PD_trial${T}.log" 2>&1 \
        || { echo "trial $T failed (see $TRIAL_DIR/PD_trial${T}.log)" >&2; exit 1; }
    snapshot_trial "$TRIAL_DIR/PD_trial${T}.json" "$T" "production_default"
done

echo "[98336ab4] Aggregating $NTRIALS trials..." >&2

python3 - "$TRIAL_DIR" "$RESULTS_DIR" "$NTRIALS" "$DATE_PREFIX" <<'PY'
import json, os, sys, csv, statistics

trial_dir = sys.argv[1]
out_dir   = sys.argv[2]
ntrials   = int(sys.argv[3])
pfx       = sys.argv[4]

records = []
for t in range(1, ntrials + 1):
    snap = os.path.join(trial_dir, f"PD_trial{t}.json")
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
raw_csv = os.path.join(out_dir, f"{pfx}-fgemm-n4096.csv")
with open(raw_csv, "w", newline="") as fh:
    w = csv.writer(fh)
    w.writerow(["prime", "n", "trial", "route", "gop_s"])
    for (prime, n, route), vals in sorted(buckets.items()):
        for trial, gop_s in sorted(vals):
            w.writerow([prime, n, trial, route, f"{gop_s:.4f}"])
print(f"WROTE {raw_csv}")

# Aggregate CSV
agg_csv = os.path.join(out_dir, f"{pfx}-fgemm-n4096-aggregate.csv")
with open(agg_csv, "w", newline="") as fh:
    w = csv.writer(fh)
    w.writerow(["prime", "n", "route", "median_gop_s", "q1", "q3", "iqr", "min", "max"])
    for (prime, n, route), vals in sorted(buckets.items()):
        mn, q1, med, q3, mx, iqr = agg(vals)
        w.writerow([prime, n, route,
                    f"{med:.3f}", f"{q1:.3f}", f"{q3:.3f}",
                    f"{iqr:.3f}", f"{mn:.3f}", f"{mx:.3f}"])
print(f"WROTE {agg_csv}")

# fflas reference values (clean — no /1e9 corruption):
#   GF(7)/4096:    136.737 Gop/s  (direct, 2026-04-26-reference.csv)
#   GF(31)/4096:   137.602 Gop/s  (direct, 2026-05-04-609855d9-gf31-supplement.csv)
#   GF(127)/4096:  136.737 Gop/s  (bracketed, GF(7)/Modular<int64_t> dispatch tier)
#   GF(241)/4096:  158.964 Gop/s  (bracketed, GF(251)/Modular<float> dispatch tier)
#   GF(251)/4096:  158.964 Gop/s  (direct, 2026-04-26-reference.csv)
#   GF(65521)/4096: 69.719 Gop/s  (direct, 2026-04-26-reference.csv)
FFLAS_N4096 = {
    7:     136.737,
    31:    137.602,
    127:   136.737,
    241:   158.964,
    251:   158.964,
    65521: 69.719,
}

# non-regression baselines at n=256,1024 from 41096af5 post-wire-in aggregate
FFLAS_NONREG = {
    7:     {256: 50.752, 1024: 96.233},
    31:    {256: 50.478, 1024: 94.643},
    127:   {256: 50.75,  1024: 96.23},
    241:   {256: 128.48, 1024: 138.32},
    251:   {256: 128.48, 1024: 138.32},
    65521: {256: 31.615, 1024: 43.381},
}

print()
print(f"{'prime':>6} {'n':>5} {'route':>18} {'median':>8} {'IQR':>7} "
      f"{'gf2/fflas':>10} {'wall_ratio':>11} {'verdict':>12}")
print("-" * 90)
for (prime, n, route), vals in sorted(buckets.items()):
    _, q1, med, q3, _, iqr = agg(vals)
    ratio_str = ""
    wall_str = ""
    verdict_str = ""
    if n == 4096 and prime in FFLAS_N4096:
        fflas = FFLAS_N4096[prime]
        ratio = med / fflas
        wall  = fflas / med
        ratio_str = f"{ratio:.3f}"
        wall_str  = f"{wall:.3f}"
        verdict_str = "PASS" if ratio >= 0.667 else "SHORTFALL"
    elif prime in FFLAS_NONREG and n in FFLAS_NONREG[prime]:
        fflas = FFLAS_NONREG[prime][n]
        ratio = med / fflas
        ratio_str = f"{ratio:.3f}"
    print(f"{prime:>6} {n:>5} {route:>18} {med:>8.2f} {iqr:>7.3f} "
          f"{ratio_str:>10} {wall_str:>11} {verdict_str:>12}")
PY

echo "[98336ab4] Done. Results in $RESULTS_DIR/${DATE_PREFIX}-fgemm-n4096*.csv" >&2
