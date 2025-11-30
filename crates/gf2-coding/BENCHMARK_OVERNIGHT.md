# Overnight Benchmark Quick Start

## Running the Full Benchmark Suite

```bash
cd crates/gf2-coding
./run_overnight_benchmarks.sh
```

**Duration**: 2-4 hours  
**Output**: `benchmark_results_<timestamp>.log`  
**Results**: `target/criterion/report/index.html`

## What Gets Measured

1. **Sequential baseline** (1 thread)
2. **Thread scaling** (1, 2, 4, 8, 12, 24 threads)
3. **LDPC throughput** (encode/decode)
4. **Batch operations** (various batch sizes)

## After Completion

### View HTML Reports
```bash
xdg-open target/criterion/report/index.html  # Linux
open target/criterion/report/index.html      # macOS
```

### Check Results
```bash
# View log file
less benchmark_results_*.log

# Quick throughput summary (at end of log)
tail -30 benchmark_results_*.log

# Extract all throughput data
grep "thrpt:" benchmark_results_*.log > throughput_summary.txt
```

### Analyze Speedup
The log contains timing data for each thread count. Calculate speedup:

```
Speedup = Time(1 thread) / Time(N threads)
Efficiency = Speedup / N threads × 100%
```

Example from log:
```
1_threads:  time: [3.45 s]  thrpt: [140 KiB/s]
8_threads:  time: [0.52 s]  thrpt: [930 KiB/s]
Speedup: 3.45 / 0.52 = 6.6×
Efficiency: 6.6 / 8 × 100% = 83%
```

## Expected Results (12-core system)

| Threads | Expected Throughput | Expected Speedup | Efficiency |
|---------|---------------------|------------------|------------|
| 1       | 1.2 Mbps            | 1.0×             | 100%       |
| 2       | 2.2 Mbps            | 1.8×             | 90%        |
| 4       | 4.0 Mbps            | 3.3×             | 83%        |
| 8       | 6.8 Mbps            | 5.7×             | 71%        |
| 12      | 8.3 Mbps            | 6.9×             | 58%        |
| 24      | 8.3 Mbps            | 6.9×             | 29%        |

Memory bandwidth saturation expected beyond 8-12 threads.

## Next Steps After Results

1. Update `ROADMAP.md` with measured performance
2. Update `PARALLELIZATION_STRATEGY.md` with scaling data
3. Document optimal thread count (likely physical cores = 12)
4. Create performance table for documentation

## Troubleshooting

**If script fails:**
- Check disk space: `df -h .`
- Check cargo is working: `cargo --version`
- Run individual benchmarks: `cargo bench --bench parallel_scaling --features parallel`

**If results are inconsistent:**
- Disable CPU frequency scaling: `sudo cpupower frequency-set --governor performance`
- Close background applications
- Run again and use median values

## Documentation References

- Full guide: [docs/PARALLEL_BENCHMARKING.md](docs/PARALLEL_BENCHMARKING.md)
- Architecture: [docs/PARALLELIZATION_STRATEGY.md](docs/PARALLELIZATION_STRATEGY.md)
- Roadmap: [ROADMAP.md](ROADMAP.md)
