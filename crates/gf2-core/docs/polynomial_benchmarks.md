# GF(2^m) Polynomial Arithmetic Benchmark Results & Optimization Roadmap

**Date**: 2024-11-15  
**Crate**: gf2-core v0.1.0  
**Purpose**: Establish baseline performance for polynomial operations and identify optimization opportunities

## Executive Summary

Polynomial arithmetic benchmarks have been established for GF(256) and GF(65536) fields. Current implementation uses:
- **Addition**: O(n) element-wise XOR
- **Multiplication**: O(n²) schoolbook algorithm
- **Division**: O(n²) long division
- **GCD**: Euclidean algorithm with monic normalization
- **Evaluation**: Horner's method

## Baseline Performance Results

### Polynomial Addition (Linear, O(n))
| Degree | GF(256) | GF(65536) | Throughput |
|--------|---------|-----------|------------|
| 10     | 42 ns   | 42 ns     | ~236 Melem/s |
| 50     | 177 ns  | 177 ns    | ~283 Melem/s |
| 100    | 368 ns  | 353 ns    | ~272-283 Melem/s |
| 500    | 1.6 µs  | 1.6 µs    | ~305 Melem/s |
| 1000   | 3.4 µs  | 3.3 µs    | ~297 Melem/s |

**Analysis**: Addition performance is excellent and field-independent (as expected, it's just XOR). ~300 Melem/s sustained throughput.

### Polynomial Multiplication (Schoolbook, O(n²))
| Degree | GF(256) Time | GF(256) Throughput | GF(65536) Time | GF(65536) Throughput |
|--------|--------------|---------------------|----------------|----------------------|
| 5      | 360 ns       | 69 Melem/s          | 529 ns         | 47 Melem/s           |
| 10     | 1.15 µs      | 87 Melem/s          | 1.70 µs        | 59 Melem/s           |
| 20     | 3.98 µs      | 100 Melem/s         | 5.89 µs        | 68 Melem/s           |
| 50     | 23.3 µs      | 107 Melem/s         | 34.5 µs        | 72 Melem/s           |
| 100    | 89.1 µs      | 112 Melem/s         | 134 µs         | 75 Melem/s           |
| 200    | 352 µs       | 114 Melem/s         | 527 µs         | 76 Melem/s           |

**Analysis**: 
- GF(256) is ~30-35% faster than GF(65536) due to table-based O(1) multiplication in GF(256)
- Schoolbook algorithm shows good cache behavior (throughput increases with degree)
- **Critical bottleneck for BCH codes**: 200-degree polynomial multiplication takes ~352 µs in GF(256)

### Polynomial Division (O(n²))
| Dividend÷Divisor | GF(256) | GF(65536) | Notes |
|------------------|---------|-----------|-------|
| 200÷5            | 34.8 µs | 53.7 µs   | Most common BCH pattern |
| 200÷10           | 66.3 µs | 102 µs    | |
| 200÷20           | 77.0 µs | 120 µs    | |

**Analysis**: Division is ~2x slower than equivalent multiplication due to repeated normalization and subtraction.

### Polynomial GCD (Euclidean Algorithm)
| Degree | GF(256) | GF(65536) |
|--------|---------|-----------|
| 10     | 2.22 µs | 5.66 µs   |
| 20     | 5.30 µs | 12.0 µs   |
| 50     | 19.3 µs | 38.3 µs   |
| 100    | 61.0 µs | 134 µs    |

**Analysis**: GCD performance dominated by division operations. GF(65536) shows ~2x slowdown vs GF(256).

### Polynomial Evaluation (Horner's Method, O(n))
| Degree | GF(256) | GF(65536) |
|--------|---------|-----------|
| 10     | 55 ns   | 84 ns     |
| 50     | 273 ns  | 413 ns    |
| 100    | 547 ns  | 829 ns    |
| 500    | 2.74 µs | 4.14 µs   |
| 1000   | 5.49 µs | 8.28 µs   |

**Analysis**: Linear scaling as expected. GF(256) table multiplication gives ~50% speedup over GF(65536).

## Optimization Opportunities (Priority Order)

### 1. Karatsuba Multiplication (HIGH PRIORITY) 🎯
**Expected Speedup**: 2-4x for degree ≥ 64  
**Complexity**: O(n^1.58) vs O(n²)  
**Target Degrees**: 64-1024 (common BCH range)

**Implementation Plan**:
```rust
// Threshold-based dispatch:
// - degree < 32: schoolbook (better cache behavior)
// - degree 32-512: Karatsuba
// - degree > 512: Toom-3 or FFT (future)
pub fn mul_karatsuba(&self, rhs: &Gf2mPoly) -> Gf2mPoly { ... }
```

**Expected Impact on BCH(255,k)**:
- Current: 352 µs for degree-200 multiply
- With Karatsuba: ~120-150 µs (2.3-2.9x speedup)

### 2. SIMD Field Multiplication (MEDIUM PRIORITY)
**Expected Speedup**: 2-4x for GF(2^m) with m ≤ 16  
**Platforms**: AVX2 (x86_64), NEON (ARM)

**Implementation Approach**:
- Use PCLMULQDQ (carry-less multiply) on x86_64 for GF(2^m) multiplication
- Batch 4-8 field multiplications in parallel using SIMD lanes
- Specialized kernels for GF(256) and GF(65536)

**Files to Create**:
- `src/gf2m_kernels/mod.rs`
- `src/gf2m_kernels/pclmul.rs` (x86_64 with PCLMULQDQ)
- `src/gf2m_kernels/neon.rs` (ARM64 with PMULL)

### 3. Batch Evaluation Optimization (LOW-MEDIUM PRIORITY)
**Current**: Sequential Horner evaluation  
**Optimization**: Vectorized batch evaluation for syndrome computation

**BCH Syndrome Pattern**:
```rust
// Typical BCH(255, k, t=16): evaluate at α, α², ..., α^32
// Current: 32 sequential evaluations
// Optimized: SIMD batch evaluation with shared coefficient buffer
```

**Expected Speedup**: 1.5-2x for batch evaluation of 32+ points

### 4. Montgomery Reduction for Large Fields (FUTURE)
**Target**: GF(2^m) with m > 16  
**Benefit**: Faster modular reduction without table overhead

## Recommended Next Steps

1. **Implement Karatsuba multiplication** (1-2 days)
   - Add `mul_karatsuba()` method with threshold-based dispatch
   - Property tests to verify correctness vs schoolbook
   - Benchmark to validate speedup

2. **Add multiplication strategy selection** (0.5 days)
   - Make multiplication algorithm configurable
   - Add builder pattern: `poly1.mul_with_strategy(poly2, Strategy::Karatsuba)`

3. **SIMD field operations** (2-3 days)
   - Implement PCLMULQDQ-based GF(2^m) multiplication
   - Integrate with existing SIMD infrastructure (gf2-kernels-simd)
   - Runtime CPU feature detection

4. **BCH-specific benchmarks** (0.5 days)
   - Add realistic BCH(255, k, t) encoding/decoding simulation
   - Measure end-to-end performance impact
   - Compare against reference implementations

## Performance Targets

| Operation | Current (deg 200) | Target (Karatsuba) | Target (+ SIMD) |
|-----------|-------------------|-------------------|-----------------|
| Multiply (GF256)  | 352 µs | 120 µs (-66%) | 50 µs (-86%) |
| Multiply (GF65536)| 527 µs | 180 µs (-66%) | 80 µs (-85%) |
| Division (GF256)  | 77 µs  | 50 µs (-35%)  | 30 µs (-61%) |
| BCH Syndrome      | N/A    | N/A           | 10-15 µs (batched) |

## Files Modified/Created

**Benchmark File**:
- ✅ `benches/polynomial.rs` - Comprehensive polynomial benchmarks

**Next Implementation**:
- `src/gf2m_poly.rs` - Extract polynomial code from gf2m.rs
- `src/alg/karatsuba.rs` - Karatsuba multiplication
- `src/gf2m_kernels/` - SIMD field operation kernels

## Notes

- Current schoolbook implementation is well-optimized for small degrees (< 32)
- Table-based multiplication in GF(256) provides significant advantage
- For m > 16 fields, SIMD will be essential due to lack of log tables
- BCH code performance will benefit most from Karatsuba + SIMD combination

## Benchmark Command

```bash
# Run all polynomial benchmarks
cargo bench --bench polynomial

# Run specific operation
cargo bench --bench polynomial -- "multiplication"

# Save baseline for comparison
cargo bench --bench polynomial -- --save-baseline polynomial_baseline
```

---

**Recommendation**: Proceed with Karatsuba implementation as Phase 7 priority, as it provides the most significant performance improvement with moderate implementation complexity.
