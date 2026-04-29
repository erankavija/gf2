# BitMatrix Strassen-family crossover — 2026-04-29

JIT issue: `59c487c3`.

This run measures the new safe Rust Strassen-family layer for square GF(2)
`BitMatrix` multiplication against the existing M4RM leaf path. The Strassen
implementation is retained as a forced correctness/benchmark hook, while
production `BitMatrix` multiplication remains on M4RM until a measured crossover
shows Strassen winning.

## Command

```bash
RUSTFLAGS="-C target-cpu=native" \
  cargo bench -p gf2-core --features test-support,simd --bench matmul -- \
  matmul_square_strassen_compare --warm-up-time 1 --measurement-time 2 --sample-size 10
```

Host-side notes: Criterion emitted wide confidence intervals at the largest
sizes because the requested two-second measurement window allowed only ten
8192×8192 samples. Treat the 8192 row as evidence that this implementation has
not crossed over by that size, not as a precise throughput claim.

## Final policy after rework

Selected automatic crossover: **none**.

Reason: forced one-level Strassen did **not** beat M4RM through n=8192, and no
measurement on this branch shows n=16,384 or any larger size winning. A threshold
above the largest measured non-crossover would be an unproven production route
into a potentially slower path. Production `BitMatrix` multiplication therefore
keeps the existing M4RM/scalar fallback behavior for all real workloads.

The safe Rust Strassen-family implementation remains available only through
`#[cfg(any(test, feature = "test-support"))]` correctness and benchmark hooks
so future scratch/view optimization work can reuse the implementation without
changing public APIs.

## Criterion results from this branch

| n | M4RM baseline mean | auto dispatch mean | forced 1-level Strassen mean | auto throughput |
|---:|---:|---:|---:|---:|
| 1024 | 0.796 ms | 0.786 ms | not run | 1,366 Gops/s |
| 2048 | 6.077 ms | 5.796 ms | 5.924 ms | 1,482 Gops/s |
| 4096 | 42.399 ms | 42.378 ms | 46.264 ms | 1,622 Gops/s |
| 8192 | 325.788 ms | 308.086 ms | 312.986 ms | 1,784 Gops/s |

The auto column was M4RM for all measured rows. The small auto/base differences
are benchmark noise and run-order effects; after this rework auto dispatch is
M4RM for all production sizes because the branch has no empirically selected
Strassen crossover.

## Status against the issue criterion

This branch proves correctness of the Strassen-family implementation and keeps
existing M4RM/scalar fallbacks available. It does **not** meet the hard
production-path crossover criterion: the copy-based Strassen layer has no
measured winning crossover within the feasible measurement budget recorded here.
Meeting that criterion requires a follow-up design decision, likely scratch/view
temporaries or another lower-allocation Strassen-Winograd schedule, followed by
new measurements that identify a real crossover.

## Comparison to pinned `64c88ae4` report

The 2026-04-26 report measured pure M4RM before the later Tier-A/B kernel work.
M4RI ratios are available there for n=1024 and n=4096.

| n | pinned gf2 wall / throughput | this branch auto wall / throughput | M4RI throughput from `64c88ae4` | ratio then | ratio now |
|---:|---:|---:|---:|---:|---:|
| 1024 | 5.542 ms / 387.462 Gops/s | 0.786 ms / 1,366 Gops/s | 3,020.762 Gops/s | 0.13× | 0.45× |
| 4096 | 125.756 ms / 1,092.904 Gops/s | 42.378 ms / 1,622 Gops/s | 6,272.592 Gops/s | 0.17× | 0.26× |

Interpretation: current Tier-A/B leaf improvements substantially improved the
baseline available to Strassen leaves, but this copy-based high-level Strassen
layer still does not cross over by n=8192. Production dispatch deliberately
stays on M4RM rather than guessing a larger unmeasured crossover.
