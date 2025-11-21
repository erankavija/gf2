# gf2-core Roadmap

This roadmap focuses on high-performance primitives for GF(2): `BitVec`, `BitMatrix`, GF(2^m) extension fields, and low-level kernels. The mission is to push the boundaries of efficient binary field arithmetic and provide foundational support for all coding theory applications in gf2-coding.

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

### Phase 7: GF(2^m) Polynomial Optimization ✅ **COMPLETE**

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

### Phase 4: Rank/Select & Scanning ✅ **COMPLETE**

**Goal**: Efficient rank and select operations on bit vectors

**Implementation:**
- Rank: Count set bits up to position i → `rank(idx) -> usize` (O(1))
- Select: Find position of k-th set bit → `select(k) -> Option<usize>` (O(log n))
- Superblock/block index structure for fast queries
- Lazy index building (built on first query)
- `RankSelectIndex` with superblocks (512-bit) and blocks (64-bit)

**Status**: Fully implemented in `src/bitvec.rs`
- Public API: `BitVec::rank()`, `BitVec::select()`
- Lazy index construction via `RefCell<Option<RankSelectIndex>>`
- Comprehensive test coverage
- Used by sparse matrix operations and bit-level search

### Phase 8: Extension Field GF(2^m) Arithmetic ✅ **COMPLETE**

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

**Implementation:** `src/gf2m.rs`

---

## Active & Planned Phases

### Phase 6: Polar Transform Operations ✅ **COMPLETE**
**Priority**: HIGH (required for polar code capacity verification)  
**Effort**: 1 day actual (1-2 weeks estimated)  
**Status**: Complete - scalar baseline ready for production use

**Goal**: Fast recursive butterfly transforms for polar code encoding/decoding and capacity verification

**Implementation:**

#### Core Transform Operations ✅
- ✅ Fast Hadamard Transform (FHT) over GF(2)
- ✅ Iterative butterfly: G_N = [1 0; 1 1]^⊗n, O(N log N)
- ✅ In-place polar encoding transform (`polar_transform_into`)
- ✅ Out-of-place transform for immutable API (`polar_transform`)
- ✅ Bit-reversal permutation (functional + `_into` variants)

#### Convenience Functions ✅
- ✅ `BitVec::bit_reversed(n_bits)` - create bit-reversed copy
- ✅ `BitVec::bit_reverse_into(n_bits)` - in-place bit reversal
- ✅ `BitVec::polar_transform(n)` - apply G_N Kronecker transform
- ✅ `BitVec::polar_transform_into(n)` - in-place transform
- ✅ `BitVec::polar_transform_inverse(n)` - inverse transform
- ✅ `BitVec::polar_transform_inverse_into(n)` - in-place inverse
- ✅ Systematic `_into` naming convention

#### Performance ✅
- ✅ 81x speedup vs. naive matrix multiply @ N=1024
- ✅ 76-105 Melem/s throughput for N=1024-16384
- ✅ O(N log N) scaling confirmed via benchmarks

**Testing:** ✅
- ✅ 23 comprehensive tests (unit + property + integration)
- ✅ Transform-inverse roundtrip (property test)
- ✅ Linearity preservation
- ✅ Equivalence to matrix form (N=2, N=4)
- ✅ Bit-reversal involution
- ✅ Functional vs `_into` equivalence

**Benchmarks:** ✅
- ✅ `benches/polar.rs` added with 4 benchmark groups
- ✅ FHT vs naive comparison
- ✅ Functional vs `_into` performance
- ✅ Roundtrip encode/decode

**Use Cases:** ✅ Ready
- 5G NR polar codes
- Successive cancellation (SC) decoder
- SC-List (SCL) decoder
- Bit-channel capacity calculations
- **Polar code FER simulation and capacity verification (gf2-coding Phase C7)**

**Future Optimizations (Optional):**
- SIMD butterfly operations (AVX2)
- Cache-blocking for N > 8192
- ARM NEON support

### Phase 9: Primitive Polynomial Verification & Generation 🎯 **IN PROGRESS**
**Priority**: HIGH (prevent BCH decoding bugs)  
**Effort**: Phase 1: 2-3 weeks  
**Status**: Phase 1 starting - TDD approach  
**Motivation**: DVB-T2 BCH bug caused by wrong primitive polynomial for GF(2^14)

**Phase 1 Goal**: Ensure we never use wrong primitive polynomials again

#### Components (Phase 1)

**9.1 Polynomial Primitivity Testing**
- ✅ Design complete (see [docs/PRIMITIVE_POLYNOMIALS.md](docs/PRIMITIVE_POLYNOMIALS.md))
- ⏳ `Gf2mField::verify_primitive()` - verify polynomial is actually primitive
- ⏳ `Gf2mField::is_irreducible_rabin()` - Rabin irreducibility test
- ⏳ Uses existing `Gf2mPoly::gcd()` and exponentiation by squaring
- ⏳ Time complexity: O(m³) for degree-m polynomial

**9.2 Standard Primitive Polynomial Database**
- ✅ Design complete
- ⏳ `PrimitivePolynomialDatabase` module in `src/gf2m/primitive_polys.rs`
- ⏳ Database covering m = 2..32 from authoritative sources:
  - Lidl & Niederreiter (1997) "Finite Fields"
  - ETSI EN 302 755 (DVB-T2 standard)
  - IEEE AES standard (GF(2^8))
  - 3GPP 5G NR standard
- ⏳ `standard(m)` - get standard polynomial for degree m
- ⏳ `trinomials(m)` - get primitive trinomials (hardware-efficient)
- ⏳ `verify(m, poly)` - check against database (returns Matches/Conflict/Unknown)

**9.3 Compile-Time Verification & Warnings**
- ✅ Design complete
- ⏳ Modify `Gf2mField::new()` to check database and warn on conflicts
- ⏳ Feature flag `verify-primitives` for runtime verification (test/debug only)
- ⏳ Clear error messages citing standard sources
- ⏳ Zero overhead for verified polynomials in release builds

#### Testing (TDD - Tests First!)

**Unit Tests** (⏳ To be written first):
- Test all database polynomials verify as primitive
- Test DVB-T2 polynomials (correct and incorrect)
- Test AES standard polynomial
- Test reducible polynomials fail verification
- Test Rabin irreducibility on known cases

**Integration Tests** (⏳ To be written first):
- Verify all DVB-T2 BCH configurations use primitive polynomials
- Ensure the bug case (0b100000000100001 for GF(2^14)) is detected
- Test 5G NR and WiFi standard configurations

**Property Tests** (⏳ To be written first):
- All database entries verify as primitive
- Reducible polynomials never verify as primitive

**See**: [docs/PRIMITIVE_POLYNOMIALS.md](docs/PRIMITIVE_POLYNOMIALS.md) for complete design and test specifications

#### Migration Path for gf2-coding
- Replace hardcoded polynomials with database lookups
- Add integration tests ensuring all codes use verified primitives
- Document standard references in code

**Future Phases** (after Phase 1):
- **Phase 2**: Primitive polynomial generation (exhaustive & trinomial search)
- **Phase 3**: Performance optimization (parallel, SIMD, compete with Magma/Sage)
- **Phase 4**: State-of-the-art algorithms for m > 64

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

**Completed:**
1. ✅ Phase 6: Polar Transforms - Production-ready scalar baseline (81x speedup)

**Near-term:**
1. Phase 2: Wide buffer optimizations (if profiling shows benefit)

**Long-term:**
- Phase 7c: Batch evaluation (minor optimization)
- Phase 6b: SIMD polar transforms (AVX2, optional enhancement)
- Phase 10: GF(p^m) (only if non-binary codes needed)
- AVX-512 and ARM NEON SIMD backends

**Note**: Core primitives for coding theory applications are mature. Phase 6 polar transforms complete and ready for gf2-coding Phase C7 capacity verification.

---

## Related Documentation

- `docs/POLAR_IMPLEMENTATION_PLAN.md` - Phase 6 polar transforms implementation details
- `docs/GF2M_SESSION_NOTES.md` - GF(2^m) implementation history
- `docs/GF2M_DESIGN.md` - Design decisions and architecture
- `docs/polynomial_benchmarks.md` - Performance baselines
- `README.md` - Usage and benchmark instructions

Refer to `crates/gf2-coding/ROADMAP.md` for higher-level coding theory phases.

---

## ✅ Phase 9: BitVec Conversion Helpers (2025-11-19) **COMPLETE**

**Priority**: HIGH (eliminates code duplication in gf2-coding)  
**Effort**: 2 hours  
**Status**: Complete

**Goal**: Add convenience methods for BitVec ↔ Gf2mPoly and BitMatrix conversions

**Implementation:**

#### Gf2mPoly Conversions ✅
- ✅ `Gf2mPoly::from_bitvec(bits: &BitVec, field: &Gf2mField)` - BitVec to polynomial
- ✅ `Gf2mPoly::to_bitvec(len: usize)` - Polynomial to BitVec (fixed length)
- ✅ `Gf2mPoly::to_bitvec_minimal()` - Polynomial to BitVec (minimal length)

#### BitMatrix Conversions ✅
- ✅ `BitMatrix::row_as_bitvec(row: usize)` - Extract row as BitVec
- ✅ `BitMatrix::col_as_bitvec(col: usize)` - Extract column as BitVec

**Testing:** ✅
- ✅ 10 unit tests for Gf2mPoly conversions
- ✅ 10 unit tests for BitMatrix extractions
- ✅ 3 property-based tests for round-trips and invariants
- ✅ All 218 tests passing
- ✅ Zero clippy warnings

**Benefits:**
- Eliminates 5 duplicated implementations in gf2-coding BCH code
- Natural, discoverable API for coding theory applications
- Comprehensive test coverage ensures correctness
- Ready for use in BCH and LDPC implementations
