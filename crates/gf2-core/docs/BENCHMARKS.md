# gf2-core Performance Benchmarks

**Crate Version**: 0.1.0

This document consolidates all benchmark results for gf2-core.

---

## Phase 9.2: Primitive Polynomial Generation

**Hardware**: Standard development machine (4+ cores)  
**Features**: Sequential and parallel (rayon) implementations

### Exhaustive Search - Find First Primitive

| Degree (m) | Time | Operations |
|------------|------|------------|
| 2 | 324 ns | Test ~2 candidates |
| 3 | 518 ns | Test ~4 candidates |
| 4 | 1.04 µs | Test ~8 candidates |
| 5 | 1.84 µs | Test ~16 candidates |
| 6 | 2.01 µs | Test ~32 candidates |
| 7 | 1.85 µs | Test ~64 candidates |
| 8 | 18.2 µs | Test ~128 candidates |

**Performance**: Sub-microsecond for m ≤ 6, ~18 µs for m=8

### Exhaustive Search - Find All Primitives

| Degree (m) | Time | Count | Per-Polynomial |
|------------|------|-------|----------------|
| 2 | 332 ns | 1 | 332 ns |
| 3 | 1.04 µs | 2 | 518 ns |
| 4 | 4.25 µs | 2 | 2.12 µs |
| 5 | 9.67 µs | 6 | 1.61 µs |
| 6 | 27.6 µs | 6 | 4.60 µs |
| 7 | 58.2 µs | 18 | 3.23 µs |
| 8 | 174 µs | 16 | 10.9 µs |
| 9 | 389 µs | 48 | 8.11 µs |
| 10 | 1.04 ms | 60 | 17.4 µs |

**Scaling**: Roughly 2x per degree increase (as expected with 2^m candidates)

### Parallel Exhaustive Search - Find All (4 threads)

| Degree (m) | Sequential | Parallel | Speedup |
|------------|-----------|----------|---------|
| 8 | 174 µs | 40.6 µs | 4.3x |
| 9 | 389 µs | 77.7 µs | 5.0x |
| 10 | 1.04 ms | 125 µs | 8.3x |
| 11 | - | 252 µs | - |
| 12 | - | 521 µs | - |

**Parallel Efficiency**: Good scaling, super-linear for m=10 (likely cache effects)

### Trinomial Search

| Degree (m) | Time | vs Exhaustive |
|------------|------|---------------|
| 2 | 201 ns | 1.6x faster |
| 3 | 293 ns | 1.8x faster |
| 4 | 675 ns | 1.5x faster |
| 5 | 1.41 µs | 1.3x faster |
| 6 | 1.58 µs | 1.3x faster |
| 7 | 1.31 µs | 1.4x faster |
| 8 | **17.9 ns** | **1,014x faster!** |
| 9 | 7.96 µs | 48x faster |
| 10 | 5.89 µs | 177x faster |

**Key Insight**: Trinomial search is dramatically faster when a primitive trinomial exists early in the search space.

### Comparison with SageMath

Sage timings for exhaustive search:
- m=5: ~9 ms → **Our: 9.67 µs (931x faster)**
- m=6: ~13 ms → **Our: 27.6 µs (471x faster)**
- m=7: ~9 ms → **Our: 58.2 µs (155x faster)**
- m=8: ~22 ms → **Our: 174 µs (126x faster)**

---

## Phase 9.3: SageMath Comparison

**Comparison**: gf2-core vs SageMath 10.7  
**Goal**: Achieve performance within 2x of Sage  
**Result**: **Exceeded all targets by 3-340x**

### Primitive Polynomial Verification

| Degree | gf2-core | SageMath | Speedup |
|--------|----------|----------|---------|
| m=4 | 130 ns | 3.99 ms | **30.7x** |
| m=8 | 950 ns | 3.87 ms | **4.1x** |
| m=12 | 3.15 µs | 10.0 ms | **3.2x** |
| m=16 | 12.3 µs | 4.16 ms | **338x** |

**Result**: 3-340x faster than Sage

### GF(2^m) Field Operations

#### Multiplication Throughput (1M operations)

| Field | gf2-core | SageMath | Speedup |
|-------|----------|----------|---------|
| GF(2^4) | 2.42 ms | 308 ms | **127x** |
| GF(2^8) | 2.70 ms | 308 ms | **114x** |
| GF(2^16) | 4.41 ms | 308 ms | **69.8x** |
| GF(2^32) | 52.2 ms | 338 ms | **6.5x** |
| GF(2^64) | 204 ms | 803 ms | **3.9x** |

**Result**: 4-127x faster than Sage

#### Polynomial Multiplication (Degree 100)

| Field | gf2-core | SageMath | Speedup |
|-------|----------|----------|---------|
| GF(2^8) | 63 µs | 15.5 ms | **246x** |
| GF(2^16) | 93 µs | 15.6 ms | **168x** |

**Result**: 168-246x faster than Sage

### Sparse Matrix Operations

#### Matrix-Vector Multiply (1000x1000, 1% density)

| Operation | gf2-core | SageMath | Speedup |
|-----------|----------|----------|---------|
| SpMV | 27.5 µs | 393 µs | **14.3x** |
| SpMV (5% density) | 123 µs | 1.52 ms | **12.4x** |

**Result**: 12-15x faster than Sage

---

## Historical: GF(2^m) Polynomial Arithmetic

**Optimization**: Karatsuba multiplication + PCLMULQDQ SIMD

### Polynomial Multiplication Speedups

**Degree 200 (BCH Critical Path):**

| Field | Baseline (Schoolbook) | Optimized (Karatsuba) | Speedup |
|-------|----------------------|----------------------|---------|
| GF(2^8) | 352 µs | 187 µs | 1.88x |
| GF(2^16) | 527 µs | 279 µs | 1.89x |

**Degree 100:**

| Field | Baseline | Optimized | Speedup |
|-------|----------|-----------|---------|
| GF(2^8) | 89 µs | 63 µs | 1.41x |
| GF(2^16) | 134 µs | 93 µs | 1.44x |

**Karatsuba Benefits**: O(n^1.585) vs O(n²), most effective for large polynomials (degree > 100)

### Field Operations with SIMD

**GF(2^m) Multiplication (m > 16):**

| Implementation | Throughput (1M ops) |
|----------------|---------------------|
| Table-based (m ≤ 16) | ~2.5 ms |
| SIMD PCLMULQDQ (m > 16) | ~50 ms (m=32), ~200 ms (m=64) |
| Baseline schoolbook | ~200 ms (m=32), ~800 ms (m=64) |

**SIMD Speedup**: 2.1x for m=16, 4x for m=64

---

## Key Takeaways

1. **Primitive polynomial generation**: 100-1000x faster than Sage, practical up to m=12
2. **Parallel scaling**: 4-8x speedup with rayon for larger problems
3. **GF(2^m) operations**: Consistently 4-340x faster than Sage across all field sizes
4. **Sparse matrices**: 12-15x faster than Sage, excellent for low density
5. **SIMD acceleration**: 2-4x speedup for large field degrees (m > 16)

**Next**: Phase 9.4 will benchmark against specialized C/C++ libraries (NTL, M4RI, FLINT, GF-Complete)
