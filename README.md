# gf2 Workspace - GF(2) Primitives and Coding Theory

[![CI](https://github.com/erankavija/gf2/workflows/CI/badge.svg)](https://github.com/erankavija/gf2/actions)
[![codecov](https://codecov.io/gh/erankavija/gf2/branch/main/graph/badge.svg)](https://codecov.io/gh/erankavija/gf2)

A high-performance Rust workspace that houses two complementary crates:

- `gf2-core`: low-level, high-throughput primitives for bit manipulation and GF(2) linear algebra.
- `gf2-coding`: research-oriented coding theory and compression algorithms built on `gf2-core`.

The shared workspace keeps primitives and applications aligned while making their goals explicit.

## Crates

- **`gf2-core`** – dense `BitVec` and `BitMatrix` types, optimized kernels, and linear algebra algorithms over GF(2).
- **`gf2-coding`** – higher-level encoders, decoders, and experiments that leverage the `gf2-core` building blocks.

## Overview

`gf2-core` provides efficient dense bit vector operations optimized for:
- Basic bitset operations (AND, OR, XOR, NOT)
- Bit manipulation and queries
- **GF(2) linear algebra**: Fast matrix operations over the binary field
- Future: GF(2) polynomial arithmetic for coding theory
- Future: SIMD acceleration on x86-64 (AVX2/AVX-512) and AArch64 (NEON)

The `gf2-coding` crate layers domain-specific constructions (e.g., Hamming codes, convolutional encoders) on top of these primitives, keeping experimental code separated from the performance-critical core.

## Features

- **Zero-cost abstractions**: Thin wrapper over `Vec<u64>` with no runtime overhead
- **Memory efficient**: Dense storage with 64-bit words
- **GF(2) matrices**: Bit-packed boolean matrices with M4RM multiplication and Gauss-Jordan inversion
- **Well-tested**: Comprehensive unit tests and property-based testing with `proptest`
- **Safe by default**: `#![deny(unsafe_code)]` at crate level
- **MSRV**: Rust 1.74+

## Design Invariants

### Storage Model
- Bits are stored in contiguous `Vec<u64>` words
- Little-endian bit numbering within each word
- Bit `i` maps to `word = i >> 6`, `mask = 1u64 << (i & 63)`

### Tail Masking
Padding bits beyond `len_bits` in the last word are always zeroed. This invariant is maintained by all mutating operations to ensure:
- Consistent behavior across operations
- Correct population counts
- Proper equality comparisons

## Usage

### Add dependencies

```toml
[dependencies]
gf2-core = "0.1"
# Optional, for coding theory algorithms built on gf2-core:
# gf2-coding = "0.1"

# Optional: enable SIMD acceleration (AVX2/AVX-512 on x86-64)
# gf2-core = { version = "0.1", features = ["simd"] }
```

When working inside this repository, prefer workspace-relative paths:

```toml
[dependencies]
gf2-core = { path = "crates/gf2-core" }
gf2-coding = { path = "crates/gf2-coding" }

# Optional: enable SIMD features
# gf2-core = { path = "crates/gf2-core", features = ["simd"] }
# gf2-coding = { path = "crates/gf2-coding", features = ["simd"] }
```

### Enabling SIMD Acceleration

SIMD acceleration is available as an optional feature:

```bash
# Build with SIMD enabled
cargo build --features simd

# Test with SIMD
cargo test --features simd

# Benchmark with SIMD
cargo bench --features simd
```

SIMD provides runtime CPU detection and automatically uses AVX2/AVX-512 instructions when available, falling back to scalar code otherwise.

### gf2-core: Basic example

```rust
use gf2_core::BitVec;

// Create and manipulate bit vectors
let mut bv = BitVec::new();
bv.push_bit(true);
bv.push_bit(false);
bv.push_bit(true);

assert_eq!(bv.len(), 3);
assert_eq!(bv.get(0), true);
assert_eq!(bv.count_ones(), 2);

// Bitwise operations
let mut a = BitVec::from_bytes_le(&[0b11110000]);
let b = BitVec::from_bytes_le(&[0b11001100]);
a.bit_xor_into(&b);
assert_eq!(a.to_bytes_le(), vec![0b00111100]);

// Pretty printing (nalgebra-like display)
println!("{}", bv);  // Displays: [ 1 0 1 ]
```

### gf2-core: Byte conversion

```rust
use gf2_core::BitVec;

// Create from bytes (little-endian)
let bv = BitVec::from_bytes_le(&[0xAA, 0x55]);
assert_eq!(bv.len(), 16);

// Convert back to bytes
let bytes = bv.to_bytes_le();
assert_eq!(bytes, vec![0xAA, 0x55]);
```

### gf2-core: Bit queries

```rust
use gf2_core::BitVec;

let bv = BitVec::from_bytes_le(&[0b00001100]);

// Find set bits
assert_eq!(bv.find_first_set(), Some(2));
assert_eq!(bv.find_last_set(), Some(3));

// Count set bits
assert_eq!(bv.count_ones(), 2);
```

### gf2-core: Shifts

```rust
use gf2_core::BitVec;

let mut bv = BitVec::from_bytes_le(&[0b00001111]);
bv.shift_left(2);
assert_eq!(bv.to_bytes_le(), vec![0b00111100]);
```

### gf2-core: Matrix operations over GF(2)

```rust
use gf2_core::matrix::BitMatrix;
use gf2_core::alg::gauss::invert;

// Create a 3x3 matrix
let mut a = BitMatrix::zeros(3, 3);
a.set(0, 0, true);
a.set(0, 1, true);
a.set(1, 1, true);
a.set(2, 2, true);

// Create identity matrix
let i = BitMatrix::identity(3);

// Matrix multiplication using the * operator (M4RM algorithm)
let product = &a * &i;
// product equals a

// Matrix inversion (using Gauss-Jordan)
let inv = invert(&i).unwrap();
// inv equals i for identity matrix

// Verify: a × a^(-1) = I
let mut b = BitMatrix::zeros(2, 2);
b.set(0, 0, true);
b.set(0, 1, true);
b.set(1, 0, true);

let b_inv = invert(&b).unwrap();
let should_be_identity = &b * &b_inv;
assert_eq!(should_be_identity.get(0, 0), true);
assert_eq!(should_be_identity.get(1, 1), true);
assert_eq!(should_be_identity.get(0, 1), false);

// Pretty printing (nalgebra-like display)
println!("{}", a);
// Displays:
//   ┌       ┐
//   │ 1 1 0 │
//   │ 0 1 0 │
//   │ 0 0 1 │
//   └       ┘
```

### gf2-coding: Hamming code workflow

```rust
use gf2_coding::{LinearBlockCode, SyndromeTableDecoder};
use gf2_coding::traits::{BlockEncoder, HardDecisionDecoder};
use gf2_core::BitVec;

// Create the classic Hamming(15, 11) code.
let code = LinearBlockCode::hamming(4);
assert_eq!((code.k(), code.n()), (11, 15));

let decoder = SyndromeTableDecoder::new(code.clone());

// Encode a simple alternating pattern.
let mut message = BitVec::new();
for i in 0..code.k() {
  message.push_bit(i % 2 == 0);
}
let codeword = code.encode(&message);

// Inject a single-bit error and decode.
let mut corrupted = codeword.clone();
corrupted.set(3, !corrupted.get(3));
let decoded = decoder.decode(&corrupted);
assert_eq!(decoded, message);
```

## API Overview

### BitVec
- `BitVec::new()` - Create empty bit vector
- `BitVec::with_capacity(bits)` - Pre-allocate capacity
- `BitVec::from_bytes_le(&[u8])` - Create from byte slice

### BitMatrix
- `BitMatrix::zeros(rows, cols)` - Create zero matrix
- `BitMatrix::identity(n)` - Create n×n identity matrix
- `get(r, c)`, `set(r, c, val)` - Access individual bits
- `swap_rows(r1, r2)` - Swap two rows
- `transpose()` - Return transposed matrix

### Matrix Algorithms (over GF(2))
- `a * b` or `&a * &b` - Infix matrix multiplication using the `*` operator (M4RM algorithm)
- `multiply(a, b)` - Matrix multiplication using M4RM (Method of the Four Russians) - also available via `*` operator
- `invert(m)` - Matrix inversion using Gauss-Jordan elimination (returns `Option<BitMatrix>`)

### BitVec Operations
- `len()`, `is_empty()` - Query size
- `get(idx)`, `set(idx, bit)` - Access individual bits
- `push_bit(bit)`, `pop_bit()` - Stack-like operations

### Bitwise Operations
- `bit_and_into(&other)` - Bitwise AND
- `bit_or_into(&other)` - Bitwise OR  
- `bit_xor_into(&other)` - Bitwise XOR
- `not_into()` - Bitwise NOT

### Shifts
- `shift_left(k)` - Logical left shift
- `shift_right(k)` - Logical right shift

### Queries
- `count_ones()` - Population count (rank)
- `find_first_set()` - Index of first set bit
- `find_last_set()` - Index of last set bit

### Utilities
- `clear()` - Remove all bits
- `resize(new_len, fill_bit)` - Resize with fill value
- `to_bytes_le()` - Convert to byte vector

## Performance Roadmap

### Phase 1: Scalar Baseline ✅ (Current)
- Tight word-level loops with branch minimization
- Optimized shifts with whole-word operations
- Efficient bit scanning with `trailing_zeros`/`leading_zeros`
- **GF(2) linear algebra**: M4RM multiplication, Gauss-Jordan inversion

### Phase 2: Matrix Optimizations (Planned)
- Gray code construction for M4RM tables (currently uses simple loop)
- Cache-oblivious blocking for large matrices
- Optimized transpose with bit-level tricks
- SIMD-accelerated row XOR operations

### Phase 3: Buffer Optimizations (Planned)
- Kernel-based dispatch for large buffers
- Loop unrolling and prefetch hints
- Cache-line aligned operations

### Phase 4: SIMD Acceleration (Partial)
- **x86-64**: AVX2 (256-bit) implementation available via `simd` feature
- AVX-512 (512-bit) implementation (planned)
- **AArch64**: NEON (128-bit) implementation (planned)
- ✅ Runtime feature detection and dispatch
- Vectorized shifts using shuffle instructions (planned)
- VPCLMULQDQ-based row operations for matrices (planned)

### Phase 5: Advanced Bit Operations (Planned)
- Rank/select with superblock/block indexes
- O(1) select using broadword techniques
- Efficient bit scanning primitives

### Phase 6: GF(2) Polynomial Arithmetic (Planned)
- Carry-less multiplication (scalar baseline)
- CLMUL/PCLMULQDQ acceleration on x86-64
- VMULL.P64 on AArch64 with crypto extensions
- Karatsuba and Toom-Cook algorithms
- Convolution-based methods exploration

### Phase 7: Coding Theory Algorithms (Future)
- Generator and parity-check matrix operations
- Syndrome computation
- Decoding primitives
- Separate crate or module organization

## Development

### Examples

Run the Hamming (7,4) error-correcting code example from `gf2-coding`:

```bash
cargo run -p gf2-coding --example hamming_7_4
```

This example demonstrates:
- Creating generator and parity-check matrices for Hamming (7,4) code
- Encoding 4-bit messages into 7-bit codewords
- Detecting and correcting single-bit errors
- Pretty-printing of matrices and bit vectors using nalgebra-like display format

### Testing

Run the full workspace test suite:

```bash
cargo test --workspace --all-features
```

Run property-based tests for `gf2-core`:

```bash
cargo test -p gf2-core --test property_tests
```

### Benchmarks

Build and run benchmarks for `gf2-core`:

```bash
cargo bench -p gf2-core
```

Current benchmarks cover:
- **BitVec operations**:
  - XOR operations (1 KiB, 64 KiB, 1 MiB)
  - Population count (1 KiB, 64 KiB, 1 MiB)
  - Left/right shifts (64 KiB with various shift amounts)
- **Matrix operations**:
  - Square matrix multiplication (64×64 to 1024×1024)
  - Rectangular matrix multiplication (various dimensions)

### Code Quality

Format code:

```bash
cargo fmt
```

Run clippy:

```bash
cargo clippy --all-targets --all-features -- -D warnings
```

Build documentation:

```bash
cargo doc --no-deps --open
```

## Test-Driven Development

This library is developed using TDD principles:
1. Write comprehensive tests first (unit, edge cases, property tests)
2. Implement minimal code to pass tests
3. Refactor while maintaining test coverage
4. Validate with benchmarks

Property-based testing ensures correctness against a simple reference implementation across:
- Random inputs of varying lengths
- Round-trip conversions (bytes ↔ bits)
- Equivalence with naive implementations
- Boundary conditions (0, 1, 63, 64, 65 bits, etc.)

## Contributing

Contributions are welcome! Areas for contribution:
- SIMD implementations (AVX2, AVX-512, NEON)
- Additional bit operations (rank/select, etc.)
- GF(2) polynomial arithmetic
- Performance optimizations
- Documentation improvements

## License

To be determined. See repository for license information.

## Roadmap

See [ROADMAP.md](ROADMAP.md) for detailed development phases and planned features.
