#!/bin/bash
# Benchmark script to test different thread configurations
# Usage: ./benchmark_threads.sh [benchmark_name]

set -e

BENCH_NAME="${1:-parallel_scaling}"

echo "=== LDPC Parallel Scaling Benchmarks ==="
echo "Benchmark: $BENCH_NAME"
echo ""

# Detect physical and logical cores
PHYSICAL_CORES=$(lscpu -p | grep -v '^#' | sort -u -t, -k 2,4 | wc -l)
LOGICAL_CORES=$(nproc)

echo "Physical cores: $PHYSICAL_CORES"
echo "Logical cores: $LOGICAL_CORES"
echo ""

# Test configurations: 1, 2, 4, 8, physical, all
THREAD_COUNTS="1 2 4 8"

echo "=== Running benchmarks with different thread counts ==="
echo ""

for THREADS in $THREAD_COUNTS; do
    if [ $THREADS -le $PHYSICAL_CORES ]; then
        echo "--- Testing with $THREADS thread(s) ---"
        RAYON_NUM_THREADS=$THREADS cargo bench --bench $BENCH_NAME --features parallel -- --quiet
        echo ""
    fi
done

echo "--- Testing with $PHYSICAL_CORES threads (physical cores) ---"
RAYON_NUM_THREADS=$PHYSICAL_CORES cargo bench --bench $BENCH_NAME --features parallel -- --quiet
echo ""

if [ $LOGICAL_CORES -gt $PHYSICAL_CORES ]; then
    echo "--- Testing with $LOGICAL_CORES threads (all logical cores) ---"
    RAYON_NUM_THREADS=$LOGICAL_CORES cargo bench --bench $BENCH_NAME --features parallel -- --quiet
    echo ""
fi

echo "=== Benchmark complete ==="
echo "Results saved in target/criterion/"
echo ""
echo "To view detailed results:"
echo "  cargo criterion --bench $BENCH_NAME"
