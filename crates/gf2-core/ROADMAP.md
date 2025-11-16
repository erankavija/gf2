# gf2-core Roadmap

**Last Updated**: 2024-11-16  
**Current Status**: GF(2^m) arithmetic and polynomial optimization complete

This roadmap focuses on high-performance primitives for GF(2): `BitVec`, `BitMatrix`, GF(2^m) extension fields, and low-level kernels.

---

## Completed Phases ✅

### Phase 1: Scalar Baseline ✅ **COMPLETE**
- Dense `BitVec` with tail masking and word-oriented internals
- `BitMatrix` with zeros/identity, get/set, transpose, row ops
- Algorithms: M4RM multiply, Gauss-Jordan inversion
- Comprehensive unit + property tests; Criterion benches

### Phase 3: SIMD Backends & Dispatch ✅ **COMPLETE**
- AVX2 backend for AND/OR/XOR/NOT/popcount on x86_64
- Runtime detection with `gf2-kernels-simd` crate
- Feature-gated SIMD dispatch in `BitVec`
- Scan kernels (`find_first_one`, `find_first_zero`)
- Word-aligned shift kernels (`shift_left/right` with k % 64 == 0)

### Phase 5: Sparse Matrix Primitives ✅ **COMPLETE**
- `SparseMatrix` type with CSR (Compressed Sparse Row) format
- `SparseMatrixDual` (CSR+CSC) for bidirectional access
- Efficient row/column iteration, matrix-vector multiply
- Conversion APIs, benchmarks, property tests

### Phase 7: GF(2^m) Polynomial Optimization ✅ **COMPLETE (2024-11-16)**

**Phase 7a: Karatsuba Multiplication**
- Recursive O(n^1.585) algorithm with threshold=32
- 1.88x speedup for degree-200 polynomials (352 µs → 187 µs)
- 7 unit tests + 3 property tests

**Phase 7b: SIMD Field Operations**
- PCLMULQDQ-based carry-less multiplication
- 2.1x speedup for GF(65536) without tables (34 ns → 16 ns)
- 3-tier dispatch: tables → SIMD → schoolbook
- `gf2-kernels-simd/src/gf2m.rs` (222 lines)

**Results:**
- Polynomial multiplication: 1.88x faster
- Direct field ops (m > 16): 2-3x faster over schoolbook
- All 90 GF(2^m) tests passing
- Documentation: `README.md` updated with benchmark instructions

### Phase 8: Extension Field GF(2^m) Arithmetic ✅ **COMPLETE (2024-11-14)**

**Phase 8.1: Core Field Arithmetic**
- `Gf2mField` and `Gf2mElement` types
- Addition (XOR) and multiplication with reduction
- Standard presets: `gf256()`, `gf65536()`

**Phase 8.2: Efficient Multiplication**
- Log/antilog tables for m ≤ 16
- O(1) table-based multiplication
- Automatic primitive element discovery

**Phase 8.3: Polynomial Operations**
- `Gf2mPoly` type with addition, multiplication, division
- GCD algorithm with monic normalization
- Horner evaluation

**Phase 8.4: Minimal Polynomial**
- Minimal polynomial computation via conjugate method
- Batch evaluation for BCH syndrome computation

**Implementation:** `src/gf2m.rs` (2506 lines, 156 tests)

---

## Active & Planned Phases

### Phase 4: Rank/Select & Scanning ⏭️ **NEXT**
**Priority**: Medium (nice optimization, not blocking)  
**Effort**: 1-2 weeks  
**Status**: Planned

**Goal**: Efficient rank and select operations on bit vectors

**Implementation:**
- Rank: Count set bits up to position i → `rank(idx) -> usize`
- Select: Find position of k-th set bit → `select(k) -> Option<usize>`
- Superblock/block index structure for O(1) queries
- Lazy index building (build on first query)
- Broadword/PDEP-PEXT strategies for x86_64

**Data Structure:**
```rust
struct RankSelectIndex {
    superblocks: Vec<u64>,  // 512-bit superblock counts
    blocks: Vec<u16>,        // 64-bit block counts within superblock
}
```

**Use Cases:**
- Sparse matrix index lookups
- Bit-level search operations
- Succinct data structures

**Testing:**
- Property tests: `rank(select(k)) == k`
- Boundary cases: empty, full, single bit
- Benchmark vs. naive linear scan

### Phase 6: Polar Transform Operations 🔮 **AFTER RANK/SELECT**
**Priority**: Medium (5G polar codes, not DVB-T2)  
**Effort**: 1-2 weeks  
**Status**: Planned

**Goal**: Fast recursive butterfly transforms for polar code encoding/decoding

**Motivation**: Polar codes (5G NR, future standards) require O(N log N) transforms exploiting Kronecker product structure.

**Implementation:**
- Fast Hadamard Transform over GF(2)
- Recursive butterfly operations: G_N = [1 0; 1 1]^⊗n
- In-place polar encoding transform
- Bit-reversal permutation with cache-optimized access
- SIMD-ready block-based kernels (AVX2 gather/scatter)

**Integration:**
- Works with Phase 4 rank/select for bit-channel reliability sorting
- Frozen bit selection for polar code construction

**Benchmarks:**
- Transform throughput vs. naive matrix multiply
- Target: 100x+ speedup for N=1024+

**Testing:**
- Transform-inverse roundtrip
- Linearity preservation
- Equivalence to matrix form

**Use Cases:**
- 5G NR polar codes
- Successive cancellation decoder
- Bit-channel capacity calculations

### Phase 7c: Batch Evaluation Optimization 🔮 **OPTIONAL**
**Priority**: Low (minor improvement)  
**Effort**: 1 day  
**Status**: Deferred

**Goal**: Optimize BCH syndrome computation

- Vectorized batch evaluation for multiple points
- SIMD Horner evaluation
- Expected: 1.5-2x speedup (17.5 µs → 10-12 µs)

**Note**: Low priority since BCH is already performant with current implementation.

---

## Deprioritized Phases

### Phase 2: Optimized Wide Buffers
**Status**: Deprioritized (polynomial optimization was more critical)

- BitSlice views; range indexing
- Unrolled scalar kernels for AND/OR/XOR/NOT
- Measurable speedups on 64 KiB+ buffers

### Phase 10: General Galois Fields GF(p^m)
**Status**: Deferred (not needed for binary codes)

**Motivation**: Reed-Solomon codes over GF(q), prime-field crypto

- Extension fields for arbitrary prime p
- Modular arithmetic (not binary)
- Would require separate crate (e.g., `gfpm-core`)

**Note**: No immediate blocking requirements. All current use cases use binary fields GF(2^m).

---

## Development Principles

- Deny `unsafe` at public API; encapsulate when kernel performance demands it
- Prefer functional style at API level; imperative in kernels where faster
- Document invariants and complexity; maintain tail masking rigorously
- TDD approach: tests first, implementation second
- Comprehensive testing: unit, property-based, integration

---

## Roadmap Priorities

**Near-term (Next 2-3 weeks):**
1. Phase 4: Rank/Select (if desired for optimization)
2. Phase 6: Polar Transforms (if targeting 5G codes)

**Long-term:**
- Phase 2: Wide buffer optimizations (if profiling shows benefit)
- Phase 7c: Batch evaluation (minor optimization)
- Phase 10: GF(p^m) (only if non-binary codes needed)

**Note**: All dependencies for DVB-T2 FEC simulation (primary project goal) are complete. Further gf2-core work is optional optimization.

---

## Related Documentation

- `docs/GF2M_SESSION_NOTES.md` - GF(2^m) implementation history
- `docs/GF2M_DESIGN.md` - Design decisions and architecture
- `docs/polynomial_benchmarks.md` - Performance baselines
- `README.md` - Usage and benchmark instructions

Refer to `crates/gf2-coding/ROADMAP.md` for higher-level coding theory phases.
