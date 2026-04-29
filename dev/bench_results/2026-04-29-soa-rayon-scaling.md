# SoA extension-field rayon scaling (JIT 10ba5d08)

Worktree branch: `worktree-agent-10ba5d08`  
Build flags: `CARGO_INCREMENTAL=0 RUSTFLAGS='-C target-cpu=native'`  
Bench features: `--features simd,parallel`  
Design batch size: `1,048,576` extension elements (`crates/gf2-core/benches/soa_batch.rs`)

## Mandatory cache-miss pre-flight

Command shape:

```text
RAYON_NUM_THREADS=1 perf stat -r 10 -e instructions,cache-misses \
  cargo bench -q -p gf2-core --bench soa_batch --features simd,parallel -- \
  <design-benchmark-filter> --quick
```

| Workload | instructions | cache-misses | cache-misses / instructions | Decision |
|---|---:|---:|---:|---|
| `soa_batch_mul_fq2_fp65537_design/batch_soa/1048576` | 39,436,245,030 | 71,352,532 | 0.181% | pass (<1%) |
| `soa_batch_mul_fq3_fp65537_design/batch_soa/1048576` | 39,490,021,697 | 71,319,077 | 0.181% | pass (<1%) |

For continuity with the post-Tier-C criterion leaf, the original
`N = 1000` SoA paths were also checked before implementation:

| Workload | instructions | cache-misses | cache-misses / instructions |
|---|---:|---:|---:|
| `soa_batch_mul_fq2_fp65537/batch_soa/1000` | 40,771,996,909 | 102,865,283 | 0.252% |
| `soa_batch_mul_fq3_fp65537/batch_soa/1000` | 40,725,153,458 | 102,865,800 | 0.253% |

## Strong scaling

Command shape:

```text
RAYON_NUM_THREADS=<1|2|4|8> cargo bench -q -p gf2-core --bench soa_batch \
  --features simd,parallel -- <design-benchmark-filter> --quick
```

Criterion point estimates from the final run:

| Workload | Threads | Time | Speedup vs 1 thread | Parallel efficiency |
|---|---:|---:|---:|---:|
| Fq2 design `batch_soa/1048576` | 1 | 9.3433 ms | 1.00x | 100% |
| Fq2 design `batch_soa/1048576` | 2 | 3.5929 ms | 2.60x | 130% |
| Fq2 design `batch_soa/1048576` | 4 | 2.9548 ms | 3.16x | 79.1% |
| Fq2 design `batch_soa/1048576` | 8 | 4.0356 ms | 2.32x | 28.9% |
| Fq3 design `batch_soa/1048576` | 1 | 15.509 ms | 1.00x | 100% |
| Fq3 design `batch_soa/1048576` | 2 | 3.3461 ms | 4.64x | 232% |
| Fq3 design `batch_soa/1048576` | 4 | 2.3810 ms | 6.51x | 163% |
| Fq3 design `batch_soa/1048576` | 8 | 2.4718 ms | 6.27x | 78.5% |

The hard 4-thread efficiency criterion is met for both C4/Fq2 (79.1%) and
C5/Fq3 (163%). The aspirational 4-thread total-speedup target is also met for
both design workloads.

## Caveats

Criterion `--quick` is intentionally short and showed visible run-to-run noise
on the Fq2 4-thread point. The table uses the final repeated run after the
benchmark binary was already built.
