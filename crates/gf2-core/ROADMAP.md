# gf2-core Roadmap

This roadmap focuses on the high-performance primitives for GF(2): `BitVec`, `BitMatrix`, and low-level kernels. It is derived from the original project plan and scoped to the core crate.

## Phase 1: Scalar Baseline ✅
- Dense `BitVec` with tail masking and word-oriented internals
- `BitMatrix` with zeros/identity, get/set, transpose, row ops
- Algorithms: M4RM multiply, Gauss-Jordan inversion
- Comprehensive unit + property tests; Criterion benches

## Phase 2: Optimized Wide Buffers (Planned)
- BitSlice views; range indexing; `from_bitslice` ctors
- Unrolled scalar kernels for AND/OR/XOR/NOT; optional prefetching
- Measurable speedups on 64 KiB+ buffers

## Phase 3: SIMD Backends & Dispatch ✅
- ✅ AVX2 backend for AND/OR/XOR/NOT/popcount on x86_64
- ✅ Runtime detection with `gf2-kernels-simd` crate
- ✅ Feature-gated SIMD dispatch in `BitVec`
- ✅ Scan kernels (`find_first_one`, `find_first_zero`)
- Shift kernels; additional vector strategies (future work)

## Phase 4: Rank/Select & Scanning (Planned)
- Rank/select with superblock/block indexes
- Broadword/PDEP-PEXT strategies; density-aware paths
- APIs: `rank(idx)`, `select(k)` with lazy indexing

## Phase 5: GF(2) Polynomials (Planned)
- `GF2Poly` wrapper over `BitVec`
- Scalar schoolbook; CLMUL/VMULL.P64 acceleration
- Karatsuba/Toom-Cook; division/mod; GCD; property tests

## Phase 6: Kernel Quality & Safety (Ongoing)
- Clear contracts for kernels (alignment, sizes)
- Microbenchmarks; perf CI matrices; `unsafe` audit where applicable

## Principles
- Deny `unsafe` at public API; encapsulate when kernel perf demands it
- Prefer functional style at API level; imperative in kernels where faster
- Document invariants and complexity; maintain tail masking rigorously
