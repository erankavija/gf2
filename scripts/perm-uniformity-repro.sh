#!/usr/bin/env bash
# perm-uniformity-repro.sh
#
# Reproducible end-to-end runner for the perm-vs-det uniformity sweep
# (JIT issue 8e4e19a0).
#
# Regenerates dev/benchmarks/perm_uniformity/results-2026-05-15.csv and
# dev/benchmarks/perm_uniformity/tvd_vs_n.png deterministically.
#
# Usage:
#   bash scripts/perm-uniformity-repro.sh
#
# Requirements:
#   - Rust toolchain (1.95+) with cargo
#   - ~3-4 min wall-clock on Ryzen 9 5900X (12 cores) for the full sweep
#
# Determinism:
#   The harness uses a pinned seed (0x00c0ffee00000001) embedded in the binary.
#   All statistical columns (tvd_perm, tvd_perm_ci_lo, tvd_perm_ci_hi, tvd_det,
#   tvd_det_ci_lo, tvd_det_ci_hi, samples) are bit-identical across runs on
#   the same binary.  The timing columns (mean_us_perm, mean_us_det) reflect
#   wall-clock measurements and vary run-to-run; they are excluded from the
#   determinism check below.

set -euo pipefail

MANIFEST="dev/research/perm_uniformity/Cargo.toml"
OUT_DIR="dev/benchmarks/perm_uniformity"

echo "=== perm-uniformity repro ==="
echo "  building..."

cargo build --manifest-path "$MANIFEST" --release

echo "  running sweep (this may take 3-4 min)..."

mkdir -p "$OUT_DIR"
OUTPUT_DIR="$OUT_DIR" \
    cargo run --manifest-path "$MANIFEST" --release

echo ""
echo "=== Output files ==="
ls -lh "$OUT_DIR"/results-*.csv "$OUT_DIR"/tvd_vs_n.png 2>/dev/null || true

echo ""
echo "=== Statistical columns SHA-256 (cols q,n,samples,tvd_*,ci_*) ==="
# Columns 1-9 are deterministic; columns 10-11 (mean_us_*) are wall-clock timings.
# We hash only the deterministic columns to verify reproducibility.
grep -v '^#' "$OUT_DIR/results-2026-05-15.csv" \
    | cut -d',' -f1-9 \
    | sha256sum \
    | awk '{print $1, "  (statistical columns only)"}'

echo ""
echo "=== Full CSV SHA-256 (informational; timing cols vary run-to-run) ==="
sha256sum "$OUT_DIR/results-2026-05-15.csv"
