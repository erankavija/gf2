# gf2-core

High-performance bit manipulation and GF(2) linear algebra in Rust.

`gf2-core` provides two core primitives for working with bits and binary matrices: **`BitVec`** for dense bit strings and **`BitMatrix`** for bit-packed matrices with efficient GF(2) operations.

## Core Primitives

### BitVec - Dense Bit Vectors

A growable bit string backed by `Vec<u64>` with word-level operations for performance.

```rust
use gf2_core::BitVec;

// Create and manipulate bit vectors
let mut bv = BitVec::new();
bv.push_bit(true);
bv.push_bit(false);
bv.push_bit(true);

assert_eq!(bv.len(), 3);
assert_eq!(bv.get(0), true);
assert_eq!(bv.get(1), false);
assert_eq!(bv.count_ones(), 2);

// Bitwise operations (in-place, functional style for chaining)
let mut a = BitVec::from_bytes_le(&[0b1010]);
let b = BitVec::from_bytes_le(&[0b1100]);
a.bit_xor_into(&b);  // a = 0b0110

// Shifts and searches
let mut v = BitVec::from_bytes_le(&[0b0001_0000]);
assert_eq!(v.find_first_one(), Some(4));
v.shift_left(2);
assert_eq!(v.find_first_one(), Some(6));
```

**Common operations:**
- `new()`, `zeros(n)`, `ones(n)` - Constructors
- `push_bit()`, `pop_bit()`, `get()`, `set()` - Element access
- `count_ones()`, `parity()` - Population count
- `bit_and_into()`, `bit_or_into()`, `bit_xor_into()`, `not_into()` - Bitwise ops
- `shift_left()`, `shift_right()` - Bit shifting
- `find_first_one()`, `find_first_zero()` - Scanning

### BitMatrix - Bit-Packed Matrices

Row-major, bit-packed boolean matrices for GF(2) linear algebra.

```rust
use gf2_core::BitMatrix;

// Create matrices
let mut m = BitMatrix::zeros(3, 4);
m.set(0, 0, true);
m.set(1, 2, true);
assert_eq!(m.get(0, 0), true);
assert_eq!(m.get(1, 2), true);

// Identity matrix
let id = BitMatrix::identity(4);
assert_eq!(id.get(0, 0), true);
assert_eq!(id.get(0, 1), false);

// Matrix operations
let a = BitMatrix::identity(3);
let b = BitMatrix::ones(3, 3);
let c = &a * &b;  // M4RM multiplication

// Row operations (core GF(2) algebra)
let mut m = BitMatrix::identity(3);
m.row_xor(0, 1);  // row[0] ^= row[1]
m.swap_rows(1, 2);

// Transpose
let t = m.transpose();
assert_eq!(t.rows(), m.cols());
assert_eq!(t.cols(), m.rows());
```

**Common operations:**
- `zeros()`, `identity()`, `ones()` - Constructors
- `get()`, `set()` - Element access
- `rows()`, `cols()` - Dimensions
- `row_xor()`, `swap_rows()` - Row operations
- `transpose()` - Matrix transpose
- `*` operator - Matrix multiplication (M4RM algorithm)
- `row_as_bitvec()`, `col_as_bitvec()` - Extract rows/columns

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
gf2-core = "0.1"

# With SIMD acceleration (3.4-3.6× faster for large operations)
gf2-core = { version = "0.1", features = ["simd"] }

# Minimal build (no random generation)
gf2-core = { version = "0.1", default-features = false }
```

## Quick Start

### 5-Minute BitVec Tutorial

```rust
use gf2_core::BitVec;

// Construction
let mut bv = BitVec::zeros(8);  // 00000000
bv.set(0, true);                 // 00000001
bv.set(7, true);                 // 10000001

// Querying
assert_eq!(bv.len(), 8);
assert_eq!(bv.count_ones(), 2);
assert_eq!(bv.find_first_one(), Some(0));
assert_eq!(bv.find_last_one(), Some(7));

// Bitwise operations (in-place)
let mut a = BitVec::from_bytes_le(&[0b1010]);
let b = BitVec::from_bytes_le(&[0b1100]);
a.bit_xor_into(&b);
assert_eq!(a.to_bytes_le(), vec![0b0110]);

// Conversion
let bytes = vec![0xFF, 0x00];
let bv = BitVec::from_bytes_le(&bytes);
assert_eq!(bv.len(), 16);
assert_eq!(bv.count_ones(), 8);
```

### 5-Minute BitMatrix Tutorial

```rust
use gf2_core::BitMatrix;

// Construction
let mut m = BitMatrix::zeros(3, 3);
m.set(0, 0, true);
m.set(1, 1, true);
m.set(2, 2, true);
// Now m is the 3×3 identity matrix

// Element access
assert_eq!(m.get(0, 0), true);
assert_eq!(m.get(0, 1), false);

// GF(2) row operations
m.row_xor(0, 1);  // row[0] = row[0] XOR row[1]
m.swap_rows(1, 2);

// Matrix multiplication (uses M4RM algorithm)
let a = BitMatrix::identity(100);
let b = BitMatrix::ones(100, 50);
let c = &a * &b;  // Efficient multiplication
assert_eq!(c.rows(), 100);
assert_eq!(c.cols(), 50);

// Transpose
let t = m.transpose();
assert_eq!(t.get(1, 0), m.get(0, 1));

// Extract rows/columns as BitVec
let row0 = m.row_as_bitvec(0);
let col0 = m.col_as_bitvec(0);
```

## Advanced Features

### Sparse Matrices

For low-density matrices (e.g., LDPC codes), use `SpBitMatrix` or `SpBitMatrixDual`:

```rust
use gf2_core::sparse::SpBitMatrix;

let coo = vec![(0, 1), (0, 3), (1, 2)];
let sparse = SpBitMatrix::from_coo(2, 5, &coo);

// Iterate over nonzero columns in a row
for col in sparse.row_iter(0) {
    println!("Column {}", col);
}
```

[→ Full sparse matrix documentation](https://docs.rs/gf2-core/latest/gf2_core/sparse/)

### GF(2^m) Extension Field Arithmetic

Fast multiplication, division, and inversion in binary extension fields:

```rust
use gf2_core::gf2m::{Gf2mField, Gf2mPoly};

// GF(2^8) with primitive polynomial x^8 + x^4 + x^3 + x + 1
let field = Gf2mField::new(8, 0b100011011);
let a = Gf2mPoly::from_int(0x53);
let b = Gf2mPoly::from_int(0xCA);
let product = field.mul(&a, &b);
```

[→ Full GF(2^m) documentation](docs/GF2M.md)

### RREF and Matrix Inversion

Compute reduced row echelon form and inverses:

```rust
use gf2_core::alg::rref::rref;

let m = BitMatrix::identity(10);
let result = rref(&m, false);  // false = pivot from left
assert_eq!(result.rank, 10);
assert_eq!(result.reduced, m);
```

[→ RREF algorithm details](docs/BENCHMARKS.md#rref-performance)

### Polar Codes Support

Fast Hadamard Transform for polar code encoding/decoding:

```rust
use gf2_core::BitVec;

let mut info = BitVec::zeros(1024);
info.set(0, true);
let encoded = info.polar_transform(1024);
let decoded = encoded.polar_transform_inverse(1024);
assert_eq!(decoded, info);
```

### Random Generation

Generate random bit vectors and matrices (requires `rand` feature, enabled by default):

```rust
use gf2_core::{BitVec, BitMatrix};
use rand::SeedableRng;

let mut rng = rand::rngs::StdRng::seed_from_u64(42);

// Random bit vector with p=0.5
let bv = BitVec::random(1000, &mut rng);

// Sparse random matrix with p=0.1
let m = BitMatrix::random_with_probability(100, 100, 0.1, &mut rng);

// Deterministic seeded generation
let bv = BitVec::random_seeded(500, 0x1234);
```

### SIMD Acceleration

Enable the `simd` feature for AVX2-accelerated operations on x86_64:

```toml
[dependencies]
gf2-core = { version = "0.1", features = ["simd"] }
```

Operations automatically use SIMD when beneficial (>512 bytes). Validated speedups:
- Logical operations (XOR, AND, OR): **3.4-3.6×** faster
- Popcount: **3.5×** faster
- Large matrix operations benefit from vectorized row XOR

[→ SIMD architecture details](docs/KERNEL_OPTIMIZATION.md)

### Matrix Visualization

Enable `visualization` feature to save matrices as PNG images:

```toml
[dependencies]
gf2-core = { version = "0.1", features = ["visualization"] }
```

```rust
let m = BitMatrix::identity(100);
m.save_image("identity.png").unwrap();
```

## Performance

Benchmarked against specialized C/C++ libraries (M4RI, NTL, FLINT):

- **M4RM matrix multiply:** Within 4× of M4RI (hand-optimized C)
- **RREF:** Within 8-10× of M4RI for large matrices (150-170× faster than naive)
- **GF(2^m) multiplication:** 13-18× faster than NTL
- **SIMD operations:** 3.4-3.6× speedup for large data (AVX2)

**See [`docs/BENCHMARKS.md`](docs/BENCHMARKS.md) for detailed performance analysis.**

## Common Patterns & Pitfalls

### Working with Word Boundaries

BitVec stores bits in 64-bit words. Operations at word boundaries (multiples of 64) are more efficient:

```rust
// Efficient: aligned to word boundary
let bv = BitVec::zeros(64);  

// Less efficient: requires word splitting
let bv = BitVec::zeros(65);  
```

### Tail Masking Invariant

BitVec maintains an invariant that padding bits beyond `len_bits` are always zero. All operations preserve this automatically - you don't need to think about it, but it's important for correctness of operations like `==` and `count_ones()`.

### When to Use Sparse vs Dense Matrices

- **Dense (`BitMatrix`)**: When >5-10% of entries are nonzero
- **Sparse (`SpBitMatrix`)**: When <5% of entries are nonzero (e.g., LDPC codes)
- **Dual (`SpBitMatrixDual`)**: When you need both row and column access

### Functional Style with In-Place Operations

Many BitVec operations are in-place for performance but follow functional naming:

```rust
let mut a = BitVec::ones(8);
let b = BitVec::zeros(8);
a.bit_xor_into(&b);  // a is mutated, b is borrowed immutably
```

For pure functional style, clone first:

```rust
let a = BitVec::ones(8);
let b = BitVec::zeros(8);
let mut result = a.clone();
result.bit_xor_into(&b);  // a and b unchanged
```

## Documentation & Examples

- **Rustdocs:** Run `cargo doc --no-deps --open` for full API documentation
- **Examples:** Run `cargo run --example <name>`:
  - `bitvec_basics` - BitVec operations walkthrough
  - `matrix_basics` - BitMatrix operations walkthrough  
  - `sparse_display` - Sparse matrix visualization
  - `random_generation` - Random BitVec/BitMatrix generation
  - `visualize_matrix` - Matrix PNG export (requires `visualization` feature)
- **Deep Dives:**
  - [BENCHMARKS.md](docs/BENCHMARKS.md) - Performance analysis & comparisons
  - [KERNEL_OPTIMIZATION.md](docs/KERNEL_OPTIMIZATION.md) - SIMD architecture
  - [GF2M.md](docs/GF2M.md) - Extension field arithmetic
  - [PRIMITIVE_POLYNOMIALS.md](docs/PRIMITIVE_POLYNOMIALS.md) - Polynomial generation

## Features Reference

| Feature | Default | Description |
|---------|---------|-------------|
| `rand` | ✅ | Random BitVec/BitMatrix generation |
| `simd` | ❌ | AVX2/NEON acceleration (opt-in, runtime detected) |
| `visualization` | ❌ | Save BitMatrix as PNG images |
| `parallel` | ❌ | Rayon-based parallelization (future) |

## Safety

This crate uses `#![deny(unsafe_code)]`. All operations are implemented in safe Rust.

## License

Licensed under either of:
- Apache License, Version 2.0 ([LICENSE-APACHE](../../LICENSE-APACHE))
- MIT License ([LICENSE-MIT](../../LICENSE-MIT))

at your option.

## Contributing

Contributions welcome! Please see the workspace-level [CONTRIBUTING.md](../../CONTRIBUTING.md).
