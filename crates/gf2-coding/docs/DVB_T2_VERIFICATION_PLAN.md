# DVB-T2 FEC Verification Plan

## Overview

This document defines the implementation plan for DVB-T2 BCH and LDPC verification using official DVB Project test vectors. The test vectors should be placed in a directory specified by the `$DVB_TEST_VECTORS_PATH` environment variable (not in repository due to copyright restrictions).

## Test Vector Format

### File Structure
- **Location**: `$DVB_TEST_VECTORS_PATH/VV001-CR35_CSP/`
- **Naming**: `<VVReference>_TP<xx>_<company>.txt`
- **Format**: Binary strings (64 bits per line) with comment lines starting with `%` or `#`
- **Organization**: 
  - Frame markers: `# frame N`
  - Block markers: `# block M of K`
  - Data: 64-character binary strings (`0` and `1`)

### VV001-CR35 Configuration
- **Name**: VV001-CR35 (Code Rate 3/5)
- **Frame Size**: NORMAL (64800 bits)
- **Code Rate**: 3/5
- **BCH**: t=12 error correction, 192 parity bits
- **LDPC**: Normal frame parameters
- **Blocks per frame**: 202 blocks (based on file structure)

### Test Point Chain

```
TestPoint4 → BCH Encoding → TestPoint5 → LDPC Encoding → TestPoint6 → Bit Interleaving → TestPoint7a
```

| Test Point | Description | File | Lines | Purpose |
|------------|-------------|------|-------|---------|
| TP04 | BCH input (BBFRAMEs) | `VV001-CR35_TP04_CSP.txt` | 489,674 | BCH encoder input |
| TP05 | BCH output (FECFRAMEs) | `VV001-CR35_TP05_CSP.txt` | 492,098 | BCH encoder output, LDPC encoder input |
| TP06 | LDPC output | `VV001-CR35_TP06_CSP.txt` | 819,338 | LDPC encoder output |
| TP07a | Bit interleaved output | `VV001-CR35_TP07a_CSP.txt` | 819,338 | Bit interleaver output |

## Implementation Plan

### Phase 1: Test Vector Parser Module (1 day)

**Module**: `tests/test_vectors/mod.rs`

#### 1.1 Core Parser (`parser.rs`)
```rust
pub struct TestVector {
    pub frame_number: usize,
    pub block_number: usize,
    pub total_blocks: usize,
    pub data: BitVec,
}

pub struct TestVectorFile {
    pub test_point: String,
    pub config: String,
    pub frames: Vec<Vec<TestVector>>, // frames[frame_idx][block_idx]
}

impl TestVectorFile {
    /// Parse test vector file from path
    /// Skips comment lines (% and #), parses binary strings
    pub fn from_file(path: &Path) -> Result<Self, ParseError>;
    
    /// Get all blocks for a specific frame
    pub fn frame(&self, frame_idx: usize) -> &[TestVector];
    
    /// Get total number of frames
    pub fn num_frames(&self) -> usize;
}
```

**Features**:
- Skip comment lines (`%` and `#`)
- Parse frame/block markers
- Concatenate 64-bit lines into `BitVec`
- Validate block count matches declared total
- Error handling for malformed files

#### 1.2 Configuration Parser (`config.rs`)
```rust
pub struct DvbConfig {
    pub name: String,
    pub frame_size: FrameSize,
    pub code_rate: CodeRate,
}

impl DvbConfig {
    /// Parse configuration from VV reference name (e.g., "VV001-CR35")
    pub fn from_reference(reference: &str) -> Result<Self, ConfigError>;
}
```

**Mappings**:
- `CR12` → CodeRate::Rate1_2
- `CR35` → CodeRate::Rate3_5
- `CR23` → CodeRate::Rate2_3
- `CR34` → CodeRate::Rate3_4
- `CR45` → CodeRate::Rate4_5
- `CR56` → CodeRate::Rate5_6

#### 1.3 Test Vector Loader (`loader.rs`)
```rust
pub struct TestVectorSet {
    pub config: DvbConfig,
    pub tp04: Option<TestVectorFile>, // BCH input
    pub tp05: Option<TestVectorFile>, // BCH output / LDPC input
    pub tp06: Option<TestVectorFile>, // LDPC output
    pub tp07a: Option<TestVectorFile>, // Bit interleaved
}

impl TestVectorSet {
    /// Load all test points for a configuration
    pub fn load(base_path: &Path, reference: &str) -> Result<Self, LoadError>;
    
    /// Load single test point
    pub fn load_test_point(base_path: &Path, reference: &str, tp: u8) 
        -> Result<TestVectorFile, LoadError>;
}
```

**Error Handling**:
- Missing files (graceful degradation)
- Parse errors with line numbers
- Configuration mismatches

### Phase 2: BCH Verification Harness (1 day)

**Module**: `tests/dvb_t2_bch_verification.rs`

#### 2.1 Test Point 4 → Test Point 5 (BCH Encoding)

```rust
#[test]
#[ignore]
fn test_bch_encoding_tp04_to_tp05() {
    require_test_vectors!();
    
    let vectors = TestVectorSet::load(
        &test_vectors_path(),
        "VV001-CR35"
    ).expect("Failed to load test vectors");
    
    let config = vectors.config;
    let bch = BchCode::dvb_t2(config.frame_size, config.code_rate);
    let encoder = BchEncoder::new(&bch);
    
    let tp04 = vectors.tp04.expect("TP04 not found");
    let tp05 = vectors.tp05.expect("TP05 not found");
    
    let mut failures = 0;
    let mut successes = 0;
    
    for frame_idx in 0..tp04.num_frames() {
        for (block_idx, input_block) in tp04.frame(frame_idx).iter().enumerate() {
            let expected_output = &tp05.frame(frame_idx)[block_idx];
            
            // Encode
            let encoded = encoder.encode(&input_block.data);
            
            // Compare
            if encoded != expected_output.data {
                failures += 1;
                eprintln!("Frame {}, Block {}: MISMATCH", frame_idx + 1, block_idx + 1);
                // Optional: Show first differing bit
            } else {
                successes += 1;
            }
        }
    }
    
    println!("BCH Encoding: {} successes, {} failures", successes, failures);
    assert_eq!(failures, 0, "BCH encoding validation failed");
}
```

#### 2.2 Test Point 5 → Test Point 4 (BCH Decoding - Error-Free)

```rust
#[test]
#[ignore]
fn test_bch_decoding_tp05_to_tp04_error_free() {
    require_test_vectors!();
    
    // Load TP05 (BCH encoded), decode, compare with TP04 (original input)
    // Tests decoder on error-free codewords
}
```

#### 2.3 BCH Error Correction Validation

```rust
#[test]
#[ignore]
fn test_bch_error_correction() {
    require_test_vectors!();
    
    // Use TP05 as clean codewords
    // Inject 1..12 random errors per codeword
    // Verify decoder recovers original TP04 data
    // Test with multiple error patterns per codeword
}
```

### Phase 3: LDPC Verification Harness (1-2 days)

**Module**: `tests/dvb_t2_ldpc_verification.rs`

#### 3.1 Test Point 5 → Test Point 6 (LDPC Encoding)

```rust
#[test]
#[ignore]
fn test_ldpc_encoding_tp05_to_tp06() {
    require_test_vectors!();
    
    let vectors = TestVectorSet::load(&test_vectors_path(), "VV001-CR35").unwrap();
    let ldpc = LdpcCode::dvb_t2_normal(CodeRate::Rate3_5);
    
    // Note: LDPC encoding requires systematic encoder implementation
    // Currently we only have decoder. This test validates our
    // understanding of the code structure.
    
    let tp05 = vectors.tp05.expect("TP05 not found");
    let tp06 = vectors.tp06.expect("TP06 not found");
    
    // For now: validate structure and dimensions
    for frame_idx in 0..tp05.num_frames() {
        for (block_idx, input_block) in tp05.frame(frame_idx).iter().enumerate() {
            let output_block = &tp06.frame(frame_idx)[block_idx];
            
            // Validate: systematic part of TP06 matches TP05
            assert_eq!(input_block.data.len(), /* k_ldpc */);
            assert_eq!(output_block.data.len(), /* n_ldpc */);
            
            // Check systematic property: first k_ldpc bits should match
            for i in 0..input_block.data.len() {
                assert_eq!(
                    input_block.data.get_bit(i),
                    output_block.data.get_bit(i),
                    "Systematic property violated at frame {}, block {}, bit {}",
                    frame_idx + 1, block_idx + 1, i
                );
            }
        }
    }
}
```

#### 3.2 Test Point 6 → Test Point 5 (LDPC Decoding - Error-Free)

```rust
#[test]
#[ignore]
fn test_ldpc_decoding_tp06_to_tp05_error_free() {
    require_test_vectors!();
    
    let vectors = TestVectorSet::load(&test_vectors_path(), "VV001-CR35").unwrap();
    let ldpc = LdpcCode::dvb_t2_normal(CodeRate::Rate3_5);
    let decoder = LdpcDecoder::new(&ldpc, MaxIterations(50));
    
    let tp05 = vectors.tp05.expect("TP05 not found");
    let tp06 = vectors.tp06.expect("TP06 not found");
    
    let mut failures = 0;
    let mut successes = 0;
    
    for frame_idx in 0..tp06.num_frames() {
        for (block_idx, codeword) in tp06.frame(frame_idx).iter().enumerate() {
            let expected_info = &tp05.frame(frame_idx)[block_idx];
            
            // Convert to LLRs (perfect channel: ±∞)
            let llrs: Vec<Llr> = codeword.data.iter()
                .map(|bit| if bit { Llr::from_prob_one(1.0) } 
                            else { Llr::from_prob_zero(1.0) })
                .collect();
            
            // Decode
            let result = decoder.decode(&llrs);
            
            // Compare information bits
            if result.decoded != expected_info.data {
                failures += 1;
                eprintln!("Frame {}, Block {}: MISMATCH", frame_idx + 1, block_idx + 1);
            } else {
                successes += 1;
            }
        }
    }
    
    println!("LDPC Decoding (error-free): {} successes, {} failures", successes, failures);
    assert_eq!(failures, 0);
}
```

#### 3.3 LDPC Soft-Decision Decoding (AWGN Channel)

```rust
#[test]
#[ignore]
fn test_ldpc_soft_decision_decoding() {
    require_test_vectors!();
    
    // Use TP06 as clean codewords
    // Modulate (BPSK), add AWGN at various SNR levels
    // Demodulate to soft LLRs
    // Decode and compare with TP05
    // Measure BER/FER vs. Eb/N0
}
```

### Phase 4: Integration Tests (1 day)

**Module**: `tests/dvb_t2_integration.rs`

#### 4.1 Full FEC Chain Validation

```rust
#[test]
#[ignore]
fn test_full_fec_chain_tp04_to_tp06() {
    require_test_vectors!();
    
    // TP04 → BCH encode → (validate TP05) → LDPC encode → (validate TP06)
    // End-to-end encoding validation
}

#[test]
#[ignore]
fn test_full_fec_chain_tp06_to_tp04() {
    require_test_vectors!();
    
    // TP06 → LDPC decode → (validate TP05) → BCH decode → (validate TP04)
    // End-to-end decoding validation (error-free)
}
```

#### 4.2 Round-Trip Tests

```rust
#[test]
#[ignore]
fn test_roundtrip_with_errors() {
    require_test_vectors!();
    
    // TP04 → BCH → LDPC → add errors → LDPC decode → BCH decode → compare TP04
    // Tests error correction capability of full chain
}
```

### Phase 5: Test Infrastructure (1 day)

#### 5.1 Test Configuration

**File**: `tests/test_vectors/config.toml` (not implemented - using environment variable instead)

#### 5.2 Environment Variables

```bash
# Set test vector location
export DVB_TEST_VECTORS_PATH=/path/to/your/dvb_test_vectors
```

#### 5.3 Test Helper Functions

```rust
/// Get test vector base path from environment or default location
fn test_vectors_path() -> PathBuf {
    env::var("DVB_TEST_VECTORS_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from(env::var("HOME").unwrap())
                .join("dvb_test_vectors")
        })
}

/// Check if test vectors are available
fn test_vectors_available() -> bool {
    test_vectors_path().join("VV001-CR35_CSP").exists()
}

/// Skip test with helpful message if vectors not found
macro_rules! require_test_vectors {
    () => {
        if !test_vectors_available() {
            eprintln!("Skipping test: DVB test vectors not found at {:?}", test_vectors_path());
            eprintln!("Set DVB_TEST_VECTORS_PATH environment variable to the test vectors directory");
            return;
        }
    };
}
```

#### 5.4 Reporting

```rust
pub struct VerificationReport {
    pub test_point_pair: String,
    pub total_blocks: usize,
    pub successes: usize,
    pub failures: usize,
    pub mismatched_blocks: Vec<(usize, usize)>, // (frame, block)
}

impl VerificationReport {
    pub fn print_summary(&self);
    pub fn save_to_file(&self, path: &Path);
    pub fn compare_blocks(&self, expected: &BitVec, actual: &BitVec) 
        -> Vec<usize>; // Indices of differing bits
}
```

## Test Execution Strategy

### Local Development
```bash
# Set environment variable
export DVB_TEST_VECTORS_PATH=/path/to/your/dvb_test_vectors

# Run all verification tests
cargo test --test 'dvb_t2_*' -- --ignored

# Run specific test point validation
cargo test --test dvb_t2_bch_verification test_bch_encoding_tp04_to_tp05 -- --ignored

# Generate verification report
cargo test --test dvb_t2_integration -- --ignored --nocapture > verification_report.txt

# Normal test run (validation tests skipped)
cargo test
```

### CI/CD
- Validation tests are `#[ignore]` and only run when explicitly requested
- Tests check environment variable and skip gracefully if vectors not available
- CI can optionally download test vectors from secure storage and run with `-- --ignored`
- Generate verification reports as artifacts

## Success Criteria

### Phase 1 (Parser)
- ✅ Successfully parse all VV001-CR35 test point files
- ✅ Correctly identify 202 blocks per frame
- ✅ Handle comment lines and frame/block markers
- ✅ Convert binary strings to `BitVec`

### Phase 2 (BCH)
- ✅ 100% match on TP04 → TP05 (BCH encoding)
- ✅ 100% match on TP05 → TP04 (BCH decoding, error-free)
- ✅ Correct error correction up to t=12 errors per codeword

### Phase 3 (LDPC)
- ✅ Validate systematic property (TP06 contains TP05 in first k bits)
- ✅ 100% match on TP06 → TP05 (LDPC decoding, error-free)
- ✅ Successful soft-decision decoding at expected SNR thresholds

### Phase 4 (Integration)
- ✅ Full FEC chain validates against all test points
- ✅ Round-trip tests pass with and without errors
- ✅ Performance meets DVB-T2 requirements

## Additional Configurations

Once VV001-CR35 validation passes, expand to:
- VV002-CR12 (Rate 1/2, Normal)
- Short frame configurations
- Other code rates (2/3, 3/4, 4/5, 5/6)

## Dependencies

### Existing Code
- `gf2_coding::bch::BchCode::dvb_t2()`
- `gf2_coding::ldpc::LdpcCode::dvb_t2_normal()`
- `gf2_coding::traits::{BlockEncoder, HardDecisionDecoder, SoftDecoder}`
- `gf2_core::BitVec`

### New Requirements
- LDPC systematic encoder (currently only decoder exists)
- Bit interleaver (for TP06 → TP07a validation, Phase 6)

## Estimated Effort

| Phase | Task | Effort |
|-------|------|--------|
| 1 | Test Vector Parser | 1 day |
| 2 | BCH Verification | 1 day |
| 3 | LDPC Verification | 1-2 days |
| 4 | Integration Tests | 1 day |
| 5 | Test Infrastructure | 1 day |
| **Total** | | **5-6 days** |

## Future Work

- **Phase 6**: Bit interleaver verification (TP06 → TP07a)
- **Phase 7**: Additional configurations (all code rates, short frames)
- **Phase 8**: Performance benchmarking (throughput, latency)
- **Phase 9**: FER curve generation and comparison with reference
- **Phase 10**: Hardware acceleration validation (if applicable)

## References

- ETSI EN 302 755: "Digital Video Broadcasting (DVB); Frame structure channel coding and modulation for a second generation digital terrestrial television broadcasting system (DVB-T2)"
- DVB Project Verification & Validation Test Vectors (copyright © DVB 2010)
- Test vector documentation: `docs/DVB_test_vectors.md`
