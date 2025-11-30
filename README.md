# gf2 - High-Performance GF(2) Computing

[![CI](https://github.com/erankavija/gf2/workflows/CI/badge.svg)](https://github.com/erankavija/gf2/actions)
[![codecov](https://codecov.io/gh/erankavija/gf2/branch/main/graph/badge.svg)](https://codecov.io/gh/erankavija/gf2)

A Rust workspace for high-performance binary field computing and coding theory, containing two complementary crates:

- **`gf2-core`** - Bit manipulation primitives, GF(2) linear algebra, and extension field GF(2^m) arithmetic
- **`gf2-coding`** - Error-correcting codes and coding theory algorithms built on gf2-core

## Project Goals

- **Performance**: Optimized kernels with SIMD acceleration for throughput-critical operations
- **Correctness**: Comprehensive testing including property-based tests and mathematical validation
- **Education**: Clear documentation with examples demonstrating coding theory concepts
- **Composability**: Clean, functional APIs that hide low-level complexity

## Crate Overview

### gf2-core - Performance Primitives

Low-level building blocks for binary field computing:
- **BitVec/BitMatrix**: Dense bit storage with word-level operations
- **GF(2) linear algebra**: M4RM multiplication, Gauss-Jordan inversion
- **Extension fields GF(2^m)**: Polynomial arithmetic with Karatsuba and SIMD multiplication
- **Primitive polynomials**: Verification, generation, and standard database (m=2..16)
- **Sparse matrices**: CSR/CSC formats for low-density operations
- **Rank/select**: O(1) rank, O(log n) select with lazy indexing
- **Polar transforms**: Fast Hadamard Transform for polar codes
- **SIMD acceleration**: Optional AVX2 kernels via `simd` feature
- **Visualization**: Optional PNG export for matrices via `visualization` feature

See [crates/gf2-core/README.md](crates/gf2-core/README.md) for detailed features and usage.

### gf2-coding - Error-Correcting Codes

Coding theory algorithms for error correction:
- **Block codes**: Hamming codes with syndrome decoding
- **Generator matrix access**: Unified trait for all linear codes (lazy, cached)
- **BCH codes**: Algebraic decoding (Berlekamp-Massey, Chien search)
- **DVB-T2 BCH**: All 12 configurations validated (202/202 blocks match ETSI EN 302 755)
- **Convolutional codes**: Viterbi decoder
- **LDPC codes**: Belief propagation with quasi-cyclic support
- **DVB-T2 LDPC**: All 12 configurations validated (202/202 blocks match test vectors)
- **Parallel batch operations**: BCH and LDPC with rayon support
- **Soft-decision LLR**: Operations for iterative decoding
- **Channel models**: AWGN simulation with BPSK modulation

See [crates/gf2-coding/README.md](crates/gf2-coding/README.md) for detailed features and examples.

## Key Design Principles

- **Functional style at API level**: Immutability and pure functions where practical
- **Imperative kernels for performance**: Low-level code optimized for speed
- **Tail masking invariant**: Padding bits always zeroed for correctness
- **Test-driven development**: Comprehensive unit and property-based tests
- **Safe by default**: `#![deny(unsafe_code)]` at crate level
- **MSRV**: Rust 1.74+

## Quick Start

Add dependencies to your `Cargo.toml`:

```toml
[dependencies]
gf2-core = "0.1"
gf2-coding = "0.1"

# Optional: enable SIMD acceleration
# gf2-core = { version = "0.1", features = ["simd"] }
```

### Basic Example

```rust
use gf2_core::BitVec;
use gf2_core::matrix::BitMatrix;

// Bit vector operations
let mut bv = BitVec::from_bytes_le(&[0b11110000]);
let b = BitVec::from_bytes_le(&[0b11001100]);
bv.bit_xor_into(&b);
assert_eq!(bv.count_ones(), 4);

// Matrix operations over GF(2)
let a = BitMatrix::identity(3);
let b = BitMatrix::zeros(3, 3);
let product = &a * &b;  // M4RM multiplication
```

### Error-Correcting Codes

```rust
use gf2_coding::{LinearBlockCode, SyndromeTableDecoder};
use gf2_coding::traits::{BlockEncoder, HardDecisionDecoder};

// Hamming(7,4) code
let code = LinearBlockCode::hamming(3);
let decoder = SyndromeTableDecoder::new(code.clone());

// Encode and inject error
let message = BitVec::from_bytes_le(&[0b1011]);
let mut codeword = code.encode(&message);
codeword.set(2, !codeword.get(2));  // Flip bit

// Decode with error correction
let decoded = decoder.decode(&codeword);
assert_eq!(decoded, message);
```

For more examples, see the individual crate READMEs and the `examples/` directory.

## Documentation

- **gf2-core API**: [crates/gf2-core/README.md](crates/gf2-core/README.md)
- **gf2-coding API**: [crates/gf2-coding/README.md](crates/gf2-coding/README.md)
- **Development roadmap**: [ROADMAP.md](ROADMAP.md)
- **Full API docs**: Run `cargo doc --no-deps --open`

## Development Roadmap

The project roadmap is divided into strategic goals (this document) and detailed implementation plans (subproject roadmaps):

- **[ROADMAP.md](ROADMAP.md)** - Strategic overview and cross-cutting themes
- **[crates/gf2-core/ROADMAP.md](crates/gf2-core/ROADMAP.md)** - Performance optimization phases
- **[crates/gf2-coding/ROADMAP.md](crates/gf2-coding/ROADMAP.md)** - Coding theory implementations

### Current Status

**gf2-core**: Core primitives feature-complete
- ✅ GF(2^m) extension field arithmetic
- ✅ Karatsuba multiplication and SIMD operations
- ✅ Primitive polynomial verification and generation
- ✅ Sparse matrix primitives (CSR/CSC)
- ✅ Rank/select operations (lazy indexing)
- ✅ Polar transforms (Fast Hadamard Transform)

**gf2-coding**: DVB-T2 FEC validation complete, parallelization active
- ✅ BCH codes: 202/202 blocks validated against ETSI EN 302 755
- ✅ LDPC codes: All 12 configurations validated
- ✅ Parallel batch operations with rayon
- 🔧 CPU parallelization for real-time decoding (50-100 Mbps target)
- 🔮 QAM modulation and full FEC chain (planned)

## Development

### Build and Test

```bash
# Build workspace
cargo build --workspace

# Run all tests
cargo test --workspace --all-features

# Run benchmarks
cargo bench -p gf2-core

# Build documentation
cargo doc --no-deps --open
```

### Examples

Educational examples demonstrating coding theory concepts:

```bash
# Block codes
cargo run -p gf2-coding --example hamming_7_4

# Convolutional codes with Viterbi decoding
cargo run -p gf2-coding --example nasa_rate_half_k3

# DVB-T2 BCH outer codes (algebraic decoding)
# Note: Verification against reference implementation pending
cargo run -p gf2-coding --example dvb_t2_bch_demo

# DVB-T2 LDPC codes from standard tables
cargo run -p gf2-coding --example dvb_t2_ldpc_basic

# Quasi-cyclic LDPC construction
cargo run -p gf2-coding --example qc_ldpc_demo

# LDPC codes over AWGN
cargo run -p gf2-coding --example ldpc_awgn --release
```

## Contributing

Contributions welcome in these areas:
- SIMD implementations (AVX-512, NEON)
- Coding theory algorithms
- Performance optimizations
- Documentation and examples

See the roadmaps in [ROADMAP.md](ROADMAP.md) and the subproject directories for specific tasks.

## License

To be determined. See repository for license information.
