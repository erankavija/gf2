# LDPC Parity Check Matrix Issue

## Problem Statement

DVB-T2 test vectors from TP06 (LDPC codewords) are failing parity check validation with our constructed LDPC parity-check matrix.

## Test Results

**Test**: `test_ldpc_parity_check` 
**Result**: FAIL - All 10 test blocks fail parity check

**Syndrome Weights**:
- Block 1: 12,956 / 25,920 (50.0%)
- Block 2: 12,904 / 25,920 (49.8%)
- Block 3-10: Similar (~50%)

**Expected**: Syndrome weight = 0 (H·c = 0 for valid codewords)
**Actual**: Syndrome weight ≈ m/2 (exactly half the parity bits)

## What Works

✅ **BCH validation**: All BCH tests pass (TP04 ↔ TP05)
✅ **Systematic property**: First k=38,880 bits of TP06 match TP05 exactly
✅ **Dual-diagonal structure**: Parity portion has correct staircase pattern
✅ **Parameters**: n=64,800, k=38,880, m=25,920 match DVB-T2 spec

## Analysis

###Syndrome Weight Pattern

The fact that ALL blocks fail with syndrome weight ≈ m/2 is highly suspicious and suggests a systematic error rather than random issues:

1. **Not random errors**: Random bit flips would give varying syndrome weights
2. **Not wrong codewords**: BCH output (TP05) validates correctly
3. **Exact 50% pattern**: Suggests bits are systematically inverted or reordered

### Dual-Diagonal Structure (VERIFIED ✓)

Our implementation now correctly implements DVB-T2 dual-diagonal:
- Row 0: Single 1 at column k+0 (NO wrap-around)
- Row p (p>0): Two 1s at columns k+p and k+(p-1)

This was fixed from incorrect wrap-around implementation.

### Possible Root Causes

1. **Information-to-Parity Connection Error**
   - DVB-T2 tables specify base parity indices
   - Expansion formula: parity_bit = (base + j * q) mod m
   - Could be wrong modulo, wrong step size, or wrong base interpretation

2. **Bit Ordering Convention**
   - Test vectors might use different bit ordering
   - LSB vs MSB convention mismatch
   - Block ordering within frame

3. **Matrix Transposition**
   - H might need to be transposed
   - Row/column indexing might be swapped

4. **Edge Interpretation**
   - (check, variable) vs (variable, check) ordering
   - Sparse matrix might be in wrong orientation

## Next Steps

### 1. Verify Test Vector Format
```bash
# Check if TP06 blocks are actually valid LDPC codewords
# Try with different LDPC code rates or frame sizes
```

### 2. Check DVB-T2 Standard Reference
- ETSI EN 302 755 Section 5.3.2 (LDPC encoding)
- Verify table interpretation (especially base parity index meaning)
- Check if there's a transpose or reordering step

### 3. Test with Simpler Case
- Create minimal LDPC code from first principles
- Verify syndrome computation works on known valid codewords
- Try DVB-T2 Short frame (smaller, easier to debug)

### 4. Compare with Reference Implementation
- Check if other DVB-T2 LDPC implementations exist
- Verify our edge construction against known-good code

### 5. Investigate Bit Ordering
```rust
// Try inverting bits
let inverted = !codeword.data;
if code.is_valid_codeword(&inverted) { ... }

// Try reversing bit order
let reversed = codeword.data.reverse();
if code.is_valid_codeword(&reversed) { ... }
```

## Code Locations

- **Builder**: `src/ldpc/dvb_t2/builder.rs` - Edge construction from tables
- **Tables**: `src/ldpc/dvb_t2/dvb_t2_matrices.rs` - DVB-T2 base matrices
- **Test**: `tests/dvb_t2_ldpc_verification_suite.rs::test_ldpc_parity_check`
- **Dual-Diagonal Fix**: Commit that changed wrap-around behavior

## References

- ETSI EN 302 755 V1.4.1 - DVB-T2 Standard
- Section 5.3.2 - LDPC encoding process
- Table 5a - LDPC code parameters
- Annex B - LDPC parity-check matrices

## Status

🔴 **BLOCKED**: Cannot proceed with LDPC validation until parity-check matrix is corrected.

The systematic encoding property validates correctly, suggesting the issue is specifically with how we construct the information-to-parity connections from the DVB-T2 tables, NOT with the dual-diagonal parity structure.

## Latest Investigation Results

### Cache-Based Encoding Test (Nov 27, 2024)

After generating LDPC cache with RREF'd generator matrices:
- **80% bits match** (51,834/64,800 correct)
- **20% bits mismatch** (12,966/64,800 wrong)

This is much better than the 50% syndrome-based test, but still not correct.

### Verified Components

1. ✅ **Dual-Diagonal Structure**: Confirmed correct
   - Row 0 has single 1 at column k+0 (NO wrap-around)
   - Rows p>0 have two 1s at columns k+p and k+(p-1)
   - Visualization shows perfect staircase pattern

2. ✅ **DVB-T2 Table Connections**: Confirmed correct
   - Block 36 correctly connects to parity check 0
   - Info bit 12960 connects to checks [0, 18539, 18661] ✓
   - Info bit 12961 connects to checks [72, 18611, 18733] ✓ (with q=72)
   - Expansion formula `(base + j*q) mod m` works correctly

3. ✅ **Systematic Property**: Confirmed correct
   - First k=38,880 bits of encoded output match TP05 input exactly

### Remaining Issue

Despite all individual components being correct, the **overall H matrix produces wrong encoding**:
- RREF-based encoder gives 80% correct output
- All TP06 test vectors fail parity check with ~50% syndrome weight
- Some parity bits match (p0, p1, p2, p5), others don't (p3, p4, p6-p9)

**Hypothesis**: There may be a subtle issue with:
- Row ordering in the H matrix
- Column ordering expectations
- Edge interpretation in sparse matrix construction
- Some DVB-T2 convention we're missing

The error is systematic but not total, suggesting we're "close" but have a fundamental misunderstanding of some aspect of the DVB-T2 LDPC matrix construction.

## Files Added

- `tests/dvb_t2_ldpc_verification_suite.rs` - Comprehensive 8-test validation suite
- `docs/LDPC_VERIFICATION_TESTS.md` - Full test documentation
- `tests/ldpc_validation.rs` - Added parity structure visualization test
- LDPC cache infrastructure (generates in ~8 minutes with SIMD)

## Next Steps for Resolution

1. Compare with reference implementation (e.g., DVB-T2 encoder reference)
2. Check ETSI EN 302 755 section 5.3.2 for encoding conventions
3. Verify if there's a specific row/column reordering in DVB-T2
4. Check if parity-check matrix needs different edge interpretation
5. Test with DVB-T2 Short frames (smaller, easier to debug)
