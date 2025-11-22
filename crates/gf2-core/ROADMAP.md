# gf2-core Roadmap

**Mission**: High-performance primitives for binary field computing - push boundaries of efficient GF(2) arithmetic to compete with specialized computer algebra systems.

---

## Completed Phases ✅

### Phase 1: Scalar Baseline
Dense bit vectors, GF(2) matrix operations (M4RM, Gauss-Jordan), comprehensive testing

### Phase 3: SIMD Acceleration  
AVX2 kernels (AND/OR/XOR/popcount), runtime dispatch, word-aligned shifts

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

### Phase 9: Primitive Polynomial Verification ✅ **RECENTLY COMPLETED**
- Efficient primitivity testing: O(m³ log m) order-based algorithm
- Enhanced Rabin irreducibility test with GCD
- Standard polynomial database (m=2..16, DVB-T2 compliant)
- Compile-time warnings for non-standard polynomials
- **Key finding**: AES polynomial (0x11B) is irreducible but NOT primitive
- **See**: `docs/PRIMITIVE_POLYNOMIALS.md` for design details

---

### Phase 9.3: Performance Benchmarking ✅ **COMPLETED 2024-11-22**
**Priority**: High  
**Goal**: Establish competitive baseline vs. Sage

**Results**:
- ✅ Primitivity testing: **3-340x faster** than Sage
- ✅ GF(2^m) multiplication: **4-127x faster** than Sage  
- ✅ Sparse matrix ops: **12-15x faster** than Sage
- ✅ **All targets exceeded** - See `docs/phase9_3_complete.md`

**Deliverables**:
- `benches/primitive_poly.rs` - Comprehensive benchmarks
- `scripts/sage_benchmarks.py` - Sage comparison suite
- Documentation: `docs/phase9_3*.md` with detailed analysis

---

## Active Development

### Phase 9.2: Primitive Polynomial Generation
**Priority**: Medium  
**Goal**: Generate primitive polynomials for arbitrary m

**Deliverables**:
- Exhaustive search with optimizations (m ≤ 16)
- Trinomial search (hardware-efficient, m > 16)
- Parallel generation algorithms with rayon
- **Target**: Compete with Magma/Sage performance

**Expected Performance** (based on Phase 9.3):
- m=16: ~12 µs/test → exhaustive search in ~13 minutes
- m=32: ~150 µs/test → trinomial search practical
- m=64: Focus on known constructions + verification

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

---

## Roadmap Priorities

**Near-term**: Performance benchmarking (Phase 9.3) to establish baselines

**Long-term**: 
- Primitive polynomial generation (Phase 9.2)
- Extended SIMD support (AVX-512, ARM NEON)
- Research algorithms as opportunities arise

---

**For high-level strategy and research goals**, see [main workspace ROADMAP.md](../../ROADMAP.md)

**For detailed design docs**, see:
- `docs/PRIMITIVE_POLYNOMIALS.md` - Phase 9 design and algorithms
- `docs/GF2M_DESIGN.md` - Extension field architecture
- `docs/POLAR_IMPLEMENTATION_PLAN.md` - Phase 6 polar transforms
- `README.md` - API usage and benchmarks
