#!/bin/bash
# Quick thread scaling test (runs in ~5 minutes)
#
# Tests thread scaling with 1, 2, 4, 8, and all available threads
# Usage: ./benchmark_quick.sh

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

echo "=== Quick Thread Scaling Benchmark ==="
echo "Start time: $(date)"
echo

# Detect number of cores
PHYSICAL_CORES=$(nproc 2>/dev/null || sysctl -n hw.physicalcpu 2>/dev/null || echo 4)
LOGICAL_CORES=$(nproc --all 2>/dev/null || sysctl -n hw.logicalcpu 2>/dev/null || echo 8)

echo "Physical cores: $PHYSICAL_CORES"
echo "Logical cores: $LOGICAL_CORES"
echo

# Thread counts to test
THREAD_COUNTS="1 2 4 8 $PHYSICAL_CORES"

# Build once
echo "Building benchmarks..."
cargo build --release --benches --features parallel
echo

# Run benchmarks for each thread count
for THREADS in $THREAD_COUNTS; do
    echo "=== Testing with $THREADS threads ==="
    RAYON_NUM_THREADS=$THREADS cargo bench --bench quick_parallel --features parallel -- --noplot
    echo
done

echo "=== Benchmark Complete ==="
echo "End time: $(date)"
echo
echo "To view results:"
echo "  xdg-open target/criterion/report/index.html  # Linux"
echo "  open target/criterion/report/index.html      # macOS"
echo
echo "To extract throughput data:"
echo "  grep 'thrpt:' target/criterion/<benchmark_name>/*/new/estimates.txt"
