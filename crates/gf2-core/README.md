# gf2-core

`gf2-core` provides the high-performance primitives that power the `gf2` workspace. It focuses on bit-level data structures and linear algebra over the binary field (GF(2)).

## Highlights

- Dense, tail-masked `BitVec` backed by `Vec<u64>`
- Bit-packed `BitMatrix` with fast M4RM multiplication and Gauss-Jordan inversion
- SIMD-accelerated operations (AVX2): logical ops, popcount, scans, word-aligned shifts
- Strict safety guarantees: `#![deny(unsafe_code)]`
- Comprehensive tests and Criterion benchmarks

## Features

- **default**: Scalar baseline implementations
- **simd**: Enables AVX2-accelerated operations on x86_64 (opt-in)

## Usage

Add the crate to your project (from crates.io or via `git` path):

```toml
[dependencies]
gf2-core = { version = "0.1", features = ["simd"] }
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

For more details, see the workspace-level [README](../../README.md) and the inlined Rustdocs.
