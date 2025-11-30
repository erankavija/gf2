# gf2-core Roadmap

**Mission**: High-performance primitives for binary field computing - push boundaries of efficient GF(2) arithmetic to compete with specialized computer algebra systems.

---

## Completed Phases ✅

### Phase 1: Scalar Baseline
Dense bit vectors, GF(2) matrix operations (M4RM, Gauss-Jordan), comprehensive testing

### Phase 3: SIMD Acceleration ✅ **ENHANCED**
AVX2 kernels (AND/OR/XOR/popcount), runtime dispatch, word-aligned shifts

**Recent improvements** (Nov 2025):
- Smart backend selection with validated 8-word threshold
- 3.4-3.6x speedup for large buffers (≥64 words)
- Unified kernel architecture with pluggable backends
- Comprehensive equivalence testing (344 tests)
- See `docs/KERNEL_OPTIMIZATION.md` for details

### Phase 4: Rank/Select
O(1) rank, O(log n) select with lazy indexing

### Phase 5: Sparse Matrices
CSR/CSC formats, efficient row/column iteration, matrix-vector multiply

### Phase 6: Polar Transforms
Fast Hadamard Transform, 81x speedup vs. naive, O(N log N) butterfly operations

### Phase 7: GF(2^m) Optimization
- Karatsuba multiplication: 1.88x speedup
- SIMD field operations: 2.1x for m > 16
- PCLMULQDQ carry-less multiplication

### Phase 8: Extension Field Arithmetic
`Gf2mField` with table-based multiplication, polynomial operations (GCD, evaluation), minimal polynomial computation

### Phase 9: Primitive Polynomial Operations ✅ **COMPLETED**

**Phase 9.1-9.3: Verification & Benchmarking**
- Efficient primitivity testing: O(m³ log m) order-based algorithm
- Enhanced Rabin irreducibility test with GCD
- Standard polynomial database (m=2..16, DVB-T2 compliant)
- **3-340x faster** than SageMath for verification
- **Key finding**: AES polynomial (0x11B) is irreducible but NOT primitive

**Phase 9.2: Generation**
- Exhaustive and trinomial search strategies
- Parallel generation with rayon
- **126-931x faster** than SageMath for m=5..8

**Phase 9.4: C/C++ Benchmarking**
- **13-18x faster** than NTL for GF(2^m) multiplication (m ≤ 16)
- Identified M4RI 5-7x gap in matrix operations (optimization target)
- **See**: `docs/BENCHMARKS.md` for comprehensive analysis

---

## Planned Phases

### Phase 11: Performance Gap Remediation ✅ **COMPLETED**

**Goal**: Address performance gaps from Phase 9.4 benchmarking

**Achievements**:
1. **M4RM Matrix Multiplication**: **46% faster**
   - Gray code table generation (16-19% improvement)
   - Flat buffer memory reuse (eliminated 99.2% allocation overhead)
   - Result: 6.71 ms → 4.58 ms (1024×1024)
   - Status: 3.9x vs M4RI (down from 5.7x) ✅

2. **Matrix Inversion**: Benchmarked and validated
   - Current: 22.12 ms (1024×1024)
   - M4RI baseline: 8.94 ms
   - Gap: 2.5x (within acceptable range) ✅

3. **Key Optimizations**:
   - Gray code ordering for table generation
   - Single flat buffer reused across panels
   - Eliminated 33.5 MB allocation churn
   - All tests passing (7 new M4RM tests added)

**Results**: Substantial performance improvement with clean, safe code. Remaining 3.9x gap is reasonable for pure Rust vs hand-optimized C. See `docs/PHASE11_IMPLEMENTATION_PLAN.md` for details.

### Phase 12: RREF (Reduced Row Echelon Form) ✅ **COMPLETED**

**Goal**: Implement high-performance Gaussian elimination for LDPC encoding

**Status**: 
- **304× speedup** vs naive (DVB-T2 Short: 553s → 1.82s)
- Within **5-8× of M4RI** (excellent for safe Rust)
- Ready for gf2-coding integration

**Implementation**:
1. **Phase 12.0-12.1**: Core TDD implementation with word-level ops
2. **Phase 12.2**: Allocation optimization (32-40% improvement)
3. **Phase 12.3**: Algorithmic optimizations (pivot search, unchecked access)
4. **Phase 12.4**: AVX2 SIMD via existing kernels (22-64% improvement)

**Results**:
- 1024×1024: 6.1 ms (M4RI: 1.22 ms = 5.0× slower) ✅
- DVB-T2 Short: 1.82 s (M4RI: 241 ms = 7.5× slower) ✅
- Small matrices: 1.3-3.7× slower than M4RI ✅

**Integration**: Migration plan exists in gf2-coding, ready for Phase 12.5

### Phase 13: Dense Matrix-Vector Multiplication ✅ **COMPLETED**

**Goal**: High-performance dense matrix-vector operations for LDPC encoding

**Status**: Spectacular results - **beating M4RI by 2.7-58×**

**Implementation**:
1. **matvec**: Dense matrix × dense vector
   - Word-level operations: AND + XOR + popcount
   - 17× faster than M4RI at 64×64, 2.7× at 1024×1024
   - 15.7 µs for 1024×1024 (M4RI: 42.93 µs)

2. **matvec_transpose**: Transposed dense matrix × dense vector
   - Initial: Bit-by-bit approach (slow)
   - Optimized: Word-level processing 64 columns simultaneously
   - **36-63× speedup** from optimization
   - 23.6× faster than M4RI at 64×64, 3.6× at 1024×1024
   - 13.95 µs for 1024×1024 (M4RI: 49.71 µs)

**Key Innovation**: Specialized word-level approach beats M4RI's general-purpose M4RM
- No gray code table overhead
- Sequential memory access (cache-friendly)
- Tight inner loops with minimal branching
- Safe Rust with O(rows × cols) complexity

**Testing**: 24 comprehensive tests, property-based validation, edge case coverage

**Impact**: Validates dense storage for 40-50% density LDPC matrices (DVB-T2)

### Phase 14: GF(2^m) Polynomial Utilities ✅ **COMPLETED**

**Goal**: Provide construction utilities for BCH/Reed-Solomon generator polynomials

**Status**: All functions implemented (2024-11-30)
- ✅ `from_exponents()` - Build polynomial from exponent list (e.g., [0,2,5] → 1+x²+x⁵)
- ✅ `monomial()` - Create single-term polynomial c·xⁿ
- ✅ `x()` - Create the indeterminate polynomial x
- ✅ `from_roots()` - Construct polynomial from roots (x-r₁)(x-r₂)...(x-rₙ)
- ✅ `product()` - Multiply list of polynomials

**Implementation**:
- TDD approach: 26 comprehensive tests written first
- Functional style with pure functions and composability
- Handles edge cases: empty inputs, duplicates, GF(2) cancellation
- Real-world examples: DVB-T2 BCH generator construction

**Performance Strategy**:
- No premature optimization (construction utilities, not runtime hotpaths)
- Simple sequential multiplication using existing optimized `*` operator
- Expected: 100-500× faster than SageMath, 2-10× faster than NTL
- Documented in `docs/POLY_UTILITIES_PERFORMANCE.md`

**Testing**: 480 total tests passing (26 new polynomial construction tests)

**Next Steps**:
- Benchmarking (optional): Add competitive comparison vs SageMath/NTL
- Migration: Replace `poly_from_exponents()` in gf2-coding BCH module
- See: `docs/GF2M_POLY_UTILITIES_REQUIREMENTS.md` for detailed specification

### Phase 15: GF(2^m) Thread Safety ✅ **COMPLETE**

**Goal**: Enable parallel BCH/Reed-Solomon batch operations

**Status**: Completed (2024-11-30)

**Related Documentation**:
- Technical requirements: `docs/GF2M_THREAD_SAFETY_REQUIREMENTS.md`
- gf2-coding integration: [Parallelization Progress](../gf2-coding/docs/PARALLELIZATION_PROGRESS.md#phase-22-gf2m-thread-safety-prerequisite-for-bchrs)

**Implementation Complete**:
1. ✅ Changed `Rc` → `Arc` in `Gf2mField` and `Gf2mElement`
2. ✅ Added `PartialEq`/`Eq` for `Gf2mField` (required for Arc)
3. ✅ Added 10 thread safety tests (all passing)
4. ✅ Added performance benchmarks (field_clone.rs)
5. ✅ All 490 gf2-core tests pass (zero breaking changes)

**Performance Validation**: 
- Field clone: 3.2 ns (Arc overhead negligible vs Rc)
- Multiplication: 4.3 ns (no change)
- **Result**: Arc overhead <15% for clone, zero for operations

**Impact**: 
- ✅ `Gf2mField` and `Gf2mElement` now `Send + Sync`
- ✅ BCH/RS batch operations ready for rayon parallelism
- ⏭ Next: gf2-coding Phase 2.2 (enable BCH parallel batch APIs)

---

## Planned Phases

### Phase 2: Wide Buffer Optimization
Unrolled scalar kernels, BitSlice views - deferred until profiling shows benefit

### Phase 6b: SIMD Polar Transforms
AVX2 butterfly operations, cache blocking for N > 8K - optional enhancement

### Phase 10: General Galois Fields GF(p^m)
Prime field arithmetic - deferred (no immediate use case)

---

## Future Directions

**SIMD Backends**:
- AVX-512 support (512-bit vectors)
- ARM NEON for AArch64/embedded

**Advanced Algorithms**:
- GPU acceleration for massive parallelism
- Batch polynomial operations
- Extended field degrees (m > 64)

**Research Integration**:
- State-of-the-art polynomial factorization
- Novel sparse matrix algorithms
- Hardware-optimized implementations

---

## Design Principles

- **Performance priority** in kernels: imperative, mutating code when benchmarks show benefit
- **Functional style** at API level: immutability, pure functions, composability
- **Test-driven**: property-based tests, mathematical validation, comprehensive coverage
- **Safe by default**: `#![deny(unsafe_code)]` at crate level
- **Compete with best**: Magma/Sage performance targets, rigorous benchmarking

### Phase 12: File I/O ✅ **COMPLETE**

**Goal**: Efficient binary serialization for GF(2) data structures

**Status**: All phases complete (2024-11-25)
- ✅ Format specification (32-byte header + JSON metadata + binary payload)
- ✅ Error handling and validation
- ✅ BitVec serialization/deserialization (47 tests)
- ✅ BitMatrix serialization/deserialization (18 tests)
- ✅ SpBitMatrix serialization (COO format, 12 tests)
- ✅ SpBitMatrixDual serialization (CSR+CSC format)
- ✅ Multiple formats: Binary, Text (Hex for dense only)
- ✅ Compression validation: >100× for sparse matrices
- ✅ Optional `io` feature (enabled by default)
- ⏸️ Compression support (Phase 3, deferred - not needed)
- ⏸️ Checksum verification (Phase 4, deferred - not needed)

**Implementation Notes**:
- Explicit format selection (no auto-detection complexity)
- Binary COO format for sparse: `[(u32, u32); nnz]` edge list
- Text format for debugging: edge list with dimensions header
- Sparse compression: DVB-T2 simulations achieve 155×+ compression

**Total Tests**: 76 I/O tests (413 total library tests)

**Impact**: Enables pre-computed LDPC generator matrices (2 min → 10ms initialization)

---

## Roadmap Priorities

**Current**: File I/O Phase 2 (Matrix serialization) - estimated 2-3 hours

**Long-term**: 
- Extended SIMD support (AVX-512, ARM NEON)
- Research algorithms as opportunities arise

---

**For high-level strategy and research goals**, see [main workspace ROADMAP.md](../../ROADMAP.md)

**For detailed design docs**, see:
- `docs/KERNEL_OPTIMIZATION.md` - Kernel architecture and SIMD integration
- `docs/BENCHMARK_RESULTS_SIMD_VS_SCALAR.md` - SIMD performance validation
- `docs/PRIMITIVE_POLYNOMIALS.md` - Phase 9 design and algorithms
- `docs/GF2M_DESIGN.md` - Extension field architecture
- `docs/POLAR_IMPLEMENTATION_PLAN.md` - Phase 6 polar transforms
- `docs/BENCHMARKS.md` - Performance comparisons vs SageMath/NTL/M4RI
- `README.md` - API usage and examples
