# gf2 Workspace Roadmap

This document provides strategic direction for the gf2 workspace. For detailed implementation plans, see:

- **[crates/gf2-core/ROADMAP.md](crates/gf2-core/ROADMAP.md)** - Performance primitives and optimization phases
- **[crates/gf2-coding/ROADMAP.md](crates/gf2-coding/ROADMAP.md)** - Coding theory algorithms and DVB-T2 FEC

## Vision

A **research-grade** toolkit for high-performance binary field computing and coding theory, **competing with specialized computer algebra systems** (Magma/Sage) while serving both production systems and academic research with clean, composable APIs that hide implementation complexity.

**Philosophy**: Standards (DVB-T2, 5G NR) provide the foundation, but the ultimate goal is to **push beyond existing implementations** with novel algorithms, competitive performance, and open research.

## Current Focus

**Next**: DVB-T2 LDPC implementation (M13) and performance benchmarking (M16)

**Recently Completed**: Primitive polynomial verification (M12 - Phase 1)
- ✅ Primitivity testing algorithms (efficient order-based verification)
- ✅ Enhanced Rabin irreducibility test with GCD
- ✅ Standard polynomial database (m=2..16, DVB-T2 compliant)
- ✅ Compile-time warnings for non-standard polynomials
- ✅ Key finding: AES polynomial (0x11B) is irreducible but NOT primitive
- **Impact**: Prevents BCH bugs from wrong primitive polynomials

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

### 1. Competitive Performance
- **Target**: Match or exceed Magma/Sage on binary field operations
- **gf2-core focus**: SIMD-accelerated kernels with rigorous benchmarking
- Word-level operations minimizing branching
- Comprehensive testing with property-based validation
- Measurable goals: Within 2x of specialized systems, exceed on SIMD-favorable ops

### 2. Research & Innovation
- **gf2-coding focus**: Beyond standards - novel decoding algorithms
- Educational examples with mathematical documentation
- Standards compliance (DVB-T2, 5G NR) as baseline validation
- Experimental algorithms: GRAND, Neural BP, SC-LDPC
- Open benchmarks for reproducible research

### 3. Composability & Clean APIs
- Functional style at high level, imperative in kernels
- Minimal dependencies, safe by default
- Clear separation between primitives and applications
- Runtime feature detection for SIMD
- Research code isolated via feature flags

### 4. Publication & Validation
- Technical reports documenting novel work
- Academic conference targets: ISIT, ICC, Globecom
- Bit-exact validation against standards
- Open-source benchmark suites
- Performance claims backed by rigorous methodology

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
| **M12** | Primitive polynomials: Verification & generation (Phase 1) | ✅ Complete |
| **M13** | DVB-T2 LDPC: Standard base matrices | Planned |
| **M14** | QAM modulation: Soft-decision demapping | Planned |
| **M15** | FEC simulation: End-to-end DVB-T2 chain | Planned |
| **M16** | Performance benchmarking: Compete with Magma/Sage | Planned |

## Research Goals

### Computational Algebra Performance
- **Compete with Magma/Sage** on binary field operations
  - Primitive polynomial testing: match or exceed specialized CAS systems
  - GF(2^m) arithmetic: leverage Rust zero-cost abstractions + SIMD
  - Target: Top-tier performance in Polynomial Systems Solving benchmarks
  
### Coding Theory Innovation
- **State-of-the-art decoding algorithms**
  - Guessing Random Additive Noise Decoding (GRAND) for short codes
  - Neural-aided belief propagation for LDPC
  - Spatially-coupled LDPC with sliding window decoding
  - Polar codes with CRC-aided SCL
  
### Algorithm Research & Publication
- **Novel constructions**: Document and validate new code designs
- **Performance analysis**: Rigorous FER curves vs. theoretical bounds
- **Open benchmarks**: Reproducible results for academic comparison
  - DVB-T2 baseline for 5G NR comparisons
  - LDPC convergence speed benchmarks
  - Decoder complexity vs. performance tradeoffs

## Future Directions

| Area | Description | Priority |
|------|-------------|----------|
| **GRAND decoding** | Guessing Random Additive Noise Decoding for short codes | Medium |
| **Neural BP** | Machine learning aided belief propagation for LDPC | Research |
| **Spatially-coupled LDPC** | SC-LDPC with sliding window decoding | Medium |
| **Raptor codes** | Fountain codes for erasure channels | Low |
| **SIMD polar transforms** | AVX2 optimization for polar codes | Medium |
| **5G polar codes** | Capacity-approaching codes with CRC-SCL | High |
| **AVX-512** | Extended SIMD support (512-bit vectors) | Low |
| **ARM NEON** | AArch64 SIMD kernels for embedded | Medium |
| **GPU acceleration** | CUDA/ROCm for massive LDPC parallelism | Research |

## Research Questions

### Performance & Algorithms
- Can Rust+SIMD match Magma/Sage for m > 32? What's the crossover point?
- Optimal SIMD width for GF(2^m) operations: AVX2 (256-bit) vs AVX-512?
- Parallelization strategies for primitive polynomial search (Phase 9.2)
- Cache-efficient algorithms for large sparse matrices (N > 100K)

### Coding Theory
- GRAND vs. algebraic decoding: crossover point for BCH codes?
- Quantized LLR precision: 3-bit vs 5-bit for LDPC in DVB-T2?
- Spatially-coupled LDPC gains over standard QC-LDPC for DVB-S2X?
- Neural network architectures for BP: What's the sweet spot for complexity/gain?
- Polar codes: How close can SCL decoders get to ML with practical list sizes?

### Sparse Representations
- Optimal sparse matrix representations: When to use CSR vs. dual CSR+CSC?
- SIMD dispatch granularity: Function-level vs. kernel-level dispatch?
- Compressed sensing: Can binary field methods accelerate recovery?

### Hardware Targets
- GPU acceleration: Feasibility for LDPC iterations? Memory bandwidth bottlenecks?
- FPGA mapping: Can functional Rust compile efficiently to HDL via HLS?
- Embedded targets: ARM NEON + quantized arithmetic for IoT - power/performance?
- Custom ASICs: Design space exploration for belief propagation

### Theoretical
- How close can practical decoders get to Shannon limit with fixed latency?
- Trade-offs in decoder complexity vs. FER performance for DVB standards
- Finite-length performance: Gap between theory and practice for polar/LDPC

## Milestone Details

### M16: Competitive Performance Benchmarking

**Goal**: Establish gf2 as competitive with specialized computer algebra systems

**Benchmarks**:
- Primitive polynomial testing (m = 2..64) vs. Magma/Sage/NTL
- GF(2^m) multiplication throughput vs. hand-optimized libraries
- Sparse matrix operations vs. NumPy/SciPy (binary field specialized)
- LDPC decoding throughput vs. AFF3CT, IT++

**Deliverables**:
- Benchmark suite with automated comparison scripts
- Performance report with profiling insights
- Optimization roadmap for identified bottlenecks
- Target: Within 2x of Magma/Sage, exceed on SIMD-friendly ops

**Timeline**: After M15 (DVB-T2 baseline complete)

## Publication & Validation

### Academic Contributions
- **Technical reports**: Document novel implementations and optimizations
- **Benchmark suites**: Open-source reproducible results
- **Conference targets**: ISIT, ICC, Globecom for coding theory work
- **Journal targets**: IEEE Trans. IT, IEEE Trans. Comm for major results

### Industry Validation
- **DVB-T2 compliance**: Bit-exact match with reference implementations
- **5G NR polar**: Validate against 3GPP test vectors
- **Interoperability**: Decode real-world DVB-T2 captures
- **Performance**: Compete with commercial SDR implementations

### Open Science Principles
- All benchmarks reproducible with published code
- Performance claims backed by methodology documentation
- Comparison with commercial tools (when licensing allows)
- Data and FER curves available for verification

## Contributing

High-impact contribution areas:

**Performance & Research**:
- Benchmarking on diverse CPU architectures (Intel, AMD, ARM)
- Novel decoding algorithms with theoretical analysis
- SIMD kernel optimization for specific operations
- GPU/FPGA acceleration experiments

**Implementation**:
- Standard code implementations (5G NR, DVB-S2X)
- Property-based tests for new algorithms
- Integration tests with real-world signals

**Documentation**:
- Educational examples with decoding traces
- Research notes documenting experiments
- Performance analysis and optimization guides

See subproject roadmaps for detailed tasks.

---

*For implementation details, see [crates/gf2-core/ROADMAP.md](crates/gf2-core/ROADMAP.md) and [crates/gf2-coding/ROADMAP.md](crates/gf2-coding/ROADMAP.md).*
