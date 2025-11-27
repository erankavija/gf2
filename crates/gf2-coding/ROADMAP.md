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
  - ✅ Factory methods with placeholders (`dvb_t2_short`, `dvb_t2_normal`, `nr_5g`)
  - ✅ 19 comprehensive tests (construction, validation, edge cases)
  - ✅ Example program: `examples/qc_ldpc_demo.rs`
- [ ] Irregular LDPC codes (degree distribution) - deferred
- ✅ DVB-T2 actual base matrices from ETSI EN 302 755 - Phase C10.2

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

## Phase C9: BCH Codes ⚠️ **COMPLETE (VERIFICATION PENDING)** 🎯 **DVB-T2 DEPENDENCY**
**Dependencies**: gf2-core Phase 8 (GF(2^m) arithmetic) ✅ **COMPLETE**

**Status**: ✅ Core implementation complete, ⚠️ DVB-T2 requires verification with reference test vectors

**Module**: `src/bch/` (~800 lines)

### BCH Code Construction ✅ COMPLETE
- ✅ `BchCode` type with DVB-T2 parameter tables
- ✅ Generator polynomial construction from consecutive roots (α, α², ..., α^(2t))
- ✅ Factory methods: `BchCode::dvb_t2(FrameSize::Short, rate)`, `BchCode::dvb_t2(FrameSize::Normal, rate)`
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

### DVB-T2 BCH Implementation ✅ COMPLETE
- ✅ DVB-T2 generator polynomials (12 polynomials each for Short/Normal frames)
- ✅ `poly_from_exponents()` utility for standard-provided generators
- ✅ `product_of_generators()` for computing g(x) = g_1(x) × ... × g_t(x)
- ✅ Factory method: `BchCode::dvb_t2(FrameSize, CodeRate)` for all 12 configurations
- ✅ Corrected parameters: n = k_ldpc (BCH output), k = Kbch (BCH input)
- ✅ Generator degree validation tests
- ✅ All frame sizes and code rates supported

### Testing & Validation ✅ COMPLETE
- ✅ Code construction tests (8 tests)
- ✅ DVB-T2 parameter tests (8 tests)
- ✅ DVB-T2 generator polynomial tests (10 tests)
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
  - Generator polynomial degree verification
- ✅ Total: **60+ BCH tests passing**
- ✅ Full encode/decode roundtrips working
- ✅ Error correction up to t=12 errors verified
- ✅ Generator polynomial roots validated
- ✅ Algebraic properties confirmed
- ⚠️ **DVB-T2 specific verification pending** - requires reference test vectors from standard or independent implementation
- [ ] Property tests with proptest (future)
- [ ] Decoding throughput benchmarks (future)

**Estimated effort**: 1-2 weeks → **Completed in 2 days (6 phases + DVB-T2 + validation)**

**Note**: DVB-T2 BCH implementation is complete but requires verification against:
1. Reference test vectors from ETSI EN 302 755 (if available)
2. Independent implementation (commercial DVB-T2 encoder/decoder)
3. Hardware test equipment output

Current tests verify mathematical correctness (polynomial properties, error correction capability, systematic encoding) but not DVB-T2 standard compliance.

**Note**: Minimal polynomial computation is in gf2-core Phase 8.4. BCH-specific algorithms (generator polynomial from roots, Berlekamp-Massey, Chien search) belong here as application-level code.

## Phase C10: DVB-T2 FEC Simulation 🔧 **IN PROGRESS**
**Simulate complete DVB-T2 FEC chain with FER performance analysis**

### Phase C10.0: Test Vector Parser & Infrastructure ✅ **COMPLETE**
**Status**: Implemented with TDD (21 tests: 14 unit + 7 integration)
**Completion Time**: 1 day

**Deliverables**:
- ✅ `TestVectorFile` parser for DVB binary string format
- ✅ `DvbConfig` parser for VV reference names (code rate extraction)
- ✅ `TestVectorSet` loader for multi-test-point configurations
- ✅ Environment variable support (`DVB_TEST_VECTORS_PATH`)
- ✅ Graceful test skipping with `require_test_vectors!()` macro
- ✅ Successfully parses VV001-CR35 (4 frames, 202 blocks/frame, TP04/05/06/07a)
- ✅ Validates structure: block counts, frame consistency, bit length progression
- ✅ 649 lines of code in `tests/test_vectors/`
- ✅ Full documentation and helper utilities

**Test Coverage**: 21 tests passing
- 8 parser unit tests (filename, markers, binary strings, error cases)
- 5 config parser tests (code rate mappings, error handling)
- 3 loader tests (consistency, structure validation)
- 5 integration tests (end-to-end parsing, bit length validation)

**Validation Results** (VV001-CR35):
- TP04 (BCH input): 38,688 bits/block
- TP05 (BCH output): 38,880 bits/block (+192 parity bits)
- TP06 (LDPC output): 64,800 bits/block (full LDPC codeword)
- All test points: 4 frames, 202 blocks/frame, consistent structure ✓

### Phase C10.0.1: BCH Verification & Bug Fix ✅ **COMPLETE**
**Status**: Bug fixed, 100% validation passing
**Completion Time**: 1 day (test infrastructure + bug fix)

**Deliverables**:
- ✅ 5 comprehensive BCH verification tests (280 lines)
- ✅ Full frame validation (202 blocks × 4 frames = 808 blocks)
- ✅ Error injection and correction testing (t=1 to t=12)
- ✅ Systematic property validation
- ✅ **Bug Fix**: Corrected polynomial-to-bit mapping for DVB-T2 convention
  - Issue: Bit position 0 must correspond to highest coefficient
  - Fix: Reversed bit-to-polynomial mapping in encoder/decoder
  - Result: **202/202 blocks match** DVB-T2 test vectors

**Verification Results**: ✅ 100% match with official ETSI EN 302 755 test vectors
- Encoding (TP04→TP05): 202/202 blocks pass
- Decoding (TP05→TP04): 202/202 blocks pass  
- Error correction: 100% success rate (t=1 to t=12)

See: `docs/DVB_T2_VERIFICATION_STATUS.md` and `docs/SYSTEMATIC_ENCODING_CONVENTION.md`

### Phase C10.1: Quasi-Cyclic LDPC Framework ✅ **COMPLETE**
**Status**: Implemented with TDD (19 tests passing, 1 example)

**Deliverables**:
- ✅ `CirculantMatrix` type for circulant submatrices
- ✅ `QuasiCyclicLdpc` structure with base matrix and expansion factor  
- ✅ Automatic expansion to sparse edge list
- ✅ Generic design supporting multiple standards (DVB-T2, 5G NR, WiFi)
- ✅ Factory method placeholders: `dvb_t2_short()`, `dvb_t2_normal()`, `nr_5g()`
- ✅ Integration with existing `LdpcCode` via `from_quasi_cyclic()`
- ✅ Comprehensive tests: construction, validation, edge cases, panic conditions
- ✅ Example: `examples/qc_ldpc_demo.rs`
- ✅ Documentation with examples and design notes

**Test Coverage**: 13 QC-LDPC tests + 6 DVB-T2 tests = 19 tests
**Lines of Code**: ~250 lines in `src/ldpc.rs`
**Completion Time**: 1 day (TDD approach)

### Phase C10.2: DVB-T2 LDPC Table Implementation ✅ **COMPLETE**
**Status**: Framework implemented via direct sparse construction (TDD)  
**Completion Time**: 1 day

**Deliverables**:
- ✅ Direct sparse matrix construction from DVB-T2 tables (bypasses QC)
- ✅ Table format parser with validation (row count, index range checks)
- ✅ Dual-diagonal parity structure implementation
- ✅ `DvbParams` with all 12 standard configurations
- ✅ Factory methods: `LdpcCode::dvb_t2_short()`, `LdpcCode::dvb_t2_normal()`
- ✅ NORMAL_RATE_1_2 table fully populated (90 rows from ETSI EN 302 755)
- ✅ 11 placeholder tables with clear TODOs for remaining configurations
- ✅ Comprehensive documentation (table format, dual-diagonal structure)
- ✅ Example: `examples/dvb_t2_ldpc_basic.rs`
- ✅ gf2-core requirements document for duplicate edge handling

**Test Coverage**: 18 unit tests + 13 integration tests = 31 tests  
**Code Structure**: `params.rs`, `builder.rs`, `dvb_t2_matrices.rs` (modular design)

**Note**: DVB-T2 matrices are NOT pure quasi-cyclic (multiple circulants per block).
Direct sparse construction used instead of QC expansion.

**Remaining Work**:
- [ ] Add 11 remaining table data files from ETSI EN 302 755
- ⚠️ **Validation with reference test vectors** (same issue as BCH - requires independent verification)
- [ ] Verify gf2-core handles duplicate edges correctly

### Phase C10.3: DVB-T2 BCH Outer Code Integration ✅ **COMPLETE (VERIFICATION PENDING)**
**Status**: Implementation complete, requires independent verification
**Completion Time**: 2 days

**Deliverables**:
- ✅ DVB-T2-specific BCH parameter tables (Short/Normal frames, all code rates)
- ✅ Generator polynomial construction from ETSI EN 302 755 explicit tables
- ✅ Factory method: `BchCode::dvb_t2(FrameSize, CodeRate)`
- ✅ Systematic encoding with exact BCH parity bit counts (168, 192, 160)
- ✅ Algebraic decoding (Berlekamp-Massey + Chien search)
- ✅ Integration tests: encode/decode roundtrips for all configurations
- ✅ Parameter validation: generator degree = n - k

**Test Coverage**: 27 DVB-T2 BCH tests + 33 core BCH tests = 60 total tests
**Code Structure**: `bch/dvb_t2/` module with `params.rs`, `generators.rs`, `mod.rs`

**Verification Status**: ⚠️
- ✅ Mathematical correctness verified (polynomial properties, error correction)
- ⚠️ DVB-T2 standard compliance unverified (no reference test vectors available)
- **Requires**: Test vectors from ETSI standard or independent DVB-T2 implementation

### Phase C10.4: LDPC Validation Suite ✅ **COMPLETE**
**Status**: Implemented with TDD (19 tests passing)
**Completion Time**: 1 day

**Deliverables**:
- ✅ Code construction validation (5 tests)
  - Zero codeword validity
  - Syndrome dimensions
  - Parameter relationships (n, m, k, rate)
  - Matrix dimensions via syndrome
- ✅ Mathematical property tests (4 tests)
  - Syndrome linearity: H·(c₁⊕c₂) = H·c₁ ⊕ H·c₂
  - Valid codeword zero syndrome
  - Syndrome detects errors
  - Codeword closure under XOR (linear code property)
- ✅ DVB-T2 parameter validation (3 tests)
  - Normal frame parameters (6 rates)
  - Short frame parameters (6 rates)
  - Standard codeword lengths
- ✅ Decoder validation (4 tests)
  - Decoder initialization
  - All-zero channel decoding
  - Convergence tracking
  - Valid codeword production
- ✅ Edge case validation (2 tests)
  - Various syndrome patterns
  - Validity check consistency
- ✅ from_edges construction tests (2 tests)

**Test Coverage**: 19 tests, all passing
**Validation Approach**: Follows BCH validation pattern with [message | parity] convention
**Lines of Code**: ~460 lines in `tests/ldpc_validation.rs`

**Verified Properties**:
- Linearity of syndrome computation
- Zero syndrome ↔ valid codeword
- Closure property (c₁ ⊕ c₂ is valid if c₁, c₂ are valid)
- DVB-T2 standard parameter compliance (all 12 configurations)
- Decoder convergence and validity

**Note**: Systematic encoding validation deferred - DVB-T2 LDPC codes require specialized systematic encoder (not yet implemented). Current tests focus on parity-check matrix properties and hard-decision decoding.

### Phase C10.5: LDPC Systematic Encoding ✅ **DEFERRED TO C10.6**
**Status**: Superseded by Richardson-Urbanke implementation in Phase C10.6

### Phase C10.6: LDPC Encoding/Decoding with DVB-T2 Validation 🔧 **IN PROGRESS**
**Goal**: Complete LDPC implementation with proper systematic encoding and test vector validation

**Prerequisites**: ✅ **gf2-core Phase 12 (File I/O) complete** + ✅ **Dense BitMatrix matvec_transpose**

#### Subphases Completed:

**C10.6.1: Richardson-Urbanke Core Algorithm** ✅ **COMPLETE**
- Richardson-Urbanke preprocessing with RREF+SIMD (0.2-2s per Short frame)
- Generator matrix computation with row reordering
- 6 unit tests passing

**C10.6.2: Dense Matrix Optimization** ✅ **COMPLETE** (2025-11-26)
- Dense `BitMatrix` storage for 40-50% dense DVB-T2 parity matrices
- All 12 DVB-T2 configs cached: 529 MB total (2.1× reduction vs sparse)
- Cache loading: 16ms for all configs
- `BitMatrix::matvec_transpose()` for fast encoding
- `generate_ldpc_cache` binary in `src/bin/`

**Performance**:
- Preprocessing: 0.2-1.6s per Short frame, up to 6min per Normal frame (RREF+SIMD)
- Total cache generation: 13 minutes for all 12 configs
- File sizes: 5-8 MB (Short), 70-126 MB (Normal) per config

#### Remaining Work:

**C10.6.5: DVB-T2 Test Vector Validation** (Next)
   - TP05 → TP06 encoding validation (202 blocks)
   - TP06 → TP05 decoding validation (error-free)
   - TP06 + errors → TP05 decoding (error correction)
   - Target: 100% match with reference vectors

**C10.6.6: Performance Benchmarks**
   - Encoding throughput: Target >10 Mbps (baseline: 50+ Mbps)
   - Decoding throughput: Target >5 Mbps (baseline: 20+ Mbps)
   - Memory usage profiling
   - Comparison with BCH outer code integration

**Estimated Remaining Effort**: 2-3 days

**Success Criteria**:
- ✅ Generator matrices cached (all 12 configs)
- ✅ Fast cache loading (<20ms)
- ✅ Dense storage (2.1× reduction vs sparse)
- [ ] All DVB-T2 test vectors pass (TP05↔TP06)
- [ ] Encode/decode roundtrips verified
- [ ] Integration with BCH outer code working

### Phase C10.7: Full DVB-T2 FEC Chain (After LDPC complete)
**Components**:
- **QAM Modulation**: QPSK, 16/64/256-QAM with Gray mapping and soft LLR demapping
- **Bit Interleaving**: DVB-T2 column-row interleaver
- **System Integration**: BCH + LDPC + QAM + Interleaving
- **FER Simulation**: Monte Carlo framework with configurable parameters
- **Full Test Vector Validation**: TP04 → TP05 → TP06 → TP07a complete chain

### Final Deliverables
- Complete DVB-T2 encoder/decoder for standard configurations
- FER vs. Eb/N0 simulation framework
- Performance comparison across code rates and modulations
- Validation against Shannon limit and reference implementations
- Comprehensive documentation and examples

**Total Estimated effort**: 6-9 weeks → **C10.6.1-2 complete**, test vector validation remaining  
**See**: [docs/DVB_T2_DESIGN.md](docs/DVB_T2_DESIGN.md) for detailed implementation plan

## Phase C11: Performance & Ergonomics Polish (Ongoing)
- Unified error handling and panic messages → shift towards `Result` where appropriate
- Trait refinements: streaming vs. batch encode/decode unification
- Doc examples with visual syndrome / decoding traces

## Phase C12: SDR and DSP Framework Integration (Planned)
**Goal**: Interface with GNU Radio, LuaRadio, and other SDR ecosystems

See [docs/SDR_INTEGRATION.md](docs/SDR_INTEGRATION.md) for comprehensive design.

### Phase C12.1: C FFI Layer (1-2 weeks)
- [ ] Create `src/ffi.rs` module with C-compatible API
- [ ] Expose LDPC decoder (create, decode, destroy functions)
- [ ] Expose BCH decoder
- [ ] Expose Viterbi decoder
- [ ] Safety wrappers and error handling
- [ ] C header file generation (`gf2_coding.h`)
- [ ] Standalone C test program

### Phase C12.2: GNU Radio OOT Module (2-3 weeks)
- [ ] Initialize `gr-gf2` with `gr_modtool`
- [ ] Implement `dvb_t2_ldpc_decoder` block
- [ ] Implement `dvb_t2_bch_decoder` block
- [ ] Implement `viterbi_decoder` block
- [ ] GRC block definitions for visual programming
- [ ] Example flowgraphs (DVB-T2 receiver chain)
- [ ] Integration tests with simulated IQ data
- [ ] Installation and usage documentation

### Phase C12.3: Real-World Validation (2-4 weeks)
- [ ] Test with DVB-T2 conformance test vectors
- [ ] Benchmark vs GNU Radio's existing FEC blocks (target: 10-50x speedup)
- [ ] Validate with RTL-SDR/HackRF captured signals
- [ ] Generate BER/FER comparison curves

### Phase C12.4: Extended SDR Support (Ongoing)
- [ ] LuaRadio FFI blocks
- [ ] SDRangel plugin
- [ ] gr-satellites contributions (Viterbi/BCH for telemetry)
- [ ] Python bindings via PyO3 (optional)

**Key deliverables**:
- High-performance FEC blocks for GNU Radio (10-50 Mbps LDPC decoding)
- C API exposing LDPC, BCH, and Viterbi decoders
- Example receiver chains for DVB-T2 and satellite telemetry
- Performance benchmarks demonstrating 10-50x speedup over existing implementations

## Technical Debt & Refactoring
- [ ] **Move `poly_from_exponents` to gf2-core**: Currently in `bch::dvb_t2::generators`, this utility for constructing `Gf2mPoly` from exponent lists should be a general method in `gf2_core::gf2m::Gf2mPoly` (e.g., `Gf2mPoly::from_exponents()`)
- [ ] **Consolidate doctests for expensive operations**: Currently 6 LDPC encoding doctests are marked `no_run` because they take 2-10 seconds each. These should be consolidated into a single comprehensive example or moved to integration tests to enable proper doctest validation.

## Research Placeholders / Open Questions
- Optimal data structures for extremely sparse parity-check matrices?
- When to switch from table-based to algebraic decoding for medium block lengths?
- Feasibility of GPU offload (via future crates) for LDPC iterations?
- Interplay between compression transforms and error correction ordering
- SDR integration: float vs fixed-point LLRs for optimal performance/accuracy tradeoff?

## gf2-core Integration

**Phase 12 File I/O**: ✅ **COMPLETE** - Integrated into LDPC cache
- Dense BitMatrix I/O used for LDPC parity matrices
- 529 MB total cache for all 12 DVB-T2 configs
- Cache loads in 16ms

## Principles
- Keep experimental algorithms isolated—avoid regressing core performance
- Favor clarity & correctness first; optimize after baseline metrics exist
- Rich documentation & visual aids for educational usability

Refer back to `crates/gf2-core/ROADMAP.md` for low-level performance phases.
