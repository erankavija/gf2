# BitMatrix Strassen-family crossover — 2026-04-29

JIT issue: `59c487c3`.

This run measures the new safe Rust Strassen-family layer for square GF(2)
`BitMatrix` multiplication against the existing M4RM leaf path.  The layer is
wired through `BitMatrix`'s `Mul` implementations, while `alg::m4rm::multiply`
remains the explicit M4RM baseline.

## Command

```bash
RUSTFLAGS="-C target-cpu=native" \
  cargo bench -p gf2-core --features test-support,simd --bench matmul -- \
  matmul_square_strassen_compare --warm-up-time 1 --measurement-time 2 --sample-size 10
```

Host-side notes: Criterion emitted wide confidence intervals at the largest
sizes because the requested two-second measurement window allowed only ten
8192×8192 samples. Treat 8192 as crossover guardrail evidence, not a precise
throughput claim.

## Final policy

Selected automatic crossover: **n ≥ 16,384**, square power-of-two matrices only,
one Strassen-family level with M4RM leaves.

Reason: forced one-level Strassen did **not** beat M4RM through n=8192. The
automatic dispatcher therefore preserves the existing M4RM/scalar fallback in
all measured sizes and only enables the Strassen-family path above the measured
non-crossover range. Rectangular, non-square, and non-power-of-two inputs always
fall back to M4RM.

## Criterion results from this branch

| n | M4RM baseline mean | auto dispatch mean | forced 1-level Strassen mean | auto throughput |
|---:|---:|---:|---:|---:|
| 1024 | 0.796 ms | 0.786 ms | not run | 1,366 Gops/s |
| 2048 | 6.077 ms | 5.796 ms | 5.924 ms | 1,482 Gops/s |
| 4096 | 42.399 ms | 42.378 ms | 46.264 ms | 1,622 Gops/s |
| 8192 | 325.788 ms | 308.086 ms | 312.986 ms | 1,784 Gops/s |

The auto column is M4RM for all rows in this table under the final n≥16384
policy. The small auto/base differences are benchmark noise and run-order
effects; the dispatch predicate is covered by unit tests. The hard production
path exists above the conservative threshold, but the measured-size speedup is
from the M4RM/Tier-A/B leaf improvements rather than from Strassen; future work
that wants a lower crossover should revisit scratch/view optimization instead
of forcing Strassen onto these measured sizes.

## Comparison to pinned `64c88ae4` report

The 2026-04-26 report measured pure M4RM before the later Tier-A/B kernel work.
M4RI ratios are available there for n=1024 and n=4096.

| n | pinned gf2 wall / throughput | this branch auto wall / throughput | M4RI throughput from `64c88ae4` | ratio then | ratio now |
|---:|---:|---:|---:|---:|---:|
| 1024 | 5.542 ms / 387.462 Gops/s | 0.786 ms / 1,366 Gops/s | 3,020.762 Gops/s | 0.13× | 0.45× |
| 4096 | 125.756 ms / 1,092.904 Gops/s | 42.378 ms / 1,622 Gops/s | 6,272.592 Gops/s | 0.17× | 0.26× |

Interpretation: current Tier-A/B leaf improvements substantially improved the
baseline available to Strassen leaves, but this copy-based high-level Strassen
layer still does not cross over by n=8192. The conservative threshold avoids
regressing measured production sizes while keeping the recursive path available
for larger square powers of two and future scratch/view optimizations.
