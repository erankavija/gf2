# gf2-coding

`gf2-coding` provides error-correcting code implementations and coding theory primitives built on `gf2-core`. It includes linear block codes, convolutional codes with Viterbi decoding, and shared traits for experimentation.

## Highlights

- **Linear block codes** with generator/parity matrices and syndrome decoding
- **Hamming codes** with efficient syndrome table decoder
- **Convolutional codes** with shift-register encoder and Viterbi decoder
- **Soft-decision LLR operations** for LDPC and turbo codes (exact and min-sum variants)
- **AWGN channel simulation** with BPSK modulation for BER/FER analysis
- **Streaming API** for both block and convolutional codes
- **Property-based tests** ensuring correctness across random inputs
- **Educational examples** with comprehensive documentation and mathematical formulas

## Features

### Block Codes
- Systematic encoding with generator matrix G
- Syndrome computation with parity-check matrix H
- Maximum-likelihood decoding for Hamming codes
- Support for Hamming(2^r-1, 2^r-r-1) codes up to r=10

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
- AWGN channel model with configurable Eb/N0

### LDPC Codes
- Sparse parity-check matrix representation (using gf2-core SparseMatrixDual)
- Regular LDPC code construction
- Belief propagation decoder with min-sum approximation
- Iterative soft-decision decoding with early stopping
- Syndrome-based convergence detection

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

## Examples

Run the educational examples:

```bash
# Hamming(7,4) code demonstration
cargo run --example hamming_7_4

# NASA convolutional code tutorial with error correction
cargo run --example nasa_rate_half_k3

# LLR operations for LDPC/turbo codes
cargo run --example llr_operations

# LDPC-coded transmission over AWGN (belief propagation)
cargo run --example ldpc_awgn --release

# Uncoded AWGN transmission baseline
cargo run --example awgn_uncoded
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
