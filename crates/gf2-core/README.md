# gf2-core

`gf2-core` provides the high-performance primitives that power the `gf2` workspace. It focuses on bit-level data structures and linear algebra over the binary field (GF(2)).

## Highlights

- Dense, tail-masked `BitVec` backed by `Vec<u64>`
- Bit-packed `BitMatrix` with M4RM multiplication and Gauss-Jordan inversion
- Polar transform operations for polar code encoding/decoding
- Sparse matrices in CSR/CSC formats for low-density matrices
- Extension field GF(2^m) arithmetic with optimized multiplication
- SIMD-accelerated operations (AVX2) for supported platforms
- Strict safety: `#![deny(unsafe_code)]`

## Features

- **default**: Scalar implementations with random generation (`rand`)
- **simd**: Enables SIMD-accelerated operations (opt-in)
  - AVX2 logical ops, popcount, scans, shifts on x86_64
  - PCLMULQDQ field multiplication for GF(2^m) when m > 16
- **rand**: Random BitVec and BitMatrix generation (enabled by default)
  - To opt-out, use `default-features = false`
- **visualization**: Save BitMatrix as PNG images (opt-in)
  - Useful for debugging and visual inspection of matrix patterns

## Performance

Validated against SageMath 10.7 (Nov 2024):
- **3-340x faster** primitive polynomial testing
- **4-127x faster** GF(2^m) polynomial operations  
- **12-15x faster** sparse matrix operations

See `docs/phase9_3_complete.md` for detailed benchmarking methodology and results.

### Benchmarks

```bash
# Primitive polynomial verification
cargo bench --bench primitive_poly

# GF(2^m) field operations
cargo bench --bench polynomial

# Sparse matrix operations
cargo bench --bench sparse

# Other: polar, matmul, bitvec, wide_logical, scan, random, shifts, rank_select
```

## Usage

Add the crate to your project (from crates.io or via `git` path):

```toml
[dependencies]
gf2-core = "0.1"  # rand enabled by default

# With SIMD acceleration
gf2-core = { version = "0.1", features = ["simd"] }

# Without rand (minimal build)
gf2-core = { version = "0.1", default-features = false }
```

Example:

```rust
use gf2_core::BitVec;

let mut bits = BitVec::new();
bits.push_bit(true);
bits.push_bit(false);
assert_eq!(bits.count_ones(), 1);

// Scan operations
let bv = BitVec::from_bytes_le(&[0b0001_0000]);
assert_eq!(bv.find_first_one(), Some(4));
```

### Random Generation (enabled by default)

```rust
use gf2_core::{BitVec, matrix::BitMatrix};
use rand::rngs::StdRng;
use rand::SeedableRng;

// Create random bit vector (p=0.5 for each bit)
let mut rng = StdRng::seed_from_u64(42);
let bv = BitVec::random(1000, &mut rng);

// Create random matrix with custom probability
let sparse_matrix = BitMatrix::random_with_probability(100, 100, 0.1, &mut rng);

// Deterministic random generation
let bv = BitVec::random_seeded(500, 0x1234);
```

### Sparse Matrices

```rust
use gf2_core::sparse::{SparseMatrix, SparseMatrixDual};
use gf2_core::BitVec;

// Build from coordinate (COO) format
let coo = vec![(0, 1), (0, 3), (1, 2), (1, 4)];
let sparse = SparseMatrix::from_coo(2, 5, &coo);

// Matrix-vector multiply for syndrome computation
let x = BitVec::random(5, &mut rng);
let y = sparse.matvec(&x);

// Row iteration
for col in sparse.row_iter(0) {
    println!("Row 0 has nonzero at column {}", col);
}

// For bidirectional access patterns, use dual representation
let dual = SparseMatrixDual::from_coo(2, 5, &coo);

// Fast row access (CSR)
for col in dual.row_iter(0) {
    println!("Row 0, column {}", col);
}

// Fast column access (CSC - no transpose!)
for row in dual.col_iter(3) {
    println!("Column 3, row {}", row);
}

// Both A×x and A^T×x are efficient
let y = dual.matvec(&x);
let yt = dual.matvec_transpose(&y);
```

### Polar Transform Operations

Fast Hadamard Transform for polar code encoding/decoding:

```rust
use gf2_core::BitVec;

// Create information bits for polar encoding
let mut info = BitVec::zeros(1024);
info.set(0, true);
info.set(512, true);

// Apply polar transform G_N = [1 0; 1 1]^⊗log2(N)
let encoded = info.polar_transform(1024);

// Decode via inverse transform
let decoded = encoded.polar_transform_inverse(1024);
assert_eq!(decoded, info);

// In-place operations for efficiency
let mut bits = BitVec::from_bytes_le(&[0xFF, 0x00, 0xFF, 0x00]);
bits.polar_transform_into(32);

// Bit-reversal permutation (used in polar code construction)
let mut bv = BitVec::from_bytes_le(&[0b11001010]);
bv.bit_reverse_into(8);
```

### Matrix Visualization (Optional)

Enable the `visualization` feature to save matrices as PNG images for debugging and inspection:

```toml
[dependencies]
gf2-core = { version = "0.1", features = ["visualization"] }
```

```rust
use gf2_core::matrix::BitMatrix;

// Create and visualize an identity matrix
let m = BitMatrix::identity(100);
m.save_image("identity.png").unwrap();

// Visualize matrix structure
let h = BitMatrix::random_with_probability(50, 100, 0.2, &mut rng);
h.save_image("parity_check_matrix.png").unwrap();
```

Each bit becomes a pixel: unset (0) → black, set (1) → white.  
To change colors, edit the `ZERO_COLOR` and `ONE_COLOR` constants in `src/matrix.rs`.

For more details, see the workspace-level [README](../../README.md) and the inlined Rustdocs.
