# LDPC Verification Tests

**Status**: ✅ All tests passing with 100% accuracy  
**Test Vectors**: ETSI EN 302 755 DVB-T2 standard

---

## Overview

Comprehensive test suite validating LDPC encoding and decoding against official DVB-T2 test vectors.

**Location**: `tests/dvb_t2_ldpc_verification_suite.rs`

---

## Test Coverage

### Core Verification Tests

1. **Encoding Validation** (`test_ldpc_encoding_tp05_to_tp06`)
   - 202/202 blocks match ETSI reference
   - Tests: TP05 (38,880 bits) → TP06 (64,800 bits)
   - Reports: Throughput in Mbps

2. **Error-Free Decoding** (`test_ldpc_decoding_tp06_to_tp05_error_free`)
   - 202/202 blocks recover correctly
   - Tests: TP06 → recovered TP05
   - Tracks: Convergence and iteration count

3. **Error Correction** (`test_ldpc_error_correction`)
   - Tests with injected random errors (0.1%, 0.5%, 1%, 2%)
   - 10 blocks × 5 trials per error rate
   - Reports: Success rate and average iterations

### Property Tests

4. **Systematic Property** (`test_ldpc_systematic_property`)
   - Verifies: First k bits match message exactly
   - Format: c = [message | parity]

5. **Parity Check** (`test_ldpc_parity_check`)
   - Verifies: H·c = 0 for all codewords
   - Validates mathematical correctness

6. **Roundtrip** (`test_ldpc_roundtrip`)
   - End-to-end: message → encode → decode → message
   - 100% recovery verification

### Consistency Tests

7. **Multi-Frame Encoding** (`test_ldpc_encoding_sample`)
   - Spot-checks across all 4 frames
   - Tests: First, middle, last block per frame

8. **Parameter Validation** (`test_ldpc_parameter_validation`)
   - DVB-T2 Normal Rate 3/5: n=64,800, k=38,880
   - Code rate and dimension checks

---

## Running Tests

### Prerequisites

**Test Vectors** (required):
```bash
export DVB_TEST_VECTORS_PATH=/path/to/dvb_test_vectors
```

**LDPC Cache** (recommended):
```bash
# One-time generation (~16 seconds)
cargo run --release --bin generate_ldpc_cache short
```
- Without cache: 2-10s preprocessing per encoder
- With cache: <1ms instant loading

### Execute Tests

```bash
# All verification tests
cargo test --test dvb_t2_ldpc_verification_suite -- --ignored --nocapture

# Specific test
cargo test --test dvb_t2_ldpc_verification_suite test_ldpc_encoding_tp05_to_tp06 -- --ignored --nocapture
```

### Expected Performance

**With pre-computed cache:**
- `test_ldpc_encoding_tp05_to_tp06`: ~4-8 seconds (202 blocks)
  - Throughput: 50-100 Mbps
- `test_ldpc_decoding_tp06_to_tp05_error_free`: ~40-80 seconds (202 blocks)
  - Throughput: 20-40 Mbps
  - Average iterations: 1-3 (most converge immediately)
- `test_ldpc_error_correction`: ~10-20 seconds (200 trials)
  - Correction rate: 90-100% depending on error rate

**Without cache:**
- Add 2-10 seconds for first encoder creation (one-time per test)

## Test Configuration

### Code Parameters (VV001-CR35)

- **Configuration**: DVB-T2 Normal, Rate 3/5
- **Codeword length (n)**: 64,800 bits
- **Message length (k)**: 38,880 bits  
- **Parity bits (m)**: 25,920 bits
- **Code rate**: 3/5 = 0.6
- **Frame structure**: 4 frames, 202 blocks per frame

### LLR Settings

Tests use appropriate LLR confidence levels:

- **Error-free decoding**: ±10.0 (high confidence)
- **Error correction**: ±3.0 (moderate confidence)
- **Channel simulation**: Based on AWGN noise variance

LLR convention:
- `+10.0`: Strong belief in bit 0
- `-10.0`: Strong belief in bit 1
- Magnitude indicates confidence

## Implementation Notes

### Cache Integration

The test suite includes a helper to automatically load cache if available:

```rust
fn try_load_cache() -> Option<EncodingCache> {
    let cache_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data/ldpc/dvb_t2");
    if cache_dir.exists() {
        EncodingCache::from_directory(&cache_dir).ok()
    } else {
        None
    }
}
```

### Encoder Creation

```rust
fn create_encoder(code: LdpcCode, cache: Option<&EncodingCache>) -> LdpcEncoder {
    match cache {
        Some(c) => LdpcEncoder::with_cache(code, c),  // Fast path
        None => LdpcEncoder::new(code)                 // Slow path (preprocessing)
    }
}
```

### BitVec to LLR Conversion

Standard pattern for converting BitVec to soft LLRs:

```rust
let mut llrs = Vec::with_capacity(bitvec.len());
for i in 0..bitvec.len() {
    let bit = bitvec.get(i);
    llrs.push(if bit {
        Llr::new(-10.0)  // Bit 1
    } else {
        Llr::new(10.0)   // Bit 0
    });
}
```

## Comparison with BCH Tests

The LDPC test suite follows the same structure as BCH verification:

| Aspect | BCH Tests | LDPC Tests |
|--------|-----------|------------|
| Test vectors | TP04 ↔ TP05 | TP05 ↔ TP06 |
| Encoding | Systematic (fast) | Systematic (needs preprocessing) |
| Decoding | Algebraic (deterministic) | Iterative (soft-decision) |
| Error correction | Up to t=12 (bounded) | Probabilistic (unbounded) |
| Throughput | 100+ Mbps | 20-100 Mbps |
| Test count | 5 tests | 8 tests |

## Success Criteria

For Phase C10.6.5 completion, all tests must pass with:

- ✅ 100% encoding match (202/202 blocks)
- ✅ 100% error-free decoding (202/202 blocks)
- ✅ >90% error correction at reasonable error rates
- ✅ Systematic property verified
- ✅ Parity check property verified (H·c = 0)
- ✅ Full roundtrip successful

## Troubleshooting

### Test Vectors Not Found

```
Test vectors not available at "/home/user/dvb_test_vectors"
```

**Solution**: Set `DVB_TEST_VECTORS_PATH` environment variable or place vectors in `~/dvb_test_vectors/`

### Slow Encoder Creation

```
Creating encoder without cache (this may take 2-10 seconds)...
```

**Solution**: Generate pre-computed cache:
```bash
cargo run --release --bin generate_ldpc_cache short
```

### Encoding Mismatch

If encoding tests fail:
1. Check generator matrix cache is loaded correctly
2. Verify systematic encoding convention (bit 0 = highest coefficient)
3. Compare with validation script output
4. Check for bit ordering issues

### Decoding Failures

If decoding tests fail:
1. Check LLR sign convention (positive = bit 0, negative = bit 1)
2. Verify iteration limit is sufficient (50 iterations)
3. Check parity-check matrix construction
4. Validate syndrome computation

## Future Enhancements

1. **Performance Benchmarks**
   - Encoding throughput measurement
   - Decoding throughput measurement
   - Iteration count analysis
   - Memory profiling

2. **Extended Configurations**
   - Test all 12 DVB-T2 code rates
   - Short frame validation
   - Multi-configuration comparison

3. **Error Pattern Analysis**
   - Burst error testing
   - Error floor characterization
   - Waterfall region analysis

4. **Integration Tests**
   - Full BCH+LDPC chain (TP04 → TP05 → TP06)
   - Round-trip with error injection at different stages
   - Performance comparison with other implementations

## References

- **DVB-T2 Standard**: ETSI EN 302 755 V1.4.1
- **Test Vectors**: DVB Project Verification & Validation
- **Implementation**: Phase C10.6 (Richardson-Urbanke systematic encoding)
- **Related Docs**:
  - [DVB_T2.md](DVB_T2.md) - DVB-T2 implementation and verification status
  - [ROADMAP.md](../ROADMAP.md) - Phase C10.6 implementation plan
  - `tests/dvb_t2_bch_verification.rs` - BCH verification (reference implementation)

## See Also

- **BCH Verification**: `tests/dvb_t2_bch_verification.rs` (similar structure)
- **Test Vector Parser**: `tests/test_vectors/` (shared infrastructure)
- **LDPC Examples**: `examples/ldpc_*.rs` (usage patterns)
- **Cache Generation**: `src/bin/generate_ldpc_cache.rs` (one-time setup)
