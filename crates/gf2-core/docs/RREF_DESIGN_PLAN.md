# RREF (Reduced Row Echelon Form) Design Plan for gf2-core

## Overview

This document specifies the design and implementation plan for high-performance reduced row echelon form (RREF) computation in gf2-core. The implementation will replace naive Gaussian elimination in gf2-coding, reducing LDPC generator matrix computation from 4-9 minutes to under 10 seconds.

## Problem Statement

The `gf2-coding` crate currently implements naive row reduction for LDPC generator matrix computation, taking **4-9 minutes per DVB-T2 matrix**. This is unacceptably slow for a one-time preprocessing step and prevents interactive development.

Current implementation: Element-by-element `get/set` operations on `BitMatrix`  
Desired: Word-level operations with proper algorithm optimization

## Performance Requirements

### Current Performance (Naive Implementation)

| Matrix Size | Rows (m) | Cols (n) | Operations | Current Time | Target Time |
|-------------|----------|----------|------------|--------------|-------------|
| DVB-T2 Short Rate 1/2 | 9,000 | 16,200 | ~1.3 trillion | 553 seconds (~9 min) | <10 seconds |
| DVB-T2 Short Rate 3/5 | 6,480 | 16,200 | ~680 billion | 262 seconds (~4 min) | <5 seconds |
| DVB-T2 Normal Rate 1/2 | 32,400 | 64,800 | ~68 trillion | Estimated 20-40 min | <60 seconds |

### Target Performance

- **50-100× speedup** over naive implementation
- **<1 second** for matrices up to 10,000 × 20,000
- **<60 seconds** for matrices up to 32,400 × 64,800 (DVB-T2 Normal)

## Use Case: LDPC Generator Matrix Computation

### Context

LDPC codes require systematic encoding using generator matrices. The Richardson-Urbanke algorithm computes generator matrices from parity-check matrices using Gaussian elimination.

### Algorithm Requirements

Given parity-check matrix **H** (m × n), compute generator matrix **G** (k × n) where k = n - m:

1. **Row reduction with column pivoting** to find m independent columns
2. **Row reordering** to align pivot structure
3. **Extraction** of generator matrix G = [I_k | P]

### Current Implementation Location

`gf2-coding/src/ldpc/encoding/richardson_urbanke.rs:compute_generator_matrix()`

**Problem Code:**
```rust
// Naive element-by-element operations
for row in 0..m {
    if row != pivot_row && h_work.get(row, col) {
        for j in 0..n {
            if h_work.get(pivot_row, j) {
                h_work.set(row, j, h_work.get(row, j) ^ true);  // SLOW!
            }
        }
    }
}
```

This performs **O(m² × n)** individual bit operations instead of word-level XORs.

## API Design

### Public API

```rust
/// Compute the reduced row echelon form (RREF) of a matrix over GF(2).
///
/// Performs row reduction with column pivoting to transform the input matrix
/// into reduced row echelon form. This is the standard form produced by
/// Gaussian elimination.
///
/// # Arguments
///
/// * `matrix` - Input matrix to reduce
/// * `pivot_from_right` - If true, search for pivots from right to left;
///                        if false, search left to right
///
/// # Returns
///
/// Result containing:
/// - The reduced matrix in RREF
/// - Pivot column indices (in order found)
/// - Row permutation applied
/// - Matrix rank
///
/// # Algorithm
///
/// Uses word-level XOR operations for ~64× speedup over bit-level operations.
/// Complexity: O(m² × n / 64) for dense matrices.
///
/// # Examples
///
/// ```
/// use gf2_core::matrix::BitMatrix;
/// use gf2_core::alg::rref;
///
/// let matrix = BitMatrix::from_bytes_row_major(3, 4, &[
///     0b1010,
///     0b0110,
///     0b1100,
/// ]);
/// let result = rref(&matrix, false);
/// assert_eq!(result.rank, 2);
/// ```
pub fn rref(
    matrix: &BitMatrix,
    pivot_from_right: bool,
) -> RrefResult;

/// Result of reduced row echelon form computation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RrefResult {
    /// Matrix in reduced row echelon form
    pub reduced: BitMatrix,
    
    /// Indices of pivot columns (in order found during reduction)
    pub pivot_cols: Vec<usize>,
    
    /// Row permutation applied: reduced_row[i] = input_row[row_perm[i]]
    pub row_perm: Vec<usize>,
    
    /// Rank of the matrix (number of linearly independent rows)
    pub rank: usize,
}
```

### Supporting Low-Level Operations (Private/Internal)

These operations will be implemented to support the RREF algorithm but may remain private initially:

```rust
// In BitMatrix implementation (may be pub(crate) or private)
impl BitMatrix {
    /// XOR row `src` into row `dst` (word-level operation)
    fn row_xor(&mut self, dst: usize, src: usize);
    
    /// Swap two rows efficiently
    fn row_swap(&mut self, row1: usize, row2: usize);
    
    /// Find first non-zero entry in column starting from given row
    fn find_pivot_in_col(&self, col: usize, start_row: usize) -> Option<usize>;
}
```

## Performance Optimization Techniques

### Required Optimizations

1. **Word-level XOR operations** (64 bits at once)
   - Current: `set(i, get(i) ^ true)` per bit
   - Required: `words[dst] ^= words[src]` for entire rows

2. **Cache-friendly access patterns**
   - Row-major iteration for XOR operations
   - Minimize cache misses

3. **Early termination**
   - Skip zero rows/columns
   - Track active row ranges

### Optional Optimizations (Nice to Have)

4. **SIMD acceleration** (via existing `simd` feature)
   - 256-bit or 512-bit XOR operations
   - Expected 4-8× additional speedup

5. **Sparse-aware operations**
   - Skip operations on zero words
   - Useful for LDPC matrices (typically 2-5% density)

6. **Parallel elimination** (via existing `parallel` feature)
   - Parallelize independent row operations
   - Expected 2-4× speedup on multi-core

## Testing Requirements

### Correctness Tests

1. **Small matrices** (exact verification)
   - 3×7 Hamming code
   - 7×15 Reed-Solomon
   - Manual verification of pivot selection

2. **Medium matrices** (property-based)
   - Random 100×200 matrices
   - Verify rank computation
   - Verify row-reduced form properties

3. **Large matrices** (DVB-T2 actual)
   - 9,000 × 16,200 (Short Rate 1/2)
   - 32,400 × 64,800 (Normal Rate 1/2)
   - Verify produces valid generator matrices

### Performance Tests (Benchmarks)

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_rref_small(c: &mut Criterion) {
    let m = BitMatrix::random(100, 200);
    c.bench_function("rref_100x200", |b| {
        b.iter(|| rref(black_box(&m), false))
    });
}

fn bench_rref_dvb_short(c: &mut Criterion) {
    let m = BitMatrix::random(9000, 16200);
    c.bench_function("rref_9000x16200", |b| {
        b.iter(|| rref(black_box(&m), false))
    });
}

fn bench_rref_dvb_normal(c: &mut Criterion) {
    let m = BitMatrix::random(32400, 64800);
    c.bench_function("rref_32400x64800", |b| {
        b.iter(|| rref(black_box(&m), false))
    });
}

criterion_group!(benches, bench_rref_small, bench_rref_dvb_short, bench_rref_dvb_normal);
criterion_main!(benches);
```

**Target:** Benchmarks should show >50× improvement over naive implementation

## Implementation Plan

### Phase 0: Baseline Measurement & Competitive Analysis
1. **Create baseline benchmark infrastructure**:
   - Benchmark current naive implementation in `gf2-coding`
   - Measure actual DVB-T2 matrix reduction times
   - Establish baseline for speedup calculations

2. **Benchmark competitive implementations**:
   - **M4RI**: Gaussian elimination on dense matrices
   - Test sizes: 256×256, 512×512, 1024×1024, 2048×2048
   - Also test: 9,000×16,200 (DVB-T2 Short), 32,400×64,800 (DVB-T2 Normal) if feasible
   - Document M4RI performance characteristics and memory usage

3. **Set performance targets**:
   - **Primary target**: <10 seconds for DVB-T2 Short (9K×16K), <60 seconds for DVB-T2 Normal (32K×65K)
   - **Competitive target**: Within 2-3× of M4RI for dense matrices
   - Document baseline vs target gaps

4. **Create comparison document section**:
   - Similar format to Phase 9.4 and Phase 11.1 in `docs/BENCHMARKS.md`
   - Track progress through optimization phases
   - Include competitive analysis table

**Deliverables**:
- M4RI C++ benchmark program: `benchmarks-cpp/m4ri_gauss.cpp`
- Baseline measurements documented
- Performance gap analysis

### Phase 1: Core Algorithm (TDD)
1. Write tests first:
   - Small matrices (3×7, 7×15) with known RREF
   - Property tests: rank(A) ≤ min(m,n), RREF properties
   - Edge cases: empty, single row/col, all zeros
2. Implement basic RREF with word-level operations
3. Verify all tests pass
4. **Initial benchmark**: Measure against Phase 0 baseline

### Phase 2: Low-Level Optimizations
1. Implement efficient row operations:
   - `row_xor()` using word-level XOR (reuse existing `kernels::ops::xor_inplace`)
   - `row_swap()` with minimal copying
   - `find_pivot_in_col()` with early termination
2. Add benchmarks for row operations
3. Profile and optimize hot paths
4. **Benchmark against M4RI**: Compare with competitive baseline

### Phase 3: Performance Testing & Competitive Analysis
1. Add comprehensive benchmarks for various matrix sizes:
   - Small: 100×200, 256×256
   - Medium: 512×512, 1024×1024
   - Large: 2048×2048
   - DVB-T2 Short: 9,000×16,200
   - DVB-T2 Normal: 32,400×64,800
2. Verify >50× speedup vs naive implementation
3. **Compare with M4RI Gaussian elimination**:
   - Target: Within 2-3× of M4RI performance
   - Document performance gap analysis (similar to Phase 11.1 matrix inversion)
   - Identify specific bottlenecks
4. Profile and create optimization priority list

### Phase 4: Advanced Optimizations (If Needed)
Based on Phase 3 competitive analysis, implement optimizations to close gap with M4RI:

**Memory Layout**:
1. Optimize cache locality for row access patterns
2. Consider block-wise processing for large matrices
3. Minimize allocations during elimination

**Algorithmic**:
1. Optimize pivot search (batch scanning, early termination)
2. Cache-aware row elimination ordering
3. Skip zero-word operations in sparse regions

**SIMD** (Optional, if `simd` feature enabled):
1. Vectorize row XOR operations beyond existing kernels
2. SIMD-accelerated pivot search
3. Benchmark SIMD vs scalar trade-offs

**Target**: Achieve within 2× of M4RI for matrices up to 2048×2048

### Phase 5: Verification & Integration
1. **Final competitive benchmark**:
   - Document final performance vs M4RI
   - Add results to `docs/BENCHMARKS.md` (new section: "RREF / Gaussian Elimination")
   - Compare with Phase 0 baseline

2. **Integration with gf2-coding**:
   - Update `richardson_urbanke.rs` to use `rref()`
   - Verify all LDPC tests pass
   - Measure actual DVB-T2 cache generation time

3. **Success verification**:
   - DVB-T2 Short: <10 seconds (vs 4-9 minutes baseline)
   - DVB-T2 Normal: <60 seconds (vs 20-40 minutes estimated)
   - Regenerate cache files with new implementation

4. Remove old naive implementation

## Integration with gf2-coding

### Usage Example (After Implementation)

```rust
use gf2_core::alg::rref;

// Convert sparse H to dense for row reduction
let h_dense = h_sparse.to_dense();

// Compute RREF with right-to-left pivoting
let result = rref(&h_dense, true);

// Extract pivot columns (for parity positions)
let parity_cols = result.pivot_cols;

// Use reduced matrix and row permutation to build generator
let generator = build_generator_from_rref(
    &result.reduced,
    &result.row_perm,
    &parity_cols,
    k,
    n,
);
```

## Success Criteria

✅ **Performance:** <10 seconds for DVB-T2 Short, <60 seconds for DVB-T2 Normal  
✅ **Correctness:** All existing LDPC tests pass with new implementation  
✅ **API Quality:** Clean, well-documented API suitable for general use  
✅ **Test Coverage:** >95% coverage with property-based tests  

## Priority

**HIGH** - This is blocking efficient LDPC code development

- Currently requires 2-5 hours to generate full DVB-T2 cache
- Interactive development is painful (4-9 minutes per test iteration)
- Large codes (DVB-T2 Normal) are impractical without optimization

## File Locations

### Implementation
- `gf2-core/src/alg/rref.rs` - New module for RREF algorithm
- `gf2-core/src/alg/mod.rs` - Export `rref` module
- `gf2-core/src/matrix.rs` - Add row operation methods if needed

### Tests
- `gf2-core/src/alg/rref.rs` - Unit tests and property tests
- `gf2-core/tests/rref_integration.rs` - Integration tests with real matrices

### Benchmarks
- `gf2-core/benches/rref.rs` - Performance benchmarks (Rust)
- `gf2-core/benchmarks-cpp/m4ri_gauss.cpp` - M4RI competitive baseline (C++)

### Documentation
- This file serves as the design specification
- Update `gf2-core/README.md` with RREF examples after implementation
- Add RREF competitive analysis section to `docs/BENCHMARKS.md`

## References

- Current implementation: `gf2-coding/src/ldpc/encoding/richardson_urbanke.rs:148-213`
- Use case: LDPC systematic encoding (Richardson-Urbanke algorithm)
- Related: gf2-core already has `alg::gauss::invert()` for matrix inversion
- Performance data: `gf2-coding/docs/DVB_T2_VERIFICATION_STATUS.md` Phase C10.6.3

## Contact

For questions or clarification, see:
- `gf2-coding/docs/DVB_T2_VERIFICATION_STATUS.md` - Full context
- `gf2-coding/src/ldpc/encoding/richardson_urbanke.rs` - Current implementation
- `gf2-coding/generate_cache.rs` - Cache generation (where slowness is visible)
