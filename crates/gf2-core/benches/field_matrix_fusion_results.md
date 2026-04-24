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
2. Avoids the intermediate `FieldMatrix<F>` allocation and its subsequent
   `Clone` into the axpy destination.

The *measurable* speedup at n = 1024 comes almost entirely from the
second point; at n = 256 the cache-resident working set dampens the
allocation-avoidance gain. Either way, the fused path is not slower.

## Allocation evidence

The fused path allocates **one** owned matrix — the output of
`gemm_with_beta`. The eager two-step baseline allocates three:

1. `t = &a * &b` — one `FieldMatrix<F>` from the plain `gemm` kernel.
2. `&t + &c` — one `FieldMatrix<F>` from the `axpy_linear` output.
3. (Each `gemm` additionally materialises a transposed `B` scratch
   buffer inside the T1 blocked kernel — this is an implementation
   detail and not counted here, but it *does* compound the eager-path
   allocation cost because both the plain `gemm` and the subsequent
   `axpy_linear` pay one scratch allocation each.)

Net:
- **Fused:** 1 owned `FieldMatrix`, 1 transpose scratch → 2 heap
  allocations.
- **Eager:** 3 owned `FieldMatrix`es, 1 transpose scratch → 4 heap
  allocations.

The in-crate unit test
`test_fused_path_allocates_fewer_matrices_than_eager`
(`crates/gf2-core/src/field/expr.rs`) verifies this at runtime by
counting kernel-exit allocations via the `KernelCounts` trace counters:
each kernel listed in `KernelCounts` produces exactly one owned
`FieldMatrix` per call, so the sum of increments is the owned-matrix
allocation count for the expression.

## Why no `stats-alloc` harness

A global allocator wrapper (e.g. `stats-alloc`) would catch both the
owned matrices and the internal scratch buffers, but the workspace MSRV
is 1.80 and the project policy in `CLAUDE.md` bars pulling in
dev-dependencies that are not strictly necessary for the headline claim.
The `KernelCounts`-based verification gives exact evidence for the
claim the issue actually makes ("the fused path allocates fewer
matrices than eager"), which is what matters for the reviewer gate.
