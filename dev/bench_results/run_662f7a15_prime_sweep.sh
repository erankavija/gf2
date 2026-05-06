#!/usr/bin/env bash
# 662f7a15 small-prime sweep bench driver.
#
# Runs N=5 sequential criterion bench trials for both Candidate F (current
# select_f32_path == true) and Candidate C (select_f32_path forced to false)
# across the small-prime sweep:
#   GF(7), GF(11), GF(13), GF(17), GF(19), GF(23), GF(29), GF(31),
#   GF(127), GF(241), GF(251) at n ∈ {256, 1024}.
#
# Per-trial isolation:
#   * taskset -c 6-11 pins to CCX1 (cores 6-11). The parent shell lives
#     on CCX0 by default, avoiding cross-CCX cache traffic.
#   * nice -n -5 raises priority (non-privileged; falls back silently).
#   * Trials run sequentially, never in parallel.
#
# Output:
#   * dev/bench_results/prime_sweep_trials/  — per-trial JSON snapshots
#   * dev/bench_results/2026-05-06-662f7a15-prime-sweep.csv — raw rows
#   * dev/bench_results/2026-05-06-662f7a15-prime-sweep-aggregate.csv
#   * dev/bench_results/2026-05-06-662f7a15-prime-sweep.md — summary
#
# Usage:
#   bash dev/bench_results/run_662f7a15_prime_sweep.sh

set -euo pipefail

WORKTREE_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
BENCH_BIN_NAME=fieldmatrix_gemm
TRIAL_DIR="$WORKTREE_ROOT/dev/bench_results/prime_sweep_trials"
RESULTS_DIR="$WORKTREE_ROOT/dev/bench_results"
DATE_PREFIX="2026-05-06-662f7a15"
NTRIALS=5

SMALL_PRIMES=(7 11 13 17 19 23 29 31 127 241 251)
SIZES=(256 1024)

mkdir -p "$TRIAL_DIR"

# ─────────────────────────────────────────────────────────────────────────────
# Helper: build bench binary and return its path.
# ─────────────────────────────────────────────────────────────────────────────
build_bench() {
    local label="$1"
    echo "[prime_sweep] Building bench ($label)…" >&2
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
    echo "[prime_sweep] Bench binary: $bin" >&2
    echo "$bin"
}

# ─────────────────────────────────────────────────────────────────────────────
# Helper: snapshot criterion estimates.json for the given primes + sizes.
# ─────────────────────────────────────────────────────────────────────────────
snapshot_trial() {
    local snap_file="$1"
    local trial="$2"
    local kernel_path="$3"
    : > "$snap_file"
    for P in "${SMALL_PRIMES[@]}"; do
        for N in "${SIZES[@]}"; do
            local est="$WORKTREE_ROOT/target/criterion/gemm_Fp_${P}/Fp_${P}/${N}/new/estimates.json"
            # GF(7) and GF(251) at full SQUARE_SIZES also have 64/4096 dirs;
            # we only care about 256 and 1024.
            if [[ -f "$est" ]]; then
                python3 -c "
import json, sys
data = json.load(open('$est'))
print(json.dumps({'trial': $trial, 'prime': $P, 'n': $N,
                  'kernel_path': '$kernel_path', 'estimates': data}))
" >> "$snap_file"
            fi
        done
    done
    echo "[prime_sweep]   trial $trial ($kernel_path): $(wc -l < "$snap_file") cells captured" >&2
}

# ─────────────────────────────────────────────────────────────────────────────
# Build filter regex for criterion.
# ─────────────────────────────────────────────────────────────────────────────
PRIME_ALT="$(IFS='|'; echo "${SMALL_PRIMES[*]}")"
SIZE_ALT="$(IFS='|'; echo "${SIZES[*]}")"
FILTER="gemm/Fp_(${PRIME_ALT})/Fp_(${PRIME_ALT})/(${SIZE_ALT})$"

# ─────────────────────────────────────────────────────────────────────────────
# Phase 1: F-path (select_f32_path == true, current code).
# ─────────────────────────────────────────────────────────────────────────────
echo "[prime_sweep] === Phase 1: Candidate F (f32-FMA cascade, select_f32_path=true) ===" >&2

BENCH_BIN_F="$(build_bench "F-path")"

for T in $(seq 1 "$NTRIALS"); do
    echo "[prime_sweep] F-path trial $T / $NTRIALS …" >&2
    taskset -c 6-11 nice -n -5 "$BENCH_BIN_F" "$FILTER" --bench \
        > "$TRIAL_DIR/F_trial${T}.log" 2>&1 \
        || { echo "F-path trial $T failed (see $TRIAL_DIR/F_trial${T}.log)" >&2; exit 1; }
    snapshot_trial "$TRIAL_DIR/F_trial${T}.json" "$T" "F"
done

# ─────────────────────────────────────────────────────────────────────────────
# Phase 2: C-path (force select_f32_path == false by patching simd_ops.rs).
# ─────────────────────────────────────────────────────────────────────────────
echo "[prime_sweep] === Phase 2: Candidate C (byte-lane Barrett, select_f32_path=false) ===" >&2

SIMD_OPS="$WORKTREE_ROOT/crates/gf2-core/src/gfp/simd_ops.rs"
SIMD_OPS_BACKUP="${SIMD_OPS}.bak_prime_sweep"

# Save original and patch select_f32_path to return false.
cp "$SIMD_OPS" "$SIMD_OPS_BACKUP"

# Patch: replace the body of select_f32_path to always return false.
python3 - "$SIMD_OPS" <<'PYPATCH'
import re, sys
path = sys.argv[1]
text = open(path).read()
# Replace the fn body: `fp_small_enabled_const::<P>()` → `false`
patched = text.replace(
    "    fp_small_enabled_const::<P>()\n}",
    "    false\n}",
    1  # replace only first occurrence (select_f32_path body)
)
if patched == text:
    print("ERR: patch target not found in simd_ops.rs", file=sys.stderr)
    sys.exit(1)
open(path, 'w').write(patched)
print("[prime_sweep] simd_ops.rs patched: select_f32_path => false", file=sys.stderr)
PYPATCH

# Ensure cleanup on exit (even on error) restores the original.
cleanup_simd_ops() {
    if [[ -f "$SIMD_OPS_BACKUP" ]]; then
        cp "$SIMD_OPS_BACKUP" "$SIMD_OPS"
        rm -f "$SIMD_OPS_BACKUP"
        echo "[prime_sweep] simd_ops.rs restored." >&2
    fi
}
trap cleanup_simd_ops EXIT

BENCH_BIN_C="$(build_bench "C-path")"

for T in $(seq 1 "$NTRIALS"); do
    echo "[prime_sweep] C-path trial $T / $NTRIALS …" >&2
    taskset -c 6-11 nice -n -5 "$BENCH_BIN_C" "$FILTER" --bench \
        > "$TRIAL_DIR/C_trial${T}.log" 2>&1 \
        || { echo "C-path trial $T failed (see $TRIAL_DIR/C_trial${T}.log)" >&2; exit 1; }
    snapshot_trial "$TRIAL_DIR/C_trial${T}.json" "$T" "C"
done

# Restore immediately (cleanup_simd_ops will also fire on EXIT but be idempotent).
cleanup_simd_ops
trap - EXIT

# ─────────────────────────────────────────────────────────────────────────────
# Aggregate across trials → CSV.
# ─────────────────────────────────────────────────────────────────────────────
echo "[prime_sweep] Aggregating $NTRIALS trials (F + C paths)…" >&2

python3 - "$TRIAL_DIR" "$RESULTS_DIR" "$NTRIALS" "$DATE_PREFIX" \
    "${SMALL_PRIMES[*]}" "${SIZES[*]}" <<'PY'
import json, os, sys, csv, statistics, math

trial_dir  = sys.argv[1]
out_dir    = sys.argv[2]
ntrials    = int(sys.argv[3])
pfx        = sys.argv[4]
primes     = list(map(int, sys.argv[5].split()))
sizes      = list(map(int, sys.argv[6].split()))

# ---- load all trials --------------------------------------------------------
records = []
for kpath in ("F", "C"):
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
    # Throughput = ops/sec; ops = 2*n^3 (mult-add pairs).
    # Criterion median is in nanoseconds.
    # ops/ns = ops / (ns * 1e-9) / 1e9 = ops/ns. (Gop/s = ops/ns directly.)
    return 2 * (n ** 3) / ns

# ---- group by (prime, n, kernel_path) → list of Gop/s ----------------------
buckets = {}
for r in records:
    key = (r["prime"], r["n"], r["kernel_path"])
    ns = r["estimates"]["median"]["point_estimate"]
    buckets.setdefault(key, []).append((r["trial"], gops(ns, r["n"])))

def agg(vals):
    """Return (min, q1, median, q3, max, iqr) for a sorted list."""
    s = sorted(v for _, v in vals)
    n = len(s)
    med = statistics.median(s)
    if n >= 4:
        q1 = s[1]
        q3 = s[-2]
    else:
        q1 = s[0]
        q3 = s[-1]
    return min(s), q1, med, q3, max(s), q3 - q1

# ---- raw CSV ----------------------------------------------------------------
raw_rows = []
for (prime, n, kpath), vals in sorted(buckets.items()):
    for trial, gop_s in sorted(vals):
        raw_rows.append({
            "prime": prime, "n": n, "trial": trial,
            "kernel_path": kpath, "gop_s": f"{gop_s:.4f}"
        })

raw_csv = os.path.join(out_dir, f"{pfx}-prime-sweep.csv")
with open(raw_csv, "w", newline="") as fh:
    w = csv.DictWriter(fh, fieldnames=["prime","n","trial","kernel_path","gop_s"])
    w.writeheader()
    for row in raw_rows:
        w.writerow(row)
print(f"WROTE {raw_csv}")

# ---- aggregate CSV ----------------------------------------------------------
agg_rows = []
for (prime, n, kpath), vals in sorted(buckets.items()):
    mn, q1, med, q3, mx, iqr = agg(vals)
    agg_rows.append({
        "prime": prime, "n": n, "kernel_path": kpath,
        "median_gop_s": f"{med:.3f}",
        "q1": f"{q1:.3f}", "q3": f"{q3:.3f}",
        "iqr": f"{iqr:.3f}",
        "min": f"{mn:.3f}", "max": f"{mx:.3f}",
    })

agg_csv = os.path.join(out_dir, f"{pfx}-prime-sweep-aggregate.csv")
with open(agg_csv, "w", newline="") as fh:
    w = csv.DictWriter(fh, fieldnames=["prime","n","kernel_path",
                                        "median_gop_s","q1","q3","iqr","min","max"])
    w.writeheader()
    for row in agg_rows:
        w.writerow(row)
print(f"WROTE {agg_csv}")

# ---- console table ----------------------------------------------------------
print()
print(f"{'prime':>6}  {'n':>5}  {'F-med':>7}  {'F-Q1':>7}  {'F-Q3':>7} "
      f" {'C-med':>7}  {'C-Q1':>7}  {'C-Q3':>7}  {'F/C':>5}  verdict")
print("-" * 85)

verdict_map = {}
for prime in primes:
    for n in sizes:
        fkey = (prime, n, "F")
        ckey = (prime, n, "C")
        if fkey not in buckets or ckey not in buckets:
            continue
        _, fq1, fmed, fq3, _, _ = agg(buckets[fkey])
        _, cq1, cmed, cq3, _, _ = agg(buckets[ckey])
        ratio = fmed / cmed if cmed > 0 else math.nan
        # IQR-aware confidence
        if fq1 > cq3:
            verdict = "F_WINS"
        elif fq3 < cq1:
            verdict = "C_WINS"
        else:
            verdict = "OVERLAP"
        verdict_map[(prime, n)] = (fmed, fq1, fq3, cmed, cq1, cq3, ratio, verdict)
        print(f"{prime:>6}  {n:>5}  {fmed:>7.2f}  {fq1:>7.2f}  {fq3:>7.2f} "
              f" {cmed:>7.2f}  {cq1:>7.2f}  {cq3:>7.2f}  {ratio:>5.3f}  {verdict}")

# ---- threshold analysis -----------------------------------------------------
print()
print("=== THRESHOLD ANALYSIS ===")
# Find lowest prime where F_WINS at BOTH n=256 and n=1024.
crossover = None
for prime in primes:
    v256  = verdict_map.get((prime,  256), ("?",)*8)[-1]
    v1024 = verdict_map.get((prime, 1024), ("?",)*8)[-1]
    if v256 == "F_WINS" and v1024 == "F_WINS":
        crossover = prime
        break

if crossover is not None:
    print(f"N_thresh_prime (F_WINS at both n=256 and n=1024): {crossover}")
else:
    # fallback: lowest prime where F_WINS at n=1024
    for prime in primes:
        v1024 = verdict_map.get((prime, 1024), ("?",)*8)[-1]
        if v1024 == "F_WINS":
            print(f"N_thresh_prime (F_WINS at n=1024 only): {prime}")
            break
    else:
        print("N_thresh_prime: NONE (C wins or overlaps everywhere)")

PY

echo "[prime_sweep] Done. Results in $RESULTS_DIR/${DATE_PREFIX}-prime-sweep*.csv" >&2
