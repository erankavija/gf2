#!/usr/bin/env bash
# 91429c1c route-B (BLAS-backed GF(251) cascade) bench driver.
#
# Runs N=5 sequential trials of the standalone harness
# `dev/research/blas_sgemm_gf251` at GF(251)/n ∈ {64, 256, 1024} on
# the CCX1-pinned Zen-3 reference host. Each trial is a separate
# process invocation so the BLAS dispatch tables, OpenMP team state,
# and caches are warm-from-zero per trial.
#
# Single-threaded BLAS is enforced two ways:
#   - `OPENBLAS_NUM_THREADS=1` in the env (read by OpenBLAS at
#     library init);
#   - the harness itself calls `openblas_set_num_threads(1)` before
#     the warmup phase.
#
# Per-trial isolation:
#   * `taskset -c 6-11` pins to CCX1 (cores 6-11). Parent shell on
#     CCX0 by default.
#   * `nice -n -5` raises priority (non-privileged; silent fall-back).
#   * Trials run sequentially, never in parallel.
#
# Output:
#   * dev/bench_results/2026-05-24-91429c1c-route-b-blas/ — per-trial logs
#   * dev/bench_results/2026-05-24-91429c1c-route-b-blas.csv (raw)
#   * dev/bench_results/2026-05-24-91429c1c-route-b-blas-aggregate.csv
#
# Usage:
#   bash dev/bench_results/run_91429c1c_route_b_bench.sh

set -euo pipefail

WORKTREE_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
PROTO_DIR="$WORKTREE_ROOT/dev/research/blas_sgemm_gf251"
TRIAL_DIR="$WORKTREE_ROOT/dev/bench_results/2026-05-24-91429c1c-route-b-blas"
RESULTS_DIR="$WORKTREE_ROOT/dev/bench_results"
DATE_PREFIX="2026-05-24-91429c1c"
NTRIALS=5
SIZES="64,256,1024"
INNER=15      # inner iterations per cell, take median
WARMUP=3

mkdir -p "$TRIAL_DIR"

echo "[route-b] Building prototype harness in release mode..." >&2
(cd "$PROTO_DIR" && cargo build --release --bin bench_blas_gf251 >&2)
BENCH_BIN="$PROTO_DIR/target/release/bench_blas_gf251"
if [[ ! -x "$BENCH_BIN" ]]; then
    echo "ERR: bench binary not built at $BENCH_BIN" >&2
    exit 1
fi
echo "[route-b] Bench binary: $BENCH_BIN" >&2

# Provider attestation.
echo "[route-b] BLAS provider:" >&2
ldd "$BENCH_BIN" 2>/dev/null | grep -iE "openblas|blas" >&2 || true
echo "[route-b] OpenBLAS config:" >&2
python3 -c "
import ctypes
libob = ctypes.CDLL('libopenblas.so.0')
libob.openblas_get_config.restype = ctypes.c_char_p
print('  ' + libob.openblas_get_config().decode())
" >&2

# ─────────────────────────────────────────────────────────────────────────
# Five sequential trials.
# ─────────────────────────────────────────────────────────────────────────
echo "[route-b] === Running $NTRIALS trials at n in {$SIZES} ===" >&2
for T in $(seq 1 "$NTRIALS"); do
    if [[ -s "$TRIAL_DIR/trial${T}.csv" ]]; then
        echo "[route-b] trial $T already done — skipping" >&2
        continue
    fi
    echo "[route-b] trial $T / $NTRIALS ..." >&2
    OPENBLAS_NUM_THREADS=1 OMP_NUM_THREADS=1 \
      taskset -c 6-11 nice -n -5 "$BENCH_BIN" \
        --trial "$T" --sizes "$SIZES" --inner "$INNER" --warmup "$WARMUP" \
        > "$TRIAL_DIR/trial${T}.csv" 2> "$TRIAL_DIR/trial${T}.log" \
        || { echo "trial $T failed (see $TRIAL_DIR/trial${T}.log)" >&2; exit 1; }
done

# ─────────────────────────────────────────────────────────────────────────
# Aggregate.
# ─────────────────────────────────────────────────────────────────────────
echo "[route-b] Aggregating $NTRIALS trials..." >&2

python3 - "$TRIAL_DIR" "$RESULTS_DIR" "$NTRIALS" "$DATE_PREFIX" <<'PY'
import csv, os, statistics, sys

trial_dir = sys.argv[1]
out_dir   = sys.argv[2]
ntrials   = int(sys.argv[3])
pfx       = sys.argv[4]

records = []
for t in range(1, ntrials + 1):
    fp = os.path.join(trial_dir, f"trial{t}.csv")
    if not os.path.exists(fp):
        continue
    with open(fp) as fh:
        for row in csv.DictReader(fh):
            records.append({
                "trial": int(row["trial"]),
                "route": row["route"],
                "prime": int(row["prime"]),
                "n": int(row["n"]),
                "median_ns": int(row["median_ns"]),
                "gop_s": float(row["gop_s"]),
            })

# Raw per-trial CSV.
raw_csv = os.path.join(out_dir, f"{pfx}-route-b-blas.csv")
with open(raw_csv, "w", newline="") as fh:
    w = csv.writer(fh)
    w.writerow(["prime", "n", "route", "trial", "median_ns", "gop_s"])
    for r in sorted(records, key=lambda r: (r["prime"], r["n"], r["route"], r["trial"])):
        w.writerow([r["prime"], r["n"], r["route"], r["trial"], r["median_ns"],
                    f"{r['gop_s']:.4f}"])
print(f"WROTE {raw_csv}")

# Aggregate CSV.
def agg(vals):
    s = sorted(vals)
    n = len(s)
    med = statistics.median(s)
    if n >= 4:
        q1, q3 = s[1], s[-2]
    else:
        q1, q3 = s[0], s[-1]
    return min(s), q1, med, q3, max(s), q3 - q1

buckets = {}
for r in records:
    buckets.setdefault((r["prime"], r["n"], r["route"]), []).append(r["gop_s"])

agg_csv = os.path.join(out_dir, f"{pfx}-route-b-blas-aggregate.csv")
with open(agg_csv, "w", newline="") as fh:
    w = csv.writer(fh)
    w.writerow(["prime", "n", "route", "median_gop_s", "q1", "q3", "iqr", "min", "max"])
    for key in sorted(buckets):
        prime, n, route = key
        mn, q1, med, q3, mx, iqr = agg(buckets[key])
        w.writerow([prime, n, route, f"{med:.3f}", f"{q1:.3f}", f"{q3:.3f}",
                    f"{iqr:.3f}", f"{mn:.3f}", f"{mx:.3f}"])
print(f"WROTE {agg_csv}")

# Console summary.
print()
print(f"{'prime':>6} {'n':>5} {'route':>14} {'median':>8} {'IQR':>7}")
print("-" * 50)
for key in sorted(buckets):
    prime, n, route = key
    _, q1, med, q3, _, iqr = agg(buckets[key])
    print(f"{prime:>6} {n:>5} {route:>14} {med:>8.2f} {iqr:>7.3f}")
PY

echo "[route-b] Done. Results in $RESULTS_DIR/${DATE_PREFIX}-route-b-blas*.csv" >&2
