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
# Build invocation note (D5):
#   This crate is excluded from the root workspace (it is a standalone research
#   prototype).  `cargo build -p perm-uniformity --release` therefore fails
#   with "package ID specification did not match any packages".
#   The correct invocation is `--manifest-path`, as used by this script and by
#   the permanent_gpu_crossover and gf2-kernels-hip precedents.
#   See .jit/issues/5c0505b2 Amendment 2026-05-15 (build-invocation).
#
# Determinism:
#   The harness uses a pinned seed (0x00c0ffee00000001) embedded in the binary.
#   All statistical columns (tvd_perm, tvd_perm_ci_lo, tvd_perm_ci_hi, tvd_det,
#   tvd_det_ci_lo, tvd_det_ci_hi, samples) are bit-identical across runs on
#   the same binary.  The timing columns (mean_us_perm, mean_us_det) reflect
#   wall-clock measurements and vary run-to-run; they are excluded from the
#   determinism check below.
#
# Criterion 3 (resolved, user-approved 2026-05-16):
#   Criterion 3 was amended (issue 8e4e19a0 Amendments §2, user sign-off) to
#   scope the bit-identical guarantee to the statistical columns (1-9) only;
#   the wall-clock timing columns (10-11) required by criterion 4 are
#   inherently nondeterministic and explicitly excluded. The sha256 below
#   hashes only columns 1-9, which is exactly the approved guarantee.

set -euo pipefail

MANIFEST="dev/research/perm_uniformity/Cargo.toml"
OUT_DIR="dev/benchmarks/perm_uniformity"

echo "=== perm-uniformity repro ==="
echo "  building with --manifest-path (not -p; crate is workspace-excluded)..."

cargo build --manifest-path "$MANIFEST" --release

echo "  running sweep (this may take 3-4 min)..."

mkdir -p "$OUT_DIR"
OUTPUT_DIR="$OUT_DIR" \
    cargo run --manifest-path "$MANIFEST" --release

echo ""
echo "=== Output files ==="
ls -lh "$OUT_DIR"/results-*.csv "$OUT_DIR"/tvd_vs_n.png 2>/dev/null || true

echo ""
echo "=== PNG validity ==="
file "$OUT_DIR/tvd_vs_n.png"

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
