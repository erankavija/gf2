#!/usr/bin/env bash
# permanent-repro.sh
#
# One-command end-to-end reproduction driver for the gf2-algebra-permanent epic
# benchmark artefact (JIT issue 7cd9afdb).
#
# Reproduces dev/benchmarks/gf2_algebra_permanent/ including:
#   S1  — single-thread AVX2 speedup (Criterion bench s1_n36_speedup, n=24/28;
#           offline n=32/36 gated by S1_OFFLINE_N32 / S1_OFFLINE_N36)
#   S2  — parallel scaling 1..12 cores (example parallel_scaling_sweep)
#   S3  — cross-CPU portability (fully automated, no manual step): Part A
#           AVX2 throughput rows are reused (copied, not re-measured) from the
#           S1 CSV; Part B scalar/avx2_sanity rows come from the ~1-min
#           s3_scalar_vs_avx2_sanity example. The script assembles the complete
#           dated s3_cross_cpu CSV deterministically.
#   S5  — GPU-vs-CPU SIMD crossover at M=256 (research harness
#           dev/research/permanent_gpu_crossover; GPU-only, skipped on non-ROCm)
#   S1g — GPU 50x speedup vs reference (research harness
#           dev/research/permanent_gpu_speedup; GPU-only, skipped on non-ROCm)
#   Figures — T27 plot script scripts/plot_permanent_benchmarks.py (all mode)
#   provenance.json — (re)written with current commit + hardware fingerprint
#
# Sister script (separate sweep, do NOT call from here):
#   scripts/perm-uniformity-repro.sh — uniformity sweep (JIT 8e4e19a0)
#
# Usage:
#   bash scripts/permanent-repro.sh
#
# Requirements:
#   - Rust toolchain (MSRV 1.95+) with cargo, cargo-nextest optional
#   - Python 3 (REQUIRED): drives the S3 CSV assembly, the T27 plot step
#     (matplotlib 3.10.x), and the provenance.json rewrite. The script
#     fail-fast aborts if python3 is unavailable.
#   - ROCm / hipcc for GPU steps S5 and S1g (auto-skipped if absent or
#     GF2_PERMANENT_REPRO_SKIP_GPU=1 is set)
#
# Approximate wall-clock (Ryzen 9 5900X, AVX2, GPU gfx1030):
#   Step 1 (workspace build):               ~1-3 min
#   Step 2 (criterion bench, n=24/28):      ~4 min  (x2 for ref + bipedal3)
#   Step 3a (S2 parallel scaling):          ~15 min (n=28/32/36 x 5 threads)
#   Step 3b (S3 sanity, CPU-only):          ~1 min  (n=16/20/24, 5 samples each)
#   Step 4a (S5 GPU crossover, n=24/28):    ~1 hr   (skipped without GPU/ROCm)
#   Step 4b (S1g GPU speedup, n=24..36):    ~2+ hr  (skipped without GPU/ROCm)
#   Step 5  (S1 offline n=32):              ~30 min (skipped unless S1_OFFLINE_N32=1)
#   Step 5  (S1 offline n=36):              ~10 hr  (skipped unless S1_OFFLINE_N36=1)
#   Step 6  (plot figures):                 ~5 s
#   Step 7  (provenance.json):              ~1 s
#
# Environment-variable knobs:
#   GF2_PERMANENT_REPRO_SKIP_GPU=1
#       Skip GPU steps S5 and S1g entirely (even if ROCm is present).
#       Set this on non-ROCm hosts to reproduce only the CPU portion.
#
#   S1_OFFLINE_N32=1
#       Run the offline (single-sample, ~20 min) n=32 timing cells in the S1
#       bench.  Off by default because the Criterion n=24/28 cells are
#       sufficient for most reproduction checks.
#
#   S1_OFFLINE_N36=1
#       Run the offline n=36 timing cell (~10 hr).  Implies S1_OFFLINE_N32=1
#       (n=32 must run first so it appears in the CSV before n=36).  Off by
#       default; only set this for a full publication-grade regeneration.
#
#   SA_DATE=YYYY-MM-DD
#       Override the date embedded in this run's output CSV filenames
#       (default: today's UTC date). Use SA_DATE=2026-05-11 (etc.) if you
#       want the top-level dated CSVs this run writes to carry the same
#       filenames as the committed canonical artefact.
#
#       SA_DATE does NOT affect figure generation: step 6 always stages
#       THIS run's freshly produced dated CSVs into a transient plot-input
#       directory under the exact pinned filenames the (closed-issue) plot
#       script resolves, then runs the plot script against that dir. The
#       figures therefore always reflect this run's data regardless of
#       SA_DATE, and the plot script itself is never modified.
#
# S3 assembly (fully automated — no manual step):
#   s3_cross_cpu-<DATE>.csv is a deterministic two-part assembly the script
#   builds in step 3:
#     Part A — AVX2 throughput rows at n in {24,28,32,36}. These are NOT
#       re-measured. They are copied from the S1 CSV (s1_speedup-<DATE>.csv,
#       falling back to the newest s1_speedup-*.csv present) rows
#       impl=permanent_bipedal3_simd, reformatted into the S3 schema with
#       impl=permanent_bipedal3_avx2 and ratio_vs_avx2=1.000. No S1 n=32/36
#       re-run is triggered.
#     Part B — scalar / avx2_sanity rows at n in {16,20,24}, parsed from the
#       ~1-min s3_scalar_vs_avx2_sanity example's stdout (data lines already in
#       the S3 schema).
#   The script writes the exact `#` provenance header of the committed
#   s3_cross_cpu CSV (date + the S1 source filename actually read are
#   substituted), then Part A rows, then Part B rows (n-ascending,
#   scalar-before-avx2_sanity), so re-runs are byte-stable modulo the Part B
#   wall-clock timing columns (same nondeterminism class as every other S*
#   timing dataset).

set -euo pipefail

# ---------------------------------------------------------------------------
# Repo root detection
# ---------------------------------------------------------------------------

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
cd "${REPO_ROOT}"

ARTEFACT_DIR="dev/benchmarks/gf2_algebra_permanent"

# ---------------------------------------------------------------------------
# GPU detection
# ---------------------------------------------------------------------------

skip_gpu=0
if [[ "${GF2_PERMANENT_REPRO_SKIP_GPU:-0}" == "1" ]]; then
    echo "=== GF2_PERMANENT_REPRO_SKIP_GPU=1 set — GPU steps S5 and S1g will be skipped ==="
    skip_gpu=1
elif ! command -v hipcc &>/dev/null; then
    echo "=== hipcc not found on PATH — GPU steps S5 and S1g will be skipped ==="
    echo "    (Set GF2_PERMANENT_REPRO_SKIP_GPU=1 to suppress this message)"
    skip_gpu=1
fi

# ---------------------------------------------------------------------------
# Offline cell flags
# ---------------------------------------------------------------------------

offline_n32=0
offline_n36=0
if [[ "${S1_OFFLINE_N36:-0}" == "1" ]]; then
    offline_n32=1
    offline_n36=1
elif [[ "${S1_OFFLINE_N32:-0}" == "1" ]]; then
    offline_n32=1
fi

# ---------------------------------------------------------------------------
# Step 1: Build workspace
# ---------------------------------------------------------------------------

echo ""
echo "=== step 1: workspace build ==="
# IMPORTANT: do NOT use --all-features. gf2-algebra's `hip` feature
# (crates/gf2-algebra/Cargo.toml) pulls in gf2-kernels-hip, whose
# build.rs unconditionally invokes /opt/rocm/bin/hipcc — that breaks
# the CPU-only / GF2_PERMANENT_REPRO_SKIP_GPU=1 path at build time,
# before any GPU-skip logic runs. We therefore build an explicit
# non-hip CPU feature set. `hip` is added to the GPU harness builds
# (steps 4a/4b) only when GPU steps are actually enabled.
#
# CPU feature set: gf2-algebra default is [simd, parallel, f5, f7];
# the criterion bench needs `test-support` (and `simd`), the examples
# need `parallel test-support`. We pass the union explicitly so the
# build is deterministic and never enables `hip`.
CPU_FEATURES="simd parallel f5 f7 test-support"
echo "    cargo build --workspace --release --features \"${CPU_FEATURES}\""
cargo build --workspace --release --features "${CPU_FEATURES}"

# Also build the GPU research harnesses WITHOUT --features hip so they
# compile to the non-ROCm stub (prints a message and exits non-zero if
# run without hip). This keeps step 1 ROCm-free; the real hip build of
# these crates happens in steps 4a/4b only when GPU is enabled.
echo "    building permanent_gpu_crossover (S5 harness) non-hip stub..."
cargo build \
    --manifest-path dev/research/permanent_gpu_crossover/Cargo.toml \
    --release
echo "    building permanent_gpu_speedup (S1g harness) non-hip stub..."
cargo build \
    --manifest-path dev/research/permanent_gpu_speedup/Cargo.toml \
    --release

# ---------------------------------------------------------------------------
# Step 2: S1 — Criterion bench (n=24/28)
# ---------------------------------------------------------------------------

echo ""
echo "=== step 2: S1 — criterion bench s1_n36_speedup (n=24, n=28; ~4 min) ==="
echo "    cargo bench -p gf2-algebra --features 'simd test-support' --bench s1_n36_speedup"
cargo bench -p gf2-algebra --features "simd test-support" --bench s1_n36_speedup

# ---------------------------------------------------------------------------
# Step 2b: S1 offline cells (n=32, n=36) — opt-in only
# ---------------------------------------------------------------------------

if [[ "${offline_n32}" == "1" ]]; then
    if [[ "${offline_n36}" == "1" ]]; then
        echo ""
        echo "=== step 2b: S1 offline n=32 + n=36 (~10+ hr total) ==="
        echo "    S1_OFFLINE=1 cargo bench -p gf2-algebra --features 'simd test-support' --bench s1_n36_speedup -- --nocapture"
        S1_OFFLINE=1 cargo bench \
            -p gf2-algebra \
            --features "simd test-support" \
            --bench s1_n36_speedup \
            -- --nocapture
    else
        echo ""
        echo "=== step 2b: S1 offline n=32 only (~20 min) ==="
        echo "    S1_OFFLINE=1 S1_OFFLINE_MAX_N=32 cargo bench -p gf2-algebra --features 'simd test-support' --bench s1_n36_speedup -- --nocapture"
        S1_OFFLINE=1 S1_OFFLINE_MAX_N=32 cargo bench \
            -p gf2-algebra \
            --features "simd test-support" \
            --bench s1_n36_speedup \
            -- --nocapture
    fi
else
    echo ""
    echo "=== step 2b: S1 offline cells (n=32/n=36) — SKIPPED ==="
    echo "    Set S1_OFFLINE_N32=1 or S1_OFFLINE_N36=1 to enable (~20 min / ~10 hr)."
fi

# ---------------------------------------------------------------------------
# Step 3a: S2 — parallel scaling sweep
# ---------------------------------------------------------------------------

echo ""
echo "=== step 3a: S2 — parallel scaling sweep (n=28/32/36, ~15 min) ==="
echo "    cargo run -p gf2-algebra --release --features 'parallel test-support' --example parallel_scaling_sweep"
cargo run -p gf2-algebra \
    --release \
    --features "parallel test-support" \
    --example parallel_scaling_sweep

# ---------------------------------------------------------------------------
# Step 3b: S3 — assemble s3_cross_cpu-<DATE>.csv (fully automated)
# ---------------------------------------------------------------------------
#
# Two-part deterministic assembly (no manual merge):
#   Part A — AVX2 throughput rows reused (copied, not re-measured) from the S1
#            CSV rows impl=permanent_bipedal3_simd, reformatted to the S3
#            schema with impl=permanent_bipedal3_avx2, ratio_vs_avx2=1.000.
#   Part B — scalar / avx2_sanity rows from the ~1-min s3_scalar_vs_avx2_sanity
#            example stdout (its data lines are already in the S3 schema).

echo ""
echo "=== step 3b: S3 — assemble s3_cross_cpu CSV (sanity sweep n=16/20/24, ~1 min) ==="
echo "    cargo run -p gf2-algebra --release --features 'simd test-support' --example s3_scalar_vs_avx2_sanity"

# Locate the S1 CSV that supplies Part A. Prefer the SA_DATE/today dated file
# (just written by step 2); fall back to the newest s1_speedup-*.csv present.
S3_DATE="${SA_DATE:-$(date -u +%F)}"
S1_CSV="${ARTEFACT_DIR}/s1_speedup-${S3_DATE}.csv"
if [[ ! -f "${S1_CSV}" ]]; then
    # Newest s1_speedup-*.csv by name (ISO dates sort lexically). find avoids
    # ls-parsing pitfalls (SC2012) and the dated names are alnum+dash only.
    S1_CSV="$(find "${ARTEFACT_DIR}" -maxdepth 1 -type f \
        -name 's1_speedup-*.csv' 2>/dev/null | sort | tail -n 1 || true)"
fi
if [[ -z "${S1_CSV}" || ! -f "${S1_CSV}" ]]; then
    echo "    ERROR: no s1_speedup-*.csv found in ${ARTEFACT_DIR}/ for S3 Part A." >&2
    echo "           S3 reuses S1 AVX2 rows; run step 2 (S1 bench) first." >&2
    exit 1
fi
S1_CSV_BASENAME="$(basename "${S1_CSV}")"
echo "    S3 Part A source (S1 AVX2 rows): ${S1_CSV}"

# Capture the sanity example stdout (Part B source).
S3_SANITY_OUT="${ARTEFACT_DIR}/s3_sanity_rows-${S3_DATE}.txt"
cargo run -p gf2-algebra \
    --release \
    --features "simd test-support" \
    --example s3_scalar_vs_avx2_sanity \
    | tee "${S3_SANITY_OUT}"

S3_CSV="${ARTEFACT_DIR}/s3_cross_cpu-${S3_DATE}.csv"
echo ""
echo "    assembling ${S3_CSV} (Part A from ${S1_CSV_BASENAME} + Part B sanity rows)..."

python3 - "${S1_CSV}" "${S3_SANITY_OUT}" "${S3_CSV}" "${S3_DATE}" "${S1_CSV_BASENAME}" <<'PYEOF'
import sys

s1_csv, sanity_txt, out_csv, date, s1_basename = sys.argv[1:6]

# --- Part A: reuse S1 rows impl=permanent_bipedal3_simd -> _avx2, ratio 1.000 ---
part_a = []
with open(s1_csv) as f:
    for line in f:
        line = line.rstrip("\n")
        if line.startswith("#") or not line:
            continue
        cols = line.split(",")
        # S1 schema: n,impl,mean_us,std_us,samples,ratio_vs_reference,hw_fingerprint
        if len(cols) < 7 or cols[1] != "permanent_bipedal3_simd":
            continue
        n, _impl, mean_us, std_us, samples = cols[0], cols[1], cols[2], cols[3], cols[4]
        hw = ",".join(cols[6:])  # fingerprint has no commas, but be safe
        # S3 schema: n,impl,mean_us,std_us,samples,ratio_vs_avx2,hw_fingerprint
        part_a.append((int(n), f"{n},permanent_bipedal3_avx2,{mean_us},{std_us},{samples},1.000,{hw}"))
part_a.sort(key=lambda t: t[0])

if not part_a:
    sys.stderr.write(f"ERROR: no permanent_bipedal3_simd rows in {s1_csv}\n")
    sys.exit(1)

# --- Part B: data rows from the sanity example stdout ---
# The example prints progress + a '#'-prefixed header + the column header line,
# then data rows. We keep only lines whose impl is _scalar or _avx2_sanity.
part_b = []
with open(sanity_txt) as f:
    for line in f:
        line = line.rstrip("\n")
        if not line or line.startswith("#"):
            continue
        cols = line.split(",")
        if len(cols) < 7:
            continue
        if cols[1] not in ("permanent_bipedal3_scalar", "permanent_bipedal3_avx2_sanity"):
            continue
        try:
            n = int(cols[0])
        except ValueError:
            continue
        # sort key: n ascending, then scalar (0) before avx2_sanity (1)
        order = 0 if cols[1] == "permanent_bipedal3_scalar" else 1
        part_b.append((n, order, line))
part_b.sort(key=lambda t: (t[0], t[1]))

if not part_b:
    sys.stderr.write(f"ERROR: no S3 data rows parsed from {sanity_txt}\n")
    sys.exit(1)

# --- Provenance header: reproduce s3_cross_cpu-2026-05-12.csv header, with
#     the date and S1 source filename actually used substituted. ---
header = [
    "# S3 (jit:363556e6) cross-CPU portability sweep — AVX2-only scope",
    f"# date: {date}",
    "# host: AMD Ryzen 9 5900X 12-Core Processor",
    "# arch: Zen 3",
    "# avx2: yes, avx512: no",
    "# seed_base: 0x363556e600000000",
    "# scope: AVX2-only per amendment 2026-05-12 (AVX-512 row deferred to f8d230ef)",
    f"# avx2_throughput_source: re-used from dev/benchmarks/gf2_algebra_permanent/{s1_basename}",
    "#   (rows: permanent_bipedal3_simd at n=24/28/32/36, reformatted to impl=permanent_bipedal3_avx2)",
    "#   s1 seed_base: 0xc98ed60300000000",
    "# scalar_vs_avx2_sanity: measured fresh via crates/gf2-algebra/examples/s3_scalar_vs_avx2_sanity.rs",
    "#   n in {16, 20, 24}, 5 samples per cell, seed = 0x363556e600000000 ^ n ^ sample",
    "#   Note: scalar is faster than AVX2 at small n (W=1 word). This is expected: the",
    "#   singleword SIMD path zero-pads to a 4-element AVX2 lane (documented in bipedal3.rs",
    "#   module comment). The timing difference confirms that two distinct code paths are",
    "#   executed (correctness checked via bit-identical assertions). At large n (n in",
    "#   {24,28,32,36} from S1), the AVX2 path is 6.9x-10.6x faster than the reference.",
    "# avx2_dispatch_correctness: verified by test_simd_vs_scalar_n8 / n16 / n24 in",
    "#   crates/gf2-algebra/src/permanent/bipedal3.rs",
    "n,impl,mean_us,std_us,samples,ratio_vs_avx2,hardware_fingerprint",
]

with open(out_csv, "w") as f:
    for h in header:
        f.write(h + "\n")
    for _n, row in part_a:
        f.write(row + "\n")
    for _n, _order, row in part_b:
        f.write(row + "\n")

print(f"    S3 CSV written: {out_csv} "
      f"({len(part_a)} Part A AVX2 rows + {len(part_b)} Part B sanity rows)")
PYEOF

echo "    Sanity-row capture retained at: ${S3_SANITY_OUT}"

# ---------------------------------------------------------------------------
# Step 4a: S5 — GPU vs CPU crossover (ROCm only)
# ---------------------------------------------------------------------------

echo ""
if [[ "${skip_gpu}" == "1" ]]; then
    echo "=== step 4a: S5 — GPU crossover — SKIPPED (no ROCm / GF2_PERMANENT_REPRO_SKIP_GPU=1) ==="
    echo "    To run: ensure ROCm + hipcc are present and unset GF2_PERMANENT_REPRO_SKIP_GPU."
    echo "    Command:"
    echo "      cargo build --manifest-path dev/research/permanent_gpu_crossover/Cargo.toml --release --features hip"
    echo "      cargo run   --manifest-path dev/research/permanent_gpu_crossover/Cargo.toml --release --features hip"
else
    echo "=== step 4a: S5 — GPU vs CPU crossover at M=256 (n=24/28; ~1 hr) ==="
    echo "    cargo build --manifest-path dev/research/permanent_gpu_crossover/Cargo.toml --release --features hip"
    cargo build \
        --manifest-path dev/research/permanent_gpu_crossover/Cargo.toml \
        --release --features hip
    echo "    cargo run   --manifest-path dev/research/permanent_gpu_crossover/Cargo.toml --release --features hip"
    cargo run \
        --manifest-path dev/research/permanent_gpu_crossover/Cargo.toml \
        --release --features hip
fi

# ---------------------------------------------------------------------------
# Step 4b: S1g — GPU 50x speedup vs reference (ROCm only)
# ---------------------------------------------------------------------------

echo ""
if [[ "${skip_gpu}" == "1" ]]; then
    echo "=== step 4b: S1g — GPU 50x speedup — SKIPPED (no ROCm / GF2_PERMANENT_REPRO_SKIP_GPU=1) ==="
    echo "    To run: ensure ROCm + hipcc are present and unset GF2_PERMANENT_REPRO_SKIP_GPU."
    echo "    Command:"
    echo "      cargo build --manifest-path dev/research/permanent_gpu_speedup/Cargo.toml --release --features hip"
    echo "      cargo run   --manifest-path dev/research/permanent_gpu_speedup/Cargo.toml --release --features hip"
    echo "    Wall-clock: n=24/28/32 ~3 reps each; n=36 ~1 rep (~7200 s). Budget ~2+ hr."
else
    echo "=== step 4b: S1g — GPU 50x speedup vs reference (n=24/28/32/36; ~2+ hr) ==="
    echo "    cargo build --manifest-path dev/research/permanent_gpu_speedup/Cargo.toml --release --features hip"
    cargo build \
        --manifest-path dev/research/permanent_gpu_speedup/Cargo.toml \
        --release --features hip
    echo "    cargo run   --manifest-path dev/research/permanent_gpu_speedup/Cargo.toml --release --features hip"
    cargo run \
        --manifest-path dev/research/permanent_gpu_speedup/Cargo.toml \
        --release --features hip
fi

# ---------------------------------------------------------------------------
# Step 5: Update csvs/ snapshot (conventional names)
# ---------------------------------------------------------------------------

echo ""
echo "=== step 5: update csvs/ snapshot from dated top-level CSVs ==="

_DATE="${SA_DATE:-$(date -u +%F)}"

# Fail-fast copy: a producing step that RAN must have emitted its CSV.
# Absence is a HARD error (criterion 4 fail-fast) — we never silently
# leave a stale csvs/ snapshot in place for a dataset whose step ran.
# GPU-skipped datasets are handled separately below (absence is allowed
# there, and the previously committed snapshot is intentionally retained).
_require_copy() {
    local src="$1"
    local dst="$2"
    local label="$3"
    if [[ -f "${src}" ]]; then
        cp "${src}" "${dst}"
        echo "    copied: ${src} -> ${dst}"
    else
        echo "    ERROR: ${label} step ran but its output is missing: ${src}" >&2
        echo "           Refusing to leave a stale ${dst} — aborting." >&2
        exit 1
    fi
}

mkdir -p "${ARTEFACT_DIR}/csvs"

# S1: dated CSV written by the bench (uses SA_DATE or today's date).
# The Criterion bench (step 2) always runs, so this file must exist.
_require_copy \
    "${ARTEFACT_DIR}/s1_speedup-${_DATE}.csv" \
    "${ARTEFACT_DIR}/csvs/s1_speedup.csv" \
    "S1 (criterion bench)"

# S2: dated CSV written by parallel_scaling_sweep example (step 3a, always runs)
_require_copy \
    "${ARTEFACT_DIR}/s2_parallel_scaling-${_DATE}.csv" \
    "${ARTEFACT_DIR}/csvs/s2_parallel_scaling.csv" \
    "S2 (parallel scaling sweep)"

# S3: dated CSV assembled automatically in step 3b (always runs)
_require_copy \
    "${ARTEFACT_DIR}/s3_cross_cpu-${_DATE}.csv" \
    "${ARTEFACT_DIR}/csvs/s3_cross_cpu.csv" \
    "S3 (cross-CPU assembly)"

if [[ "${skip_gpu}" == "0" ]]; then
    # GPU steps actually ran — their outputs are now MANDATORY.
    _require_copy \
        "${ARTEFACT_DIR}/s5_gpu_crossover-${_DATE}.csv" \
        "${ARTEFACT_DIR}/csvs/s5_gpu_crossover.csv" \
        "S5 (GPU crossover)"
    _require_copy \
        "${ARTEFACT_DIR}/s1g_gpu_speedup-${_DATE}.csv" \
        "${ARTEFACT_DIR}/csvs/s1g_gpu_speedup.csv" \
        "S1g (GPU speedup)"
else
    # GPU steps were INTENTIONALLY skipped (no ROCm or
    # GF2_PERMANENT_REPRO_SKIP_GPU=1). Their absence is expected; the
    # previously committed csvs/ snapshots are intentionally retained.
    echo "    INFO: GPU steps skipped — csvs/s5_gpu_crossover.csv and csvs/s1g_gpu_speedup.csv"
    echo "         remain the previously committed snapshots (not regenerated this run)."
fi

# ---------------------------------------------------------------------------
# Step 6: Regenerate figures via plot_permanent_benchmarks.py
# ---------------------------------------------------------------------------

echo ""
echo "=== step 6: regenerate figures via scripts/plot_permanent_benchmarks.py ==="
#
# The closed-issue plot script resolves four PINNED historical filenames
# via strict constants (no silent date substitution):
#   S1_FILENAME = s1_speedup-2026-05-11.csv
#   S2_FILENAME = s2_parallel_scaling-2026-05-11.csv
#   S3_FILENAME = s3_cross_cpu-2026-05-12.csv
#   S5_FILENAME = s5_gpu_crossover-2026-05-15.csv
# (S1g is NOT plotted.) If we pointed the plot script at the artefact dir
# directly, it would render from the OLD committed pinned-name CSVs, not
# this run's freshly produced dated CSVs.
#
# Fix (without touching the plot script): stage THIS run's fresh dated
# CSVs into a dedicated plot-input dir under the EXACT pinned names the
# plot script resolves, then point the plot script at that dir. The
# figures are therefore generated from this run's data. Idempotent and
# diff-stable: the staging dir is rebuilt from scratch each run.
PLOT_PIN_S1="s1_speedup-2026-05-11.csv"
PLOT_PIN_S2="s2_parallel_scaling-2026-05-11.csv"
PLOT_PIN_S3="s3_cross_cpu-2026-05-12.csv"
PLOT_PIN_S5="s5_gpu_crossover-2026-05-15.csv"

PLOT_INPUT_DIR="${ARTEFACT_DIR}/.plot-input"
rm -rf "${PLOT_INPUT_DIR}"
mkdir -p "${PLOT_INPUT_DIR}"

# S1/S2/S3 always produced this run — required.
_require_copy \
    "${ARTEFACT_DIR}/s1_speedup-${_DATE}.csv" \
    "${PLOT_INPUT_DIR}/${PLOT_PIN_S1}" \
    "S1 (plot input)"
_require_copy \
    "${ARTEFACT_DIR}/s2_parallel_scaling-${_DATE}.csv" \
    "${PLOT_INPUT_DIR}/${PLOT_PIN_S2}" \
    "S2 (plot input)"
_require_copy \
    "${ARTEFACT_DIR}/s3_cross_cpu-${_DATE}.csv" \
    "${PLOT_INPUT_DIR}/${PLOT_PIN_S3}" \
    "S3 (plot input)"

# S5: if GPU ran this run, use the fresh CSV; if GPU was intentionally
# skipped, fall back to the committed canonical S5 CSV so the S5 figure
# still renders (clearly: S5 was not regenerated this run).
if [[ "${skip_gpu}" == "0" ]]; then
    _require_copy \
        "${ARTEFACT_DIR}/s5_gpu_crossover-${_DATE}.csv" \
        "${PLOT_INPUT_DIR}/${PLOT_PIN_S5}" \
        "S5 (plot input)"
else
    if [[ -f "${ARTEFACT_DIR}/${PLOT_PIN_S5}" ]]; then
        cp "${ARTEFACT_DIR}/${PLOT_PIN_S5}" "${PLOT_INPUT_DIR}/${PLOT_PIN_S5}"
        echo "    S5 plot input: GPU skipped — using committed ${PLOT_PIN_S5}"
        echo "    (S5 figure reflects the committed measurement, not this run)."
    else
        echo "    ERROR: GPU skipped and committed ${PLOT_PIN_S5} is absent;" >&2
        echo "           cannot render the S5 figure. Aborting." >&2
        exit 1
    fi
fi

echo "    python3 scripts/plot_permanent_benchmarks.py all \\"
echo "        --input-dir ${PLOT_INPUT_DIR}/ \\"
echo "        --output-dir ${ARTEFACT_DIR}/figures/"

mkdir -p "${ARTEFACT_DIR}/figures"
python3 scripts/plot_permanent_benchmarks.py all \
    --input-dir "${PLOT_INPUT_DIR}/" \
    --output-dir "${ARTEFACT_DIR}/figures/"

# Clean up the transient staging dir so the artefact tree stays pristine
# and the run is idempotent (no leftover .plot-input on re-runs).
rm -rf "${PLOT_INPUT_DIR}"

echo "    Figures written to: ${ARTEFACT_DIR}/figures/ (from THIS run's CSVs)"

# ---------------------------------------------------------------------------
# Step 7: Rewrite provenance.json
# ---------------------------------------------------------------------------

echo ""
echo "=== step 7: rewrite provenance.json ==="

_HEAD_SHA="$(git rev-parse HEAD 2>/dev/null || echo "unknown")"
_RUSTC="$(rustc --version 2>/dev/null || echo "unknown")"
_OS="$(uname -srm 2>/dev/null || echo "unknown")"
_NOW="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

# Read existing provenance.json — preserve all fields that depend on hardware
# measurements (seeds, dataset_source_commits, rocminfo, lscpu_full, etc.)
# and update only the fields this script can determine automatically.
PROV="${ARTEFACT_DIR}/provenance.json"

if command -v python3 &>/dev/null; then
    python3 - <<PYEOF
import json, sys

prov_path = "${PROV}"
try:
    with open(prov_path) as f:
        prov = json.load(f)
except Exception as e:
    # Fail-fast: this step UPDATES an existing provenance.json, preserving
    # measurement-dependent fields (dataset_source_commits, seeds, rocminfo,
    # lscpu_full, ...). If the existing file cannot be read/parsed we must NOT
    # silently rewrite it from an empty object — that would drop those fields
    # and leave a partial artefact while exiting 0. Abort the whole repro.
    print(f"ERROR: cannot read/parse {prov_path}: {e}", file=sys.stderr)
    print("       refusing to overwrite provenance.json from an empty object "
          "(would drop dataset_source_commits/seeds/rocminfo/lscpu_full). "
          "Aborting the reproduction run.", file=sys.stderr)
    sys.exit(1)

prov["artefact_assembly_commit"] = "${_HEAD_SHA}"
prov["rustc"] = "${_RUSTC}"
prov["os"] = "${_OS}"
prov["repro_script"] = "scripts/permanent-repro.sh"
prov["repro_timestamp_utc"] = "${_NOW}"

with open(prov_path, "w") as f:
    json.dump(prov, f, indent=2)
    f.write("\n")

print(f"    provenance.json updated (artefact_assembly_commit={prov['artefact_assembly_commit'][:8]}...)")
PYEOF
else
    # python3 is a documented requirement (it also drives the plot step).
    # Fail-fast: a missing provenance.json update is a partial artefact, so
    # abort non-zero rather than exit 0 with a warning.
    echo "ERROR: python3 not available; cannot update provenance.json." >&2
    echo "       python3 is a documented requirement of this repro script." >&2
    exit 1
fi

# ---------------------------------------------------------------------------
# Final status
# ---------------------------------------------------------------------------

echo ""
echo "========================================================"
echo " permanent-repro.sh complete"
echo "========================================================"
echo " Artefact dir:  ${REPO_ROOT}/${ARTEFACT_DIR}/"
echo " Head commit:   ${_HEAD_SHA}"
echo " Date tag used: ${SA_DATE:-$(date -u +%F)}"
if [[ "${skip_gpu}" == "1" ]]; then
    echo " GPU steps:     SKIPPED (S5, S1g)"
    echo "   -> csvs/s5_gpu_crossover.csv and csvs/s1g_gpu_speedup.csv are"
    echo "      the previously committed snapshots, not freshly measured."
fi
echo ""
echo " S3 CSV: assembled automatically (no manual step):"
echo "   ${ARTEFACT_DIR}/s3_cross_cpu-${SA_DATE:-$(date -u +%F)}.csv"
echo "   Part A = S1 AVX2 rows (reused, not re-measured); Part B = fresh sanity."
echo "   Sanity-row capture retained at: ${S3_SANITY_OUT}"
echo ""
echo " This script does NOT git add or git commit any files."
echo "========================================================"
