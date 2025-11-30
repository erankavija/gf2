#!/bin/bash
# Run comprehensive parallel scaling benchmarks overnight
# Expected duration: 2-4 hours
# Results saved to: benchmark_results_<timestamp>.log and target/criterion/

set -e

TIMESTAMP=$(date +%Y%m%d_%H%M%S)
LOG_FILE="benchmark_results_${TIMESTAMP}.log"

echo "=== Starting Overnight Benchmark Suite ===" | tee $LOG_FILE
echo "Start time: $(date)" | tee -a $LOG_FILE
echo "System: $(nproc) logical cores, $(lscpu -p | grep -v '^#' | sort -u -t, -k 2,4 | wc -l) physical cores" | tee -a $LOG_FILE
echo "" | tee -a $LOG_FILE

# Save baseline with 1 thread (sequential)
echo "=== Phase 1: Establishing Sequential Baseline ===" | tee -a $LOG_FILE
RAYON_NUM_THREADS=1 cargo bench --bench parallel_scaling --features parallel -- \
  --save-baseline sequential 2>&1 | tee -a $LOG_FILE

# Run full parallel scaling benchmark (auto-detects threads)
echo "" | tee -a $LOG_FILE
echo "=== Phase 2: Thread Scaling Analysis ===" | tee -a $LOG_FILE
cargo bench --bench parallel_scaling --features parallel 2>&1 | tee -a $LOG_FILE

# Run throughput benchmarks
echo "" | tee -a $LOG_FILE
echo "=== Phase 3: LDPC Throughput Benchmarks ===" | tee -a $LOG_FILE
cargo bench --bench ldpc_throughput --features parallel 2>&1 | tee -a $LOG_FILE

# Run batch operations benchmarks
echo "" | tee -a $LOG_FILE
echo "=== Phase 4: Batch Operations Benchmarks ===" | tee -a $LOG_FILE
cargo bench --bench batch_operations --features parallel 2>&1 | tee -a $LOG_FILE

echo "" | tee -a $LOG_FILE
echo "=== Benchmark Suite Complete ===" | tee -a $LOG_FILE
echo "End time: $(date)" | tee -a $LOG_FILE
echo "" | tee -a $LOG_FILE
echo "Results saved to:" | tee -a $LOG_FILE
echo "  - Log file: $LOG_FILE" | tee -a $LOG_FILE
echo "  - HTML reports: target/criterion/report/index.html" | tee -a $LOG_FILE
echo "  - Raw data: target/criterion/*/base/" | tee -a $LOG_FILE
echo "" | tee -a $LOG_FILE
echo "To view results:" | tee -a $LOG_FILE
echo "  xdg-open target/criterion/report/index.html" | tee -a $LOG_FILE

# Generate quick summary
echo "" | tee -a $LOG_FILE
echo "=== Quick Throughput Summary ===" | tee -a $LOG_FILE
grep -h "thrpt:" $LOG_FILE | tail -20 | tee -a $LOG_FILE
