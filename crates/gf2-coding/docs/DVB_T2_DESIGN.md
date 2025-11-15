# DVB-T2 FEC Simulation - Design Document

**Status**: Planned (Phases C9-C10)  
**Priority**: HIGH - Primary ambitious goal  
**Estimated Effort**: 8-11 weeks total

## Goal

Simulate Frame Error Rate (FER) vs. Eb/N0 for DVB-T2 FEC chain with BCH outer code, LDPC inner code, bit interleaving, and bit-interleaved coded modulation (BICM) over AWGN channel.

## System Architecture

```
Transmit Chain:
  Data → BCH Encode → LDPC Encode → Bit Interleave → QAM Map → AWGN Channel

Receive Chain:
  Noisy Symbols → LLR Demap → Bit Deinterleave → LDPC Decode → BCH Decode → Data
```

## DVB-T2 FEC Specification

### LDPC Codes
- **Type**: Irregular quasi-cyclic LDPC
- **Frame sizes**: Normal (16200 bits) or Long (64800 bits)
- **Code rates**: 1/2, 3/5, 2/3, 3/4, 4/5, 5/6
- **Structure**: Parity-check matrix composed of circulant submatrices
- **Construction**: Base matrix + expansion factor per ETSI EN 302 755

### BCH Codes
- **Purpose**: Outer code to reduce LDPC error floor
- **Type**: Shortened BCH over GF(2^m)
- **Parameters**: Depend on LDPC code rate
  - Normal frame: BCH(16200, Kbch) with t = 12 or 10 error correction
  - Long frame: BCH(64800, Kbch) with t = 12 or 10 error correction

### Bit-Interleaved Coded Modulation (BICM)
- **Interleaver**: Column-row structure per DVB-T2 spec
- **Modulation**: QPSK, 16-QAM, 64-QAM, 256-QAM
- **Mapping**: Gray-coded constellations
- **Soft Decoding**: LLR computation via max-log approximation

## Implementation Phases

### Phase C9: BCH Code Implementation (1-2 weeks)
**Dependencies**: gf2-core Phase 8 (GF(2^m) arithmetic) ✅ **UNBLOCKED** - Phases 1-3 complete

**New module: `src/bch.rs`**

#### Components
```rust
pub struct BchCode {
    n: usize,              // Codeword length
    k: usize,              // Message length
    t: usize,              // Error correction capability
    field: Gf2mField,      // Extension field GF(2^m)
    generator: Gf2mPoly,   // Generator polynomial g(x)
}

pub struct BchEncoder {
    code: BchCode,
}

pub struct BchDecoder {
    code: BchCode,
}
```

#### Key Algorithms
1. **Encoding**: Systematic encoding via polynomial division
2. **Syndrome Computation**: Evaluate received polynomial at roots
3. **Berlekamp-Massey**: Find error locator polynomial from syndromes
4. **Chien Search**: Find error positions
5. **Error Correction**: Apply corrections

#### DVB-T2 Specific
```rust
impl BchCode {
    pub fn dvb_t2_normal(rate: CodeRate) -> Self;
    pub fn dvb_t2_long(rate: CodeRate) -> Self;
}
```

#### Testing
- Encode/decode roundtrips (no errors)
- Error correction up to t errors
- Known answer tests from DVB-T2 standard
- Property tests: linearity, error bounds

**Deliverables**:
- [ ] BCH encoding/decoding
- [ ] DVB-T2 parameter tables
- [ ] Integration with `BlockEncoder`/`HardDecisionDecoder` traits
- [ ] Comprehensive tests
- [ ] Documentation with examples

---

### Phase C10: DVB-T2 LDPC Construction (1 week)
**Dependencies**: Current LDPC implementation

**Extension to: `src/ldpc.rs`**

#### Quasi-Cyclic Matrix Construction
```rust
pub struct QuasiCyclicLdpc {
    base_matrix: Vec<Vec<i32>>,  // -1 = no edge, ≥0 = shift value
    expansion_factor: usize,      // Circulant block size
}

impl LdpcCode {
    pub fn from_quasi_cyclic(qc: &QuasiCyclicLdpc) -> Self;
    pub fn dvb_t2_normal(rate: CodeRate) -> Self;
    pub fn dvb_t2_long(rate: CodeRate) -> Self;
}

pub enum CodeRate {
    Rate1_2, Rate3_5, Rate2_3, Rate3_4, Rate4_5, Rate5_6
}
```

#### Data Entry
- Base matrices from ETSI EN 302 755 Tables 6-8
- Expansion factors: 360 (normal), 1440 (long)
- All 12 configurations (2 frame sizes × 6 code rates)

#### Testing
- Verify matrix dimensions and structure
- Check row/column weights match specification
- Test syndrome computation on valid codewords
- Validate against reference implementation if available

**Deliverables**:
- [ ] DVB-T2 LDPC parity-check matrices
- [ ] Factory functions for standard configurations
- [ ] Validation tests
- [ ] Documentation referencing standard

---

### Phase C11: QAM Modulation (1 week)
**Extension to: `src/channel.rs`**

#### QAM Modulator
```rust
pub enum Modulation {
    QPSK,   // 2 bits/symbol
    QAM16,  // 4 bits/symbol
    QAM64,  // 6 bits/symbol
    QAM256, // 8 bits/symbol
}

pub struct QamModulator {
    modulation: Modulation,
    constellation: Vec<Complex<f64>>,  // Gray-mapped points
    avg_power: f64,                     // For normalization
}

impl QamModulator {
    pub fn new(modulation: Modulation) -> Self;
    pub fn modulate(&self, bits: &BitVec) -> Vec<Complex<f64>>;
    pub fn bits_per_symbol(&self) -> usize;
}
```

#### QAM Demodulator with LLR Calculation
```rust
pub struct QamDemodulator {
    modulation: Modulation,
    noise_variance: f64,
}

impl QamDemodulator {
    pub fn demodulate_soft(&self, symbols: &[Complex<f64>]) -> Vec<Llr>;
}
```

#### LLR Computation (Max-Log Approximation)
For each bit position i in symbol:
```
LLR_i = (min{||r - s||² : s with bit_i=0} - min{||r - s||² : s with bit_i=1}) / N0
```

#### Gray Mapping
- Minimize bit errors for adjacent constellation points
- Standard mappings per DVB-T2 specification

#### Testing
- Constellation properties (average power, minimum distance)
- Gray mapping verification
- LLR accuracy under various SNR
- Hard decision matches direct demapping

**Deliverables**:
- [ ] QAM modulation with Gray mapping
- [ ] Soft LLR demapping
- [ ] Tests and benchmarks
- [ ] Documentation

---

### Phase C12: Bit Interleaving (3-4 days)
**New module: `src/interleaver.rs`**

#### Generic Interleaver
```rust
pub struct BitInterleaver {
    permutation: Vec<usize>,        // Forward permutation
    inverse_permutation: Vec<usize>, // Cached inverse
}

impl BitInterleaver {
    pub fn new(permutation: Vec<usize>) -> Self;
    pub fn interleave(&self, bits: &BitVec) -> BitVec;
    pub fn deinterleave(&self, bits: &BitVec) -> BitVec;
    pub fn deinterleave_llrs(&self, llrs: &[Llr]) -> Vec<Llr>;
}
```

#### DVB-T2 Bit Interleaver
Column-row structure:
1. Write bits column-wise into Nr × Nc matrix
2. Read row-wise with column permutation
3. Parameters depend on modulation and FEC frame length

```rust
impl BitInterleaver {
    pub fn dvb_t2(modulation: Modulation, fec_length: usize) -> Self;
}
```

#### Testing
- Roundtrip property: deinterleave(interleave(x)) = x
- Permutation validity (bijection)
- LLR deinterleaving preserves values

**Deliverables**:
- [ ] Bit interleaver implementation
- [ ] DVB-T2 specific construction
- [ ] Tests
- [ ] Documentation

---

### Phase C13: System Integration (3-4 days)
**New module: `src/dvb_t2.rs`**

#### Transmitter
```rust
pub struct DvbT2Encoder {
    bch: BchEncoder,
    ldpc: LdpcCode,
    interleaver: BitInterleaver,
    modulator: QamModulator,
}

impl DvbT2Encoder {
    pub fn new(config: DvbT2Config) -> Self;
    pub fn encode(&self, data: &BitVec) -> Vec<Complex<f64>>;
}
```

#### Receiver
```rust
pub struct DvbT2Decoder {
    demodulator: QamDemodulator,
    interleaver: BitInterleaver,
    ldpc_decoder: LdpcDecoder,
    bch_decoder: BchDecoder,
}

impl DvbT2Decoder {
    pub fn new(config: DvbT2Config, noise_variance: f64) -> Self;
    pub fn decode(&mut self, symbols: &[Complex<f64>]) -> Result<BitVec, DecodeError>;
}
```

#### Configuration
```rust
pub struct DvbT2Config {
    pub frame_size: FrameSize,      // Normal or Long
    pub code_rate: CodeRate,        // 1/2, 3/5, 2/3, 3/4, 4/5, 5/6
    pub modulation: Modulation,     // QPSK, 16/64/256-QAM
}

pub enum FrameSize {
    Normal,  // 16200 bits
    Long,    // 64800 bits
}
```

#### Error Handling
```rust
pub enum DecodeError {
    LdpcFailed { iterations: usize },
    BchFailed { uncorrectable_errors: usize },
}
```

#### Testing
- End-to-end encode/decode under no noise
- Verify data integrity
- Check proper error propagation
- Test all standard configurations

**Deliverables**:
- [ ] Complete DVB-T2 chain
- [ ] Configuration presets
- [ ] Error handling
- [ ] Integration tests

---

### Phase C14: FER Simulation Framework (1 week)
**New example: `examples/dvb_t2_fer.rs`**

#### Monte Carlo Simulator
```rust
pub struct FerSimulator {
    config: DvbT2Config,
    encoder: DvbT2Encoder,
    channel: AwgnChannel,
}

pub struct SimConfig {
    pub dvb_t2: DvbT2Config,
    pub eb_n0_range: Vec<f64>,
    pub frames_per_point: usize,
    pub max_frame_errors: usize,
    pub max_ldpc_iterations: usize,
}

pub struct SimResults {
    pub eb_n0_db: f64,
    pub fer: f64,
    pub ber: f64,
    pub avg_iterations: f64,
    pub frames_tested: usize,
}
```

#### Early Stopping
- Stop when max_frame_errors reached
- Minimum confidence interval
- Time-based limits

#### Parallelization
- Optional rayon support for multi-threaded simulation
- Independent frame processing
- Thread-safe RNG

#### Output Formats
- CSV for plotting: `eb_n0_db,fer,ber,avg_iter,frames`
- ASCII table summary
- JSON for programmatic access

#### Visualization
```
DVB-T2 FER Simulation
Config: Normal frame, Rate 2/3, 64-QAM

┌──────────┬──────────┬──────────┬──────────┬──────────┐
│ Eb/N0 dB │   FER    │   BER    │ Avg Iter │  Frames  │
├──────────┼──────────┼──────────┼──────────┼──────────┤
│   6.0    │ 0.8500   │ 0.0156   │   48.3   │   118    │
│   7.0    │ 0.3200   │ 0.0045   │   35.2   │   313    │
│   8.0    │ 0.0520   │ 0.0008   │   22.1   │  1000    │
└──────────┴──────────┴──────────┴──────────┴──────────┘

Shannon limit (R=2/3): 0.55 dB
Gap to Shannon at FER=0.01: ~7.5 dB

Results: dvb_t2_normal_rate2_3_64qam.csv
```

**Deliverables**:
- [ ] Full FER simulation
- [ ] Configurable parameters
- [ ] Multiple output formats
- [ ] Performance comparison utilities
- [ ] Comprehensive documentation

---

### Phase C15: Validation & Optimization (1-2 weeks)

#### Validation
- Compare with reference implementations (GNU Radio, others)
- Verify waterfall region matches theory
- Check error floor behavior
- Cross-validate different configurations

#### Performance Optimization
- LDPC decoder: Add normalized min-sum with damping factor
- LDPC decoder: Improved early termination
- LLR computation: SIMD opportunities
- Parallel frame processing
- Memory allocation optimization

#### Documentation
- System overview with block diagrams
- Mathematical foundations (brief)
- Usage guide with examples
- Performance characteristics
- Troubleshooting guide

**Deliverables**:
- [ ] Validation results
- [ ] Performance benchmarks
- [ ] Optimization report
- [ ] Complete documentation
- [ ] Example configurations

---

## Timeline Summary

| Phase | Component | Effort | Dependencies |
|-------|-----------|--------|--------------|
| C9  | BCH codes | 1-2 weeks | gf2-core Phase 8 ⚠️ |
| C10 | DVB-T2 LDPC | 1 week | Current LDPC |
| C11 | QAM modulation | 1 week | Current channel |
| C12 | Bit interleaving | 3-4 days | None |
| C13 | Integration | 3-4 days | C9-C12 |
| C14 | FER simulation | 1 week | C13 |
| C15 | Validation | 1-2 weeks | C14 |

**Total**: 8-11 weeks (includes gf2-core dependency)

---

## Success Criteria

✅ **Functionality**:
- Encode/decode DVB-T2 frames for all standard configurations
- Generate FER curves showing waterfall and error floor
- Match expected Shannon-limit proximity

✅ **Quality**:
- Comprehensive tests (unit, integration, property-based)
- TDD development throughout
- Clear documentation
- Performance benchmarks

✅ **Scientific Value**:
- Demonstrate coding gain vs. uncoded
- Show BCH impact on LDPC error floor
- Compare code rates and modulations
- Validate against theory

---

## Future Extensions

- DVB-S2/S2X standards
- Fading channel models (Rayleigh, Rician)
- Multi-threaded decoder implementations
- Fixed-point LLR quantization
- Rate matching and puncturing
- HARQ simulation
- GPU acceleration exploration

---

## References

- ETSI EN 302 755 v1.4.1 (DVB-T2 standard)
- *Error Control Coding* by Lin & Costello
- DVB Project specifications: https://dvb.org/
- GNU Radio DVB-T2 implementation (for reference)
