# gf2-core Roadmap

This roadmap focuses on the high-performance primitives for GF(2): `BitVec`, `BitMatrix`, and low-level kernels. It is derived from the original project plan and scoped to the core crate.

## Phase 1: Scalar Baseline ✅
- Dense `BitVec` with tail masking and word-oriented internals
- `BitMatrix` with zeros/identity, get/set, transpose, row ops
- Algorithms: M4RM multiply, Gauss-Jordan inversion
- Comprehensive unit + property tests; Criterion benches

## Phase 2: Optimized Wide Buffers (Deprioritized)
- BitSlice views; range indexing; `from_bitslice` ctors
- Unrolled scalar kernels for AND/OR/XOR/NOT; optional prefetching
- Measurable speedups on 64 KiB+ buffers
- **Status**: Moved down in priority; polynomial optimization is more critical for coding applications

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

## Phase 7: GF(2^m) Polynomial Optimization 🎯 **IN PROGRESS**
**Motivation**: Polynomial multiplication is the critical bottleneck for BCH codes. Baseline: 352 µs for degree-200 multiply in GF(256).

**Status**: Baseline benchmarks complete (2024-11-15)

### Phase 7a: Karatsuba Multiplication ⏭️ **NEXT**
**Expected Speedup**: 2-3x for degree ≥ 64  
**Effort**: 1-2 days  
**Target**: ~120-150 µs for degree-200 multiply

- ✅ Comprehensive benchmarks established (`benches/polynomial.rs`)
- ✅ Performance analysis documented (`docs/polynomial_benchmarks.md`)
- ✅ Session notes with implementation plan (`docs/performance_session_notes.md`)
- ⏭️ Implement threshold-based dispatch (schoolbook < 32, Karatsuba ≥ 32)
- ⏭️ Property tests comparing schoolbook vs Karatsuba
- ⏭️ Validate 2-3x speedup with benchmarks

**Implementation**:
```rust
impl Gf2mPoly {
    pub fn mul(&self, rhs: &Gf2mPoly) -> Gf2mPoly {
        // Threshold dispatch: schoolbook for small, Karatsuba for large
        if self.degree().unwrap_or(0) < 32 || rhs.degree().unwrap_or(0) < 32 {
            self.mul_schoolbook(rhs)
        } else {
            self.mul_karatsuba(rhs)
        }
    }
}
```

### Phase 7b: SIMD Field Operations (Planned)
**Expected Speedup**: Additional 2-4x on top of Karatsuba  
**Effort**: 2-3 days  
**Platforms**: x86_64 PCLMULQDQ, ARM64 PMULL

- Create `src/gf2m_kernels/` module
- Implement PCLMULQDQ-based field multiplication
- Runtime CPU feature detection
- Batch 4-8 field multiplications in SIMD lanes
- Target: ~50-70 µs for degree-200 multiply (5-7x total speedup)

### Phase 7c: Batch Evaluation Optimization (Future)
**Expected Speedup**: 1.5-2x for BCH syndrome computation  
**Effort**: 1 day

- Vectorized batch evaluation for multiple points
- SIMD Horner evaluation
- Optimized for syndrome computation patterns

**Combined Target**: 
- Degree 200 multiply: 352 µs → **50 µs** (7x improvement)
- BCH syndrome (32 evals): 17.5 µs → **10-12 µs** (1.5x improvement)

**Benchmarks**: Run `cargo bench --bench polynomial` to measure progress against baseline.

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

### Phase 2: Efficient Multiplication ✅ COMPLETE
- ✅ Division operation using Fermat's Little Theorem
- ✅ Log/antilog table generation for m ≤ 16
- ✅ Automatic primitive element discovery
- ✅ Table-based O(1) multiplication: `a * b = exp[log[a] + log[b] mod (2^m - 1)]`
- ✅ 40 comprehensive tests (34 unit + 6 property-based tests)
- ✅ All tests passing with zero clippy warnings

**Implementation**: File `src/gf2m.rs` (1099 lines, up from 553)

### Phase 3: Polynomial Operations ✅ COMPLETE
- ✅ `Gf2mPoly` type for polynomials with GF(2^m) coefficients
- ✅ Polynomial addition and multiplication (schoolbook algorithm)
- ✅ Polynomial division with remainder (long division)
- ✅ GCD algorithm using Euclidean method with monic normalization
- ✅ Polynomial evaluation using Horner's method
- ✅ 26 comprehensive polynomial tests (20 unit + 6 property-based)
- ✅ All 142 lib tests + 69 doc tests passing

**Implementation**: File `src/gf2m.rs` (1808 lines, up from 1099)

### Phase 4: Minimal Polynomial ✅ COMPLETE
**Motivation**: Core mathematical primitive for extension field theory, reusable beyond BCH applications.

- ✅ Minimal polynomial computation for field elements
- ✅ Find minimal polynomial of α via conjugate method
- ✅ Efficient algorithm using repeated squaring to find conjugates
- ✅ Product construction: (x - α)(x - α²)(x - α⁴)...(x - α^(2^(d-1)))
- ✅ Batch polynomial evaluation helper (`eval_batch`) for BCH syndrome computation
- ✅ 11 unit tests covering edge cases (zero, one, known values, batch evaluation)
- ✅ 3 property tests: element as root, degree divides m, monic
- ✅ All 156 lib tests + 70 doc tests passing

**Implementation**: File `src/gf2m.rs` (2188 lines, up from 1808)

**Algorithm Details:**
- Finds conjugates via repeated squaring until cycle
- Builds product of (x - conjugate) terms
- O(m² × d) time complexity where d is minimal polynomial degree
- Special cases: m_0(x) = x, m_1(x) = x + 1
- Batch evaluation: `eval_batch(&[Gf2mElement])` for efficient syndrome computation

**Note**: BCH-specific algorithms (generator polynomial construction, Berlekamp-Massey, Chien search) belong in `gf2-coding` Phase C9, not in core primitives. The batch evaluation helper is included as a performance primitive.

**Overall effort**: All phases complete (~3 weeks total). Core GF(2^m) implementation finished.

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
