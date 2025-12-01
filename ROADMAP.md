# gf2 Workspace Roadmap

This document provides strategic direction for the gf2 workspace. For detailed implementation plans, see:

- **[crates/gf2-core/ROADMAP.md](crates/gf2-core/ROADMAP.md)** - Performance primitives and optimization phases
- **[crates/gf2-coding/ROADMAP.md](crates/gf2-coding/ROADMAP.md)** - Coding theory algorithms and DVB-T2 FEC

## Vision

A **research-grade** toolkit for high-performance binary field computing and coding theory, **competing with specialized computer algebra systems** (Magma/Sage) while serving both production systems and academic research with clean, composable APIs that hide implementation complexity.

**Philosophy**: Standards (DVB-T2, 5G NR) provide the foundation, but the ultimate goal is to **push beyond existing implementations** with novel algorithms, competitive performance, and open research.

## Strategic Pillars

### 1. Research-Driven Development
**Philosophy**: Standards provide validation, innovation drives value
- Standards (DVB-T2, 5G NR) establish correctness baseline
- Research focus: Novel algorithms, performance insights, open questions
- Documentation emphasizes "why" and "what's unknown" over "how"
- Experimental features behind feature flags for safe exploration
- Publication-ready: All work reproducible with documented methodology

### 2. Competitive Performance Through Understanding
**Goal**: Match specialized systems by understanding bottlenecks
- SIMD-first design with fallback scalar paths
- Profile-guided optimization with documented hotspots
- Measurable targets: Within 2× of Magma/Sage, exceed on SIMD ops
- Research questions guide optimization priorities
- Performance claims backed by rigorous profiling methodology

### 3. Composable, Functional APIs
**Principle**: Clean abstractions that hide complexity
- Functional programming at API level, imperative in kernels
- Pure functions with immutability where practical
- Type-driven design with strong compile-time guarantees
- Performance critical paths clearly documented and profiled
- Test-driven development with property-based validation

### 4. Open Science & Academic Rigor
**Commitment**: Reproducible research and open benchmarks
- All performance claims with published profiling data
- Bit-exact validation against official test vectors
- Open-source benchmark suites for competitive analysis
- Technical reports document novel insights
- Target venues: ISIT, ICC, Globecom, IEEE Trans. IT

## Key Dependencies

**Cross-crate dependencies enabling higher-level features:**
- Extension field GF(2^m) (core) → BCH algebraic decoding (coding)
- **Primitive polynomials (core) → BCH field construction (coding)** ⬅️ **NEW**
- Sparse matrices (core) → LDPC belief propagation (coding)
- Polynomial arithmetic (core) → BCH syndrome computation (coding)
- Rank/select (core) → Sparse graph operations (coding)
- Polar transforms (core) → 5G polar code research (coding)

## Completed Milestones

| Milestone | Description | Completion |
|-----------|-------------|------------|
| **M1** | Scalar baseline: BitVec, BitMatrix, basic algorithms | 2024-Q4 |
| **M2** | SIMD acceleration: AVX2 kernels with runtime dispatch | 2024-Q4 |
| **M3** | Extension fields: GF(2^m) arithmetic and polynomials | 2024-Q4 |
| **M4** | Sparse matrices: CSR/CSC for low-density operations | 2025-Q1 |
| **M5** | Polynomial optimization: Karatsuba and SIMD | 2025-Q1 |
| **M6** | Block codes: Hamming and syndrome decoding | 2024-Q4 |
| **M7** | Convolutional codes: Viterbi decoder | 2025-Q1 |
| **M8** | BCH codes: Algebraic decoding for DVB-T2 | 2025-Q4 |
| **M9** | LDPC framework: Belief propagation and QC codes | 2025-Q4 |
| **M10** | Rank/select: Succinct bit operations | 2025-Q1 |
| **M11** | Polar transforms: Fast Hadamard Transform | 2025-Q1 |
| **M12** | Primitive polynomials: Verification & generation | 2025-Q2 |
| **M13** | DVB-T2 LDPC: All 12 configurations + validation | 2025-Q4 |
| **M14** | LDPC Performance: Profiling & baseline | 2025-Q4 |
| **M15** | Parallel Computing Framework: CPU backend | 2025-Q4 |

## In Progress

| Milestone | Description | Status |
|-----------|-------------|--------|

## Planned

| Milestone | Description | Priority | Research Focus |
|-----------|-------------|----------|----------------|
| **M16** | GPU/FPGA acceleration: Belief propagation prototypes | High | Hardware acceleration, memory vs compute bottlenecks |
| **M17** | QAM modulation: Soft-decision demapping for FEC chain | High | Channel modeling, LLR integration |
| **M18** | End-to-end DVB-T2: Full FEC + BICM simulation | High | System integration, FER curves vs Shannon limit |
| **M19** | Competitive benchmarking: vs Magma/Sage/AFF3CT | High | Performance positioning, gap analysis |
| **M20** | GRAND decoding: Universal decoder for short codes | Research | Alternative to algebraic methods |
| **M21** | 5G polar codes: CRC-aided SCL decoder | Research | Modern capacity-approaching codes |
| **M22** | Neural-aided BP: ML-enhanced LDPC decoding | Research | Iteration reduction for fixed FER |
| **M23** | SDR integration: GNU Radio blocks for real signals | Research | Practical validation, throughput |

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

## Open Research Questions

### Hardware Acceleration
- **GPU LDPC**: Is belief propagation memory-bound or compute-bound?
- **FPGA feasibility**: Can functional Rust map to efficient HDL?
- **Crossover points**: When does GPU beat multi-core CPU for LDPC?

### Algorithm Development
- **GRAND vs. Algebraic**: When is GRAND faster for short codes?
- **Min-sum variants**: Normalized/offset gains over standard min-sum?
- **Quantized LLRs**: 3-8 bit precision vs accuracy tradeoff for embedded
- **Alternative encodings**: Structured LDPC avoiding dense matrices?
- **Neural-aided BP**: Can ML reduce iterations while maintaining FER?

### System Integration
- **End-to-end DVB-T2**: Latency budget for deinterleave + BCH + LDPC + QAM?
- **SDR performance**: Can Rust match/exceed GNU Radio C++ throughput?
- **Real signal validation**: Performance on captured RF vs test vectors

### Performance Comparison
- **Computer algebra**: Rust+SIMD vs Magma/Sage crossover point (m > 32?)
- **FEC decoders**: How close to AFF3CT/IT++ can we get?
- **Polynomial arithmetic**: Is Karatsuba optimal, or FFT-based for m > 64?

### Theoretical Analysis
- **Shannon gap**: How close are practical LDPC decoders to capacity?
- **Finite-length effects**: Polar vs LDPC competitive analysis for N < 10K
- **Spatially-coupled LDPC**: Threshold saturation gains over QC-LDPC?

## Long-Term Vision

- **Competitive CAS**: Establish gf2 as go-to for binary field research
- **Novel constructions**: Publication-worthy code designs and algorithms
- **Open benchmarks**: Industry-standard suite for FEC comparisons
- **Educational tool**: Research-grade toolkit with pedagogical examples

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
