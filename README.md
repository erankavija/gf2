# gf2 - High-Performance Bit String Manipulation

[![CI](https://github.com/erankavija/gf2/workflows/CI/badge.svg)](https://github.com/erankavija/gf2/actions)

A high-performance Rust library for bit string manipulation with a focus on GF(2) operations and coding theory applications.

## Overview

`gf2` provides efficient dense bit vector operations optimized for:
- Basic bitset operations (AND, OR, XOR, NOT)
- Bit manipulation and queries
- **GF(2) linear algebra**: Fast matrix operations over the binary field
- Future: GF(2) polynomial arithmetic for coding theory
- Future: SIMD acceleration on x86-64 (AVX2/AVX-512) and AArch64 (NEON)

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

Add this to your `Cargo.toml`:

```toml
[dependencies]
gf2 = "0.1"
```

### Basic Example

```rust
use gf2::BitVec;

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
```

### Byte Conversion

```rust
use gf2::BitVec;

// Create from bytes (little-endian)
let bv = BitVec::from_bytes_le(&[0xAA, 0x55]);
assert_eq!(bv.len(), 16);

// Convert back to bytes
let bytes = bv.to_bytes_le();
assert_eq!(bytes, vec![0xAA, 0x55]);
```

### Bit Queries

```rust
use gf2::BitVec;

let bv = BitVec::from_bytes_le(&[0b00001100]);

// Find set bits
assert_eq!(bv.find_first_set(), Some(2));
assert_eq!(bv.find_last_set(), Some(3));

// Count set bits
assert_eq!(bv.count_ones(), 2);
```

### Shifts

```rust
use gf2::BitVec;

let mut bv = BitVec::from_bytes_le(&[0b00001111]);
bv.shift_left(2);
assert_eq!(bv.to_bytes_le(), vec![0b00111100]);
```

### Matrix Operations over GF(2)

```rust
use gf2::matrix::BitMatrix;
use gf2::alg::m4rm::multiply;
use gf2::alg::gauss::invert;

// Create a 3x3 matrix
let mut a = BitMatrix::new_zero(3, 3);
a.set(0, 0, true);
a.set(0, 1, true);
a.set(1, 1, true);
a.set(2, 2, true);

// Create identity matrix
let i = BitMatrix::identity(3);

// Matrix multiplication (using M4RM algorithm)
let product = multiply(&a, &i);
// product equals a

// Matrix inversion (using Gauss-Jordan)
let inv = invert(&i).unwrap();
// inv equals i for identity matrix

// Verify: a × a^(-1) = I
let mut b = BitMatrix::new_zero(2, 2);
b.set(0, 0, true);
b.set(0, 1, true);
b.set(1, 0, true);

let b_inv = invert(&b).unwrap();
let should_be_identity = multiply(&b, &b_inv);
assert_eq!(should_be_identity.get(0, 0), true);
assert_eq!(should_be_identity.get(1, 1), true);
assert_eq!(should_be_identity.get(0, 1), false);
```

## API Overview

### BitVec
- `BitVec::new()` - Create empty bit vector
- `BitVec::with_capacity(bits)` - Pre-allocate capacity
- `BitVec::from_bytes_le(&[u8])` - Create from byte slice

### BitMatrix
- `BitMatrix::new_zero(rows, cols)` - Create zero matrix
- `BitMatrix::identity(n)` - Create n×n identity matrix
- `get(r, c)`, `set(r, c, val)` - Access individual bits
- `swap_rows(r1, r2)` - Swap two rows
- `transpose()` - Return transposed matrix

### Matrix Algorithms (over GF(2))
- `multiply(a, b)` - Matrix multiplication using M4RM (Method of the Four Russians)
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

### Phase 4: SIMD Acceleration (Planned)
- **x86-64**: AVX2 (256-bit) and AVX-512 (512-bit) implementations
- **AArch64**: NEON (128-bit) implementation
- Runtime feature detection and dispatch
- Vectorized shifts using shuffle instructions
- VPCLMULQDQ-based row operations for matrices

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

### Testing

Run the full test suite:

```bash
cargo test --all-features
```

Run property-based tests:

```bash
cargo test --test property_tests
```

### Benchmarks

Build and run benchmarks:

```bash
cargo bench
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
