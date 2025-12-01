# gf2-core Documentation Index

This directory contains supplementary documentation for the `gf2-core` crate. For API documentation, run `cargo doc --no-deps --open`.

## User Documentation

Documentation for users of the library:

### Getting Started
- **[../README.md](../README.md)** - Main introduction with quick start guides for BitVec and BitMatrix
- **[../examples/](../examples/)** - Runnable code examples:
  - `bitvec_basics.rs` - Essential BitVec operations tutorial
  - `matrix_basics.rs` - Essential BitMatrix operations tutorial
  - `sparse_display.rs` - Sparse matrix visualization
  - `random_generation.rs` - Random generation examples
  - `visualize_matrix.rs` - Matrix PNG export (requires `visualization` feature)

### Performance & Benchmarks
- **[BENCHMARKS.md](BENCHMARKS.md)** - Comprehensive performance analysis
  - Comparisons with M4RI, NTL, FLINT, SageMath
  - SIMD validation results (3.4-3.6× speedup)
  - M4RM matrix multiplication performance
  - RREF performance (150-170× faster than naive)
  - GF(2^m) multiplication benchmarks

### Architecture & Design
- **[KERNEL_OPTIMIZATION.md](KERNEL_OPTIMIZATION.md)** - Kernel architecture guide
  - Three-layer design: Public API → Kernel Ops → Backends
  - Smart dispatch strategy (<512 bytes: scalar, ≥512 bytes: SIMD)
  - Backend implementation details (Scalar, SIMD, future GPU/FPGA)
  - Performance optimization guidelines

### Advanced Features
- **[GF2M.md](GF2M.md)** - Extension field GF(2^m) arithmetic
  - Mathematical foundations
  - Table-based multiplication (m ≤ 16)
  - SIMD PCLMULQDQ multiplication (m > 16)
  - Primitive polynomial representation
  - Performance characteristics

- **[PRIMITIVE_POLYNOMIALS.md](PRIMITIVE_POLYNOMIALS.md)** - Primitive polynomial utilities
  - Database of standard primitive polynomials
  - Verification algorithms
  - Trinomial search
  - Parallel generation strategies

- **[COMPUTE_BACKEND_DESIGN.md](COMPUTE_BACKEND_DESIGN.md)** - Compute backend abstraction
  - Algorithm-level operations (matmul, RREF, batch encode/decode)
  - CpuBackend implementation
  - Future GPU backend design
  - Feature flag system (`parallel`, `gpu`)

## Design & Implementation Docs

Documentation for contributors and maintainers:

### Algorithm Design
- **[RREF_DESIGN_PLAN.md](RREF_DESIGN_PLAN.md)** - RREF algorithm design
  - Pivoting strategies
  - Gaussian elimination over GF(2)
  - Word-level optimization

- **[POLAR_IMPLEMENTATION_PLAN.md](POLAR_IMPLEMENTATION_PLAN.md)** - Polar transform design
  - Fast Hadamard Transform (FHT)
  - Bit-reversal permutation
  - Recursive implementation strategy

- **[SPARSE_DEDUP_DESIGN.md](SPARSE_DEDUP_DESIGN.md)** - Sparse matrix deduplication
  - CSR/CSC format details
  - Efficient duplicate handling
  - Memory optimization

### Performance Analysis
- **[POLY_UTILITIES_PERFORMANCE.md](POLY_UTILITIES_PERFORMANCE.md)** - Polynomial utilities performance
  - Exhaustive vs trinomial search
  - Parallel generation strategies
  - Verification algorithm benchmarks

- **[SYNC_SOLUTION_COMPARISON.md](SYNC_SOLUTION_COMPARISON.md)** - Thread safety analysis
  - Lazy rank/select index synchronization
  - Mutex vs OnceCell vs lazy_static comparison
  - Thread safety verification

## Quality Assurance

Documentation related to testing and quality:

- **[QUALITY_AUDIT_PLAN.md](QUALITY_AUDIT_PLAN.md)** - Quality audit plan (historical)
- **[QUALITY_AUDIT_REPORT.md](QUALITY_AUDIT_REPORT.md)** - Quality audit findings (historical)
- **[DOCUMENTATION_AUDIT_PLAN.md](DOCUMENTATION_AUDIT_PLAN.md)** - Documentation audit plan (2025-12-01)
- **[DOCUMENTATION_AUDIT_REPORT.md](DOCUMENTATION_AUDIT_REPORT.md)** - Documentation audit findings and deliverables

## Document Categories by Audience

### For New Users
1. Start with [../README.md](../README.md) - Focus on BitVec/BitMatrix quick starts
2. Run examples: `cargo run --example bitvec_basics` and `cargo run --example matrix_basics`
3. Explore rustdocs: `cargo doc --no-deps --open`

### For Performance-Conscious Users
1. [BENCHMARKS.md](BENCHMARKS.md) - Understand performance characteristics
2. [KERNEL_OPTIMIZATION.md](KERNEL_OPTIMIZATION.md) - Learn about SIMD acceleration
3. Enable `simd` feature for large-scale operations

### For Advanced Users
1. [GF2M.md](GF2M.md) - Extension field arithmetic
2. [PRIMITIVE_POLYNOMIALS.md](PRIMITIVE_POLYNOMIALS.md) - Polynomial generation
3. [COMPUTE_BACKEND_DESIGN.md](COMPUTE_BACKEND_DESIGN.md) - Backend customization

### For Contributors
1. [KERNEL_OPTIMIZATION.md](KERNEL_OPTIMIZATION.md) - Understand kernel architecture
2. [RREF_DESIGN_PLAN.md](RREF_DESIGN_PLAN.md), [POLAR_IMPLEMENTATION_PLAN.md](POLAR_IMPLEMENTATION_PLAN.md) - Algorithm designs
3. [DOCUMENTATION_AUDIT_PLAN.md](DOCUMENTATION_AUDIT_PLAN.md) - Current documentation goals
4. [../ROADMAP.md](../ROADMAP.md) - Future development plans

## Maintenance

This index is maintained as part of the documentation audit process. Last updated: 2025-12-01

If you're adding new documentation:
1. Add the file to the appropriate section above
2. Include a brief description of its purpose and audience
3. Update the "Last updated" date
4. Ensure the document follows the project's documentation standards (see `../.github/copilot-instructions.md`)
