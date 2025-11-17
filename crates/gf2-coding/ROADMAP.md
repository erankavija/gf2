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

## Phase C3: Soft-Decision Framework & Channel Modeling (Complete) ✅
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
- ✅ Shannon capacity calculation and verification
- ✅ Shannon limit computation for target rates

### Integration & Testing
- ✅ Property-based tests for LLR operations
- ✅ Channel capacity verification against Shannon limit
- ✅ BER/FER curve generation utilities (`simulation` module)
- ✅ Baseline performance: uncoded transmission over AWGN
- ✅ Monte Carlo simulation framework with configurable parameters
- ✅ CSV export for plotting results

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
- ✅ **Quasi-cyclic LDPC framework** (Phase C10.1 complete)
  - ✅ `CirculantMatrix` type for circulant submatrices
  - ✅ `QuasiCyclicLdpc` structure with base matrix and expansion factor
  - ✅ Automatic expansion to full parity-check matrix
  - ✅ Generic design supporting DVB-T2, 5G NR, WiFi standards
  - ✅ Factory methods with placeholders (`dvb_t2_normal`, `dvb_t2_long`, `nr_5g`)
  - ✅ 19 comprehensive tests (construction, validation, edge cases)
  - ✅ Example program: `examples/qc_ldpc_demo.rs`
- [ ] Irregular LDPC codes (degree distribution) - deferred
- [ ] DVB-T2 actual base matrices from ETSI EN 302 755 - Phase C10.2

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
**Long-term goal**: Verify polar codes are capacity-approaching via FER simulation

### Polar Code Construction
- Bit-channel reliability sorting using Bhattacharyya parameters
- Information/frozen bit selection based on channel reliability
- Rate-compatible polar code families
- Fast Hadamard-like transforms leveraging `BitMatrix`

### Polar Decoding
- Successive cancellation (SC) decoder
- SC-List (SCL) decoder with path metrics
- CRC-aided SCL for improved performance
- Efficient factor graph representation

### Capacity Verification
- FER simulation over AWGN channels (similar to LDPC framework)
- Performance comparison with Shannon limit
- Rate vs. Eb/N0 characterization
- Verification of capacity-approaching behavior for various block lengths
- Comparison with LDPC codes

### Integration
- Reuse AWGN channel and simulation framework from Phase C3
- Leverage soft-decision infrastructure (LLR operations)
- Evaluate integration points with rank/select primitives

## Phase C8: Compression Experiments (Exploratory)
- Bit-level transforms (run-length, delta, XOR chaining) using `BitVec` APIs
- Entropy modeling playground (simple adaptive frequency coder over GF(2) residuals)
- Comparative benchmarks: raw vs. transformed bitstreams

## Phase C9: BCH Codes ✅ **COMPLETE** 🎯 **DVB-T2 DEPENDENCY**
**Dependencies**: gf2-core Phase 8 (GF(2^m) arithmetic) ✅ **COMPLETE**

**Status**: ✅ Encoder and decoder complete (Phases 1-6 finished)

**Module**: `src/bch.rs` (536 lines)

### BCH Code Construction ✅ COMPLETE
- ✅ `BchCode` type with DVB-T2 parameter tables
- ✅ Generator polynomial construction from consecutive roots (α, α², ..., α^(2t))
- ✅ Factory methods: `BchCode::dvb_t2_normal(rate)`, `BchCode::dvb_t2_long(rate)`
- ✅ DVB-T2 standard parameters (t=12 error correction)
- ✅ Automatic field table generation
- ✅ 8 construction tests + 4 DVB-T2 parameter tests

### Systematic Encoding ✅ COMPLETE
- ✅ `BchEncoder` with polynomial division for parity computation
- ✅ Integration with `BlockEncoder` trait
- ✅ BitVec ↔ Polynomial conversion helpers
- ✅ Systematic form: c(x) = x^r·m(x) + remainder
- ✅ 6 encoding tests including codeword validation
- [ ] Encoding throughput benchmarks (future)

### Algebraic Decoding ✅ COMPLETE
- ✅ `BchDecoder` with syndrome computation (batch polynomial evaluation)
- ✅ Berlekamp-Massey algorithm for error locator polynomial
- ✅ Chien search for finding error positions
- ✅ Error correction and message extraction
- ✅ Integration with `HardDecisionDecoder` trait
- ✅ 5 syndrome tests + 4 Berlekamp-Massey tests + 4 Chien search tests

### Testing & Validation ✅ COMPLETE
- ✅ Code construction tests (8 tests)
- ✅ DVB-T2 parameter tests (4 tests)
- ✅ Systematic encoding tests (6 tests)
- ✅ Syndrome computation tests (5 tests)
- ✅ Berlekamp-Massey tests (4 tests)
- ✅ Chien search tests (4 tests)
- ✅ Decoder integration tests (5 tests)
- ✅ **Validation tests (10 tests)**
  - Known BCH codes (BCH(15,7,2), BCH(15,11,1))
  - Linearity property verification
  - Error correction limit testing
  - Systematic encoding validation
  - Codeword divisibility checks
  - DVB-T2 parameter validation
- ✅ Total: **45 BCH tests passing**
- ✅ Full encode/decode roundtrips working
- ✅ Error correction up to t=12 errors verified
- ✅ Generator polynomial roots validated
- ✅ Algebraic properties confirmed
- [ ] Known answer tests from DVB-T2 standard (future - requires reference vectors)
- [ ] Property tests with proptest (future)
- [ ] Decoding throughput benchmarks (future)

**Estimated effort**: 1-2 weeks → **Completed in 1 day (6 phases + validation)**
- [ ] Decoding throughput benchmarks

**Estimated effort**: 1-2 weeks

**Note**: Minimal polynomial computation is in gf2-core Phase 8.4. BCH-specific algorithms (generator polynomial from roots, Berlekamp-Massey, Chien search) belong here as application-level code.

## Phase C10: DVB-T2 FEC Simulation (In Progress) 🎯 **PRIMARY GOAL**
**Simulate complete DVB-T2 FEC chain with FER performance analysis**

### Phase C10.1: Quasi-Cyclic LDPC Framework ✅ **COMPLETE**
**Status**: Implemented with TDD (19 tests passing, 1 example)

**Deliverables**:
- ✅ `CirculantMatrix` type for circulant submatrices
- ✅ `QuasiCyclicLdpc` structure with base matrix and expansion factor  
- ✅ Automatic expansion to sparse edge list
- ✅ Generic design supporting multiple standards (DVB-T2, 5G NR, WiFi)
- ✅ Factory method placeholders: `dvb_t2_normal()`, `dvb_t2_long()`, `nr_5g()`
- ✅ Integration with existing `LdpcCode` via `from_quasi_cyclic()`
- ✅ Comprehensive tests: construction, validation, edge cases, panic conditions
- ✅ Example: `examples/qc_ldpc_demo.rs`
- ✅ Documentation with examples and design notes

**Test Coverage**: 13 QC-LDPC tests + 6 DVB-T2 tests = 19 tests
**Lines of Code**: ~250 lines in `src/ldpc.rs`
**Completion Time**: 1 day (TDD approach)

### Phase C10.2: DVB-T2 LDPC Base Matrices (Next)
**Priority**: HIGH  
**Estimated effort**: 3-5 days

**Tasks**:
- [ ] Enter base matrices from ETSI EN 302 755 Tables 6-8
- [ ] Implement all 12 standard configurations:
  - [ ] Normal frame (n=16200, Z=360): rates 1/2, 3/5, 2/3, 3/4, 4/5, 5/6
  - [ ] Long frame (n=64800, Z=360): same 6 rates
- [ ] Validation tests against standard
- [ ] Known answer tests from reference implementations
- [ ] Property tests for code structure
- [ ] Update documentation with actual parameters

### Components (Remaining)
- **QAM Modulation**: QPSK, 16/64/256-QAM with Gray mapping and soft LLR demapping
- **Bit Interleaving**: DVB-T2 column-row interleaver
- **System Integration**: End-to-end transmit/receive chain
- **FER Simulation**: Monte Carlo framework with configurable parameters

### Final Deliverables
- Complete DVB-T2 encoder/decoder for standard configurations
- FER vs. Eb/N0 simulation framework
- Performance comparison across code rates and modulations
- Validation against Shannon limit and reference implementations
- Comprehensive documentation and examples

**Total Estimated effort**: 6-9 weeks → **1 week complete**, 5-8 weeks remaining  
**See**: [docs/DVB_T2_DESIGN.md](docs/DVB_T2_DESIGN.md) for detailed implementation plan

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
