#!/usr/bin/env bash
# perm-uniformity-gpu-repro.sh
#
# Reproducible end-to-end runner for the GPU-accelerated high-N perm-vs-det
# uniformity resample (JIT issue b293af5a).  Supersedes the noise-limited
# cells of 8e4e19a0 (q=3 n in {24,28,32}, F_5/F_7 extended past n<=14).
#
# Usage:
#   bash scripts/perm-uniformity-gpu-repro.sh            # full sweep
#   PLOT_ONLY=1 bash scripts/perm-uniformity-gpu-repro.sh # figure-only, no run
#
#   # A CELLS-filtered (partial/resume) run MUST use a separate staging
#   # OUTPUT_DIR — the harness fail-fast-refuses to truncate the SSOT CSV:
#   OUTPUT_DIR=/tmp/pug_resume CELLS=q3n24,q3n28,q3n32 \
#       cargo run --manifest-path dev/research/perm_uniformity_gpu/Cargo.toml \
#       --release --features hip
#   # then merge the produced rows into the SSOT CSV and refresh the figure:
#   PLOT_ONLY=1 bash scripts/perm-uniformity-gpu-repro.sh
#
# Requirements:
#   - Rust toolchain (1.95+) with cargo
#   - ROCm + a gfx1030-class GPU (build-time hipcc on PATH, device at runtime)
#
# Build invocation note:
#   This crate is intentionally excluded from the root workspace (it is a
#   standalone research prototype, like permanent_gpu_crossover and
#   perm_uniformity).  `cargo build -p perm-uniformity-gpu` therefore fails;
#   the correct invocation is `--manifest-path` with `--features hip`.
#
# Determinism:
#   The harness pins SEED = 0x00c0ffee00000001 and reuses the 8e4e19a0
#   cell_seed derivation, so the statistical CSV columns
#   (q,n,samples,tvd_perm,tvd_perm_ci_lo,tvd_perm_ci_hi,tvd_det,
#   tvd_det_ci_lo,tvd_det_ci_hi) are bit-identical across runs for the same
#   seed.  The wall-clock timing columns (mean_us_perm, mean_us_det) are
#   inherently nondeterministic and excluded from the bit-identical guarantee
#   (8e4e19a0 Amendments §2 precedent).
#
# gfx1030 GPU watchdog: DEFEATED (writeup §2.5).
#   The earlier gfx1030 "GPU Hang" HW exception on sustained long kernels
#   (originally F_5 n=20) is fully resolved by the bounded sub-batch +
#   host-cooldown chunked-kernel mitigation. All 18 measured cells
#   (q=3 n=6..32, F_5 n=8..24, F_7 n=8..20) are genuine PASS with zero GPU
#   hangs. q=5 n=28 and q=7 n=24 are hardware-infeasible at the
#   noise-floor-required N (writeup §9 limitation 3) and are deliberately
#   NOT in the grid — they are not under-sampled to fake a PASS.
#
# Figure regeneration (PLOT_ONLY):
#   The per-cell sweep is split across sessions (some cells are multi-hour),
#   so the SSOT artefact is the incrementally-written CSV. The faceted
#   tvd_vs_n_gpu.png is regenerated FROM that CSV — without re-running any
#   cell — via the harness PLOT_ONLY mode. This script always refreshes the
#   figure from the persisted CSV at the end, so the plot matches the CSV
#   regardless of which (or how many) cells ran in this invocation.

set -euo pipefail

MANIFEST="dev/research/perm_uniformity_gpu/Cargo.toml"
OUT_DIR="dev/benchmarks/perm_uniformity"
CSV="$OUT_DIR/results-2026-05-17-gpu.csv"

echo "=== perm-uniformity-gpu repro (JIT b293af5a) ==="
echo "  building with --manifest-path --features hip (workspace-excluded crate)..."

cargo build --manifest-path "$MANIFEST" --release --features hip

mkdir -p "$OUT_DIR"

if [ -n "${PLOT_ONLY:-}" ]; then
    echo "  PLOT_ONLY: regenerating figure from persisted SSOT CSV (no cell run)..."
    PLOT_ONLY=1 OUTPUT_DIR="$OUT_DIR" \
        cargo run --manifest-path "$MANIFEST" --release --features hip
else
    echo "  running GPU resample sweep..."
    OUTPUT_DIR="$OUT_DIR" \
        cargo run --manifest-path "$MANIFEST" --release --features hip
    echo "  refreshing figure from persisted SSOT CSV (PLOT_ONLY)..."
    PLOT_ONLY=1 OUTPUT_DIR="$OUT_DIR" \
        cargo run --manifest-path "$MANIFEST" --release --features hip
fi

echo ""
echo "=== Output files ==="
ls -lh "$CSV" "$OUT_DIR"/tvd_vs_n_gpu.png 2>/dev/null || true

echo ""
echo "=== Statistical columns SHA-256 (cols q,n,samples,tvd_*,ci_*) ==="
# Columns 1-9 are deterministic; columns 10-11 (mean_us_*) are wall-clock.
grep -v '^#' "$CSV" \
    | cut -d',' -f1-9 \
    | sha256sum \
    | awk '{print $1, "  (statistical columns only)"}'
