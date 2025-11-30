# Parallel Performance Benchmarking Guide

This guide explains how to benchmark parallel performance with different thread configurations.

## Quick Start

### Run all parallel scaling benchmarks
```bash
cargo bench --bench parallel_scaling --features parallel
```

### Run with specific thread count
```bash
RAYON_NUM_THREADS=8 cargo bench --bench parallel_scaling --features parallel
```

### Run automated thread scaling test
```bash
./benchmark_threads.sh
```

### Run overnight benchmark suite (recommended)
```bash
./run_overnight_benchmarks.sh
```

## Running Complete Benchmark Suite (Overnight)

To collect comprehensive performance data across all thread counts and save results:

### Option 1: Full benchmark suite with Criterion
```bash
# Run all benchmarks and save results to target/criterion/
cargo bench --features parallel 2>&1 | tee benchmark_results_$(date +%Y%m%d_%H%M%S).log

# This will take 2-4 hours and produce:
# - HTML reports in target/criterion/
# - Raw data in target/criterion/<bench_name>/base/
# - Console output saved to log file
```

### Option 2: Thread scaling analysis only
```bash
# Run parallel_scaling benchmark (fastest, ~30-60 minutes)
cargo bench --bench parallel_scaling --features parallel 2>&1 | \
  tee parallel_scaling_$(date +%Y%m%d_%H%M%S).log

# Results saved to target/criterion/parallel_scaling/
```

### Option 3: Automated script with multiple thread counts
```bash
# Run benchmark_threads.sh and save output
./benchmark_threads.sh 2>&1 | tee thread_scaling_$(date +%Y%m%d_%H%M%S).log

# This tests: 1, 2, 4, 8, 12 (physical), 24 (all) threads
# Estimated time: 1-2 hours
```

### Results Location

After benchmarking completes:

```bash
# View HTML reports
open target/criterion/report/index.html  # macOS
xdg-open target/criterion/report/index.html  # Linux

# Raw CSV data (for analysis)
ls target/criterion/*/base/*.csv

# Compare against baseline
cargo bench --features parallel -- --baseline <name>
```

### Recommended: Save baseline before changes
```bash
# Save current performance as baseline
cargo bench --features parallel -- --save-baseline before_optimization

# After making changes, compare:
cargo bench --features parallel -- --baseline before_optimization
```

## Available Benchmarks

### 1. `parallel_scaling.rs` - Thread Scaling Analysis
Measures performance across different thread counts to find optimal parallelization.

**Benchmarks included:**
- `ldpc_decode_thread_scaling`: Decode 100 blocks with 1, 2, 4, 8, physical, and all cores
- `ldpc_encode_thread_scaling`: Encode 100 blocks with varying thread counts
- `parallel_vs_sequential`: Compare parallel vs sequential for batch sizes 10, 50, 100, 202
- `optimal_threads`: Test with physical cores only (recommended configuration)

**Run specific benchmark:**
```bash
cargo bench --bench parallel_scaling --features parallel -- ldpc_decode_thread_scaling
```

### 2. `ldpc_throughput.rs` - Overall Throughput
Existing benchmark for single and batch operations (uses all available threads).

```bash
cargo bench --bench ldpc_throughput --features parallel
```

### 3. `batch_operations.rs` - Backend Comparison
Tests LDPC and BCH batch operations with different batch sizes.

```bash
cargo bench --bench batch_operations --features parallel
```

## Thread Configuration

### Environment Variable Control
Set `RAYON_NUM_THREADS` to control thread count:

```bash
# Use 1 thread (sequential)
RAYON_NUM_THREADS=1 cargo bench --features parallel

# Use 8 threads
RAYON_NUM_THREADS=8 cargo bench --features parallel

# Use physical cores only (recommended)
RAYON_NUM_THREADS=$(lscpu -p | grep -v '^#' | sort -u -t, -k 2,4 | wc -l) \
  cargo bench --features parallel
```

### Programmatic Control
The `parallel_scaling` benchmark automatically tests multiple thread counts.

## Understanding Results

### Throughput Calculation
Criterion reports throughput in **MB/s** (information bits, not codeword bits).

For DVB-T2 Normal Rate 3/5:
- k = 38880 information bits per block
- Batch of 100 blocks = 3,888,000 bits = 486 KB

**Example output:**
```
ldpc_decode_thread_scaling/1_threads
                        time:   [12.50 s 12.52 s 12.54 s]
                        thrpt:  [38.78 KiB/s 38.84 KiB/s 38.90 KiB/s]

ldpc_decode_thread_scaling/8_threads
                        time:   [1.870 s 1.875 s 1.880 s]
                        thrpt:  [258.5 KiB/s 259.4 KiB/s 260.3 KiB/s]
```

**Speedup**: 12.52s / 1.875s = **6.68×** with 8 threads

### Parallel Efficiency
Efficiency = (Speedup / Num Threads) × 100%

**Example with 8 threads:**
- Speedup: 6.68×
- Efficiency: (6.68 / 8) × 100% = **83.5%**

Good parallel efficiency: >80%  
Excellent efficiency: >90%

### Interpreting Scaling Behavior

**Linear scaling** (ideal):
- 2 threads → 2× speedup
- 4 threads → 4× speedup
- 8 threads → 8× speedup

**Sub-linear scaling** (typical):
- 2 threads → 1.8× speedup (90% efficiency)
- 4 threads → 3.2× speedup (80% efficiency)
- 8 threads → 5.6× speedup (70% efficiency)

**Common bottlenecks:**
- Memory bandwidth (dominant at high core counts)
- Cache contention
- Synchronization overhead
- Hyperthreading (SMT) diminishing returns

## Recommended Configurations

### Development/Testing
```bash
# Fast iteration with single thread
cargo bench --features parallel -- --quick

# Quick scaling test (1, 2, 4, 8 threads)
RAYON_NUM_THREADS=1 cargo bench --bench parallel_scaling --features parallel -- optimal
RAYON_NUM_THREADS=8 cargo bench --bench parallel_scaling --features parallel -- optimal
```

### Performance Analysis
```bash
# Full thread scaling analysis
cargo bench --bench parallel_scaling --features parallel

# Compare against sequential baseline
cargo bench --bench parallel_scaling --features parallel -- parallel_vs_sequential
```

### Production Tuning
```bash
# Test physical cores only (avoids hyperthreading overhead)
./benchmark_threads.sh

# Profile specific thread count
RAYON_NUM_THREADS=12 cargo bench --bench ldpc_throughput --features parallel
```

## Expected Performance (DVB-T2 Normal Rate 3/5)

| Configuration | Throughput | Speedup | Efficiency |
|--------------|------------|---------|------------|
| 1 thread (sequential) | 1.2 Mbps | 1.0× | 100% |
| 2 threads | 2.2 Mbps | 1.8× | 90% |
| 4 threads | 4.0 Mbps | 3.3× | 83% |
| 8 threads | 6.8 Mbps | 5.7× | 71% |
| 12 threads (physical) | 8.3 Mbps | 6.9× | 58% |
| 24 threads (all) | 8.3 Mbps | 6.9× | 29% |

**Note**: Hyperthreading (12 → 24 threads) provides minimal benefit due to memory bandwidth saturation.

## Comparing Configurations

### Sequential vs Parallel
```bash
# Without parallel feature (sequential only)
cargo bench --bench ldpc_throughput

# With parallel feature
cargo bench --bench ldpc_throughput --features parallel
```

### Different Batch Sizes
Small batches (< 10) may not benefit from parallelization due to overhead.

```bash
cargo bench --bench parallel_scaling --features parallel -- parallel_vs_sequential
```

## Profiling Integration

For detailed performance analysis, combine with profiling tools:

```bash
# Profile with perf
RAYON_NUM_THREADS=8 cargo flamegraph --bench ldpc_throughput --features parallel

# Profile with cachegrind
RAYON_NUM_THREADS=8 cargo bench --bench parallel_scaling --features parallel -- \
  --profile-time=10
```

## Tips for Optimal Performance

1. **Use physical cores only**: Hyperthreading provides minimal benefit for memory-bound workloads
   ```bash
   RAYON_NUM_THREADS=$(lscpu -p | grep -v '^#' | sort -u -t, -k 2,4 | wc -l)
   ```

2. **Batch size matters**: Aim for batch_size ≥ 2× num_threads to avoid idle threads
   ```rust
   let batch_size = num_cpus::get_physical() * 2;
   ```

3. **Disable turbo boost for consistent results**:
   ```bash
   echo 1 | sudo tee /sys/devices/system/cpu/intel_pstate/no_turbo
   ```

4. **Pin to NUMA node for multi-socket systems**:
   ```bash
   numactl --cpunodebind=0 cargo bench --features parallel
   ```

5. **Warm up the cache**: Run benchmark multiple times, first run is often slower

## Troubleshooting

### No speedup observed
- Check if `parallel` feature is enabled: `cargo bench --features parallel`
- Verify rayon is being used: add `--verbose` flag to see compilation
- Ensure batch size is large enough (>= 10 blocks)

### Performance degrades with more threads
- Memory bandwidth saturation (expected >8 threads)
- Try reducing to physical cores only
- Check for NUMA effects on multi-socket systems

### Inconsistent results
- Disable CPU frequency scaling: `sudo cpupower frequency-set --governor performance`
- Close background applications
- Run multiple times and use median values

## Analyzing Results

### Extract throughput data from Criterion reports

```bash
# Parse throughput from all benchmarks
grep -r "thrpt:" target/criterion/*/base/benchmark.txt | \
  awk '{print $1, $2, $3}' > throughput_summary.txt

# Extract timing data
grep -r "time:" target/criterion/*/base/benchmark.txt | \
  awk '{print $1, $3, $4, $5}' > timing_summary.txt
```

### Calculate speedup

```python
# Example Python script to analyze results
import json
from pathlib import Path

criterion_dir = Path("target/criterion")
for bench_dir in criterion_dir.glob("*/base"):
    estimates = bench_dir / "estimates.json"
    if estimates.exists():
        with open(estimates) as f:
            data = json.load(f)
            mean_ns = data["mean"]["point_estimate"]
            print(f"{bench_dir.parent.name}: {mean_ns/1e9:.3f}s")
```

## Integration with CI/CD

Add performance regression tests:

```bash
# Baseline: 1 thread
RAYON_NUM_THREADS=1 cargo bench --features parallel -- --save-baseline sequential

# Test: 8 threads should be >5× faster
RAYON_NUM_THREADS=8 cargo bench --features parallel -- --baseline sequential
```

## Expected Output Format

When you run the overnight benchmarks, expect output like:

```
Benchmarking ldpc_decode_thread_scaling/1_threads
Benchmarking ldpc_decode_thread_scaling/1_threads: Warming up for 3.0000 s
Benchmarking ldpc_decode_thread_scaling/1_threads: Collecting 100 samples in estimated 345.24 s
Benchmarking ldpc_decode_thread_scaling/1_threads: Analyzing
ldpc_decode_thread_scaling/1_threads
                        time:   [3.4501 s 3.4524 s 3.4548 s]
                        thrpt:  [140.62 KiB/s 140.72 KiB/s 140.81 KiB/s]

Benchmarking ldpc_decode_thread_scaling/2_threads
...
```

The log file will contain:
- Time per benchmark: mean, std dev, confidence intervals
- Throughput: in KiB/s or MiB/s
- Speedup vs baseline (if --baseline used)

## References

- [Rayon Documentation](https://docs.rs/rayon/)
- [Criterion User Guide](https://bheisler.github.io/criterion.rs/book/)
- [PARALLELIZATION_STRATEGY.md](./PARALLELIZATION_STRATEGY.md) - Overall architecture
- [LDPC_PERFORMANCE_PLAN.md](./LDPC_PERFORMANCE_PLAN.md) - LDPC-specific optimizations
