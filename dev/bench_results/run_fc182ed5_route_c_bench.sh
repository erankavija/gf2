#!/usr/bin/env bash
# fc182ed5 route-C (pure-integer Goto/BLIS-style panelized GF(251)
# micro-kernel) bench driver.
#
# Runs N=5 sequential criterion bench trials for the route-C path at
# GF(251)/n ∈ {64, 256, 1024} on the CCX1-pinned Zen-3 reference host.
# The route-C path is opted into via the launcher-convenience env var
# `GF2_GF251_ROUTE_C=1`, which the GF(251) bench function reads (safe
# `std::env::var`) and dispatches to the safe
# `gf2_core::gfp::simd_ops::set_route_c_gf251_enabled(true)` setter.
# All other primes / cells use the default Candidate C dispatch and
# share a single bench binary.
#
# Per-trial isolation:
#   * taskset -c 6-11 pins to CCX1 (cores 6-11). The parent shell lives
#     on CCX0 by default, avoiding cross-CCX cache traffic.
#   * nice -n -5 raises priority (non-privileged; falls back silently).
#   * Trials run sequentially, never in parallel.
#
# Output:
#   * dev/bench_results/2026-05-24-fc182ed5-route-c/      — per-trial JSON snapshots
#   * dev/bench_results/2026-05-24-fc182ed5-route-c-integer-panel.csv
#   * dev/bench_results/2026-05-24-fc182ed5-route-c-integer-panel-aggregate.csv
#
# Usage:
#   bash dev/bench_results/run_fc182ed5_route_c_bench.sh

set -euo pipefail

WORKTREE_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
BENCH_BIN_NAME=fieldmatrix_gemm
TRIAL_DIR="$WORKTREE_ROOT/dev/bench_results/2026-05-24-fc182ed5-route-c"
RESULTS_DIR="$WORKTREE_ROOT/dev/bench_results"
DATE_PREFIX="2026-05-24-fc182ed5"
NTRIALS=5

mkdir -p "$TRIAL_DIR"

build_bench() {
    echo "[route-c] Building bench…" >&2
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
    echo "[route-c] Bench binary: $bin" >&2
    echo "$bin"
}

snapshot_trial() {
    local snap_file="$1"
    local trial="$2"
    local route_label="$3"
    : > "$snap_file"
    # Capture GF(251) (route-C target) and non-regression controls
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
    echo "[route-c]   trial $trial ($route_label): $(wc -l < "$snap_file") cells captured" >&2
}

BENCH_BIN="$(build_bench)"

# Filter pattern matching all four primes and the criterion sizes
# {64, 256, 1024}. (GF(7)/GF(127)/GF(251) bench groups expose
# n=64 only when SQUARE_SIZES is used — Fp_251 uses SQUARE_SIZES which
# includes 64. GF(7) uses SQUARE_SIZES too. GF(31) uses
# SQUARE_SIZES_GF31_SMALL_N which has 64. GF(127) uses
# SQUARE_SIZES_SMALL_PRIME which omits 64 — its n=64 cell is absent
# but the n=256 / n=1024 controls remain.)
FILTER='gemm/Fp_(7|31|127|251)/Fp_(7|31|127|251)/(64|256|1024)$'

# ─────────────────────────────────────────────────────────────────────────
# Phase 1: route-C enabled (GF(251) uses panelized integer kernel).
# Non-GF(251) cells still use Candidate C (env var is GF(251)-scoped).
# ─────────────────────────────────────────────────────────────────────────
echo "[route-c] === Phase 1: route-C enabled (GF2_GF251_ROUTE_C=1) ===" >&2
for T in $(seq 1 "$NTRIALS"); do
    if [[ -s "$TRIAL_DIR/RC_trial${T}.json" ]]; then
        echo "[route-c] route-C trial $T already snapshotted — skipping" >&2
        continue
    fi
    echo "[route-c] route-C trial $T / $NTRIALS …" >&2
    GF2_GF251_ROUTE_C=1 taskset -c 6-11 nice -n -5 "$BENCH_BIN" "$FILTER" --bench \
        > "$TRIAL_DIR/RC_trial${T}.log" 2>&1 \
        || { echo "route-C trial $T failed (see $TRIAL_DIR/RC_trial${T}.log)" >&2; exit 1; }
    snapshot_trial "$TRIAL_DIR/RC_trial${T}.json" "$T" "route_c"
done

# ─────────────────────────────────────────────────────────────────────────
# Phase 2: default (Candidate C for GF(251); same baseline for controls).
# ─────────────────────────────────────────────────────────────────────────
echo "[route-c] === Phase 2: default Candidate C (env var unset) ===" >&2
for T in $(seq 1 "$NTRIALS"); do
    if [[ -s "$TRIAL_DIR/C_trial${T}.json" ]]; then
        echo "[route-c] default trial $T already snapshotted — skipping" >&2
        continue
    fi
    echo "[route-c] default trial $T / $NTRIALS …" >&2
    unset GF2_GF251_ROUTE_C
    taskset -c 6-11 nice -n -5 "$BENCH_BIN" "$FILTER" --bench \
        > "$TRIAL_DIR/C_trial${T}.log" 2>&1 \
        || { echo "default trial $T failed (see $TRIAL_DIR/C_trial${T}.log)" >&2; exit 1; }
    snapshot_trial "$TRIAL_DIR/C_trial${T}.json" "$T" "default"
done

# ─────────────────────────────────────────────────────────────────────────
# Aggregate.
# ─────────────────────────────────────────────────────────────────────────
echo "[route-c] Aggregating $NTRIALS trials (route-C + default)…" >&2

python3 - "$TRIAL_DIR" "$RESULTS_DIR" "$NTRIALS" "$DATE_PREFIX" <<'PY'
import json, os, sys, csv, statistics

trial_dir = sys.argv[1]
out_dir   = sys.argv[2]
ntrials   = int(sys.argv[3])
pfx       = sys.argv[4]

records = []
for kpath in ("RC", "C"):
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
raw_csv = os.path.join(out_dir, f"{pfx}-route-c-integer-panel.csv")
with open(raw_csv, "w", newline="") as fh:
    w = csv.writer(fh)
    w.writerow(["prime", "n", "trial", "route", "gop_s"])
    for (prime, n, route), vals in sorted(buckets.items()):
        for trial, gop_s in sorted(vals):
            w.writerow([prime, n, trial, route, f"{gop_s:.4f}"])
print(f"WROTE {raw_csv}")

# Aggregate CSV
agg_csv = os.path.join(out_dir, f"{pfx}-route-c-integer-panel-aggregate.csv")
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

echo "[route-c] Done. Results in $RESULTS_DIR/${DATE_PREFIX}-route-c-integer-panel*.csv" >&2
