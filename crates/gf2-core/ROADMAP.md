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

## Phase 5: Sparse Matrix Primitives ✅
**Motivation**: Low-density matrices (<5% nonzeros) require specialized storage for memory efficiency and fast iteration patterns.

- ✅ `SparseMatrix` type with CSR (Compressed Sparse Row) format
- ✅ Efficient row/column iteration patterns
- ✅ Memory-efficient storage: GF(2) optimization (values array omitted)
- ✅ Conversion APIs: `BitMatrix::to_sparse()`, `SparseMatrix::to_dense()` for interop
- ✅ Sparse matrix-vector multiply
- ✅ Transpose and `SparseMatrixDual` (CSR+CSC) for bidirectional access
- ✅ Benchmarks: memory footprint, multiply throughput, dual vs. single format
- ✅ Property tests: equivalence with dense operations
- 🔮 Future: Batch row/column iterators to amortize overhead (process multiple rows/cols together)
- 🔮 Future: SIMD-friendly index access for regular sparsity patterns

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

## Phase 8: Extension Field GF(2^m) Arithmetic (In Progress) 🎯 **HIGH PRIORITY**
**Motivation**: DVB-T2 BCH codes require extension field operations. Blocks gf2-coding DVB-T2 FEC simulation.

**Status**: Phase 1 (Core Field Arithmetic) ✅ COMPLETE as of 2024-11-14

### Phase 1: Core Field Arithmetic ✅ COMPLETE
- ✅ `Gf2mField` and `Gf2mElement` types with Rc-based field parameter sharing
- ✅ Addition (XOR) and multiplication (schoolbook with reduction) operators
- ✅ Standard presets: `gf256()` and `gf65536()`
- ✅ Comprehensive field axiom tests (19 unit tests)
- ✅ Educational documentation with GF(2^4) worked examples
- ✅ All tests passing: 95 lib tests + 63 doc tests
- ✅ Zero unsafe code, zero compiler warnings

**Implementation Notes**:
- Used `Rc<FieldParams>` instead of raw pointers to maintain `#![deny(unsafe_code)]`
- Elements are not `Copy` due to `Rc`; use reference operators `&a + &b` or owned `a + b`
- Multiplication uses schoolbook algorithm with modular reduction
- File: `src/gf2m.rs` (529 lines including tests and docs)

### Phase 2: Efficient Multiplication (Next - Planned)
- [ ] Log/antilog table generation for m ≤ 16
- [ ] Table-based O(1) multiplication: `a * b = exp[log[a] + log[b] mod (2^m - 1)]`
- [ ] Benchmarks comparing table vs. schoolbook multiplication
- [ ] Memory optimization: lazy table initialization
- [ ] Property-based tests with `proptest`

**Estimated effort**: 3-5 days

### Phase 3: Polynomial Operations (Planned)
- [ ] `Gf2mPoly` type with coefficient arithmetic
- [ ] Polynomial multiply, divide, GCD
- [ ] Evaluation at field elements

**Estimated effort**: 3-4 days

### Phase 4: BCH Primitives (Planned)
- [ ] Minimal polynomial computation
- [ ] Generator polynomial construction
- [ ] Syndrome computation
- [ ] Chien search for error locator roots
- [ ] Integration tests with DVB-T2 standard parameters

**Estimated effort**: 5-7 days

**Overall effort**: 2-3 weeks total (Phase 1 complete, ~10-16 days remaining)

**Note**: Binary field arithmetic (characteristic 2) enables specialized optimizations (XOR addition, CLMUL multiply) not applicable to general prime-characteristic fields. GF(2^m) requires independent implementation optimized for binary operations.

## Phase 9: Kernel Quality & Safety (Ongoing)
- Clear contracts for kernels (alignment, sizes)
- Microbenchmarks; perf CI matrices; `unsafe` audit where applicable

## Phase 10: General Galois Fields GF(p^m) (Future Consideration)
**Motivation**: Support for prime-characteristic fields (p ≠ 2) enables Reed-Solomon codes over GF(q), algebraic geometry codes, and broader algebraic coding theory applications.

- **Scope**: Extension fields GF(p^m) for arbitrary prime p
- **Arithmetic**: Modular addition/multiplication in characteristic p (not binary)
- **Implementation**: Separate from GF(2^m) - no shared optimizations due to fundamentally different arithmetic
- **Use cases**: Classical Reed-Solomon (GF(256) with p=256), prime-field crypto
- **Complexity**: Requires modular arithmetic, Barrett/Montgomery reduction, different multiplication strategies
- **Timeline**: Low priority - no immediate blocking requirements

**Note**: This would likely belong in a separate crate (e.g., `gfpm-core`) or module to maintain clean separation from binary field optimizations.

## Principles
- Deny `unsafe` at public API; encapsulate when kernel perf demands it
- Prefer functional style at API level; imperative in kernels where faster
- Document invariants and complexity; maintain tail masking rigorously
