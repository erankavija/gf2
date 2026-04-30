## Epic Complete: Implement generic finite field linear algebra (FieldMatrix) (bb85c68a)

**Started:** 2026-04-23
**Completed:** 2026-04-30
**Assignee:** agent:project-lead

### Summary

Delivered generic dense and sparse finite-field linear algebra for `gf2-core`: `FieldMatrix<F>`, shared `MatrixLike` API parity, expression-template fusion, matmul, triangular primitives, PLE-derived decompositions, inverse/solve/determinant, characteristic/minimal polynomial support, sparse CSR/CSC operations, and reproducible benchmark publication. State-of-the-art performance closure is explicitly handed off to `jit:97bf0879` with raw baseline evidence.

### Metrics

| Metric | Value |
|---|---|
| Dependencies completed | 73 done / 77 total (4 rejected as scoped follow-ons or obsolete alternatives) |
| Waves executed | 4 |
| Review/rework cycles | 45+ recorded review/rework cycles across design, implementation, benchmark, and story-closure gates |
| Escalations | 5 user-approved scope/architecture decisions |
| Sub-agent dispatches | 17 primary work items plus rework dispatches |
| Issues created during execution | 11 child stories/tasks created during breakdown |

### Success Criteria

- [x] [hard] **Dense `FieldMatrix<F>`** with BitMatrix-parity public surface, Armadillo-shape free functions, shared `MatrixLike<Elem>`, and no `BitMatrix` behavior change — delivered by `ab791e27`, `d48a3cfd`, `83b1ad8b`, `c3f8c1cb`, `ae1d1e88`.
- [x] [hard] **Expression-template algebra** with canonical fusions (`A·B + βC`, `αA + βB`, `Aᵀ·B`) — delivered by `cdcebf6a`, `7e6183bb`.
- [x] [hard] **Matrix multiplication** with delayed reduction, Strassen-Winograd recursion, bound propagation, and owned/ref operator combos — delivered by `91c06222`, `7e6183bb`, `ad597ede`, `d48a3cfd`.
- [x] [hard] **PLE decomposition** with block-recursion reducing to `gemm` + `trsm`, non-generic rank profile handling, and LU/echelon/RREF/rank/nullspace projections — delivered by `83b1ad8b`, `c3f8c1cb`.
- [x] [hard] **Triangular primitives** `trsm / trmm / trtri / trtrm` as first-class block-recursive kernels — delivered by `83b1ad8b`.
- [x] [hard] **Matrix inversion, solve, determinant** built on PLE + triangular primitives — delivered by `ae1d1e88`.
- [x] [hard] **Characteristic & minimal polynomial** deterministic cubic baseline plus measured Keller-Gehrig variant where permitted — delivered by `f01298db`, `1454ec2d`, `e47231cd`.
- [x] [hard] **Sparse `FieldMatrix`** CSR/CSC, SpMV/SpMM, and naming parity with `SpBitMatrix` — delivered by `8a90882e`.
- [x] [hard] **Benchmarking** per-story Criterion micro-benchmarks and terminal reproducible baseline against fflas-ffpack/M4RI — delivered by `a03b2556`, `6ed7f050`, `a9ab0a4f`, `64c88ae4`.
- [x] [hard] **Correctness on rank-deficient inputs** for PLE and derivatives — delivered by `c3f8c1cb`, `ae1d1e88`, `64c88ae4`.
- [x] [hard] Public documentation with examples, complexity notes, and panics sections where applicable — enforced across child `code-review`/`doc-review` gates.
- [x] [hard] All `cargo-ci`, `code-review`, and `doc-review` gates pass on every child story — completed through `64c88ae4`.
- [x] [aspirational] GF(p) within 2x of fflas-ffpack — measured as a miss in the baseline; handed off to `jit:97bf0879`.
- [x] [aspirational] GF(2^m) exceeds fflas-ffpack — reference harness gap plus internal GF(2^m) performance gap documented; handed off to `jit:97bf0879`.
- [x] [aspirational] GF(2) within 1.5x of M4RI — measured as a miss in the baseline; handed off to `jit:97bf0879`.

### Wave Execution Log

**Wave 1:** 2 design stories — established `FieldMatrix<F>`, `MatrixLike`, and expression-template architecture.

**Wave 2:** core implementation stories/tasks — delivered dense matrix storage/API, matmul/fusion/Strassen, triangular primitives, sparse matrix support, and PLE/echelon/rank/nullspace foundations.

**Wave 3:** derived operations — delivered inverse/solve/determinant and characteristic/minimal polynomial algorithms.

**Wave 4:** benchmark publication — delivered pinned rootless-podman reference harness, gf2 Criterion/CSV suite, published side-by-side report, and explicit SOTA handoff.

### Key Decisions

- Kept `BitMatrix` specialized and added shared API parity through `MatrixLike`, rather than unifying `BitMatrix` with `FieldMatrix<GF(2)>`.
- Used PLE as the central factorization, with LU/echelon/RREF/rank/nullspace as projections, matching the Dumas-Pernet design.
- Preserved performance over cosmetic single-source-of-truth rewrites where the user approved explicit amendments, notably in PLE/RREF paths and benchmark scope.
- Treated empirical performance misses as baseline evidence, not epic blockers, and transferred SOTA closure to `jit:97bf0879`.
- Recorded `GF(31)` and `GF(2^32)` as explicit baseline coverage gaps under a user-approved amendment and attached the baseline report to `jit:97bf0879`.

### Escalations

- User approved documented panic behavior for zero-inner-dimension operations on runtime-context fields.
- User approved expression-template runtime-speedup criterion amendment after empirical evidence showed the allocation win could not dominate n=1024 compute.
- User approved performance-preserving SSOT amendments for PLE/RREF paths.
- User directed rootless-podman/container naming and base-image refresh decisions for benchmark infrastructure.
- User approved handing off missing `GF(31)` and `GF(2^32)` coverage to `jit:97bf0879`.

### Issues Discovered During Execution

- `cdcebf6a` — expression-template design story created during breakdown.
- `83b1ad8b` — triangular primitive story created during breakdown.
- `e47231cd` — characteristic/minimal polynomial story created during breakdown.
- `91c06222`, `7e6183bb`, `ad597ede` — matmul story split into executable tasks.
- `a03b2556`, `6ed7f050`, `a9ab0a4f` — benchmark story split into container, gf2-suite, and publication tasks.
- `f01298db`, `1454ec2d` — characteristic/minimal polynomial story split into cubic and Keller-Gehrig tasks.

### Holistic Quality Notes

- The epic converged on a consistent dense-linear-algebra architecture: view-based recursion, `gemm`/triangular routing, and allocation-budget regression tests where performance-sensitive.
- The benchmark report is intentionally a baseline/post-PPC scorecard, not a SOTA closure claim; every measured miss or missing comparable row is now explicit and routed to `jit:97bf0879`.
- Repeated review cycles exposed and resolved several hidden contract tensions; the final issue descriptions preserve those amendments so future work does not rediscover the same traps.
