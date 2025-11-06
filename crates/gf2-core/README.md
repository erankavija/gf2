# gf2-core

`gf2-core` provides the high-performance primitives that power the `gf2` workspace. It focuses on bit-level data structures and linear algebra over the binary field (GF(2)).

## Highlights

- Dense, tail-masked `BitVec` backed by `Vec<u64>`
- Bit-packed `BitMatrix` with fast M4RM multiplication and Gauss-Jordan inversion
- Strict safety guarantees: `#![deny(unsafe_code)]`
- Comprehensive tests and Criterion benchmarks

## Usage

Add the crate to your project (from crates.io or via `git` path):

```toml
[dependencies]
gf2-core = "0.1"
```

Example:

```rust
use gf2_core::BitVec;

let mut bits = BitVec::new();
bits.push_bit(true);
bits.push_bit(false);
assert_eq!(bits.count_ones(), 1);
```

For more details, see the workspace-level [README](../../README.md) and the inlined Rustdocs.
