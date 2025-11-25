# LDPC Systematic Encoding Implementation Plan

## Phase C10.6: LDPC Encoding/Decoding with DVB-T2 Validation

**Status**: Next priority after C10.5 analysis  
**Date**: 2025-11-25  
**Estimated Effort**: 2-3 days

---

## Executive Summary

Complete LDPC systematic encoding using **Richardson-Urbanke (RU)** algorithm and validate against DVB-T2 test vectors. This unblocks full DVB-T2 FEC chain implementation.

### Current State
- ✅ LDPC decoder working (belief propagation)
- ✅ Parity-check matrices constructed for all DVB-T2 configurations
- ❌ Systematic encoder blocked (two failed approaches)
- ❌ Test vector validation failing (0/10 blocks match)

### Blockers Resolved
1. **Encoding algorithm**: Implement Richardson-Urbanke
2. **Performance**: Preprocessing + cached matrices
3. **DVB-T2 compliance**: Test vector validation

---

## Technical Approach

### 1. Richardson-Urbanke Algorithm

#### Overview
Generic LDPC systematic encoding via matrix preprocessing:
- **One-time preprocessing** per code configuration (expensive but cached)
- **Fast encoding** using precomputed matrices (O(edges))

#### Algorithm Steps

**Preprocessing (once per DVB-T2 configuration):**

```rust
// Input: Parity-check matrix H (m × n)
// Output: Encoding matrices (φ, ψ) and permutation π

1. Apply Gaussian elimination to H to get systematic form:
   
   H' = [A  B  T]  (top m-g rows)
        [C  D  E]  (bottom g rows)
   
   Where:
   - T is lower triangular (easy to invert)
   - E is invertible (small, can invert)
   - g = gap parameter (typically small)

2. Compute encoding matrices:
   φ = E^(-1)  (g × g matrix)
   ψ = T^(-1)  (lower triangular, easy)

3. Store (φ, ψ, π, A, B, C, T, E) for this code
```

**Encoding (fast, repeated):**

```rust
// Input: message m (length k)
// Output: systematic codeword c = [m | p] (length n)

1. p₁ = -φ · (C · m)          // First parity segment
2. p₂ = -ψ · (A·m + B·p₁)     // Second parity segment  
3. c = π^(-1) · [m | p₁ | p₂] // Permute back
```

**Complexity:**
- Preprocessing: O(n³) worst case, but H is sparse → O(n·m·density)
- Encoding: O(k·w) where w is average column weight (~constant)
- Memory: Store ~O(n) for encoding matrices (sparse)

#### DVB-T2 Optimization

DVB-T2 LDPC codes have **dual-diagonal parity structure**. We can exploit this:

1. **Partial preprocessing**: The parity part already has structure
2. **Simplified T**: Dual-diagonal → very easy to invert
3. **Sparse φ, ψ**: Keep in sparse format

**Alternative**: Since DVB-T2 is quasi-cyclic, could use **block-circulant** structure for even faster encoding, but RU is more general and sufficient.

---

### 2. Implementation Plan

#### Phase 1: Core RU Algorithm (Day 1 morning)

**File**: `src/ldpc/encoding/richardson_urbanke.rs`

```rust
pub struct RuEncodingMatrices {
    phi: SparseBitMatrix,      // E^(-1)
    psi: SparseBitMatrix,      // T^(-1) 
    a: SparseBitMatrix,
    b: SparseBitMatrix,
    c: SparseBitMatrix,
    permutation: Vec<usize>,    // Column permutation
    k: usize,
    n: usize,
}

impl RuEncodingMatrices {
    /// Preprocess parity-check matrix H for fast encoding
    pub fn preprocess(h: &SpBitMatrixDual) -> Result<Self, PreprocessError>;
    
    /// Encode message using preprocessed matrices
    pub fn encode(&self, message: &BitVec) -> BitVec;
}
```

**Tests**: 
- Small LDPC codes (7,4), (15,11)
- Verify H·encode(m) = 0 for all test messages
- Property test: roundtrip with decoder

#### Phase 2: DVB-T2 Integration (Day 1 afternoon)

**File**: `src/ldpc/core.rs` - Update `LdpcEncoder`

```rust
pub struct LdpcEncoder {
    code: LdpcCode,
    encoding_matrices: Arc<RuEncodingMatrices>,  // Cached, shared
}

impl LdpcEncoder {
    pub fn new(code: LdpcCode) -> Self {
        // Preprocess H and cache matrices
        let matrices = RuEncodingMatrices::preprocess(code.parity_check_matrix())
            .expect("Failed to preprocess LDPC code");
        Self {
            code,
            encoding_matrices: Arc::new(matrices),
        }
    }
}
```

**Optimization**: Cache preprocessing per DVB-T2 configuration globally
- 12 configurations total (6 rates × 2 frame sizes)
- Preprocess once at startup or lazily
- Share matrices across encoder instances

**Tests**:
- All DVB-T2 configurations can be preprocessed
- Encoding produces valid codewords
- Systematic form preserved

#### Phase 3: Test Vector Validation (Day 2 morning)

**File**: `tests/dvb_t2_ldpc_verification.rs`

```rust
#[test]
fn test_ldpc_encoding_tp05_to_tp06_full() {
    // Load VV001-CR35 test vectors
    // Test all 202 blocks in frame 1
    // Verify bit-exact match with TP06
}

#[test]
fn test_ldpc_decoding_tp06_to_tp05() {
    // Decode TP06 (perfect channel)
    // Verify matches TP05
}

#[test]
fn test_ldpc_error_correction() {
    // Add errors to TP06
    // Verify decoder corrects back to TP05
}
```

**Success Criteria**:
- 202/202 blocks match for encoding
- 202/202 blocks match for error-free decoding
- Error correction tests pass (vary error rates)

#### Phase 4: Roundtrip Tests (Day 2 afternoon)

**File**: `tests/ldpc_roundtrip.rs`

```rust
#[test]
fn test_ldpc_encode_decode_roundtrip() {
    // For each DVB-T2 configuration:
    //   Generate random messages
    //   Encode to codeword
    //   Decode back (no errors)
    //   Verify message recovered
}

proptest! {
    #[test]
    fn test_ldpc_roundtrip_property(
        rate in dvb_t2_code_rate(),
        message in random_message_for_rate(rate)
    ) {
        let code = LdpcCode::dvb_t2_normal(rate);
        let encoder = LdpcEncoder::new(code.clone());
        let decoder = LdpcDecoder::new(code.clone());
        
        let codeword = encoder.encode(&message);
        let llrs = perfect_channel_llrs(&codeword);
        let decoded = decoder.decode_soft(&llrs);
        
        // Extract message bits (systematic encoding)
        let recovered_message = decoded.slice(0, code.k());
        
        assert_eq!(message, recovered_message);
    }
}
```

#### Phase 5: Performance Tuning (Day 3)

**Benchmarks**: `benches/ldpc_encoding.rs`

```rust
fn bench_dvb_t2_normal_encoding(c: &mut Criterion) {
    let code = LdpcCode::dvb_t2_normal(CodeRate::Rate3_5);
    let encoder = LdpcEncoder::new(code);
    let message = random_message(32400);
    
    c.bench_function("dvb_t2_normal_3_5_encode", |b| {
        b.iter(|| encoder.encode(&message))
    });
}
```

**Targets**:
- DVB-T2 Normal encoding: <50ms per frame (>1 Mbps)
- DVB-T2 Short encoding: <10ms per frame (>1.5 Mbps)
- Preprocessing: <5 seconds per configuration (acceptable for one-time cost)

**Optimizations**:
1. Sparse matrix multiplication (already in gf2-core)
2. Cache locality in matrix-vector ops
3. SIMD opportunities in XOR operations
4. Lazy preprocessing (compute on first use)

---

## Integration with BCH

### Full DVB-T2 Encoding Chain

```
Input bits (Kbch)
      ↓
   BCH Encoder  (Kbch → k_ldpc, adds BCH parity)
      ↓
  LDPC Encoder  (k_ldpc → n_ldpc, adds LDPC parity)
      ↓
Output (64800 or 16200 bits)
```

**Test**: TP04 → TP05 → TP06 full chain validation

---

## Success Metrics

### Correctness
- ✅ All DVB-T2 test vectors pass (202/202 blocks)
- ✅ Encode/decode roundtrip tests pass (all configs)
- ✅ Property-based tests pass (1000+ random cases)
- ✅ Integration with BCH works

### Performance
- ✅ Encoding: >1 Mbps (preferably >10 Mbps)
- ✅ Decoding: >1 Mbps (already working)
- ✅ Preprocessing: <10 seconds per config
- ✅ Memory: <500MB for all 12 configs

### Code Quality
- ✅ TDD approach: tests written first
- ✅ Comprehensive documentation
- ✅ Clean API (no breaking changes to decoder)
- ✅ Examples demonstrating usage

---

## Risks and Mitigations

### Risk 1: RU Preprocessing Complexity
**Impact**: High  
**Probability**: Medium  
**Mitigation**: 
- Start with reference implementation from literature
- Test on small codes first
- Use existing Gaussian elimination from generator matrix code

### Risk 2: DVB-T2 Test Vectors Still Don't Match
**Impact**: High  
**Probability**: Low (RU is standard algorithm)  
**Mitigation**:
- Verify preprocessing matrices match expected structure
- Debug with small subset of blocks
- Check bit ordering conventions (MSB vs LSB)

### Risk 3: Performance Insufficient
**Impact**: Medium  
**Probability**: Low  
**Mitigation**:
- RU should be fast enough (standard in practice)
- Can optimize sparse matrix ops if needed
- DVB-T2 structure provides further speedup opportunities

---

## References

1. **Richardson, T. and Urbanke, R.** (2001). "Efficient encoding of low-density parity-check codes." IEEE Trans. Information Theory.

2. **ETSI EN 302 755** V1.4.1 (2015). "Digital Video Broadcasting (DVB); Frame structure channel coding and modulation for a second generation digital terrestrial television broadcasting system (DVB-T2)."

3. **Modern Coding Theory** by Richardson & Urbanke, Cambridge University Press, 2008. Chapter 4: LDPC Codes.

4. **Implementation references**:
   - GNU Radio gr-dtv module (DVB-T2 LDPC encoder)
   - AFF3CT library (LDPC encoding/decoding)
   - Intel FlexRAN (optimized LDPC implementations)

---

## Next Steps After Completion

With working LDPC encoding/decoding:
1. ✅ Full BCH + LDPC chain validation
2. → QAM modulation and demapping
3. → Bit interleaving
4. → Complete DVB-T2 FEC simulation
5. → FER vs Eb/N0 curves
6. → Shannon limit comparison
