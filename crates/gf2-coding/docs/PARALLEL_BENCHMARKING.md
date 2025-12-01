# Parallel Performance Benchmarking Guide

Quick benchmarks for fast feedback on parallel performance scaling.

## Quick Benchmark (~2 minutes)

Fast benchmark with small batch sizes for rapid iteration:

```bash
# Test with 1 thread (baseline)
RAYON_NUM_THREADS=1 cargo bench --bench quick_parallel --features parallel

# Test with 8 threads
RAYON_NUM_THREADS=8 cargo bench --bench quick_parallel --features parallel

# Test with all available cores (default)
cargo bench --bench quick_parallel --features parallel
```

## Automated Thread Scaling (~5 minutes)

Run quick benchmark across multiple thread counts:

```bash
./benchmark_quick.sh
```

This tests: 1, 2, 4, 8, and physical_cores threads.

## Results Location

```bash
# View HTML reports
xdg-open target/criterion/report/index.html  # Linux
open target/criterion/report/index.html      # macOS
```

## Available Benchmarks

### `quick_parallel.rs` - Fast Iteration (Recommended)
Quick benchmark with small batches (10-20 blocks) for rapid feedback.

**Benchmarks:**
- `ldpc_encode_quick`: Encode 10 blocks
- `ldpc_decode_quick`: Decode 10 blocks (20 iterations)
- `batch_size_scaling`: Compare batch sizes 1, 5, 10, 20

**Usage:**
```bash
RAYON_NUM_THREADS=1 cargo bench --bench quick_parallel --features parallel
RAYON_NUM_THREADS=8 cargo bench --bench quick_parallel --features parallel
```

### `ldpc_throughput.rs` - Full Throughput Baseline
Comprehensive throughput measurement (slower, more accurate).

```bash
cargo bench --bench ldpc_throughput --features parallel
```

### `batch_operations.rs` - Backend Comparison
Tests LDPC and BCH batch operations with various sizes.

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
The `quick_parallel` benchmark tests different batch sizes with the configured thread count.

## Understanding Results

### Throughput Calculation
Criterion reports throughput in **MB/s** (information bits, not codeword bits).

For DVB-T2 Normal Rate 3/5:
- k = 38880 information bits per block
- Batch of 100 blocks = 3,888,000 bits = 486 KB

**Example output:**
```
batch_size_scaling/10 (1 thread)
                        time:   [323.77 ms 323.51 ms 324.36 ms]
                        thrpt:  [146.32 KiB/s 146.71 KiB/s 147.04 KiB/s]

batch_size_scaling/10 (8 threads)
                        time:   [82.216 ms 82.686 ms 83.145 ms]
                        thrpt:  [570.82 KiB/s 573.99 KiB/s 577.27 KiB/s]
```

**Speedup**: 323.51ms / 82.686ms = **3.9×** with 8 threads

### Parallel Efficiency
Efficiency = (Speedup / Num Threads) × 100%

**Example with 8 threads:**
- Speedup: 3.9×
- Efficiency: (3.9 / 8) × 100% = **49%**

Good parallel efficiency: >70%  
Excellent efficiency: >85%

**Note**: Current 49% efficiency suggests room for optimization (SIMD LLR operations next)

## Thread Configuration

Control thread count via `RAYON_NUM_THREADS`:

```bash
# Sequential baseline
RAYON_NUM_THREADS=1 cargo bench --bench quick_parallel --features parallel

# Physical cores (recommended)
RAYON_NUM_THREADS=12 cargo bench --bench quick_parallel --features parallel

# All cores (with hyperthreading)
cargo bench --bench quick_parallel --features parallel
```

## Interpreting Results

**Speedup** = Time(1 thread) / Time(N threads)  
**Efficiency** = Speedup / N × 100%

Good parallel efficiency: >70%  
Excellent efficiency: >85%

**Common bottlenecks:**
- Memory bandwidth saturation at high core counts
- Small batch sizes (overhead dominates)
- Hyperthreading diminishing returns
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
