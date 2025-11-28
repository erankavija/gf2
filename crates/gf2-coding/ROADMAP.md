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

## Phase C3: Soft-Decision & AWGN Channel ✅ **COMPLETE**

- ✅ LLR operations with box-plus and min-sum variants
- ✅ BPSK modulation and AWGN noise generation
- ✅ Channel simulation (Eb/N0 → noise variance)
- ✅ BER/FER curve generation with Shannon limit comparison
- ✅ Monte Carlo framework with CSV export
- [ ] Quantization strategies (fixed-point LLRs) - deferred

## Phase C4: Advanced Decoding Algorithms (Planned)
- Syndrome table optimization (compressed mapping)
- Berlekamp–Massey for BCH-like codes (depends on polynomial arithmetic)
- Chien search integration for root finding in GF(2^m)
- Soft-input Viterbi Algorithm (SOVA) for convolutional codes

## Phase C5: LDPC Codes ✅ **COMPLETE**

- ✅ Sparse parity-check matrix format (SparseMatrixDual from gf2-core)
- ✅ Regular/quasi-cyclic LDPC code generation
- ✅ DVB-T2 tables from ETSI EN 302 755 (all 12 configs)
- ✅ Min-sum belief propagation decoder
- ✅ Richardson-Urbanke systematic encoding (cached generators)
- ✅ BER/FER simulation over AWGN (example: ldpc_awgn.rs)
- [ ] Irregular LDPC codes (degree distribution) - deferred
- [ ] Normalized/offset min-sum - optimization phase

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

## Phase C9: BCH Codes ✅ **COMPLETE**
**Status**: Core implementation + DVB-T2 verification complete (60+ tests, 100% match with ETSI test vectors)

- ✅ Systematic encoding/decoding with Berlekamp-Massey + Chien search
- ✅ DVB-T2 factory methods for all 12 configurations (t=12 error correction)
- ✅ **DVB-T2 test vector validation**: 202/202 blocks match official ETSI EN 302 755 vectors
- ✅ Error correction verified up to t=12 errors
- [ ] Throughput benchmarks (future)

## Phase C10: DVB-T2 FEC Simulation 🔧 **IN PROGRESS**

### Completed Phases ✅

**C10.0-0.1: Test Vector Infrastructure & BCH Verification** (2 days)
- ✅ Test vector parser (21 tests, 649 lines)
- ✅ BCH validation: **202/202 blocks match ETSI vectors** (5 tests, 808 blocks verified)
- ✅ Error correction verified (t=1 to t=12, 100% success)

**C10.1-10.4: LDPC Framework** (3 days)
- ✅ Quasi-cyclic LDPC framework (19 tests)
- ✅ DVB-T2 table implementation (31 tests, all 12 configs)
- ✅ Mathematical property validation (19 tests)
- [ ] Remaining: 11 DVB-T2 table data files (only NORMAL_RATE_1_2 populated)

**C10.6.1-10.6.4: LDPC Systematic Encoding** (3 days) ✅ **COMPLETE**
- ✅ Richardson-Urbanke algorithm with RREF+SIMD (0.2-6min preprocessing)
- ✅ Dense matrix cache: 529 MB for all 12 configs (2.1× reduction, 16ms load)
- ✅ `generate_ldpc_cache` binary for cache generation
- ✅ **Bug fix**: RREF right-pivoting with correct row reordering (gf2-core)
- ✅ Property tests: H × G^T = 0 verified for all configurations

**C10.6.5: DVB-T2 LDPC Test Vector Validation** ✅ **COMPLETE**
- ✅ Comprehensive test suite: 8 tests (encoding, decoding, error correction, properties)
- ✅ Test infrastructure: cache support, LLR conversion, multi-frame validation
- ✅ RREF bug discovered and fixed via diagnostic tests
- ✅ Mathematical correctness verified: H × G^T = 0 for all basis vectors
- ✅ All LDPC unit tests passing (40 tests)
- ⚠️ **Full test vector validation pending** - requires running ignored tests with test vectors

### Current Work 🔧

**C10.6.6: Test Vector Validation** ✅ **COMPLETE** (2025-11-27)
- ✅ **Encoding validated**: TP05 → TP06 **202/202 blocks match** (12.4s, 0.63 Mbps)
- ✅ **Decoding validated**: TP06 → TP05 **202/202 blocks match** (5.8s, 1.35 Mbps)
- ✅ **Systematic property**: Verified (first k bits match message)
- ✅ **Parity check**: H·c = 0 verified for test vectors
- ✅ **Roundtrip**: Encode → decode → message verified
- 🐛 **Bug fixed**: Decoder was returning full codeword instead of extracting message bits

**C10.6.7: Performance Benchmarks & Baseline** ✅ **COMPLETE** (2025-11-27)
- ✅ Criterion benchmark suite created (`benches/ldpc_throughput.rs`)
- ✅ Baseline measured: **3.85 Mbps encoding** (9.87 ms/block)
- ✅ Key finding: No batching optimization (constant throughput across batch sizes)
- ✅ Performance plan documented (`docs/LDPC_PERFORMANCE_PLAN.md`)
- **Gap**: 2.6-13× slower than 10-50 Mbps target
- **Next**: CPU profiling to identify hotspots

### Current Work 🎯

**C10.6.8: Performance Optimization** (1-2 weeks) - **REAL-TIME TARGET**

**Real-Time Requirement**: DVB-T2 8 MHz mode requires **31.4 Mbps encoding / 50 Mbps decoding**
- Current: 3.85 Mbps encoding (8.2× too slow), 8.29 Mbps decoding (6.0× too slow)
- Parallel batch decoding: **6.7× speedup achieved** (24-core CPU)

**Profiling Complete** ✅ (2025-11-27):
- ✅ Encoding: 97.5% in `BitMatrix::matvec_transpose` (gf2-core)
- ✅ Decoding: 69.8% BP loop + 17.7% sparse iteration + 4.9% malloc
- ✅ SIMD enabled: 178 SIMD instructions found in binary
- See: [docs/LDPC_PROFILING_RESULTS.md](docs/LDPC_PROFILING_RESULTS.md)

**Week 1 Goal**: 10-20 Mbps (Software Recording) ✅ PARTIAL
- ✅ Pre-allocate decoder state (buffers already pre-allocated)
- ✅ Implement batch processing (sequential for encoder, parallel for decoder)
- ✅ Add block-level parallelism (6.7× speedup with rayon on 24-core)
- ✅ Achieved: 8.29 Mbps decoding (16% real-time, batch of 202)
- ⚠️ Encoder: 3.85 Mbps (sequential, Sync bounds need resolution)

**Week 2 Goal**: 50-100 Mbps (Live Reception)
- [ ] SIMD vectorization for LLRs (4-8× speedup, 1-2 days)
- [ ] Optimize sparse iteration (2× speedup, 1-2 days)
- [ ] Profile BP loop internals (1 day)
- [ ] Target: 100-200% real-time (live DVB-T2 on PC)

**See**: [docs/OPTIMIZATION_ACTION_PLAN.md](docs/OPTIMIZATION_ACTION_PLAN.md) for detailed plan

### Future Phases

**C10.7: Full FEC Chain** (2-3 weeks, after real-time performance achieved)
- [ ] QAM modulation (QPSK, 16/64/256-QAM)
- [ ] Bit interleaving (DVB-T2 column-row)
- [ ] System integration (BCH + LDPC + QAM)
- [ ] FER simulation over AWGN
- [ ] Full TP04 → TP05 → TP06 → TP07a validation
- [ ] Live DVB-T2 reception demo with SDR hardware

**See**: [docs/DVB_T2_DESIGN.md](docs/DVB_T2_DESIGN.md) and [docs/DVB_T2_VERIFICATION_STATUS.md](docs/DVB_T2_VERIFICATION_STATUS.md)

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
