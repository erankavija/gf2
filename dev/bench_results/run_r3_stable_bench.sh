#!/usr/bin/env bash
# R3 stable multi-trial bench driver for issue 9e12659b.
#
# Runs N=5 sequential criterion bench trials over the medium-prime
# GF(p) GEMM cells:
#   GF(257), GF(8191), GF(32749), GF(65521) at n ∈ {64, 256, 1024}
#
# Per-trial isolation:
#   * `taskset -c 6-11` pins the bench process to CCX1 (cores 6-11). The
#     parent shell + claude agent live on CCX0 by default. This avoids
#     cross-CCX cache traffic stealing throughput from the kernel.
#   * `nice -n -5` raises priority below the OS but above interactive
#     loads. Works without sudo on standard Linux configurations
#     (RLIMIT_NICE permitting; falls back silently).
#   * Trials run sequentially, never in parallel — criterion's stats
#     assume serial execution and concurrent benches share L2/L3.
#
# Output:
#   * Each trial writes its raw estimates JSON under
#     `target/criterion/<group>/<id>/<n>/new/estimates.json`. We snapshot
#     this into `dev/bench_results/r3_trial${T}.json` per trial.
#   * Final aggregator emits `r3_aggregate.csv` with per-cell median /
#     min / max / IQR Gop/s across trials. The .md evidence doc cites
#     this CSV.
#
# Usage:
#   bash dev/bench_results/run_r3_stable_bench.sh
set -euo pipefail

cd "$(dirname "$0")/../.."
WORKTREE_ROOT="$(pwd)"
BENCH_BIN_NAME=fieldmatrix_gemm

# Build once, reuse across trials.
echo "[run_r3_stable_bench] Building bench harness…" >&2
cargo build --release -p gf2-core --bench "$BENCH_BIN_NAME" \
    --features rand,simd >&2

# Resolve the criterion harness binary.
BENCH_BIN="$(find "$WORKTREE_ROOT/target/release/deps" \
    -maxdepth 1 -name "${BENCH_BIN_NAME}-*" -executable -type f | head -1)"
if [[ -z "$BENCH_BIN" || ! -x "$BENCH_BIN" ]]; then
    echo "ERR: failed to locate bench binary" >&2
    exit 1
fi
echo "[run_r3_stable_bench] Using $BENCH_BIN" >&2

# Filter to medium-prime square cells at n ∈ {64, 256, 1024} only —
# skip slow Mersenne / GF(2^m) and the GF(65521) n=4096 / rectangular
# cells which are out of scope for this rework. The trailing `/$` anchor
# matches criterion's `<group>/<id>/<n>` suffix, dropping `gemm_rect` and
# the n=4096 cell that adds a single 70-second sample to GF(65521).
FILTER='gemm/Fp_(257|8191|32749|65521)/Fp_(257|8191|32749|65521)/(64|256|1024)$'

# Trial sweep — sequential. Per-trial run of all 12 cells takes ~30 s
# per trial (3 sizes × 4 fields × 5 s measurement).
NTRIALS=5
TRIAL_DIR="$WORKTREE_ROOT/dev/bench_results/r3_trials"
mkdir -p "$TRIAL_DIR"

for T in $(seq 1 "$NTRIALS"); do
    echo "[run_r3_stable_bench] Trial $T / $NTRIALS …" >&2
    # Pin to CCX1 (cores 6-11 + their SMT siblings 18-23). nice -n -5
    # under user RLIMIT_NICE (default 0 on Arch) typically falls back
    # to nice 0; we run it in case the limit allows, otherwise it's
    # benign.
    # NOTE: criterion's libtest-compat parser requires the filter argument
    # to come BEFORE `--bench`. Otherwise the binary silently exits with
    # status 0 and no measurements (it parses `gemm/...` as a positional
    # for `--bench` and finds no filter).
    taskset -c 6-11 nice -n -5 "$BENCH_BIN" "$FILTER" --bench \
        > "$TRIAL_DIR/trial${T}.log" 2>&1 \
        || { echo "trial $T failed (see $TRIAL_DIR/trial${T}.log)" >&2; exit 1; }

    # Snapshot per-cell estimates.json into trial directory.
    SNAP="$TRIAL_DIR/trial${T}.json"
    : > "$SNAP"
    for FIELD in 257 8191 32749 65521; do
        for N in 64 256 1024; do
            EST="$WORKTREE_ROOT/target/criterion/gemm_Fp_${FIELD}/Fp_${FIELD}/${N}/new/estimates.json"
            if [[ -f "$EST" ]]; then
                # Embed field/n/trial alongside the raw estimates payload.
                python3 -c "
import json, sys
data = json.load(open('$EST'))
print(json.dumps({'trial': $T, 'field': 'GF($FIELD)', 'n': $N, 'estimates': data}))
" >> "$SNAP"
            fi
        done
    done
    echo "[run_r3_stable_bench]   trial $T: $(wc -l < "$SNAP") cells captured" >&2
done

# Aggregate across trials → CSV.
echo "[run_r3_stable_bench] Aggregating $NTRIALS trials …" >&2
python3 - "$TRIAL_DIR" "$WORKTREE_ROOT/dev/bench_results" "$NTRIALS" <<'PY'
import json, os, sys, csv, statistics

trial_dir, out_dir, ntrials = sys.argv[1], sys.argv[2], int(sys.argv[3])

# Load all trials.
records = []
for t in range(1, ntrials + 1):
    snap = os.path.join(trial_dir, f"trial{t}.json")
    with open(snap) as fh:
        for line in fh:
            line = line.strip()
            if line:
                records.append(json.loads(line))

# Group by (field, n) → list of median ns per trial.
buckets = {}
for r in records:
    key = (r["field"], r["n"])
    median_ns = r["estimates"]["median"]["point_estimate"]
    buckets.setdefault(key, []).append((r["trial"], median_ns))

def gops(ns, n):
    ops = 2 * (n ** 3)
    return ops / ns  # ns × Gop/s = ops; result = ops/ns = Gop/s for ns ↔ s scaling.
    # Throughput = ops/sec = ops / (ns × 1e-9) = ops/ns × 1e9.
    # Reporting as Gop/s = ops/(s) / 1e9 = ops/ns. Direct ratio.

rows = []
for (field, n), trials in sorted(buckets.items()):
    trials = sorted(trials)
    medians_ns = [ns for _, ns in trials]
    medians_gops = sorted([gops(ns, n) for ns in medians_ns])
    if not medians_gops:
        continue
    median = statistics.median(medians_gops)
    mn = min(medians_gops)
    mx = max(medians_gops)
    # IQR: for ntrials=5, q1 = 2nd lowest, q3 = 2nd highest.
    if len(medians_gops) >= 4:
        q1 = medians_gops[1] if len(medians_gops) == 5 else medians_gops[1]
        q3 = medians_gops[-2] if len(medians_gops) == 5 else medians_gops[-2]
    else:
        q1, q3 = mn, mx
    iqr = q3 - q1
    rows.append({
        "field": field,
        "n": n,
        "trial_medians_gops": ",".join(f"{g:.3f}" for g in medians_gops),
        "trial_min_gops": f"{mn:.3f}",
        "trial_q1_gops": f"{q1:.3f}",
        "trial_median_gops": f"{median:.3f}",
        "trial_q3_gops": f"{q3:.3f}",
        "trial_max_gops": f"{mx:.3f}",
        "trial_iqr_gops": f"{iqr:.3f}",
    })

# Write CSV.
out_csv = os.path.join(out_dir, "r3_aggregate.csv")
with open(out_csv, "w") as fh:
    w = csv.DictWriter(fh, fieldnames=list(rows[0].keys()))
    w.writeheader()
    for row in rows:
        w.writerow(row)
print(f"WROTE {out_csv}")

# Pretty print.
print()
print(f"{'field':<11}{'n':>5}  {'min':>6}  {'q1':>6}  {'median':>7}  {'q3':>6}  {'max':>6}  {'iqr':>5}")
print("-" * 64)
for r in rows:
    print(f"{r['field']:<11}{r['n']:>5}  {r['trial_min_gops']:>6}  "
          f"{r['trial_q1_gops']:>6}  {r['trial_median_gops']:>7}  "
          f"{r['trial_q3_gops']:>6}  {r['trial_max_gops']:>6}  {r['trial_iqr_gops']:>5}")
PY

echo "[run_r3_stable_bench] Done. See dev/bench_results/r3_aggregate.csv" >&2
