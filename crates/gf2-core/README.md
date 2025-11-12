# gf2-core

`gf2-core` provides the high-performance primitives that power the `gf2` workspace. It focuses on bit-level data structures and linear algebra over the binary field (GF(2)).

## Highlights

- Dense, tail-masked `BitVec` backed by `Vec<u64>`
- Bit-packed `BitMatrix` with fast M4RM multiplication and Gauss-Jordan inversion
- **Sparse matrices** in CSR/CSC formats for low-density matrices (< 5% density)
- SIMD-accelerated operations (AVX2): logical ops, popcount, scans, word-aligned shifts
- Strict safety guarantees: `#![deny(unsafe_code)]`
- Comprehensive tests and Criterion benchmarks

## Features

- **default**: Scalar implementations with random generation (`rand`)
- **simd**: Enables AVX2-accelerated operations on x86_64 (opt-in)
- **rand**: Random BitVec and BitMatrix generation (enabled by default)
  - To opt-out, use `default-features = false`

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

For more details, see the workspace-level [README](../../README.md) and the inlined Rustdocs.
