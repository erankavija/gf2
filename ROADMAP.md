# gf2 Workspace Roadmap

This document provides strategic direction for the gf2 workspace. For detailed implementation plans, see:

- **[crates/gf2-core/ROADMAP.md](crates/gf2-core/ROADMAP.md)** - Performance primitives and optimization phases
- **[crates/gf2-coding/ROADMAP.md](crates/gf2-coding/ROADMAP.md)** - Coding theory algorithms and DVB-T2 FEC
- **[dev/plans/ae82bd73-gf2-algebra-permanent/gf2_algebra_permanent.md](dev/plans/ae82bd73-gf2-algebra-permanent/gf2_algebra_permanent.md)** - gf2-algebra-permanent epic: bipedal F_3/F_5/F_7 permanents, SIMD/GPU acceleration, Lean verification

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
| **M16** | gf2-algebra-permanent epic (W0–W7): bipedal F_3/F_5/F_7 packed permanents | 2026-Q2 |

### gf2-algebra-permanent outcomes (M16)

The `gf2-algebra` crate (W0–W7 waves) delivered the following headline outcomes against the reference paper (Scheinerman 2024, arXiv 2407.20205v2):

- **50× speedup target**: the epic's headline `≥ 50×` speedup goal vs the reference. On the **single-thread CPU** path this target was empirically falsified — the Rust-vs-Rust comparison measured **~10.6×** at n=36 (`dev/benchmarks/gf2_algebra_permanent/s1_speedup-2026-05-11.csv`; ratio=10.6434), because the paper's 50×/86.9× figures reflect a Julia baseline with JIT/GC overhead the Rust reference lacks. Per a user escalation (2026-05-12) the **50× target moved to the GPU contender** (issue `9480f8a6`, S1g): the gfx1030 batched GPU path is where `≥ 50×` vs `permanent_mod3_reference` at n=36 is pursued (measurement landing in W7).
- **~10.6× single-thread AVX2 speedup** over the in-tree Rust reference (`permanent_mod3_reference`) at n=36 on AMD Ryzen 9 5900X / Zen 3 (source: `dev/benchmarks/gf2_algebra_permanent/s1_speedup-2026-05-11.csv`; ratio=10.6434).
- **GPU batch M=256 baseline qualification**: the reported n=24 28.65× and n=28 30.32× ratios on AMD Radeon RX 6950 XT / gfx1030 reproduce against the sequential single-thread AVX2 path (`dev/benchmarks/gf2_algebra_permanent/s5_gpu_crossover-2026-05-15.csv`). Against the best measured processor path, the feasibility evidence restates those comparable configurations as 0.46× and 0.44×. The [replicated backend-ordering receipt](dev/benchmarks/permanent_campaign/backend-ordering.md) preserves both readings and their contradiction; its new q=3, n=28 accelerator timing used M=1024 and did not remeasure the older M=256 ratios. The S1g GPU-vs-reference 50× measurement is tracked in `9480f8a6`.
- **F_5 and F_7 packed kernels**: `Packed5` (64 lanes/u64-triple), `Packed7` (16 lanes/u64-pair), `permanent_bipedal5` and `permanent_bipedal7` fast paths.
- **Lean V1 complete**: `proofs/Gf2Algebra/Proofs/Bipedal3Correctness.lean` proves all four bipedal F_3 operations (add/sub/mul/neg) correct via Charon/Aeneas extraction from live Rust source.
- **Lean V2 in progress**: `proofs/Gf2Algebra/Proofs/RyserBounded.lean` (bounded n<=63 Ryser formula correctness; sessions 1-3 landed, full proof pending).

## In Progress

| Milestone | Description | Status |
|-----------|-------------|--------|

## Planned

| Milestone | Description | Priority | Research Focus |
|-----------|-------------|----------|----------------|
| **M17** | GPU/FPGA acceleration: Belief propagation prototypes | High | Hardware acceleration, memory vs compute bottlenecks |
| **M18** | QAM modulation: Soft-decision demapping for FEC chain | High | Channel modeling, LLR integration |
| **M19** | End-to-end DVB-T2: Full FEC + BICM simulation | High | System integration, FER curves vs Shannon limit |
| **M20** | Competitive benchmarking: vs Magma/Sage/AFF3CT | High | Performance positioning, gap analysis |
| **M21** | GRAND decoding: Universal decoder for short codes | Research | Alternative to algebraic methods |
| **M22** | 5G polar codes: CRC-aided SCL decoder | Research | Modern capacity-approaching codes |
| **M23** | Neural-aided BP: ML-enhanced LDPC decoding | Research | Iteration reduction for fixed FER |
| **M24** | SDR integration: GNU Radio blocks for real signals | Research | Practical validation, throughput |

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

*For implementation details, see [crates/gf2-core/ROADMAP.md](crates/gf2-core/ROADMAP.md), [crates/gf2-coding/ROADMAP.md](crates/gf2-coding/ROADMAP.md), and [dev/plans/ae82bd73-gf2-algebra-permanent/gf2_algebra_permanent.md](dev/plans/ae82bd73-gf2-algebra-permanent/gf2_algebra_permanent.md).*
