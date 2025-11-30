# gf2-coding

`gf2-coding` provides error-correcting code implementations and coding theory primitives built on `gf2-core`. It includes linear block codes, convolutional codes with Viterbi decoding, and shared traits for experimentation.

## Highlights

- **Linear block codes** with generator/parity matrices and syndrome decoding
- **Hamming codes** with efficient syndrome table decoder
- **BCH codes** with algebraic decoding over GF(2^m) for DVB-T2 ✅
- **Convolutional codes** with shift-register encoder and Viterbi decoder
- **LDPC codes** with belief propagation decoding over sparse matrices
- **Soft-decision LLR operations** for LDPC and turbo codes (exact and min-sum variants)
- **AWGN channel simulation** with BPSK modulation and Shannon capacity analysis
- **Monte Carlo simulation framework** for BER/FER curve generation
- **Streaming API** for both block and convolutional codes
- **Property-based tests** ensuring correctness across random inputs
- **Educational examples** with comprehensive documentation and mathematical formulas

## Features

### Block Codes
- Systematic encoding with generator matrix G
- Syndrome computation with parity-check matrix H
- **Generator matrix access**: Unified `GeneratorMatrixAccess` trait for all linear codes (lazy, cached)
- Maximum-likelihood decoding for Hamming codes
- Support for Hamming(2^r-1, 2^r-r-1) codes up to r=10
- **BCH codes**: Full algebraic encoder/decoder with Berlekamp-Massey and Chien search
- **DVB-T2 BCH codes** from ETSI EN 302 755 standard ⚠️ (implementation complete, requires verification)
  - Short frames (n=7200-13320) over GF(2^14), t=12 error correction
  - Normal frames (n=32400-54000) over GF(2^16), t=10 or t=12 error correction
  - Generator polynomials from explicit standard tables
  - All 6 code rates supported: 1/2, 3/5, 2/3, 3/4, 4/5, 5/6

### Convolutional Codes  
- Configurable constraint length and code rate
- Hard-decision Viterbi decoding
- Termination support for trellis closure
- Industry-standard generator polynomials (NASA/CCSDS)

### Soft-Decision Decoding
- Log-likelihood ratio (LLR) types with comprehensive operations
- Multi-operand box-plus for LDPC check node updates
- Min-sum approximations: standard, normalized (α), and offset (β)
- Numerical stability helpers for iterative decoding
- Soft-decision decoder traits for single-shot and iterative decoders
- DecoderResult type with convergence and iteration tracking

### Channel Modeling
- AWGN channel with configurable Eb/N0
- BPSK modulation and demodulation
- Shannon capacity calculation for BPSK over AWGN
- Shannon limit computation for target code rates
- Monte Carlo simulation framework with CSV export
- BER/FER curve generation utilities

### LDPC Codes
- Sparse parity-check matrix representation (using gf2-core SparseMatrixDual)
- Regular LDPC code construction
- **DVB-T2 LDPC codes** from ETSI EN 302 755 standard tables ✅
  - Direct sparse construction (not pure quasi-cyclic)
  - Dual-diagonal parity structure for efficient encoding
  - Normal (n=64800) and Short (n=16200) frames
  - All 6 code rates: 1/2, 3/5, 2/3, 3/4, 4/5, 5/6
  - Rate 1/2 Normal frame fully implemented
- **Quasi-cyclic (QC) LDPC framework** for 5G NR, WiFi standards ✅
- Belief propagation decoder with min-sum approximation
- Iterative soft-decision decoding with early stopping
- Syndrome-based convergence detection
- **High-performance preprocessing**: Uses gf2-core's RREF with word-level operations and SIMD acceleration (AVX2/AVX512)
  - Generator matrix computation: 256-512× faster than bit-level Gaussian elimination
  - SIMD enabled by default for optimal performance

## Performance

### SIMD Acceleration

LDPC code preprocessing (generator matrix computation) uses gf2-core's optimized RREF implementation with:
- **Word-level operations**: 64× faster than bit-level
- **SIMD vectorization**: Additional 4-8× speedup with AVX2/AVX512
- **Total speedup**: 256-512× faster than manual Gaussian elimination

SIMD is **enabled by default** for best performance. To build without SIMD:
```bash
cargo build --no-default-features
```

For more details, see [SIMD_PERFORMANCE_GUIDE.md](SIMD_PERFORMANCE_GUIDE.md).

### Parallel Processing

Opt-in parallel processing with rayon:

```bash
# Enable parallel batch operations
cargo build --features parallel

# Benchmark parallel scaling
cargo bench --bench parallel_scaling --features parallel

# Run comprehensive overnight benchmarks
./run_overnight_benchmarks.sh
```

See [docs/PARALLEL_BENCHMARKING.md](docs/PARALLEL_BENCHMARKING.md) for thread configuration and performance analysis.

## Usage

Add to your `Cargo.toml`:

```toml
[dependencies]
gf2-core = "0.1"
gf2-coding = "0.1"
```

### Linear Block Code Example

```rust
use gf2_coding::{LinearBlockCode, SyndromeTableDecoder};
use gf2_coding::traits::{BlockEncoder, HardDecisionDecoder};
use gf2_core::BitVec;

// Hamming(7,4) code
let code = LinearBlockCode::hamming(3);
let decoder = SyndromeTableDecoder::new(code.clone());

let mut msg = BitVec::from_bytes_le(&[0b1011]);
msg.resize(code.k(), false);

// Encode and introduce error
let mut codeword = code.encode(&msg);
codeword.set(2, !codeword.get(2));

// Decode with error correction
let decoded = decoder.decode(&codeword);
assert_eq!(decoded, msg);
```

### Generator Matrix Access Example

All linear block codes (Hamming, BCH, LDPC) support unified generator matrix access:

```rust
use gf2_coding::LinearBlockCode;
use gf2_coding::traits::GeneratorMatrixAccess;

let code = LinearBlockCode::hamming(3);

// Access generator matrix (computed lazily, cached)
let g = code.generator_matrix();
assert_eq!(g.rows(), code.k()); // 4
assert_eq!(g.cols(), code.n()); // 7

// Check if code is systematic
assert!(code.is_systematic());
```

Same API works for BCH and LDPC codes. Generator matrices are computed on-demand and cached for subsequent calls, with zero impact on encoding/decoding performance.

### Convolutional Code Example

```rust
use gf2_coding::{ConvolutionalEncoder, ConvolutionalDecoder};
use gf2_coding::traits::{StreamingEncoder, StreamingDecoder};

// NASA rate-1/2, K=3 encoder (generators: [7, 5] octal)
let mut encoder = ConvolutionalEncoder::new(3, vec![0b111, 0b101]);
let mut decoder = ConvolutionalDecoder::new(3, vec![0b111, 0b101]);

encoder.reset();
decoder.reset();

// Encode message
let message = vec![true, false, true, true];
let mut codeword = Vec::new();
for &bit in &message {
    codeword.extend(encoder.encode_bit(bit));
}

// Terminate with K-1 zeros
for _ in 0..2 {
    codeword.extend(encoder.encode_bit(false));
}

// Decode
let decoded = decoder.decode_symbols(&codeword);
assert_eq!(&decoded[..message.len()], &message[..]);
```

### DVB-T2 LDPC Example

```rust
use gf2_coding::ldpc::LdpcCode;
use gf2_coding::CodeRate;
use gf2_core::BitVec;

// Create DVB-T2 normal frame, rate 1/2 LDPC code
let code = LdpcCode::dvb_t2_normal(CodeRate::Rate1_2);
assert_eq!(code.n(), 64800);  // Codeword length
assert_eq!(code.k(), 32400);  // Information bits
assert_eq!(code.rate(), 0.5);

// Verify zero codeword (all codes contain all-zeros)
let zero = BitVec::zeros(64800);
assert!(code.is_valid_codeword(&zero));
```

DVB-T2 codes are constructed directly from ETSI EN 302 755 standard tables with dual-diagonal parity structure. Currently, only Normal Rate 1/2 is fully implemented; other configurations require table data entry.

## Examples

Run the educational examples:

```bash
# Hamming(7,4) code demonstration
cargo run --example hamming_7_4

# NASA convolutional code tutorial with error correction
cargo run --example nasa_rate_half_k3

# DVB-T2 BCH outer codes (algebraic decoding)
# (Note: Requires verification against reference implementation)
cargo run --example dvb_t2_bch_demo

# DVB-T2 LDPC codes from standard tables
cargo run --example dvb_t2_ldpc_basic

# Quasi-cyclic LDPC codes (5G NR foundation)
cargo run --example qc_ldpc_demo

# LLR operations for LDPC/turbo codes
cargo run --example llr_operations

# LDPC-coded transmission over AWGN (belief propagation)
cargo run --example ldpc_awgn --release

# Uncoded AWGN transmission baseline
cargo run --example awgn_uncoded

# Visualize large generator matrices (>500 rows/cols) as PNG images
cargo run --example visualize_large_matrices --features visualization
```

## Utility Binaries

The crate includes utility binaries in `src/bin/`:

```bash
# Generate LDPC encoding cache files (DVB-T2)
# Creates ~530 MB of pre-computed generator matrices
# Run once, then load in <16ms (vs 13 minutes preprocessing)
cargo run --release --bin generate_ldpc_cache all

# Validate LDPC cache with error correction tests
# Tests encoding/decoding with various error counts
cargo run --release --bin validate_ldpc_cache

# Quick encoding sanity check
# Verifies encoder produces valid codewords
cargo run --bin check_encoding
```

## Testing

```bash
# Run all tests
cargo test

# Run with property-based tests
cargo test --features proptest
```

## Documentation

See the [workspace README](../../README.md) for:
- API overview and design principles
- Testing and benchmarking guidelines
- Development roadmap

For detailed API documentation:
```bash
cargo doc --no-deps --open
```
