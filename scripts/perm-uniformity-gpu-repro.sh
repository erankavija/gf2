#!/usr/bin/env bash
# perm-uniformity-gpu-repro.sh
#
# Reproducible end-to-end runner for the GPU-accelerated high-N perm-vs-det
# uniformity resample (JIT issue b293af5a).  Supersedes the noise-limited
# cells of 8e4e19a0 (q=3 n in {24,28,32}, F_5/F_7 extended past n<=14).
#
# Usage:
#   bash scripts/perm-uniformity-gpu-repro.sh            # full sweep
#   CELLS=q3n24,q3n28,q3n32 bash scripts/perm-uniformity-gpu-repro.sh
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
# Known gfx1030 GPU watchdog hang (2026-05-17):
#   On the dev host, the F_5 n=20 (N=40,000) kernel reproducibly triggers a
#   gfx1030 "GPU Hang" HW exception (the GPU recovers; an isolated F_7 n=8
#   re-run PASSes, so the harness/kernels are correct — it is a hardware
#   watchdog limit on sustained long kernels). The 12 q=3 (n=6..32) + F_5
#   (n=8,12,16) cells complete and are written incrementally; the F_5 n>=20
#   and F_7 cells are REMAINING. To complete them, resume with a CELLS
#   filter and a smaller N per the handoff dev/active/b293af5a-impl-handoff.md
#   e.g.:  CELLS=q7n8,q7n12,q7n16,q7n20,q7n24 OUTPUT_DIR=... \
#          cargo run --manifest-path "$MANIFEST" --release --features hip

set -euo pipefail

MANIFEST="dev/research/perm_uniformity_gpu/Cargo.toml"
OUT_DIR="dev/benchmarks/perm_uniformity"
CSV="$OUT_DIR/results-2026-05-17-gpu.csv"

echo "=== perm-uniformity-gpu repro (JIT b293af5a) ==="
echo "  building with --manifest-path --features hip (workspace-excluded crate)..."

cargo build --manifest-path "$MANIFEST" --release --features hip

echo "  running GPU resample sweep..."
mkdir -p "$OUT_DIR"
OUTPUT_DIR="$OUT_DIR" \
    cargo run --manifest-path "$MANIFEST" --release --features hip

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
