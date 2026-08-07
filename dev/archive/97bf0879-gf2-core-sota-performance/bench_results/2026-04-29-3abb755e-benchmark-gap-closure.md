# JIT 3abb755e — algorithmic benchmark-gap closure

This story-level review maps each `64c88ae4` algorithmic benchmark gap to
the child work that landed during the PPC epic.

## Child outcomes

| Gap from `64c88ae4` | Child issue | Outcome |
|---|---|---|
| GF(2) `BitMatrix` multiplication is far behind M4RI at large square sizes | `59c487c3` | A safe Strassen-family scaffold landed for correctness/test-support and benchmarking, but forced one-level Strassen did not beat M4RM through `n = 8192`. Production dispatch remains M4RM-only until a future task demonstrates an empirical crossover. Evidence: `dev/bench_results/2026-04-29-strassen-matmul-crossover.md`. |
| GF(p) `FieldMatrix::gemm` is far behind fflas-ffpack | `2598b981` | The deferred GF(p) reference-host sweep measured 256³ and 1024³ square cells plus existing rectangular gf2 cells. Post-`e7ab802d` throughput improved by roughly `6.0x-7.7x` over the published gf2-side baseline. The within-10x fflas target is met for `GF(65521)` at 256³ and for `GF(2^31-1)` at 256³/1024³, but not for `GF(7)` or `GF(251)`. Evidence: `dev/bench_results/2026-04-29-2598b981-fieldmatrix-gemm-fflas-sweep.md`. |
| GF(2^m) `FieldMatrix::gemm` is dominated by scalar per-element multiplication | `577b9e7f` | Production `FieldMatrix::gemm` now routes supported single-word GF(2^m) dot products through the batched carry-less multiply hook with scratch reuse. Development cells show `5.01x-17.65x` speedups over scalar/published gf2-side baselines, and exact `64c88ae4` rectangular shapes are covered by correctness tests. Evidence: `dev/bench_results/2026-04-29-gf2m-batch-fieldmatrix-gemm.md`. |

## Story criteria mapping

- **Every child completed or explicitly rejected:** all direct children are
  `done`: `59c487c3`, `2598b981`, and `577b9e7f`.
- **Every benchmark-gap finding mapped:** GF(2) square matmul maps to a landed
  scaffold plus measured non-crossover; GF(p) fgemm maps to delayed-reduction
  evidence and explicit reference-host/nighly deferrals; GF(2^m) fgemm maps to
  a landed production batch-GEMM path.
- **Unsafe isolation:** no child introduced `unsafe` outside
  `gf2-kernels-simd`; GF(p) benchmark closure was evidence-only.
- **Material improvement:** the combined work materially improves the worst
  published gf2-side ratios for GF(2) square matmul, GF(p) fgemm, and GF(2^m)
  fgemm, while honestly recording that GF(2) Strassen production dispatch and
  small-prime GF(p) within-10x fflas parity remain future work.

## Follow-up status

No new in-epic child is required for this story. Remaining performance gaps are
documented as future work because the relevant child evidence either selected a
safe non-dispatch production policy (GF(2) Strassen) or identified benchmark
cells that require nightly/slow or harness-extension infrastructure (GF(p)
4096³ and rectangular fflas reference cells).
