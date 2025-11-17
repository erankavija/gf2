# gf2-core Documentation

This directory contains design documents, implementation notes, and performance analysis for the gf2-core crate.

## Current Documentation

### Implementation Notes
- **[Polar Transform Implementation Plan](POLAR_IMPLEMENTATION_PLAN.md)** - Phase 6 complete implementation details
- **[GF(2^m) Session Notes](GF2M_SESSION_NOTES.md)** - Complete development history of extension field implementation
- **[GF(2^m) Design](GF2M_DESIGN.md)** - Architecture and design decisions

### Performance Analysis
- **[Polynomial Benchmarks](polynomial_benchmarks.md)** - Karatsuba optimization results (1.88x speedup)
- **[Rank/Select Performance](rank_select_performance.md)** - Lazy index performance analysis

## Feature Status

### Completed ✅
- **Polar Transforms** (Phase 6): 81x speedup vs naive @ N=1024, 76-105 Melem/s throughput
- **GF(2^m) Arithmetic** (Phase 8): Table-based multiplication for m ≤ 16, SIMD for m > 16
- **Karatsuba Multiplication** (Phase 7a): 1.88x speedup for degree-200 polynomials
- **SIMD Field Ops** (Phase 7b): 2.1x speedup for GF(65536) without tables
- **Sparse Matrices** (Phase 5): CSR/CSC formats with bidirectional access
- **Rank/Select** (Phase 4): O(1) rank, O(log n) select with lazy indexing

### Planned
- Wide buffer optimizations (Phase 2)
- Batch evaluation optimization (Phase 7c)
- SIMD polar transforms (Phase 6b)

## Quick Navigation

**Implementing polar codes?** → See [POLAR_IMPLEMENTATION_PLAN.md](POLAR_IMPLEMENTATION_PLAN.md)  
**Understanding GF(2^m) internals?** → See [GF2M_DESIGN.md](GF2M_DESIGN.md)  
**Benchmarking performance?** → See [polynomial_benchmarks.md](polynomial_benchmarks.md) or run `cargo bench`

See the main [ROADMAP.md](../ROADMAP.md) for the complete development timeline.
