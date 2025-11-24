# gf2-core Documentation

This directory contains design documents, implementation notes, and performance analysis for the gf2-core crate.

## Active Documentation

### Design & Architecture
- **[Kernel Optimization](KERNEL_OPTIMIZATION.md)** - SIMD backend architecture and optimization strategy
- **[GF(2^m) Design](GF2M_DESIGN.md)** - Extension field architecture and design decisions
- **[Sparse Matrix Deduplication](SPARSE_DEDUP_DESIGN.md)** - Design notes for CSR/CSC deduplication

### Implementation Plans
- **[Phase 11: Performance Gap Remediation](PHASE11_IMPLEMENTATION_PLAN.md)** - M4RM optimization (gray code + flat buffer)
- **[Polar Transform Implementation](POLAR_IMPLEMENTATION_PLAN.md)** - Fast Hadamard transform implementation
- **[Primitive Polynomials](PRIMITIVE_POLYNOMIALS.md)** - Generation and verification methodology

### Performance & Benchmarks
- **[Benchmarks](BENCHMARKS.md)** - Comprehensive performance comparisons vs SageMath/NTL/M4RI/FLINT
- **[SIMD vs Scalar Results](BENCHMARK_RESULTS_SIMD_VS_SCALAR.md)** - AVX2 acceleration analysis
- **[Rank/Select Performance](rank_select_performance.md)** - Lazy indexing performance analysis

### Historical Notes
- **[GF(2^m) Session Notes](GF2M_SESSION_NOTES.md)** - Complete development history
- **[Phase 9.2 Plan](phase9_2_implementation_plan.md)** - Primitive polynomial generation methodology
- **[Phase 9.4 Plan](phase9_4_implementation_plan.md)** - C/C++ benchmarking that led to Phase 11

## Feature Status

### Completed ✅
- **M4RM Optimization** (Phase 11): 46% faster, gray code + flat buffer reuse
- **Primitive Polynomials** (Phase 9): 100-1000x faster than SageMath
- **Polar Transforms** (Phase 6): 81x speedup vs naive @ N=1024
- **GF(2^m) Arithmetic** (Phase 8): Table-based for m ≤ 16, SIMD for m > 16
- **SIMD Kernels** (Phase 3): 3.4x speedup with AVX2, smart backend selection
- **Sparse Matrices** (Phase 5): CSR/CSC formats with bidirectional access
- **Rank/Select** (Phase 4): O(1) rank, O(log n) select with lazy indexing

### Planned
- Wide buffer optimizations (Phase 2)
- SIMD polar transforms (Phase 6b)

## Quick Navigation

**Understanding SIMD kernels?** → See [KERNEL_OPTIMIZATION.md](KERNEL_OPTIMIZATION.md)  
**Implementing polar codes?** → See [POLAR_IMPLEMENTATION_PLAN.md](POLAR_IMPLEMENTATION_PLAN.md)  
**Understanding GF(2^m) internals?** → See [GF2M_DESIGN.md](GF2M_DESIGN.md)  
**Benchmarking performance?** → See [BENCHMARKS.md](BENCHMARKS.md) or run `cargo bench`  
**Recent optimizations?** → See [PHASE11_IMPLEMENTATION_PLAN.md](PHASE11_IMPLEMENTATION_PLAN.md)

See the main [ROADMAP.md](../ROADMAP.md) for the complete development timeline.
