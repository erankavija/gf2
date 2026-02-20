# gf2-coding

Error-correcting code implementations and coding theory primitives built on `gf2-core`.

`gf2-coding` provides implementations of linear block codes (Hamming, BCH, LDPC), convolutional codes with Viterbi decoding, and soft-decision decoding infrastructure for AWGN channel simulation.

## Quick Start

New to error correction? Start with the Hamming code example below, then explore the examples organized by difficulty level.

### Installation

```toml
[dependencies]
gf2-core = "0.1"
gf2-coding = "0.1"
```

### Example: Hamming(7,4) Error Correction

```rust
use gf2_coding::{LinearBlockCode, SyndromeTableDecoder};
use gf2_coding::traits::{BlockEncoder, HardDecisionDecoder};
use gf2_core::BitVec;

// Create Hamming(7,4) code: 4 data bits → 7 bits with 1-bit error correction
let code = LinearBlockCode::hamming(3);
let decoder = SyndromeTableDecoder::new(code.clone());

// Encode a 4-bit message
let msg = BitVec::from_bytes_le(&[0b1010]);
let codeword = code.encode(&msg);
assert_eq!(codeword.len(), 7);

// Simulate transmission error
let mut received = codeword.clone();
received.set(2, !received.get(2));  // Flip one bit

// Decode and correct
let decoded = decoder.decode(&received);
assert_eq!(decoded, msg);  // Error corrected!
```

Run `cargo run --example hamming_basic` for a complete walkthrough.

## Core Concepts

### Block Codes vs Streaming Codes

- **Block codes** (Hamming, BCH, LDPC): Encode fixed-length messages → fixed-length codewords
  - Use: Data storage, packetized communication, DVB-T2 broadcast
- **Streaming codes** (Convolutional): Process bits one-at-a-time with internal state
  - Use: Real-time communications, satellite links, Viterbi decoding

### Hard-Decision vs Soft-Decision Decoding

- **Hard-decision**: Binary input (0 or 1) → simpler, faster
  - Example: Syndrome table decoder for Hamming codes
- **Soft-decision**: Probabilistic input (LLRs) → better error correction (1-3 dB gain)
  - Example: LDPC belief propagation with channel reliability

### Which Code Should I Use?

| Need | Code Type | Example |
|------|-----------|---------|
| Simple error correction (1-2 bits) | **Hamming** | Storage checksums, simple comms |
| Moderate errors (10-12 bits) | **BCH** | DVB-T2 outer code, flash memory |
| High performance near Shannon limit | **LDPC** | DVB-T2 inner code, 5G NR, WiFi 6 |
| Streaming/real-time | **Convolutional** | Satellite links, deep space (NASA) |

## Usage by Experience Level

### Beginner: Your First Error-Correcting Code

Learn the basics of encoding, decoding, and error correction with Hamming(7,4):

```rust
use gf2_coding::{LinearBlockCode, SyndromeTableDecoder};
use gf2_coding::traits::{BlockEncoder, HardDecisionDecoder};
use gf2_core::BitVec;

let code = LinearBlockCode::hamming(3); // Create Hamming(7,4)
let decoder = SyndromeTableDecoder::new(code.clone());

// Encode 4-bit message → 7-bit codeword
let message = BitVec::from_bytes_le(&[0b1101]); 
let codeword = code.encode(&message);

// Introduce error and correct
let mut received = codeword.clone();
received.set(0, !received.get(0)); // Flip bit 0
let corrected = decoder.decode(&received);
assert_eq!(corrected, message); // ✓ Corrected!
```

**Examples**: `hamming_basic`, `block_code_intro`  
**API docs**: [`LinearBlockCode`](https://docs.rs/gf2-coding/latest/gf2_coding/struct.LinearBlockCode.html), [`SyndromeTableDecoder`](https://docs.rs/gf2-coding/latest/gf2_coding/struct.SyndromeTableDecoder.html)

### Intermediate: Real-World Applications

Build DVB-T2 digital TV codes and simulate transmission over noisy channels:

```rust
use gf2_coding::ldpc::LdpcCode;
use gf2_coding::{CodeRate, AwgnChannel, BpskModulator};
use gf2_core::BitVec;

// DVB-T2 LDPC code: 32,400 data bits → 64,800 coded bits
let code = LdpcCode::dvb_t2_normal(CodeRate::Rate1_2);
assert_eq!(code.k(), 32400);
assert_eq!(code.n(), 64800);

// Verify zero codeword
let zero_cw = BitVec::zeros(64800);
assert!(code.is_valid_codeword(&zero_cw));

// Simulate AWGN channel (see ldpc_awgn example for full pipeline)
let channel = AwgnChannel::new(3.0, 0.5); // Eb/N0 = 3dB, rate 0.5
```

**Examples**: `dvb_t2_ldpc_basic`, `ldpc_awgn`, `qc_ldpc_demo`, `ldpc_cache_file_io`  
**API docs**: [`LdpcCode`](https://docs.rs/gf2-coding/latest/gf2_coding/ldpc/struct.LdpcCode.html), [`AwgnChannel`](https://docs.rs/gf2-coding/latest/gf2_coding/channel/struct.AwgnChannel.html)  
**Guides**: [DVB_T2.md](docs/DVB_T2.md), [LDPC_PERFORMANCE.md](docs/LDPC_PERFORMANCE.md)

### Advanced: Performance Optimization

High-performance encoding with caching and parallel decoding:

- **SIMD acceleration**: 256-512× faster matrix operations (enabled by default)
- **Generator matrix caching**: Save preprocessing results to disk (13min → <16ms load time)
- **Parallel decoding**: Batch decode multiple frames with Rayon

**Examples**: `hamming_7_4` (comprehensive tutorial), `nasa_rate_half_k3` (Viterbi decoding), `ldpc_encoding_with_cache`  
**Guides**: [SIMD_PERFORMANCE_GUIDE.md](docs/SIMD_PERFORMANCE_GUIDE.md), [PARALLELIZATION.md](docs/PARALLELIZATION.md)

## Supported Codes

### Block Codes

| Code Family | Parameters | Error Correction | Applications |
|-------------|------------|------------------|--------------|
| **Hamming** | (2^r-1, 2^r-r-1) | 1-bit | Simple ECC, educational |
| **BCH** | (n, k, t) over GF(2^m) | t-bit algebraic | DVB-T2 outer, flash memory |
| **LDPC** | (n, k) sparse | Near Shannon limit | DVB-T2 inner, 5G NR, WiFi 6 |

### Streaming Codes

| Code Family | Parameters | Decoding | Applications |
|-------------|------------|----------|--------------|
| **Convolutional** | (n, k, K) | Viterbi | Satellite, deep space |

### Standards Compliance

- **DVB-T2** (ETSI EN 302 755): LDPC inner + BCH outer codes
- **5G NR**: Quasi-cyclic LDPC framework
- **NASA/CCSDS**: Convolutional code generator polynomials

## Performance

### SIMD Acceleration (Enabled by Default)

LDPC preprocessing uses `gf2-core`'s optimized RREF with:
- **Word-level operations**: 64× faster than bit-level
- **AVX2/AVX512 SIMD**: Additional 4-8× speedup
- **Total**: 256-512× faster than naive Gaussian elimination

```bash
# Disable SIMD if needed
cargo build --no-default-features
```

See [SIMD_PERFORMANCE_GUIDE.md](docs/SIMD_PERFORMANCE_GUIDE.md) for details.

### Parallel Processing (Opt-in)

Batch decode multiple frames in parallel with Rayon:

```bash
# Benchmark with different thread counts
RAYON_NUM_THREADS=1 cargo bench --bench quick_parallel --features parallel
RAYON_NUM_THREADS=8 cargo bench --bench quick_parallel --features parallel

# Automated scaling test
./benchmark_quick.sh
```

See [PARALLELIZATION.md](docs/PARALLELIZATION.md) for details.

## Examples by Difficulty

### Beginner (Start Here)

| Example | Concepts | Runtime |
|---------|----------|---------|
| `hamming_basic` | Encoding, decoding, 1-bit correction | <1s |
| `block_code_intro` | Generator matrices, systematic codes | <1s |
| `awgn_uncoded` | Channel simulation, BER measurement | <1s |
| `dvb_t2_ldpc_basic` | LDPC construction, code validation | <1s |

### Intermediate

| Example | Concepts | Runtime |
|---------|----------|---------|
| `qc_ldpc_demo` | Quasi-cyclic LDPC codes | <1s |
| `ldpc_awgn` | Belief propagation, soft decoding | 5-10s |
| `llr_operations` | Soft-decision LLR ops, min-sum | <1s |
| `ldpc_cache_file_io` | File-based caching, performance | <1s |
| `ldpc_encoding_with_cache` | Cached encoder creation | <1s |
| `generator_from_parity_check` | Matrix algebra, Gaussian elim | <1s |
| `visualize_large_matrices` | Matrix visualization (PNG export) | 2-5s |
| `dvb_t2_bch_demo` | BCH algebraic decoding (in development) | <1s |

### Advanced (Deep Dives)

| Example | Concepts | Runtime |
|---------|----------|---------|
| `hamming_7_4` | Complete tutorial with BSC simulation | <1s |
| `nasa_rate_half_k3` | Convolutional codes, Viterbi decoding | <1s |

Run an example: `cargo run --example hamming_basic`

## Utility Binaries

Pre-compute generator matrices for faster LDPC encoding:

```bash
# Generate cache files (~530 MB, one-time 13min preprocessing)
cargo run --release --bin generate_ldpc_cache all

# Validate cache integrity with error correction tests
cargo run --release --bin validate_ldpc_cache

# Quick encoding sanity check
cargo run --bin check_encoding
```

Cached encoders load in <16ms (vs 13 minutes preprocessing).

## Testing

```bash
# Run all tests
cargo test

# Run with property-based tests
cargo test --features proptest

# Run doc tests only
cargo test --doc

# Run examples (verifies they compile and run)
cargo build --examples
```

## Documentation

- **API Reference**: `cargo doc --no-deps --open` or [docs.rs](https://docs.rs/gf2-coding)
- **Specialized Guides**: See [`docs/`](docs/) directory
  - [DVB_T2.md](docs/DVB_T2.md) - DVB-T2 implementation and verification
  - [SIMD_PERFORMANCE_GUIDE.md](docs/SIMD_PERFORMANCE_GUIDE.md) - SIMD optimization
  - [PARALLELIZATION.md](docs/PARALLELIZATION.md) - Parallel processing
  - [LDPC_PERFORMANCE.md](docs/LDPC_PERFORMANCE.md) - Benchmarks and profiling
  - [SDR_INTEGRATION.md](docs/SDR_INTEGRATION.md) - Software-defined radio usage
- **Workspace README**: [../../README.md](../../README.md) - Project overview and roadmap

## Contributing

Contributions welcome! When adding features:
- Write tests first (TDD)
- Add rustdoc with examples for public APIs
- Update README if adding major functionality
- Follow [workspace guidelines](../../README.md#contributing)

## License

MIT OR Apache-2.0
