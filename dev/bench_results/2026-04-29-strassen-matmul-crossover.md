# BitMatrix Strassen-family crossover — 2026-04-29

JIT issue: `59c487c3`.

This document records the square GF(2) `BitMatrix` Strassen-family crossover
measurements on branch `worktree-agent-59c487c3`. The latest rework replaces
up-front quadrant copies with word-aligned square views where possible and XORs
intermediate products directly into the output quadrants, with per-product
scopes so large temporaries are dropped promptly. Recursive leaves still call
the existing M4RM path, so scalar/SIMD fallback behaviour is preserved.

## Commands

Final measured command:

```bash
RUSTFLAGS="-C target-cpu=native" \
  cargo bench -p gf2-core --features test-support,simd --bench matmul -- \
  matmul_square_strassen_compare --warm-up-time 1 --measurement-time 2 --sample-size 10
```

Validation commands for the rework:

```bash
cargo fmt -p gf2-core -- --check
RUSTFLAGS="-C target-cpu=native" \
  cargo nextest run --manifest-path Cargo.toml -p gf2-core \
  --features test-support,simd --release --profile ci -E 'test(strassen)'
```

Host-side notes: Criterion emitted warnings at the largest sizes because the
two-second measurement window allowed only ten 8192×8192 samples. Treat the
8192 row as evidence that this implementation has not crossed over by that
size, not as a precise throughput claim.

## Final policy after scratch/view rework

Selected automatic crossover: **none**.

Reason: forced one-level Strassen did **not** beat M4RM through n=8192 after the
view/reduced-temporary rework. A threshold above the largest measured
non-crossover would be an unproven production route into a slower path.
Production `BitMatrix` multiplication therefore keeps the existing M4RM/scalar
fallback behaviour for all real workloads.

The safe Rust Strassen-family implementation remains available only through
`#[cfg(any(test, feature = "test-support"))]` correctness and benchmark hooks so
future lower-overhead experiments can reuse the implementation without changing
public APIs.

## Criterion results from the final scratch/view rework

| n | M4RM baseline mean | auto dispatch mean | forced 1-level Strassen mean | forced / M4RM | auto throughput |
|---:|---:|---:|---:|---:|---:|
| 1024 | 0.858 ms | 0.837 ms | not run | n/a | 1,283 Gops/s |
| 2048 | 5.241 ms | 5.298 ms | 6.278 ms | 0.83× | 1,622 Gops/s |
| 4096 | 35.900 ms | 35.667 ms | 41.820 ms | 0.86× | 1,927 Gops/s |
| 8192 | 256.700 ms | 257.560 ms | 290.310 ms | 0.88× | 2,134 Gops/s |

The auto column was M4RM for all measured rows. The small auto/base differences
are benchmark noise and run-order effects; after this rework auto dispatch is
M4RM for all production sizes because the branch has no empirically selected
Strassen crossover.

## Intermediate measurement during this rework pass

Before adding explicit scopes to drop per-product temporary matrices promptly,
the same command measured:

| n | M4RM baseline mean | auto dispatch mean | forced 1-level Strassen mean | forced / M4RM |
|---:|---:|---:|---:|---:|
| 1024 | 0.849 ms | 0.878 ms | not run | n/a |
| 2048 | 5.344 ms | 5.360 ms | 5.906 ms | 0.90× |
| 4096 | 35.262 ms | 35.535 ms | 42.059 ms | 0.84× |
| 8192 | 266.950 ms | 266.770 ms | 294.610 ms | 0.91× |

This confirmed that the view/direct-accumulation rewrite reduced some overhead
relative to the previous copy-heavy report, but still did not create a winning
crossover. The final scoped version reduced peak live temporary storage but did
not change the production decision.

## Status against the issue criterion

This branch preserves correctness of the Strassen-family implementation and
keeps existing M4RM/scalar fallbacks available. It still does **not** meet the
hard production-path crossover criterion: the focused scratch/view rework found
no measured winning crossover within the feasible measurement budget recorded
here. Meeting that criterion likely requires a deeper design, such as an M4RM
leaf that consumes square views directly without materialising leaf quadrants,
a lower-addition Winograd schedule that wins empirically, or a different
matrix-level algorithm.

## Comparison to pinned `64c88ae4` report

The 2026-04-26 report measured pure M4RM before the later Tier-A/B kernel work.
M4RI ratios are available there for n=1024 and n=4096.

| n | pinned gf2 wall / throughput | this branch auto wall / throughput | M4RI throughput from `64c88ae4` | ratio then | ratio now |
|---:|---:|---:|---:|---:|---:|
| 1024 | 5.542 ms / 387.462 Gops/s | 0.837 ms / 1,283 Gops/s | 3,020.762 Gops/s | 0.13× | 0.42× |
| 4096 | 125.756 ms / 1,092.904 Gops/s | 35.667 ms / 1,927 Gops/s | 6,272.592 Gops/s | 0.17× | 0.31× |

Interpretation: current Tier-A/B leaf improvements substantially improved the
baseline available to Strassen leaves, but the safe high-level Strassen layer
still does not cross over by n=8192. Production dispatch deliberately stays on
M4RM rather than guessing a larger unmeasured crossover.
