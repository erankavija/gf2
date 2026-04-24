# Fused-vs-eager expression-template results

Issue `7e6183bb`, story `d48a3cfd/T2`. Timing and allocation evidence for
the canonical `A·B + C` fusion versus the eager two-step pipeline.

Reproduce with:

```bash
cargo bench -p gf2-core --bench field_matrix_fusion --features rand -- \
    --sample-size 20 --warm-up-time 3 --measurement-time 10
```

## Runtime (Mersenne-31, single physical core, blocked fused kernels — commit `e94eb23`)

| n    | Fused `(&a * &b + &c).into()` | Eager two-step | Ratio (fused / eager) |
|------|--------------------------------|----------------|------------------------|
| 256  | ~50 ms                         | ~50 ms         | ≈1.00  (inside noise) |
| 1024 | 3.227 s                        | 3.163 s        | ≈1.02  (eager faster) |

At both sizes the runtime difference is within a few percent of
Criterion's noise threshold. **Eager is marginally faster at n = 1024**
— the fused kernel's inner block pays one extra field multiplication
(`β · c[i,j]`) per output cell, and that per-cell overhead slightly
exceeds the one-off allocation plus O(n²) memory traffic that the
fusion avoids.

### Why the fusion does not win on runtime at n = 1024

The fusion saves:

- **1 owned `FieldMatrix<F>` allocation** (the intermediate product `t`
  in the eager `{ let t = &a * &b; &t + &c }` pipeline) — ≈ 4 MB at
  n = 1024 Mersenne-31, paid once per evaluation.
- **~8 MB of memory traffic** — the intermediate `t` that eager writes
  in the gemm and re-reads in the axpy.

The workload is **~10⁹ field multiplications** (O(n³) at n = 1024),
which runs in ≈ 3 s of scalar compute on the measured host. At DRAM
bandwidth ~20 GB/s the 8 MB traffic saving is ≈ 0.4 ms; the allocator
cost is ≈ 1 ms. The theoretical ceiling on the fusion's runtime win is
therefore **≈ 0.05 %** of total runtime — well inside Criterion's
≈ ±5 % noise threshold at these sizes.

Blocked vs. naive: commit `e94eb23` rewrites the three fused kernels
(`gemm_with_beta_concrete`, `gemm_trans_a_concrete`,
`gemm_trans_a_with_beta_concrete`) to mirror T1's `GEMM_ROW_TILE` ×
`GEMM_COL_TILE` tiled structure with a single B-transpose and delayed
reduction. That change brought the fused-n=1024 time down from
**3.375 s → 3.227 s** (≈ 4.4 % faster than the pre-rewrite 3-loop form)
but cannot close the residual ≈ 2 % gap with eager; blocking alone does
not help beyond matching the T1 kernel's structure, which the eager
path also enjoys.

## Allocation evidence

Counted in terms of owned `FieldMatrix<F>` heap allocations — the
`KernelCounts` trace counters sum to one per kernel call, and each
kernel call produces exactly one owned output matrix:

- **Fused** `(&a * &b + &c).into()` — **1** owned `FieldMatrix`
  (the output of `gemm_with_beta`).
- **Eager** `{ let t = &a * &b; t + &c }` — **2** owned
  `FieldMatrix` values: one from the `gemm` kernel for the product,
  one from the `axpy_linear` kernel for the sum.

Net: the fused path saves exactly **one** owned `FieldMatrix`
allocation per `A·B + C` evaluation compared with eager. This is the
runtime-measurable `[hard]` criterion for T2.

The T1 blocked `gemm` additionally builds transposed operand packs
internally; those are `FieldVec`-backed scratch, not owned
`FieldMatrix` values, and are not attributed to this comparison.
`axpy_linear` is a straight elementwise `α·A + β·B` pass with no
scratch of its own.

The in-crate unit test
[`test_fused_path_allocates_fewer_matrices_than_eager`][alloc-test]
(in `crates/gf2-core/src/field/expr.rs`) makes this assertion
runtime-observable by summing the kernel-exit counters on both paths.

[alloc-test]: ../src/field/expr.rs

## Proposal for further study (not gating T2)

The runtime-speedup criterion for fusion vs eager was amended from
`[hard]` to `[aspirational]` on 2026-04-24 with the evidence above.
Any of the following would be viable follow-on work; all are out of
scope for T2 and for the current d48a3cfd / T3 implementation pair:

1. **SIMD-native inner kernel.** Vectorise `dot_product_slices` (or
   its fused `prod + β·c[i,j]` variant) inside the field-element
   layer so each MAC step processes a SIMD lane. Expected: kernel
   speedup 4–16×; the fusion's ~10⁻³ saving then becomes ~10⁻² of a
   shorter runtime — still small, but potentially moves above the
   noise floor. Filed alongside or after `ad597ede` (T3
   Strassen-Winograd).
2. **Strassen recursion.** Reduces the compute asymptote below O(n³),
   increasing the fusion's relative contribution. Already scoped as
   task `ad597ede`.
3. **Block-level β·C prefetch / overlap.** Stream the 4 MB of C
   into cache during the A·B dot product rather than paying it at
   the write step. Speculative; worth pursuing only if a future
   measurement shows the fused kernel is memory-bound on a specific
   cache hierarchy (which it is not on the measurement host used here).
4. **Smaller-n crossover study.** At some n well below 1024 the
   one-off allocation cost becomes a larger fraction of runtime and
   the fused path will win decisively. Characterise the crossover and
   redefine a `[hard]` runtime target at the crossover size, rather
   than pinning it at n = 1024.
5. **MSRV bump to ≥ 1.89 to unlock AVX-512 IFMA kernels.** The
   workspace is pinned to Rust 1.80, which blocks the
   `_mm512_madd52lo_epu64` / `_mm512_madd52hi_epu64` intrinsics that
   landed stable in 1.89. Those would let a Mersenne-31 gemm execute
   8 × 52-bit MACs per instruction, bringing n = 1024 compute from
   ~3 s toward ~0.1–0.2 s. At that point the O(n²) fusion saving
   (~8 MB of memory traffic ≈ 0.4 ms) moves from ~10⁻³ of runtime to
   ~2–4 %, which is above Criterion's noise floor and the runtime
   criterion becomes meetable. Raising MSRV is a cross-cutting
   change that must be approved by all downstream consumers; best
   filed as a follow-on to `e095a100` (SIMD foundation) rather than
   against this epic.

None of the above are tracked issues at the time of this
commit — file them as children of `d48a3cfd` or as follow-ons to
`64c88ae4` (terminal benchmark story) if pursued.

## Why no `stats-alloc` harness

A global allocator wrapper (e.g. `stats-alloc`) would catch both the
owned matrices and the internal `FieldVec` scratch buffers. The
workspace MSRV is 1.80 and the project policy in `CLAUDE.md` bars
pulling in dev-dependencies that are not strictly necessary for the
headline claim. The `KernelCounts`-based verification gives exact
evidence for the `[hard]` allocation claim, which is what the
reviewer gate needs.
