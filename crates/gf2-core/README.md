# gf2-core

`gf2-core` provides the high-performance primitives that power the `gf2` workspace. It focuses on bit-level data structures and linear algebra over the binary field (GF(2)).

## Highlights

- Dense, tail-masked `BitVec` backed by `Vec<u64>`
- Bit-packed `BitMatrix` with fast M4RM multiplication and Gauss-Jordan inversion
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

For more details, see the workspace-level [README](../../README.md) and the inlined Rustdocs.
