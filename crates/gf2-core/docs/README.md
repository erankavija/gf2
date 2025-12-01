# gf2-core Documentation

High-performance Rust library for bit string manipulation with focus on GF(2) operations and coding theory.

## Core Documentation

### Architecture & Design
- **[KERNEL_OPTIMIZATION.md](KERNEL_OPTIMIZATION.md)** - SIMD backend architecture and optimization strategy
- **[GF2M.md](GF2M.md)** - Extension field GF(2^m) arithmetic (BCH, Reed-Solomon codes)
- **[RREF_DESIGN_PLAN.md](RREF_DESIGN_PLAN.md)** - Reduced row echelon form for LDPC encoding
- **[COMPUTE_BACKEND_DESIGN.md](COMPUTE_BACKEND_DESIGN.md)** - Runtime CPU detection and backend selection
- **[SYNC_SOLUTION_COMPARISON.md](SYNC_SOLUTION_COMPARISON.md)** - Thread safety approaches

### Implementation Guides
- **[POLAR_IMPLEMENTATION_PLAN.md](POLAR_IMPLEMENTATION_PLAN.md)** - Fast Hadamard transform for polar codes
- **[PRIMITIVE_POLYNOMIALS.md](PRIMITIVE_POLYNOMIALS.md)** - Generation and verification methodology
- **[POLY_UTILITIES_PERFORMANCE.md](POLY_UTILITIES_PERFORMANCE.md)** - Polynomial construction performance strategy
- **[SPARSE_DEDUP_DESIGN.md](SPARSE_DEDUP_DESIGN.md)** - CSR/CSC sparse matrix deduplication

### Performance Analysis
- **[BENCHMARKS.md](BENCHMARKS.md)** - Comprehensive performance vs SageMath/NTL/M4RI/FLINT

## Feature Status

### Production Ready ✅
- **Matrix-Vector Operations**: 2.7-23.6× faster than M4RI
- **RREF/Gaussian Elimination**: 304× speedup over naive, practical for DVB-T2 LDPC
- **Rank/Select**: 58-2,040× speedup, O(1) rank, O(log n) select
- **GF(2^m) Arithmetic**: 13-18× faster than NTL (m ≤ 16), 100-1000× faster than SageMath
- **SIMD Kernels**: 3.4× speedup with AVX2, optimal 8-word threshold
- **Primitive Polynomials**: 126-931× faster than SageMath
- **Polar Transforms**: 81× speedup vs naive @ N=1024
- **Sparse Matrices**: CSR/CSC formats with bidirectional access

### In Development
- Wide buffer optimizations
- SIMD polar transforms
- GF(2^m) polynomial utilities

## Quick Navigation

| Task | Document |
|------|----------|
| Understanding SIMD acceleration | [KERNEL_OPTIMIZATION.md](KERNEL_OPTIMIZATION.md) |
| Implementing BCH/Reed-Solomon | [GF2M.md](GF2M.md) |
| LDPC encoding with RREF | [RREF_DESIGN_PLAN.md](RREF_DESIGN_PLAN.md) |
| Polar code construction | [POLAR_IMPLEMENTATION_PLAN.md](POLAR_IMPLEMENTATION_PLAN.md) |
| Performance comparisons | [BENCHMARKS.md](BENCHMARKS.md) |
| Thread-safe design | [SYNC_SOLUTION_COMPARISON.md](SYNC_SOLUTION_COMPARISON.md) |

## Archive

Historical implementation plans and detailed session notes are preserved in `archive/` for reference.

See [../ROADMAP.md](../ROADMAP.md) for the complete development timeline and future plans.
