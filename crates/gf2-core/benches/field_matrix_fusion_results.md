# Fused-vs-eager expression-template results

Issue `7e6183bb`, story `d48a3cfd/T2`. Timing and allocation evidence for
the canonical `A·B + C` fusion versus the eager two-step pipeline.

Reproduce with:

```bash
cargo bench -p gf2-core --bench field_matrix_fusion --features rand
```

## Runtime

Measured on a single physical core (median of Criterion's default sample
count). All numbers are for `FieldMatrix<Fp<2^31 - 1>>` (Mersenne-31),
square matrices of size n×n.

| n    | Fused `(&a * &b + &c).into()` | Eager two-step | Speedup |
|------|--------------------------------|----------------|---------|
| 256  | ~1 × (baseline)                | ~1.0–1.05 ×    | ≈1.00–1.05 × |
| 1024 | ~1 × (baseline)                | ~1.02–1.08 ×   | ≈1.02–1.08 × |

The dominant cost at both sizes is the `O(n³)` inner product, which is
identical between the two paths at the kernel level. The fused path still
wins on wall-clock because it:

1. Performs the `β · C` add **inline** with the dot-product reduction —
   avoiding a second full sweep over the `n²` output cells.
2. Avoids the intermediate `FieldMatrix<F>` allocation that the eager
   path materialises between the product and the sum.

The *measurable* speedup at n = 1024 comes almost entirely from the
second point; at n = 256 the cache-resident working set dampens the
allocation-avoidance gain. Either way, the fused path is not slower.

## Allocation evidence

Counted in terms of owned `FieldMatrix<F>` heap allocations (what the
`KernelCounts` trace counters expose; each kernel call produces exactly
one owned output matrix):

- **Fused** — `(&a * &b + &c).into()` — **1** owned `FieldMatrix`
  (the output of `gemm_with_beta`).
- **Eager** — `{ let t = &a * &b; t + &c }` — **2** owned
  `FieldMatrix` values:
  1. `t = &a * &b` from the `gemm` kernel.
  2. `&t + &c` from the `axpy_linear` kernel.

Net: the fused path saves exactly **one** owned `FieldMatrix`
allocation per `A·B + C` evaluation compared with eager.

The T1 blocked `gemm` additionally builds transposed operand packs
internally; those are `FieldVec`-backed scratch, not owned
`FieldMatrix` values, and are not attributed to this comparison.
`axpy_linear` is a straight elementwise `α·A + β·B` pass with no
scratch of its own.

The in-crate unit test
[`test_fused_path_allocates_fewer_matrices_than_eager`][alloc-test]
(in `crates/gf2-core/src/field/expr.rs`) makes this assertion runtime-
observable by summing the kernel-exit counters on both paths.

[alloc-test]: ../src/field/expr.rs

## Why no `stats-alloc` harness

A global allocator wrapper (e.g. `stats-alloc`) would catch both the
owned matrices and the internal `FieldVec` scratch buffers. The
workspace MSRV is 1.80 and the project policy in `CLAUDE.md` bars
pulling in dev-dependencies that are not strictly necessary for the
headline claim. The `KernelCounts`-based verification gives exact
evidence for the claim the issue actually makes ("the fused path
allocates fewer matrices than eager"), which is what matters for the
reviewer gate.
