# GF(2^m) Polynomial Utilities - Performance Strategy

**Date**: 2025-11-30  
**Status**: Implementation Phase  
**Related**: GF2M_POLY_UTILITIES_REQUIREMENTS.md

## Performance Philosophy

Based on gf2-core's proven track record:
- **13-18× faster than NTL** for small field operations (m ≤ 16)
- **100-1000× faster than SageMath** across all operations
- **Competitive with specialized C++ libraries** for medium fields

### Strategy: Start Simple, Optimize Strategically

1. ✅ **No premature optimization** - Profile first, optimize hot paths only
2. ✅ **Benchmark against competition from day one** - NTL, SageMath, FLINT
3. ✅ **No premature SIMD** - These are construction/setup utilities, not runtime hotpaths
4. ✅ **Focus on algorithmic efficiency** - Correct algorithm choice matters most

## Operation Performance Classification

### Low Priority for Optimization (Setup/Construction)

**`from_exponents(field, &[usize])`**
- **Complexity**: O(max_exp) coefficient copy
- **Usage**: Called once during BCH generator setup
- **Cost**: Microseconds for typical BCH polynomials (degree < 1000)
- **Optimization**: None needed - simple vector operations

**`monomial(coeff, degree)`**
- **Complexity**: O(degree) allocation
- **Usage**: Helper for constructing single-term polynomials
- **Cost**: Nanoseconds to microseconds
- **Optimization**: None needed - trivial operation

**`x(field)`**
- **Complexity**: O(1) - delegates to `monomial()`
- **Usage**: Called once for symbolic construction
- **Cost**: Negligible
- **Optimization**: None needed

### Medium Priority (May Be Called in Loops)

**`from_roots(&[Gf2mElement])`**
- **Complexity**: O(n²) polynomial multiplications where n = number of roots
- **Usage**: BCH generator polynomial construction from consecutive powers of α
- **Typical n**: 2-24 for BCH codes (DVB-T2 uses t=12)
- **Cost**: ~microseconds for n < 50, may grow for larger codes
- **Optimization Strategy**:
  - **Phase 1**: Simple sequential multiplication using existing optimized `*` operator
  - **Phase 2** (if needed): Divide-and-conquer with FFT for n > 100
  - **Trigger**: Only if profiling shows >10% of BCH encoder initialization time

### Low Priority IF Only Used at Setup

**`product(&[Gf2mPoly])`**
- **Complexity**: O(n · d²) where n = number of polynomials, d = average degree
- **Usage**: DVB-T2 multiplies t generator polynomials (t ≤ 12)
- **Cost**: Delegates to existing optimized `*` operator
- **Optimization**: None needed - uses existing fast polynomial multiplication

## Performance Targets

Based on existing gf2-core achievements:

| Operation | vs SageMath Target | vs NTL Target | Rationale |
|-----------|-------------------|---------------|-----------|
| `from_exponents()` | **100-500× faster** | **10-20× faster** | Simple O(n) with table lookups |
| `from_roots()` | **50-200× faster** | **2-5× faster** | Uses optimized poly mult |
| `product()` | **50-200× faster** | **1-3× faster** | Delegates to `*` operator |

### Success Criteria

**Functional**:
- ✅ All functions implemented with TDD approach
- ✅ Comprehensive tests including edge cases
- ✅ >95% code coverage

**Performance**:
- ✅ **Benchmarks exist** for all constructors
- ✅ **Within 5× of NTL** for all operations (competitive threshold)
- ✅ **100× faster than SageMath** minimum
- ✅ **No regressions** in existing polynomial benchmarks

**Documentation**:
- ✅ Complexity analysis in doc comments
- ✅ Benchmark results in BENCHMARKS.md
- ✅ Comparison table with NTL/SageMath

## Benchmarking Plan

### Test Matrix

| Operation | Field Sizes | Input Sizes | Compare Against |
|-----------|-------------|-------------|-----------------|
| `from_exponents()` | GF(2^8), GF(2^16) | 5, 10, 50, 100, 1000 exponents | SageMath |
| `from_roots()` | GF(2^8), GF(2^16) | 2, 5, 10, 20, 50 roots | NTL `BuildFromRoots()` |
| `product()` | GF(2^8) | 2-10 polys, deg 10-100 | Manual loop baseline |

### Benchmark Implementation

**Location**: `benches/polynomial_construction.rs`

```rust
//! Benchmarks for polynomial construction utilities.
//!
//! # Competitive Comparison
//!
//! These benchmarks have equivalent implementations in:
//! - `scripts/sage_polynomial_construction.py` (SageMath)
//! - `benchmarks-cpp/ntl_polynomial_construction.cpp` (NTL)
//!
//! ## Target Performance
//!
//! - vs SageMath: 100-1000× faster
//! - vs NTL: 2-10× faster for small fields, competitive for large
```

### Competitive Benchmark Scripts

**SageMath Comparison** (`scripts/sage_polynomial_construction.py`):
```python
# Equivalent implementations for:
# - from_exponents()
# - from_roots()
# - product()
```

**NTL Comparison** (`benchmarks-cpp/ntl_polynomial_construction.cpp`):
```cpp
// Equivalent implementations using:
// - Manual construction for from_exponents()
// - BuildFromRoots() for from_roots()
// - Sequential multiplication for product()
```

## When to Optimize Further

### Optimization Trigger for `from_roots()`

Implement divide-and-conquer variant **ONLY IF**:

1. ✅ Profiling shows >10% of BCH encoder initialization time
2. ✅ Typical usage involves n > 50 roots
3. ✅ Benchmark shows NTL is >5× faster
4. ✅ Real-world use cases demonstrate need

### Advanced Optimization Strategy

```rust
pub fn from_roots_optimized(roots: &[Gf2mElement]) -> Self {
    const THRESHOLD: usize = 50;
    
    if roots.len() <= THRESHOLD {
        return Self::from_roots(roots);  // Simple sequential
    }
    
    // Divide-and-conquer with parallel tree construction
    let mid = roots.len() / 2;
    let left = Self::from_roots_optimized(&roots[..mid]);
    let right = Self::from_roots_optimized(&roots[mid..]);
    
    // Uses existing optimized polynomial multiplication
    // (Karatsuba for degree > 100, schoolbook otherwise)
    &left * &right
}
```

**Complexity**:
- Simple: O(n²) multiplications
- Divide-and-conquer: O(n log² n) with Karatsuba
- Speedup: ~5-10× for n > 100

### SIMD Considerations

**Current State**: Not needed
- Polynomial multiplication already uses optimized field operations
- Field multiplication uses table lookups for m ≤ 16 (already optimal)
- SIMD (PCLMULQDQ) used for m > 16 in existing implementation

**Future**: Only consider if:
- Profiling identifies specific bottleneck
- Benchmark shows clear benefit (>2× speedup)
- Real-world use case justifies complexity

## Real-World Performance Expectations

### DVB-T2 BCH Code Example

**Scenario**: t=12 error correction (worst case)
- **Generator**: Product of 12 polynomials, each degree 14
- **Operations**:
  - 12× `from_exponents()` calls: ~5-10 µs total
  - 1× `product()` with 12 polynomials: ~50-100 µs
  - **Total initialization**: <150 µs

**Comparison**:
- **SageMath**: ~15-50 ms (100-300× slower)
- **NTL**: ~500 µs - 2 ms (5-20× slower)
- **Target**: <150 µs (achieved with simple implementation)

### BCH(255, 239) Code (t=2)

**Scenario**: Common BCH code
- **Generator**: `g(x) = (x - α)(x - α²)` over GF(2^8)
- **Operations**:
  - 1× `from_roots()` with 2 roots: ~1-2 µs
  - **Total**: <5 µs including field setup

**Comparison**:
- **SageMath**: ~2-5 ms (500-1000× slower)
- **NTL**: ~10-20 µs (5-10× slower)
- **Target**: <5 µs

## Documentation Requirements

### In Code (Doc Comments)

For each utility function, document:
1. **Complexity**: Big-O analysis
2. **Use case**: When to use this constructor
3. **Performance characteristics**: Expected runtime for typical inputs
4. **Examples**: Real-world usage (DVB-T2, BCH codes)

### In BENCHMARKS.md

After implementation, add section:
```markdown
## Phase N: Polynomial Construction Utilities

### Comparison with Competition

| Operation | gf2-core | SageMath | NTL | Speedup (vs NTL) |
|-----------|----------|----------|-----|------------------|
| from_exponents (deg 14) | X µs | Y ms | Z µs | A× |
| from_roots (n=12, GF(256)) | X µs | Y ms | Z µs | A× |
| product (12 polys, deg ~14) | X µs | Y ms | Z µs | A× |

### Analysis

- Achieved target of 100-500× faster than SageMath ✅
- Competitive with NTL (within 5×) ✅
- No optimization needed for typical use cases ✅
```

## Migration Strategy

### Step 1: Implement and Benchmark (Week 1)
1. Write tests (TDD)
2. Implement functions
3. Add benchmarks
4. Run competitive comparisons

### Step 2: Validate Performance (Week 1)
1. Compare against targets
2. Profile BCH encoder initialization
3. Document results

### Step 3: Migrate gf2-coding (Week 2)
1. Only proceed if performance targets met
2. Deprecate old `poly_from_exponents()`
3. Update all call sites
4. Verify no performance regression in gf2-coding

### Step 4: Future Optimization (If Needed)
1. Monitor real-world usage
2. Collect profiling data from applications
3. Optimize only if evidence supports it

## Summary

**Philosophy**: Start simple, optimize strategically, benchmark continuously.

**Expected outcome**: High-quality, well-tested utilities that match or exceed gf2-core's track record of 10-1000× speedups over competition, achieved through clean functional code and smart algorithmic choices rather than premature low-level optimization.

**Key insight**: For construction utilities called during setup, correctness and clarity trump micro-optimization. The existing optimized polynomial multiplication operator provides all the performance we need.
