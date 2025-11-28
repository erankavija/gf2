# gf2 Workspace Roadmap

This document provides strategic direction for the gf2 workspace. For detailed implementation plans, see:

- **[crates/gf2-core/ROADMAP.md](crates/gf2-core/ROADMAP.md)** - Performance primitives and optimization phases
- **[crates/gf2-coding/ROADMAP.md](crates/gf2-coding/ROADMAP.md)** - Coding theory algorithms and DVB-T2 FEC

## Vision

A **research-grade** toolkit for high-performance binary field computing and coding theory, **competing with specialized computer algebra systems** (Magma/Sage) while serving both production systems and academic research with clean, composable APIs that hide implementation complexity.

**Philosophy**: Standards (DVB-T2, 5G NR) provide the foundation, but the ultimate goal is to **push beyond existing implementations** with novel algorithms, competitive performance, and open research.

## Status Summary (2025-11-27)

**gf2-core**: Production-grade primitives with SIMD acceleration
- ✅ GF(2^m) extension field arithmetic with Karatsuba multiplication
- ✅ SIMD field operations (AVX2, PCLMULQDQ, ~178 SIMD instructions active)
- ✅ Primitive polynomial verification and generation (m=2..16)
- ✅ Dense and sparse matrix primitives (bit-packed + CSR/CSC dual format)
- ✅ Rank/select operations with lazy indexing
- ✅ Polar transforms (Fast Hadamard Transform)
- ✅ File I/O for matrix serialization (529 MB DVB-T2 cache)

**gf2-coding**: DVB-T2 validation complete, optimization phase active
- ✅ BCH codes: 202/202 blocks match ETSI EN 302 755 test vectors
- ✅ LDPC codes: 202/202 blocks match test vectors (encoding + decoding)
- ✅ All 12 DVB-T2 LDPC configurations implemented
- ✅ Richardson-Urbanke systematic encoding with dense matrix cache
- ✅ Min-sum belief propagation decoder
- 🔧 **Active**: Performance optimization (3.85 Mbps → 50-100 Mbps target)
- 🔮 QAM modulation and full FEC chain (planned)

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

## In Progress

| Milestone | Description | Status | Timeline |
|-----------|-------------|--------|----------|
| **M14** | LDPC Performance: Real-time DVB-T2 decoding | 🔧 Active | 2025-11/12 |

## Planned

| Milestone | Description | Priority | Research Focus |
|-----------|-------------|----------|----------------|
| **M15** | QAM modulation: Soft-decision demapping | Medium | Channel modeling |
| **M16** | FEC simulation: End-to-end DVB-T2 chain | High | System integration |
| **M17** | Performance benchmarking vs. specialized systems | High | Competitive analysis |
| **M18** | GRAND decoding: Universal decoder for short codes | Research | Novel algorithms |
| **M19** | Neural-aided BP: ML-enhanced LDPC decoding | Research | AI integration |

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

## Research Directions

### Near-Term (2025-Q4 / 2026-Q1)
| Topic | Focus | Justification |
|-------|-------|---------------|
| **LDPC Performance** | 50-100 Mbps real-time | Active bottleneck blocking DVB-T2 chain |
| **QAM Integration** | Soft-decision demapping | Required for end-to-end FEC simulation |
| **FER Curves** | AWGN channel validation | Establish baseline vs. Shannon limit |

### Medium-Term (2026)
| Topic | Focus | Justification |
|-------|-------|---------------|
| **GRAND Decoding** | Universal decoder for short codes | Alternative to algebraic methods; research novelty |
| **5G Polar Codes** | CRC-aided SCL decoder | Modern capacity-approaching codes |
| **Performance Benchmarking** | vs. Magma/Sage/AFF3CT | Competitive positioning and gap analysis |
| **SDR Integration** | GNU Radio blocks | Practical validation with real signals |

### Exploratory (Research)
| Topic | Focus | Research Question |
|-------|-------|-------------------|
| **Neural-Aided BP** | ML-enhanced LDPC | Can neural nets reduce iterations for fixed FER? |
| **Spatially-Coupled LDPC** | SC-LDPC with sliding window | Threshold saturation gains over QC-LDPC? |
| **GPU Acceleration** | CUDA/ROCm BP | Memory bandwidth vs. compute bottlenecks? |
| **Quantized Arithmetic** | 3-5 bit LLRs | Performance/accuracy trade-off for embedded |
| **Alternative Encodings** | Structured LDPC encoding | Avoid dense 529 MB generator matrix? |

### Long-Term Vision
- **Competitive CAS**: Establish gf2 as go-to for binary field research
- **Novel Constructions**: Publication-worthy code designs and algorithms
- **Open Benchmarks**: Industry-standard suite for FEC comparisons
- **Educational Tool**: Research-grade toolkit with pedagogical examples

## Active Research Questions

### Performance & Architecture (M14 Focus)
- **LDPC Decoding**: Can we achieve 50-100 Mbps with software-only BP?
  - Measured: 69.8% time in BP loop, 17.7% sparse iteration, 4.9% malloc
  - Hypothesis: SIMD LLR ops + batch parallelism → 10-25× speedup
  - **Current**: 1.35 Mbps → **Target**: 50-100 Mbps (37× improvement needed)
- **Encoding**: Is dense matvec (97.5% hotspot) the fundamental limit?
  - SIMD enabled: 178 instructions active, but only 3.85 Mbps achieved
  - Alternative: Structured encoding methods avoiding dense multiply?
- **Memory hierarchy**: How do cache effects limit LDPC scaling?
  - Generator matrix: 529 MB cache for DVB-T2 (loads in 16 ms)
  - Belief propagation: Message passing patterns and cache locality?

### Algorithmic Trade-offs
- **GRAND vs. Algebraic**: For short BCH codes, when is GRAND faster?
  - BCH validated but slow for t=12 errors; GRAND complexity?
- **Quantization**: LLR precision impact on DVB-T2 FER curves
  - Current: f64, Target: 3-5 bit fixed-point for embedded
- **Min-sum variants**: Normalized/offset min-sum gains over standard?
  - Current: Standard min-sum; optimization potential?

### System-Level Research
- **DVB-T2 Chain**: What's the end-to-end latency budget?
  - Deinterleaving + BCH + LDPC + QAM: 150 ms target feasible?
- **SDR Integration**: Can Rust compete with GNU Radio C++ blocks?
  - Target: 10-50× speedup over existing gr-dvbt2 blocks
- **Real Signals**: Validation against captured RF (not just test vectors)

### Computer Algebra Performance
- **Crossover Point**: When does Rust+SIMD match Magma/Sage?
  - Hypothesis: m > 32 favors SIMD, m < 16 favors specialized CAS
  - Measurement needed: Controlled benchmark suite
- **Polynomial Arithmetic**: Is Karatsuba optimal for all m?
  - Current: Karatsuba for m ≥ 8; FFT-based for m > 64?

### Hardware Acceleration
- **GPU LDPC**: Is belief propagation memory-bound or compute-bound?
  - 37× speedup needed; realistic GPU gain?
  - Research: Profile memory bandwidth vs. FLOPs
- **FPGA**: Can functional Rust map to efficient HDL?
  - Experimental: Compile gf2-core to VHDL via HLS tools

### Theoretical Foundations
- **Shannon Gap**: How close are practical LDPC decoders?
  - DVB-T2 LDPC: Measured FER curves vs. capacity
- **Finite-Length Effects**: Polar vs LDPC for N < 10K
  - Research: CRC-aided SCL competitive analysis

## Milestone Details

### M13: DVB-T2 LDPC Implementation ✅ COMPLETE

**Status**: Implementation and validation complete (2025-11-27)
- ✅ All 12 DVB-T2 LDPC configurations (Short/Normal × 6 rates)
- ✅ Validation: 202/202 blocks match ETSI EN 302 755 test vectors
- ✅ Richardson-Urbanke systematic encoding with 529 MB dense cache
- ✅ Min-sum belief propagation decoder
- ✅ BCH outer code integration tested

**Performance**: Baseline established, optimization in progress
- Current: 3.85 Mbps encoding, 1.35 Mbps decoding
- Target: 50-100 Mbps (real-time DVB-T2 reception)
- Profiling: Hotspots identified (97.5% dense matvec, 69.8% BP loop)

### M14: LDPC Performance Optimization 🔧 ACTIVE

**Goal**: Achieve real-time DVB-T2 FEC decoding (50 Mbps minimum)

**Phase 1 - Quick Wins** (Week 1, Target: 10-20 Mbps):
- Pre-allocate decoder state (eliminate 4.9% malloc overhead)
- Batch processing API for multi-core parallelism (4-8× speedup)
- Thread-local decoder pool for parallel decoding

**Phase 2 - SIMD** (Week 2, Target: 50-100 Mbps):
- Vectorize LLR operations (min-sum, additions) with AVX2
- Optimize sparse matrix iteration (CSR format investigation)
- Profile belief propagation loop internals

**Phase 3 - Advanced** (Research):
- GPU-accelerated belief propagation (investigate CUDA/ROCm)
- Alternative encoding methods (avoid dense matrix multiply)
- Quantized LLR precision analysis (3-bit vs 5-bit)

### M17: Competitive Performance Benchmarking

**Goal**: Position gf2 competitively against specialized systems

**Baseline Established**:
- ✅ LDPC decoding profiled: Clear optimization targets identified
- ✅ SIMD verification: 178 SIMD instructions active in binary
- ✅ Correctness validated: 202/202 blocks match test vectors

**Planned Comparisons**:
- **Computer Algebra**: Primitive polynomial testing vs. Magma/Sage/NTL
- **Numerical**: GF(2^m) multiplication vs. hand-optimized C libraries
- **FEC Decoders**: LDPC throughput vs. AFF3CT, IT++, commercial SDR
- **Memory Efficiency**: Sparse matrix representations vs. SciPy

**Research Questions**:
- What's the crossover point where Rust+SIMD matches Magma? (m > 32?)
- Can we achieve 2× improvement over AFF3CT with better SIMD utilization?
- GPU acceleration: Realistic speedup potential for LDPC BP?

**Deliverables**:
- Automated benchmark suite with reproducible results
- Performance report with profiling methodology
- Open-source comparison data for academic use

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
