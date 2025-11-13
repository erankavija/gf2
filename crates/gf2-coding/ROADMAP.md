# gf2-coding Roadmap

This roadmap captures the higher-level coding theory and compression research layers built atop `gf2-core` primitives. It intentionally separates exploratory, algorithmic work from low-level performance engineering.

## Primary Goal
**Simulate DVB-T2 FEC chain Frame Error Rate (FER) over AWGN channels**. This includes BCH outer codes, LDPC inner codes, bit interleaving, and bit-interleaved coded modulation (BICM).

See [docs/DVB_T2_DESIGN.md](docs/DVB_T2_DESIGN.md) for detailed design and implementation plan.

## Phase C1: Foundational Block Codes (Complete)
- ✅ Linear block code abstraction (`LinearBlockCode`) with generator (G) & parity-check (H) matrices
- ✅ Systematic encoding path; syndrome computation; simple Hamming construction helper
- ✅ Deterministic tests for encode/decode roundtrips
- ✅ Property-based tests with `proptest` (roundtrip, linearity, error correction, orthogonality)
- ✅ Integration tests covering full workflows and edge cases
- ✅ Benchmarks: encoding throughput, syndrome computation, decoding with/without errors, batch operations

## Phase C2: Convolutional Code Framework (Complete)
- ✅ Shift-register based encoder (full implementation)
- ✅ Viterbi decoder with hard-decision decoding
- ✅ Streaming encode/decode tests
- ✅ Educational example with comprehensive documentation (`nasa_rate_half_k3`)

## Phase C3: Soft-Decision Framework & Channel Modeling (In Progress)
**Goal: Enable LDPC simulation over AWGN channels**

### Soft-Decision Infrastructure
- ✅ LLR (log-likelihood ratio) types and basic operations
- ✅ Multi-operand box-plus for LDPC check nodes (exact tanh-based)
- ✅ Min-sum approximation variants (standard, normalized, offset)
- ✅ Numerical stability helpers (safe operations, finite checks)
- ✅ Soft-decision decoder traits (`SoftDecoder`, `IterativeSoftDecoder`)
- ✅ `DecoderResult` type with convergence tracking
- ✅ Soft-input conversion utilities (symbol → LLR mapping)
- [ ] Quantization strategies (floating-point vs. fixed-point LLRs) - deferred

### AWGN Channel Modeling
- ✅ BPSK modulation (bit → symbol mapping)
- ✅ AWGN noise generation (Box-Muller transform via Normal distribution)
- ✅ Channel simulation framework (Eb/N0 → noise variance)
- ✅ Batched transmission/reception for Monte Carlo trials

### Integration & Testing
- ✅ Property-based tests for LLR operations
- [ ] Channel capacity verification against Shannon limit
- [ ] BER/FER curve generation utilities
- [ ] Baseline performance: uncoded transmission over AWGN

## Phase C4: Advanced Decoding Algorithms (Planned)
- Syndrome table optimization (compressed mapping)
- Berlekamp–Massey for BCH-like codes (depends on polynomial arithmetic)
- Chien search integration for root finding in GF(2^m)
- Soft-input Viterbi Algorithm (SOVA) for convolutional codes

## Phase C5: Sparse & Graph-Based Codes (Complete) ✅
**LDPC codes with belief propagation decoding**

### LDPC Code Construction
- ✅ Sparse parity-check matrix format (SparseMatrixDual from gf2-core)
- ✅ Regular LDPC code generation (column/row weight specified)
- ✅ Tanner graph representation (implicit via sparse matrix)
- [ ] Irregular LDPC codes (degree distribution) - deferred
- [ ] DVB-T2 quasi-cyclic LDPC codes - Phase C10

### Belief Propagation Decoder
- ✅ Sum-product algorithm (SPA) with LLR messages (min-sum implementation)
- ✅ Min-sum approximation for reduced complexity
- ✅ Early stopping criteria (syndrome check, iteration limit)
- [ ] Normalized/offset min-sum with damping - optimization phase
- [ ] Systematic LDPC encoding - future

### Performance Analysis
- ✅ BER/FER simulation over AWGN (example: ldpc_awgn.rs)
- [ ] Waterfall and error floor characterization - manual from examples
- [ ] Comparison with Shannon limit
- [ ] Performance profiling

## Phase C6: Advanced Decoding Algorithms (Planned)
- Syndrome table optimization (compressed mapping)
- Berlekamp–Massey for BCH-like codes (depends on GF(2^m) from gf2-core Phase 8)
- Chien search integration for root finding
- Soft-input Viterbi Algorithm (SOVA) for convolutional codes

## Phase C7: Polar & Modern Codes (Research)
- Successive cancellation (SC) and SC-List decoder prototypes
- Bit-channel reliability sorting; fast Hadamard-like transforms leveraging `BitMatrix`
- Evaluate integration points with rank/select primitives

## Phase C8: Compression Experiments (Exploratory)
- Bit-level transforms (run-length, delta, XOR chaining) using `BitVec` APIs
- Entropy modeling playground (simple adaptive frequency coder over GF(2) residuals)
- Comparative benchmarks: raw vs. transformed bitstreams

## Phase C9: BCH Codes (Planned) 🎯 **DVB-T2 DEPENDENCY**
**Dependencies**: gf2-core Phase 8 (GF(2^m) arithmetic) ⚠️ **BLOCKING**

- BCH code construction over GF(2^m)
- Systematic encoding via generator polynomial
- Algebraic decoding: syndrome computation, Berlekamp-Massey, Chien search
- DVB-T2 standard BCH parameters (Normal/Long frames)
- Integration with `BlockEncoder`/`HardDecisionDecoder` traits
- Comprehensive tests including known answer tests from standards

**Estimated effort**: 1-2 weeks (after gf2-core Phase 8)

## Phase C10: DVB-T2 FEC Simulation (Planned) 🎯 **PRIMARY GOAL**
**Simulate complete DVB-T2 FEC chain with FER performance analysis**

### Components
- **DVB-T2 LDPC**: Quasi-cyclic LDPC codes per ETSI EN 302 755 (all rates, both frame sizes)
- **QAM Modulation**: QPSK, 16/64/256-QAM with Gray mapping and soft LLR demapping
- **Bit Interleaving**: DVB-T2 column-row interleaver
- **System Integration**: End-to-end transmit/receive chain
- **FER Simulation**: Monte Carlo framework with configurable parameters

### Deliverables
- Complete DVB-T2 encoder/decoder for standard configurations
- FER vs. Eb/N0 simulation framework
- Performance comparison across code rates and modulations
- Validation against Shannon limit and reference implementations
- Comprehensive documentation and examples

**Estimated effort**: 6-9 weeks (Phases C10-C15, see [docs/DVB_T2_DESIGN.md](docs/DVB_T2_DESIGN.md))

## Phase C11: Performance & Ergonomics Polish (Ongoing)
- Unified error handling and panic messages → shift towards `Result` where appropriate
- Trait refinements: streaming vs. batch encode/decode unification
- Doc examples with visual syndrome / decoding traces

## Research Placeholders / Open Questions
- Optimal data structures for extremely sparse parity-check matrices?
- When to switch from table-based to algebraic decoding for medium block lengths?
- Feasibility of GPU offload (via future crates) for LDPC iterations?
- Interplay between compression transforms and error correction ordering

## Principles
- Keep experimental algorithms isolated—avoid regressing core performance
- Favor clarity & correctness first; optimize after baseline metrics exist
- Rich documentation & visual aids for educational usability

Refer back to `crates/gf2-core/ROADMAP.md` for low-level performance phases.
