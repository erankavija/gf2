# DVB-T2 Verification Status

## Current Status Summary

| Component | Status | Test Vectors | Details |
|-----------|--------|--------------|---------|
| **BCH Outer Code** | ✅ **VERIFIED** | 202/202 blocks match | Phase 2 complete |
| **LDPC Encoding** | ✅ **VERIFIED** | 202/202 blocks match | Phase 3.6 complete |
| **LDPC Decoding** | ✅ **VERIFIED** | 202/202 blocks match | Phase 3.6 complete |
| **Test Infrastructure** | ✅ **COMPLETE** | Parser + cache | Phase 1 complete |
| **Full FEC Chain** | ⏳ **READY** | - | BCH + LDPC both verified |

**Last Updated**: 2025-11-27 19:12 UTC

**Key Achievements**:
- ✅ BCH: 100% match with ETSI EN 302 755 test vectors (202/202 blocks)
- ✅ LDPC Encoding: 100% match with test vectors (202/202 blocks, 12.4s, 0.63 Mbps)
- ✅ LDPC Decoding: 100% match with test vectors (202/202 blocks, 5.8s, 1.35 Mbps)
- ✅ LDPC Math: H × G^T = 0 verified, systematic property confirmed
- ✅ RREF bug discovered and fixed in gf2-core
- ✅ Decoder bug fixed: Now correctly extracts message bits from systematic codewords

**Next Step**: Performance optimization to achieve real-time DVB-T2 throughput (31-50 Mbps)

---

## Overview

Implementation progress for DVB-T2 BCH and LDPC verification using official DVB Project test vectors.

## Phase 1: Test Vector Parser Module ✅ **COMPLETE**

**Completion Date**: 2025-11-24  
**Effort**: 1 day  
**Status**: All tests passing (21/21)

### Implementation Summary

Created a complete test vector parsing infrastructure in `tests/test_vectors/`:

1. **Parser Module** (`parser.rs` - 330 lines)
   - `TestVector` struct: frame/block metadata + bit data
   - `TestVectorFile` struct: multi-frame, multi-block structure
   - Binary string parser (64 bits/line format)
   - Frame and block marker parsing (`# frame N`, `# block M of K`)
   - Comment line handling (`%` and `#`)
   - Block count validation per frame
   - Error handling with detailed line numbers

2. **Configuration Module** (`config.rs` - 95 lines)
   - `DvbConfig` struct: frame size + code rate
   - VV reference name parser (e.g., "VV001-CR35" → Rate3_5)
   - Code rate mappings: CR12, CR35, CR23, CR34, CR45, CR56
   - Frame size detection (Normal/Short)

3. **Loader Module** (`loader.rs` - 160 lines)
   - `TestVectorSet` struct: TP04/05/06/07a collection
   - Configuration directory scanner
   - Test point file locator with flexible naming
   - Graceful degradation for missing test points
   - Multi-configuration support

4. **Infrastructure** (`mod.rs` - 42 lines)
   - `test_vectors_path()`: Environment variable resolution
   - `test_vectors_available()`: Availability check
   - `require_test_vectors!()`: Macro for test skipping
   - Default path: `$HOME/dvb_test_vectors`

5. **Integration Tests** (`test_vector_parser.rs` - 165 lines)
   - End-to-end parsing validation
   - Structure consistency checks
   - Bit length progression validation
   - Multi-test-point coordination

### Test Coverage

**Unit Tests** (14 tests):
- ✅ Filename parsing
- ✅ Frame marker parsing
- ✅ Block marker parsing  
- ✅ Binary string conversion
- ✅ Simple file parsing
- ✅ Multi-frame parsing
- ✅ Invalid binary detection
- ✅ Block count mismatch detection
- ✅ Code rate mappings (all 6 rates)
- ✅ Invalid reference detection
- ✅ Configuration directory errors
- ✅ Missing file handling

**Integration Tests** (7 tests, requires test vectors):
- ✅ Full VV001-CR35 loading
- ✅ TP04 structure validation
- ✅ Multi-test-point consistency
- ✅ Bit length progression (BCH → LDPC)
- ✅ Frame/block count validation
- ✅ Data integrity checks

### Validation Results: VV001-CR35

**Configuration**:
- Reference: VV001-CR35
- Frame Size: NORMAL (64,800 bits)
- Code Rate: 3/5
- BCH: t=12, 192 parity bits
- LDPC: Normal frame, rate 3/5

**Test Point Structure**:

| Test Point | Description | Blocks/Frame | Bits/Block | Total Frames |
|------------|-------------|--------------|------------|--------------|
| TP04 | BCH input (BBFRAMEs) | 202 | 38,688 | 4 |
| TP05 | BCH output (FECFRAMEs) | 202 | 38,880 | 4 |
| TP06 | LDPC output | 202 | 64,800 | 4 |
| TP07a | Bit interleaved | 202 | 64,800 | 4 |

**Bit Length Validation**:
- TP04 → TP05: +192 bits (BCH parity) ✓
- TP05 → TP06: +25,920 bits (LDPC parity) ✓
- Rate calculation: 38,688 / 64,800 = 0.597 ≈ 3/5 ✓

**File Statistics**:
- TP04: 489,674 lines (~80 MB parsed)
- TP05: 492,098 lines (~80 MB parsed)
- TP06: 819,338 lines (~130 MB parsed)
- TP07a: 819,338 lines (~130 MB parsed)
- Total: ~2.6M lines, ~420 MB data parsed successfully

### Usage

```bash
# Set test vector location
export DVB_TEST_VECTORS_PATH=/path/to/your/dvb_test_vectors

# Run verification tests
cargo test --test test_vector_parser -- --ignored --nocapture

# Run normal tests (verification tests skipped automatically)
cargo test
```

### Code Quality

- ✅ Follows TDD methodology (tests written first)
- ✅ Comprehensive error handling with `thiserror`
- ✅ Zero unsafe code
- ✅ Functional programming style
- ✅ Clear documentation
- ✅ All warnings addressed
- ✅ Modular design for extensibility

### Next Steps

With the parser infrastructure complete, we can now proceed to:

**Phase 2: BCH Verification** (1 day)
- Implement BCH encoding validation (TP04 → TP05)
- Implement BCH decoding validation (TP05 → TP04)
- Test error correction with injected errors

**Phase 3: LDPC Verification** (1-2 days)
- Validate systematic encoding property (TP05 ⊂ TP06)
- Implement LDPC decoding validation (TP06 → TP05)
- Test soft-decision decoding with AWGN

**Phase 4: Integration Tests** (1 day)
- Full FEC chain validation (TP04 → TP06 → TP04)
- Round-trip with error injection
- Performance benchmarking

## Dependencies

### Rust Crates Added
- `tempfile = "3.8"` (dev-dependency for tests)
- `thiserror = "1.0"` (dev-dependency for error handling)

### Existing Code Used
- `gf2_core::BitVec`: Bit vector storage
- `gf2_coding::CodeRate`: Code rate enum
- `gf2_coding::ldpc::dvb_t2::FrameSize`: Frame size enum

### External Resources Required
- DVB test vectors (not in repository due to copyright)
- Location: Set via `$DVB_TEST_VECTORS_PATH` environment variable
- Source: DVB Project Verification & Validation working group

## Performance

**Parsing Performance**:
- TP04 (490K lines): ~1.5 seconds
- TP05 (492K lines): ~1.5 seconds  
- TP06 (819K lines): ~2.5 seconds
- TP07a (819K lines): ~2.5 seconds
- **Total**: ~7.2 seconds for full VV001-CR35 suite

**Memory Usage**:
- Efficient streaming parser
- BitVec uses compact 64-bit word storage
- Scales to multiple configurations

## Phase 2: BCH Verification ✅ **COMPLETE**

**Completion Date**: 2025-11-24  
**Effort**: 1 day (test infrastructure + bug fix)
**Status**: ✅ **100% validation passing** - All 202 blocks match ETSI EN 302 755 test vectors

### Implementation Summary

Created comprehensive BCH verification test suite in `tests/dvb_t2_bch_verification.rs` (280 lines):

**Test Functions** (5 tests):
1. `test_bch_encoding_tp04_to_tp05` - Validates BCH encoding against TP04→TP05
2. `test_bch_decoding_tp05_to_tp04_error_free` - Validates error-free decoding  
3. `test_bch_error_correction` - Tests error correction up to t=12 errors
4. `test_bch_systematic_property` - Verifies test vectors use systematic encoding
5. `test_bch_encoding_sample` - Spot checks across multiple frames

**Test Coverage**:
- Full frame validation (202 blocks)
- Error injection and correction (t=1 to t=12)
- Systematic property verification
- Multi-frame consistency checks
- Detailed diagnostic output

### Implementation Summary

**Test Infrastructure** (280 lines in `tests/dvb_t2_bch_verification.rs`):
- 5 comprehensive verification tests
- Full frame validation (202 blocks × 4 frames = 808 blocks)
- Error injection and correction testing (t=1 to t=12)
- Systematic property validation
- Multi-frame consistency checks

### Bug Discovery and Fix

**Issue Found**: Polynomial-to-bit mapping was reversed

**Root Cause**: DVB-T2 uses the convention where bit position 0 corresponds to the highest polynomial coefficient, not the lowest. This is critical for systematic encoding to work correctly.

**Fix Applied**:
1. Updated encoder to use reversed bit-to-polynomial mapping
2. Updated decoder syndrome computation for correct bit ordering
3. Updated error correction to map polynomial degrees to correct bit positions
4. Added comprehensive unit test for systematic property

**Details**: The root cause was a reversed polynomial-to-bit mapping. DVB-T2 convention has bit 0 as the highest coefficient, which is critical for systematic encoding.

### Verification Results ✅

**All tests passing with 100% accuracy:**

| Test | Result | Details |
|------|--------|---------|
| `test_bch_systematic_property` | ✅ PASS | Confirms test vectors use [message\|parity] format |
| `test_bch_encoding_tp04_to_tp05` | ✅ PASS | **202/202 blocks match** (Frame 1) |
| `test_bch_decoding_tp05_to_tp04_error_free` | ✅ PASS | **202/202 blocks match** (Frame 1) |
| `test_bch_error_correction` | ✅ PASS | **100% correction rate** (t=1 to t=12) |
| `test_bch_encoding_sample` | ✅ PASS | All sample blocks across 4 frames match |

**Performance**:
- Full verification suite: ~55 seconds
- 808 total blocks verified across 4 frames
- Error correction tested: 12 × 50 = 600 test cases

**Code Quality**:
- ✅ Follows TDD methodology
- ✅ Comprehensive error handling
- ✅ Zero unsafe code
- ✅ Functional programming style
- ✅ All warnings addressed
- ✅ DVB-T2 standard compliance verified

## Phase 3: LDPC Encoding Implementation 🔧 **IN PROGRESS**

### Phase 3.1: Richardson-Urbanke Core Algorithm ✅ **COMPLETE**
**Completion Date**: 2025-11-25  
**Effort**: 2 hours  
**Status**: Core algorithm working on small codes

**Deliverables**:
- ✅ `src/ldpc/encoding/richardson_urbanke.rs` (399 lines) - RU preprocessing
- ✅ `RuEncodingMatrices` struct with generator matrix computation
- ✅ Gaussian elimination with row reordering (critical fix from HPC specialist)
- ✅ `LdpcEncoder::new()` integration
- ✅ 3 unit tests passing (Hamming [7,4] code)
- ✅ 3 integration tests passing

**Key Achievement**: Correct generator matrix computation via row reordering after Gaussian elimination.

### Phase 3.2: Dense Matrix Optimization ✅ **COMPLETE**
**Completion Date**: 2025-11-26  
**Effort**: 4 hours total (initial sparse + dense migration)  
**Status**: Dense parity matrices with 28× file size reduction

**Deliverables**:
- ✅ `src/ldpc/encoding/cache.rs` - Opt-in cache with dense I/O
- ✅ `EncodingCache` struct (user-controlled, no global state)
- ✅ `LdpcEncoder::with_cache()` - alternative constructor
- ✅ 6 cache unit tests passing
- ✅ **Dense parity matrices** (`BitMatrix` for 40-50% density DVB-T2 matrices)
- ✅ `examples/ldpc_encoding_with_cache.rs` - demonstration

**Design Properties**:
- Cache is explicit parameter, not hidden global state
- `LdpcEncoder::new()` works without cache (unchanged)
- `LdpcEncoder::with_cache(code, &cache)` for performance
- Each test creates its own local cache
- Thread-safe with RwLock
- **Dense storage**: 28× smaller files than sparse for high-density matrices

**Performance Achievements**:
- **File size**: 1.1 GB → 39 MB for 6 Short frames (28× reduction) ✅ CONFIRMED
- **Parity density**: 40-50% for DVB-T2 (optimal for dense storage)
- **Cache loading**: 16ms for all 6 configs ✅ CONFIRMED
- **Encoding**: Uses `BitMatrix::matvec_transpose()` with word-level ops
- **Preprocessing**: 0.2-1.6s per Short frame with RREF+SIMD ✅

**Test Results**: 218 tests passing + validation script confirms correctness

**Key Achievement**: Dense matrix storage is optimal for DVB-T2's high-density parity matrices (40-50%), giving 28× smaller files vs sparse format while maintaining fast encoding via word-level operations.

### Phase 3.5: LDPC Test Suite & Bug Fix ✅ **COMPLETE**

**Completion Date**: 2025-11-27  
**Effort**: 2 days (test infrastructure + bug discovery & fix)
**Status**: ✅ Mathematical correctness verified, full test vector validation ready

**Deliverables**:
- ✅ Comprehensive test suite: 8 tests in `dvb_t2_ldpc_verification_suite.rs`
  - `test_ldpc_encoding_tp05_to_tp06` - Encoding validation
  - `test_ldpc_decoding_tp06_to_tp05_error_free` - Error-free decoding
  - `test_ldpc_error_correction` - Error correction capability
  - `test_ldpc_systematic_property` - Systematic form validation
  - `test_ldpc_encoding_sample` - Multi-frame consistency
  - `test_ldpc_parameter_validation` - DVB-T2 parameter compliance
  - `test_ldpc_parity_check` - Parity check property (H·c = 0)
  - `test_ldpc_roundtrip` - Full encode/decode roundtrip

- ✅ Test infrastructure improvements
  - Cache support with automatic fallback
  - BitVec to LLR conversion helpers
  - Multi-frame validation utilities
  - Throughput measurement and reporting

- ✅ **Bug Discovery & Fix** (Critical)
  - **Issue Found**: Initial tests showed 80% encoding accuracy, parity check failures
  - **Root Cause**: RREF right-pivoting bug in gf2-core (incorrect pivot column identification)
  - **Fix**: Corrected row reordering in RREF algorithm (gf2-core commit 7963634)
  - **Verification**: 22 property tests added validating H × G^T = 0
  - **Result**: All 446 tests passing (40 LDPC + 406 gf2-core)

- ✅ Diagnostic test suite
  - `verify_cached_generator.rs` - Property test H × G^T = 0 for all basis vectors
  - Tests validate mathematical correctness of Richardson-Urbanke encoding
  - Confirms generator matrices are consistent with parity-check matrices

**Test Coverage**:
- 8 comprehensive validation tests (all ignored, ready to run with test vectors)
- 3 diagnostic property tests (verify mathematical correctness)
- 40 LDPC unit tests (all passing)
- Documentation: `docs/LDPC_VERIFICATION_TESTS.md` (283 lines)
- Issue tracking: `docs/LDPC_PARITY_CHECK_ISSUE.md` (173 lines)

**Mathematical Verification**: ✅ **COMPLETE**
- H × G^T = 0 verified for all standard basis vectors
- Systematic property: G = [I_k | P] confirmed
- Zero syndrome for all encoded messages verified
- Linearity property validated

**Next Step**: Phase 3.6

### Phase 3.6: Test Vector Validation Results 🔧 **IN PROGRESS**

**Completion Date**: 2025-11-27 (started)
**Status**: Encoding verified, decoding requires fix
**Effort**: <1 day (debugging decoder)

**Prerequisites**:
- ✅ Test vectors at `$DVB_TEST_VECTORS_PATH` (VV001-CR35)
- ✅ Pre-computed LDPC cache at `data/ldpc/dvb_t2/` (optional but recommended)
- ✅ Test suite implemented and debugged
- ✅ Mathematical correctness verified

**Test Results** (2025-11-27):

✅ **Encoding Validation**: **PASSED**
- Test: `test_ldpc_encoding_tp05_to_tp06`
- Result: **202/202 blocks match** ETSI EN 302 755 test vectors
- Time: 12.43 seconds (release build)
- Throughput: 0.63 Mbps
- **Status**: Same perfect match as BCH achieved! 🎉

✅ **Systematic Property**: **PASSED**
- Test: `test_ldpc_systematic_property`
- Result: First k=38,880 bits match message exactly
- Time: 1.03 seconds
- **Status**: Verified ✓

✅ **Parity Check Property**: **PASSED**
- Test: `test_ldpc_parity_check`
- Result: H·c = 0 for all 10 test blocks
- Time: 1.04 seconds
- **Status**: Mathematical correctness confirmed ✓

✅ **Decoding Validation**: **PASSED** (after fix)
- Test: `test_ldpc_decoding_tp06_to_tp05_error_free`
- Result: **202/202 blocks match** ETSI EN 302 755 test vectors
- Time: 5.84 seconds (release build)
- Throughput: 1.35 Mbps
- Convergence: All blocks converge in 1 iteration (expected for error-free)
- **Status**: Perfect match! 🎉

✅ **Roundtrip Validation**: **PASSED**
- Test: `test_ldpc_roundtrip`
- Result: 10/10 blocks pass encode → decode → message verification
- Time: 2.14 seconds
- **Status**: Full roundtrip works correctly ✓

**Bug Found & Fixed** (2025-11-27):
- **Issue**: Decoder returned full n-bit codeword instead of k-bit message
- **Root Cause**: `decode_iterative()` didn't extract message from systematic codeword
- **Fix**: Added message extraction logic (bits [0..k) from decoded codeword)
- **Code**: `src/ldpc/core.rs` lines 848-858
- **Convention**: Follows systematic `[message | parity]` format documented in `SYSTEMATIC_ENCODING_CONVENTION.md`
- **Fix Time**: 30 minutes (diagnosis + implementation + validation)

### Phase 3.3: File I/O Integration ✅ **COMPLETE**
**Completion Time**: 4 hours (TDD)
**Status**: File-based caching implemented and tested

**Deliverables**:
- ✅ `CacheIoError` - Error type for file I/O operations
- ✅ `EncodingCache::save_to_directory()` - Save cache as `.gf2` files
- ✅ `EncodingCache::from_directory()` - Load cache from disk
- ✅ `EncodingCache::precompute_and_save_dvb_t2()` - One-time generation utility
- ✅ `RuEncodingMatrices::generator()` - Access generator for saving
- ✅ `RuEncodingMatrices::from_generator()` - Reconstruct from loaded matrix
- ✅ 8 file I/O tests (2 fast, 6 slow marked `#[ignore]`)
- ✅ Example: `examples/ldpc_cache_file_io.rs` (147 lines)
- ✅ Total: 218 library tests passing + 2 I/O tests passing

**File Format**:
- Naming: `n{codeword_length}_k{message_length}_h{hash}.gf2`
- Format: Binary COO (Coordinate) sparse matrix format from gf2-core
- Size: ~800 KB (Short) to ~1.6 MB (Normal) per config
- Total: ~10 MB for all 12 DVB-T2 configurations

**Design Philosophy**:
- ✅ No forced global state (user-controlled cache)
- ✅ `LdpcEncoder::new()` still works without cache
- ✅ File I/O completely opt-in
- ✅ Uses gf2-core's `SpBitMatrixDual::save_to_file()` / `load_from_file()`

**Performance Achieved**:
- **Without cache**: 2-3 seconds per encoder creation
- **With memory cache**: <1μs after first (2,000,000× speedup)
- **With file cache**: <10ms always (12,000× speedup) ✅ TARGET MET
- **Disk space**: 10 MB total for all 12 configs

**Integration with gf2-core Phase 12**:
- Uses binary COO format: >100× compression
- Leverages existing I/O infrastructure
- Multiple format support (Binary/Text)

**Key Achievement**: Eliminated preprocessing bottleneck entirely by caching generator matrices to disk.

### Phase 3.4: Cache Generation & Validation ✅ **COMPLETE**
**Completion Date**: 2025-11-26  
**Status**: 6 Short frame caches generated, storage optimization completed

#### Storage Optimization (2025-11-26)

**Problem Identified:**
- Original implementation stored full generator matrix G = [I_k | P]
- Identity part is completely redundant for systematic codes
- Sparse format inefficient for 40-50% dense DVB-T2 matrices

**Solution Implemented:**
- ✅ Store ONLY parity part P (not full generator G)
- ✅ API: `parity_part()` returns P, `generator()` reconstructs G on demand
- ✅ Single `.gf2` file per code (no `.meta` files)
- ✅ Assumes standard systematic form: systematic in [0..k), parity in [k..n)
- ✅ Incremental saving (each file saved after preprocessing)
- ✅ Flexible generation: `short` (6 configs) or `all` (12 configs)

**Current State:**
- Format: Sparse `SpBitMatrixDual` in `.gf2` files
- Size: 1.1 GB for 6 Short frames (~190 MB per matrix)
- Generation time: 16 seconds with RREF+SIMD
- Location: `data/ldpc/dvb_t2/*.gf2`

**Files Generated:**
```
n16200_k7200_*.gf2   (Rate 1/2) - 240 MB
n16200_k9720_*.gf2   (Rate 3/5) - 200 MB
n16200_k10800_*.gf2  (Rate 2/3) - 226 MB
n16200_k11880_*.gf2  (Rate 3/4) - 195 MB
n16200_k12600_*.gf2  (Rate 4/5) - 140 MB
n16200_k13320_*.gf2  (Rate 5/6) - 128 MB
```

**Commands:**
```bash
# Generate Short frames only (16 seconds)
cargo run --release --bin generate_cache short

# Generate all 12 configs (~5-10 minutes)
cargo run --release --bin generate_cache all
```

**Performance with RREF+SIMD (from gf2-core):**
- Preprocessing: 0.4-2.0 seconds per Short frame
- Total Short frames: 16 seconds for all 6
- Normal frames: 25-211 seconds each (6 configs total)
- Speedup: 100× faster than naive Gaussian elimination

#### Future Dense Storage Optimization ⏳ **BLOCKED**

**Potential 30× Size Reduction:**
- Current: 1.1 GB (sparse format)
- Potential: 40 MB (dense format)
- Reason: DVB-T2 parity matrices are 40-50% dense

**Blocking Issue:**
- Dense storage requires `BitMatrix::matvec_transpose()` method
- Currently only available for sparse matrices
- Documented in: `crates/gf2-core/REQUIRED_FEATURES.md`

**Benefits of Dense Storage:**
- 30× smaller files (40 MB vs 1.1 GB for Short frames)
- Potentially faster encoding with SIMD for high-density matrices
- Better cache locality for dense operations

**When Unblocked:**
- Switch from `SpBitMatrixDual` to dense `BitMatrix`
- Update save/load to use dense format
- Expected 30× file size reduction

## Implementation Notes

### Richardson-Urbanke Algorithm (Phase C10.6.1)
The systematic encoding uses Gaussian elimination to transform the parity-check matrix H into a form that enables efficient encoding. Key insight: row reordering after elimination is critical to align the identity structure correctly for extracting parity relationships.

**Algorithm**:
1. Gaussian elimination with column pivoting to find m independent parity columns
2. **Critical**: Reorder rows so row i has its pivot in parity_cols[i]
3. Build generator G = [I_k | P] where P[i,j] = H_work[row_order[j], message_cols[i]]

### Sparse Generator Matrices (Phase C10.6.2)
Switched from dense `BitMatrix` to `SpBitMatrixDual` for generator storage:
- Memory: 255 MB → 1.6 MB per DVB-T2 Normal matrix (160× reduction)
- Total cache: 1.5 GB → 10 MB for all 12 configs
- Encoding speed: Same O(edges) complexity with sparse matvec_transpose
- Benefits: Enables file caching with reasonable disk usage

### File I/O Design (Phase C10.6.3)
File-based caching eliminates preprocessing entirely:
- **Format**: Binary COO (edge list) from gf2-core
- **Naming**: `n{n}_k{k}_h{hash}.gf2` for unique identification
- **Loading**: `SpBitMatrixDual::load_from_file()` + wrap in `RuEncodingMatrices`
- **Saving**: Extract generator with `matrices.generator().save_to_file()`
- **Philosophy**: Completely opt-in, no forced global state

## References

- [DVB_T2_VERIFICATION_PLAN.md](DVB_T2_VERIFICATION_PLAN.md) - Full implementation plan
- [DVB_test_vectors.md](DVB_test_vectors.md) - Test vector format specification
- [SYSTEMATIC_ENCODING_CONVENTION.md](SYSTEMATIC_ENCODING_CONVENTION.md) - Bit ordering conventions
- ETSI EN 302 755 - DVB-T2 standard specification
- `examples/ldpc_cache_file_io.rs` - File cache demonstration
- `examples/ldpc_encoding_with_cache.rs` - Memory cache demonstration
