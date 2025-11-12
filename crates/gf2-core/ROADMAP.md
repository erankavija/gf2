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
- ✅ Word-aligned shift kernels (`shift_left/right` with k % 64 == 0)
- Bit-level shift SIMD (future work if profiling shows benefit)

## Phase 4: Rank/Select & Scanning (Planned)
- Rank/select with superblock/block indexes
- Broadword/PDEP-PEXT strategies; density-aware paths
- APIs: `rank(idx)`, `select(k)` with lazy indexing

## Phase 5: Sparse Matrix Primitives (Planned)
**Motivation**: LDPC codes (gf2-coding Phase C5) require sparse parity-check matrices with <1% density. Dense `BitMatrix` wastes memory and cycles on mostly-zero data.

- `SparseMatrix` type with CSR (Compressed Sparse Row) / COO (Coordinate) formats
- Efficient row/column iteration for belief propagation message passing (required by LDPC decoders)
- Memory-efficient storage: bit-packed nonzero values with integer index arrays
- Conversion APIs: `BitMatrix::to_sparse()`, `SparseMatrix::to_dense()` for interop
- Sparse matrix-vector multiply (required for syndrome computation in LDPC)
- Transpose and row/column access optimized for cache locality (critical for iterative decoders)
- Benchmarks: memory footprint vs. density; multiply throughput vs. dense baseline
- Property tests: equivalence with dense operations at low sparsity

## Phase 6: Polar Transform Operations (Planned)
**Motivation**: Polar codes (gf2-coding Phase C6) require fast recursive butterfly transforms for O(N log N) encoding/decoding, exploiting Kronecker product structure of polar generator matrix.

- Fast Hadamard Transform over GF(2) with recursive butterfly operations
- In-place polar encoding transform (G_N = [1 0; 1 1]^⊗n Kronecker power)
- Bit-reversal permutation with cache-optimized access patterns (required for natural vs. bit-reversed order)
- Block-based butterfly kernels prepared for SIMD optimization (AVX2 gather/scatter)
- Integration with Phase 4 rank/select for bit-channel reliability sorting (frozen bit selection)
- Benchmarks: transform throughput vs. naive matrix multiply (target 100x+ speedup for N=1024+)
- Property tests: transform-inverse roundtrip, linearity preservation, equivalence to matrix form

## Phase 7: GF(2) Polynomials (Planned)
- `GF2Poly` wrapper over `BitVec`
- Scalar schoolbook; CLMUL/VMULL.P64 acceleration
- Karatsuba/Toom-Cook; division/mod; GCD; property tests
- Note: CLMUL operations also accelerate polar transforms (Phase 6) due to recursive structure

## Phase 8: Kernel Quality & Safety (Ongoing)
- Clear contracts for kernels (alignment, sizes)
- Microbenchmarks; perf CI matrices; `unsafe` audit where applicable

## Principles
- Deny `unsafe` at public API; encapsulate when kernel perf demands it
- Prefer functional style at API level; imperative in kernels where faster
- Document invariants and complexity; maintain tail masking rigorously
