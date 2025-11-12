# gf2-coding Roadmap

This roadmap captures the higher-level coding theory and compression research layers built atop `gf2-core` primitives. It intentionally separates exploratory, algorithmic work from low-level performance engineering.

## Primary Goal
**Achieve the ability to simulate LDPC error rate performance over AWGN channels**. This guides prioritization toward soft-decision decoding infrastructure and channel modeling.

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

## Phase C5: Sparse & Graph-Based Codes (Critical for Primary Goal)
**Target: LDPC codes with belief propagation decoding**

### LDPC Code Construction
- [ ] Sparse parity-check matrix format (CSR / bit-packed hybrid)
- [ ] Regular LDPC code generation (column/row weight specified)
- [ ] Irregular LDPC codes (degree distribution)
- [ ] Tanner graph representation (bipartite graph abstraction)

### Belief Propagation Decoder
- [ ] Sum-product algorithm (SPA) with LLR messages
- [ ] Min-sum approximation for reduced complexity
- [ ] Normalized/offset min-sum variants
- [ ] Early stopping criteria (syndrome check, iteration limit)
- [ ] Damping strategies for convergence

### Performance Analysis
- [ ] BER/FER simulation over AWGN
- [ ] Waterfall region characterization
- [ ] Error floor analysis
- [ ] Comparison with Shannon limit and turbo codes
- [ ] Profiling: memory bandwidth vs. iteration count

## Phase C6: Polar & Modern Codes (Research)
- Successive cancellation (SC) and SC-List decoder prototypes
- Bit-channel reliability sorting; fast Hadamard-like transforms leveraging `BitMatrix`
- Evaluate integration points with rank/select primitives

## Phase C7: Compression Experiments (Exploratory)
- Bit-level transforms (run-length, delta, XOR chaining) using `BitVec` APIs
- Entropy modeling playground (simple adaptive frequency coder over GF(2) residuals)
- Comparative benchmarks: raw vs. transformed bitstreams

## Phase C8: Performance & Ergonomics Polish (Ongoing)
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
