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
#   S3  — cross-CPU portability / scalar-vs-AVX2 sanity (example
#           s3_scalar_vs_avx2_sanity; NOTE: S3 CSV requires a manual assembly
#           step — see MANUAL STEP notice below and the step 4b banner at runtime)
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
#   - Python 3 with matplotlib 3.10.x for the plot step
#   - ROCm / hipcc for GPU steps S5 and S1g (auto-skipped if absent or
#     GF2_PERMANENT_REPRO_SKIP_GPU=1 is set)
#   - jq for provenance.json rewrite (optional; falls back to sed if absent)
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
#       Override the date embedded in output CSV filenames (default: today's UTC
#       date).  Use SA_DATE=2026-05-11 to reproduce the exact canonical filenames
#       in the committed artefact.  The plot script reads specific dated filenames
#       (e.g. s1_speedup-2026-05-11.csv); set SA_DATE accordingly if you want
#       the figures to reflect the newly generated CSVs.
#
# MANUAL STEP NOTICE (S3):
#   The S3 CSV (s3_cross_cpu) is a two-part assembly:
#     Part A (automated, step 3b): the s3_scalar_vs_avx2_sanity example writes
#       scalar-vs-AVX2 sanity rows to stdout. This script captures stdout and
#       saves it as s3_sanity_rows-<DATE>.txt alongside the benchmark artefacts.
#     Part B (manual): the AVX2 throughput rows at n in {24,28,32,36} are reused
#       from the S1 CSV (dev/benchmarks/gf2_algebra_permanent/s1_speedup-
#       <DATE>.csv rows impl=permanent_bipedal3_simd). To build a new dated
#       s3_cross_cpu CSV, a human must merge Part A rows with the S1 SIMD rows
#       following the format in s3_cross_cpu-2026-05-12.csv.
#   The plot step (step 6) always reads the committed s3_cross_cpu-2026-05-12.csv
#   unless you manually create a new dated file and update plot_permanent_benchmarks.py
#   (S3_FILENAME constant). This script does NOT update the plot script constant.

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
echo "    cargo build --workspace --all-features --release"
cargo build --workspace --all-features --release

# Also build the GPU research harnesses (even on non-GPU hosts: without
# --features hip they compile to a stub that prints an error and exits).
echo "    building permanent_gpu_crossover (S5 harness) stub..."
cargo build \
    --manifest-path dev/research/permanent_gpu_crossover/Cargo.toml \
    --release
echo "    building permanent_gpu_speedup (S1g harness) stub..."
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
# Step 3b: S3 — scalar-vs-AVX2 sanity (stdout capture)
# ---------------------------------------------------------------------------

echo ""
echo "=== step 3b: S3 — scalar-vs-AVX2 sanity sweep (n=16/20/24, ~1 min) ==="
echo "    cargo run -p gf2-algebra --release --features 'simd test-support' --example s3_scalar_vs_avx2_sanity"
echo ""
echo "    NOTE: S3 CSV ASSEMBLY IS A MANUAL STEP."
echo "    This step captures the sanity-row stdout into a .txt file."
echo "    To regenerate s3_cross_cpu-<DATE>.csv:"
echo "      1. Take the AVX2 rows from the S1 CSV (impl=permanent_bipedal3_simd,"
echo "         n in {24,28,32,36}) and prepend them to the sanity rows below."
echo "      2. Write a header block matching s3_cross_cpu-2026-05-12.csv format."
echo "    See dev/plans/s3_cross_cpu_portability.md for the full methodology."
echo ""

S3_SANITY_OUT="${ARTEFACT_DIR}/s3_sanity_rows-${SA_DATE:-$(date -u +%F)}.txt"
cargo run -p gf2-algebra \
    --release \
    --features "simd test-support" \
    --example s3_scalar_vs_avx2_sanity \
    | tee "${S3_SANITY_OUT}"

echo ""
echo "    Sanity rows saved to: ${S3_SANITY_OUT}"
echo "    (Plot step 6 reads the committed s3_cross_cpu-2026-05-12.csv;"
echo "     manual CSV assembly required to produce a new dated S3 CSV.)"

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

_copy_if_exists() {
    local src="$1"
    local dst="$2"
    if [[ -f "${src}" ]]; then
        cp "${src}" "${dst}"
        echo "    copied: ${src} -> ${dst}"
    else
        echo "    WARNING: source not found, skipping: ${src}"
    fi
}

mkdir -p "${ARTEFACT_DIR}/csvs"

# S1: dated CSV written by the bench (uses SA_DATE or today's date)
_copy_if_exists \
    "${ARTEFACT_DIR}/s1_speedup-${_DATE}.csv" \
    "${ARTEFACT_DIR}/csvs/s1_speedup.csv"

# S2: dated CSV written by parallel_scaling_sweep example
_copy_if_exists \
    "${ARTEFACT_DIR}/s2_parallel_scaling-${_DATE}.csv" \
    "${ARTEFACT_DIR}/csvs/s2_parallel_scaling.csv"

# S3: dated CSV is a manual assembly (see step 3b); copy only if present
# The committed s3_cross_cpu-2026-05-12.csv is already in csvs/s3_cross_cpu.csv.
if [[ -f "${ARTEFACT_DIR}/s3_cross_cpu-${_DATE}.csv" ]]; then
    _copy_if_exists \
        "${ARTEFACT_DIR}/s3_cross_cpu-${_DATE}.csv" \
        "${ARTEFACT_DIR}/csvs/s3_cross_cpu.csv"
else
    echo "    INFO: s3_cross_cpu-${_DATE}.csv not found (S3 CSV requires manual assembly)."
    echo "         csvs/s3_cross_cpu.csv remains the previously committed snapshot."
fi

if [[ "${skip_gpu}" == "0" ]]; then
    # S5: dated CSV written by permanent_gpu_crossover harness
    _copy_if_exists \
        "${ARTEFACT_DIR}/s5_gpu_crossover-${_DATE}.csv" \
        "${ARTEFACT_DIR}/csvs/s5_gpu_crossover.csv"

    # S1g: dated CSV written by permanent_gpu_speedup harness
    _copy_if_exists \
        "${ARTEFACT_DIR}/s1g_gpu_speedup-${_DATE}.csv" \
        "${ARTEFACT_DIR}/csvs/s1g_gpu_speedup.csv"
else
    echo "    INFO: GPU steps skipped — csvs/s5_gpu_crossover.csv and csvs/s1g_gpu_speedup.csv"
    echo "         remain the previously committed snapshots."
fi

# ---------------------------------------------------------------------------
# Step 6: Regenerate figures via plot_permanent_benchmarks.py
# ---------------------------------------------------------------------------

echo ""
echo "=== step 6: regenerate figures via scripts/plot_permanent_benchmarks.py ==="
echo "    python3 scripts/plot_permanent_benchmarks.py all \\"
echo "        --input-dir ${ARTEFACT_DIR}/ \\"
echo "        --output-dir ${ARTEFACT_DIR}/figures/"

mkdir -p "${ARTEFACT_DIR}/figures"
python3 scripts/plot_permanent_benchmarks.py all \
    --input-dir "${ARTEFACT_DIR}/" \
    --output-dir "${ARTEFACT_DIR}/figures/"

echo "    Figures written to: ${ARTEFACT_DIR}/figures/"

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
    print(f"WARNING: could not read {prov_path}: {e}", file=sys.stderr)
    prov = {}

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
    echo "    WARNING: python3 not available; provenance.json not updated."
    echo "    Manually set artefact_assembly_commit to ${_HEAD_SHA}."
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
echo " S3 CSV assembly is a MANUAL STEP."
echo "   Sanity rows captured at: ${S3_SANITY_OUT}"
echo "   See dev/plans/s3_cross_cpu_portability.md for assembly instructions."
echo ""
echo " This script does NOT git add or git commit any files."
echo "========================================================"
