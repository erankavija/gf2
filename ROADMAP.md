# gf2 Workspace Roadmap

This document provides strategic direction for the gf2 workspace. For detailed implementation plans, see:

- **[crates/gf2-core/ROADMAP.md](crates/gf2-core/ROADMAP.md)** - Performance primitives and optimization phases
- **[crates/gf2-coding/ROADMAP.md](crates/gf2-coding/ROADMAP.md)** - Coding theory algorithms and DVB-T2 FEC

## Vision

A cohesive toolkit for high-performance binary field computing and coding theory, serving both production systems and research applications with clean, composable APIs that hide implementation complexity.

## Current Focus

**gf2-core**: Primitive polynomial verification (M12 - Phase 1)
- 🎯 Primitivity testing algorithms (Rabin test)
- 🎯 Standard polynomial database (DVB-T2, AES, 5G NR)
- 🎯 Compile-time warnings for non-standard polynomials
- 🎯 Prevent BCH bugs from wrong primitive polynomials
- **Motivation**: DVB-T2 BCH failure from incorrect GF(2^14) polynomial

**gf2-core**: Core primitives mature and feature-complete
- ✅ GF(2^m) extension field arithmetic
- ✅ Karatsuba multiplication (1.88x speedup)
- ✅ SIMD field operations (2.1x for large fields)
- ✅ Sparse matrix primitives (CSR/CSC)
- ✅ Rank/select operations (lazy indexing)
- ✅ Polar transforms (81x speedup vs naive)

**gf2-coding**: DVB-T2 FEC simulation
- ✅ BCH codes with algebraic decoding (45 tests passing)
- ✅ Quasi-cyclic LDPC framework
- ⏸️ DVB-T2 LDPC base matrices (paused for M12)
- 🔮 QAM modulation and FEC chain (planned)

## Strategic Pillars

### 1. Performance & Correctness
- **gf2-core focus**: Optimized kernels with SIMD acceleration
- Word-level operations minimizing branching
- Comprehensive testing with property-based validation
- Tail masking invariant maintained rigorously

### 2. Algorithmic Breadth & Education
- **gf2-coding focus**: Error-correcting codes and channel models
- Educational examples with mathematical documentation
- DVB-T2 standard compliance
- Research-oriented experimentation

### 3. Composability & Clean APIs
- Functional style at high level, imperative in kernels
- Minimal dependencies, safe by default
- Clear separation between primitives and applications
- Runtime feature detection for SIMD

### 4. Documentation & Demonstrations
- Comprehensive API documentation with examples
- Working examples demonstrating coding theory concepts
- Performance benchmarks establishing baselines
- Design documentation for complex algorithms

## Key Dependencies

**Cross-crate dependencies enabling higher-level features:**
- Extension field GF(2^m) (core) → BCH algebraic decoding (coding)
- **Primitive polynomials (core) → BCH field construction (coding)** ⬅️ **NEW**
- Sparse matrices (core) → LDPC belief propagation (coding)
- Polynomial arithmetic (core) → BCH syndrome computation (coding)
- Rank/select (core) → Sparse graph operations (coding)
- Polar transforms (core) → 5G polar code research (coding)

## Completed Milestones

| Milestone | Description | Status |
|-----------|-------------|--------|
| **M1** | Scalar baseline: BitVec, BitMatrix, basic algorithms | ✅ Complete |
| **M2** | SIMD acceleration: AVX2 kernels with runtime dispatch | ✅ Complete |
| **M3** | Extension fields: GF(2^m) arithmetic and polynomials | ✅ Complete |
| **M4** | Sparse matrices: CSR/CSC for low-density operations | ✅ Complete |
| **M5** | Polynomial optimization: Karatsuba and SIMD | ✅ Complete |
| **M6** | Block codes: Hamming and syndrome decoding | ✅ Complete |
| **M7** | Convolutional codes: Viterbi decoder | ✅ Complete |
| **M8** | BCH codes: Algebraic decoding for DVB-T2 | ✅ Complete |
| **M9** | LDPC framework: Belief propagation and QC codes | ✅ Complete |
| **M10** | Rank/select: Succinct bit operations | ✅ Complete |
| **M11** | Polar transforms: Fast Hadamard Transform | ✅ Complete |

## Active Development

| Milestone | Description | Status |
|-----------|-------------|--------|
| **M12** | Primitive polynomials: Verification & generation (Phase 1) | 🎯 Starting |
| **M13** | DVB-T2 LDPC: Standard base matrices | Planned |
| **M14** | QAM modulation: Soft-decision demapping | Planned |
| **M15** | FEC simulation: End-to-end DVB-T2 chain | Planned |

## Future Directions

| Area | Description | Priority |
|------|-------------|----------|
| **SIMD polar transforms** | AVX2 optimization for polar codes | Low |
| **5G polar codes** | Capacity-approaching codes research | Medium |
| **AVX-512** | Extended SIMD support | Low |
| **ARM NEON** | AArch64 SIMD kernels | Low |

## Research Questions

- **Optimal sparse matrix representations**: When to use CSR vs. dual CSR+CSC?
- **SIMD dispatch granularity**: Function-level vs. kernel-level dispatch?
- **Quantization strategies**: Fixed-point LLRs for embedded systems?
- **GPU acceleration**: Feasibility for LDPC iterations?

## Contributing

High-impact contribution areas:
- **Performance**: Benchmarking on diverse CPU architectures
- **Documentation**: Educational examples with decoding traces
- **Testing**: Property-based tests for new algorithms
- **Research**: Novel code constructions with validation

See subproject roadmaps for detailed tasks.

---

*For implementation details, see [crates/gf2-core/ROADMAP.md](crates/gf2-core/ROADMAP.md) and [crates/gf2-coding/ROADMAP.md](crates/gf2-coding/ROADMAP.md).*
