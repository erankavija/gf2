#!/bin/bash
# Build the fflas-ffpack reference benchmark.
#
# Prerequisites: fflas-ffpack and givaro installed (pkg-config must find them).
#
# Usage:
#   ./benches/build_fflas_bench.sh
#   ./benches/fflas_fdot_bench
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
SRC="$SCRIPT_DIR/fflas_fdot_bench.cpp"
OUT="$SCRIPT_DIR/fflas_fdot_bench"

CXXFLAGS=$(pkg-config --cflags fflas-ffpack givaro)
LDFLAGS=$(pkg-config --libs fflas-ffpack givaro)

echo "Compiling $SRC ..."
# shellcheck disable=SC2086
g++ -O3 -march=native -std=c++17 $CXXFLAGS "$SRC" $LDFLAGS -o "$OUT"
echo "Built: $OUT"
