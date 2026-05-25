#!/usr/bin/env bash
# e8a0c47a post-refactor bench driver — Phase 2 non-regression.
#
# Measures GF(251)/n ∈ {64, 256, 1024} and non-regression control primes
# GF(7), GF(31), GF(127) at n ∈ {256, 1024} under the production default
# dispatch, after the Phase 2 Barrett-reduction SSOT consolidation.
#
# The Phase 2 refactor (e8a0c47a) extracted the shared `barrett_reduce_lane32`
# primitive in `fp_small.rs` and wired fp_small_f32, fp_small_panel, and
# fp_medium to use it. This driver verifies that the refactor is semantically
# equivalent (within ±5%) to the pre-refactor 41096af5 baseline.
#
# Acceptance criteria (SC#4):
#   * GF(251)/n=1024: ratio vs fflas-ffpack 138.32 Gop/s must STILL be >= 0.667
#   * GF(7/31/127)/n in {256,1024}: within ±5% of 41096af5-post-wire-in-aggregate.csv
#
# N=5 sequential trials, CCX1-pinned (taskset -c 6-11, nice -n -5).
# Quiet-host check before each trial.
#
# Output:
#   * dev/bench_results/2026-05-25-e8a0c47a-post-refactor/   — per-trial snapshots
#   * dev/bench_results/2026-05-25-e8a0c47a-post-refactor.csv          (raw)
#   * dev/bench_results/2026-05-25-e8a0c47a-post-refactor-aggregate.csv (median/IQR)
#
# Usage:
#   bash dev/bench_results/run_e8a0c47a_post_refactor_bench.sh

set -euo pipefail

WORKTREE_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
BENCH_BIN_NAME=fieldmatrix_gemm
TRIAL_DIR="$WORKTREE_ROOT/dev/bench_results/2026-05-25-e8a0c47a-post-refactor"
RESULTS_DIR="$WORKTREE_ROOT/dev/bench_results"
DATE_PREFIX="2026-05-25-e8a0c47a"
NTRIALS=5

mkdir -p "$TRIAL_DIR"

build_bench() {
    echo "[e8a0c47a] Building bench…" >&2
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
    echo "[e8a0c47a] Bench binary: $bin" >&2
    echo "$bin"
}

quiet_host_check() {
    local trial="$1"
    # Exclude jit-server daemons (idle management processes, ≤0.1% CPU each)
    # and the bench binary itself (fieldmatrix_gemm). Only abort if a real
    # competing build/bench process is found.
    if pgrep -af 'cargo|rustc|criterion' | grep -v 'jit-server' | grep -v "fieldmatrix_gemm" | grep -v "pgrep" | grep -qv "^[[:space:]]*$"; then
        echo "ERR: trial $trial: competing cargo/rustc/criterion process detected — aborting" >&2
        echo "     Running processes:" >&2
        pgrep -af 'cargo|rustc|criterion' | grep -v 'jit-server' | grep -v "fieldmatrix_gemm" >&2 || true
        exit 1
    fi
}

snapshot_trial() {
    local snap_file="$1"
    local trial="$2"
    local route_label="$3"
    : > "$snap_file"
    # Capture GF(251) (route-A target at n>=512) and non-regression controls
    # GF(7), GF(31), GF(127) at the criterion cells.
    for P in 7 31 127 251; do
        for N in 64 256 1024; do
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
    echo "[e8a0c47a]   trial $trial ($route_label): $(wc -l < "$snap_file") cells captured" >&2
}

BENCH_BIN="$(build_bench)"

# Filter pattern matching all four primes and three sizes.
FILTER='gemm/Fp_(7|31|127|251)/Fp_(7|31|127|251)/(64|256|1024)$'

# ─────────────────────────────────────────────────────────────────────────────
# Phase: production default (env var unset, AtomicBool=false).
# GF(251)/n>=512 routes to route A via select_f32_path.
# All other cells use Candidate C.
# ─────────────────────────────────────────────────────────────────────────────
echo "[e8a0c47a] === Production-default dispatch (env var unset) ===" >&2
for T in $(seq 1 "$NTRIALS"); do
    if [[ -s "$TRIAL_DIR/PD_trial${T}.json" ]]; then
        echo "[e8a0c47a] trial $T already snapshotted — skipping" >&2
        continue
    fi
    quiet_host_check "$T"
    echo "[e8a0c47a] trial $T / $NTRIALS …" >&2
    unset GF2_GF251_ROUTE_A GF2_GF251_ROUTE_C
    taskset -c 6-11 nice -n -5 "$BENCH_BIN" "$FILTER" --bench \
        > "$TRIAL_DIR/PD_trial${T}.log" 2>&1 \
        || { echo "trial $T failed (see $TRIAL_DIR/PD_trial${T}.log)" >&2; exit 1; }
    snapshot_trial "$TRIAL_DIR/PD_trial${T}.json" "$T" "production_default"
done

# ─────────────────────────────────────────────────────────────────────────────
# Aggregate + non-regression comparison.
# ─────────────────────────────────────────────────────────────────────────────
echo "[e8a0c47a] Aggregating $NTRIALS trials…" >&2

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
raw_csv = os.path.join(out_dir, f"{pfx}-post-refactor.csv")
with open(raw_csv, "w", newline="") as fh:
    w = csv.writer(fh)
    w.writerow(["prime", "n", "trial", "route", "gop_s"])
    for (prime, n, route), vals in sorted(buckets.items()):
        for trial, gop_s in sorted(vals):
            w.writerow([prime, n, trial, route, f"{gop_s:.4f}"])
print(f"WROTE {raw_csv}")

# Aggregate CSV
agg_csv = os.path.join(out_dir, f"{pfx}-post-refactor-aggregate.csv")
with open(agg_csv, "w", newline="") as fh:
    w = csv.writer(fh)
    w.writerow(["prime", "n", "route", "median_gop_s", "q1", "q3", "iqr", "min", "max"])
    for (prime, n, route), vals in sorted(buckets.items()):
        mn, q1, med, q3, mx, iqr = agg(vals)
        w.writerow([prime, n, route,
                    f"{med:.3f}", f"{q1:.3f}", f"{q3:.3f}",
                    f"{iqr:.3f}", f"{mn:.3f}", f"{mx:.3f}"])
print(f"WROTE {agg_csv}")

# Non-regression comparison vs 41096af5 baseline.
# Baseline numbers from dev/bench_results/2026-05-25-41096af5-post-wire-in-aggregate.csv
BASELINE = {
    (7,   256): 70.746,
    (7,  1024): 75.791,
    (31,  256): 69.996,
    (31, 1024): 76.509,
    (127, 256): 69.804,
    (127,1024): 76.191,
    (251,  64): 31.521,
    (251, 256): 69.926,
    (251,1024): 94.425,
}
FFLAS = {(251, 1024): 138.32}
THRESHOLD_RATIO = 0.667  # GF(251)/n=1024 vs fflas minimum

print()
print(f"{'prime':>6} {'n':>5} {'route':>18} {'median':>8} {'IQR':>7} {'baseline':>9} {'delta%':>8} {'vs_fflas':>10}")
print("-" * 80)
all_pass = True
for (prime, n, route), vals in sorted(buckets.items()):
    _, q1, med, q3, _, iqr = agg(vals)
    base = BASELINE.get((prime, n))
    delta_str = ""
    if base is not None:
        delta = (med - base) / base * 100.0
        delta_str = f"{delta:+.1f}%"
        if abs(delta) > 5.0:
            delta_str += " FAIL(>5%)"
            all_pass = False
    ratio_str = ""
    if (prime, n) in FFLAS:
        ratio = med / FFLAS[(prime, n)]
        ratio_str = f"{ratio:.3f}"
        if ratio < THRESHOLD_RATIO:
            ratio_str += " FAIL(<0.667)"
            all_pass = False
    base_str = f"{base:>9.3f}" if base is not None else f"{'N/A':>9}"
    print(f"{prime:>6} {n:>5} {route:>18} {med:>8.2f} {iqr:>7.3f} {base_str} {delta_str:>8} {ratio_str:>10}")

print()
if all_pass:
    print("NON-REGRESSION: PASS — all cells within ±5% of 41096af5 baseline; GF(251)/n=1024 ratio >= 0.667")
else:
    print("NON-REGRESSION: FAIL — see FAIL markers above")
    sys.exit(1)
PY

echo "[e8a0c47a] Done. Results in $RESULTS_DIR/${DATE_PREFIX}-post-refactor*.csv" >&2
