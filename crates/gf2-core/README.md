# gf2-core

Finite-field and GF(2) linear-algebra primitives in safe Rust: dense and sparse bit matrices, GF(2^m) arithmetic with strategy dispatch, prime fields `Fp<P>` with Montgomery multiplication (plus specialized Mersenne/Proth backends), and tower extensions `QuadraticExt` / `CubicExt` over the `ExtConfig` trait.

`gf2-core` is the mathematical foundation for the `gf2-coding` codes crate and the target of Lean4 proofs in `proofs/`. The crate is `#![deny(unsafe_code)]`; SIMD lives in the sibling `gf2-kernels-simd` crate and is reached through runtime dispatch.

## Module map

| Module | Contents |
|---|---|
| `bitvec`, `bitslice` | Dense bit storage in `Vec<u64>`, word-aligned ops, shifts, scans, rank/select |
| `matrix` | `BitMatrix` — row-major bit-packed matrix, M4RM multiply, row ops, transpose |
| `sparse` | CSR / CSC / `SpBitMatrixDual` for LDPC-scale sparse matrices |
| `alg/` | M4RM multiply, Gauss–Jordan inversion, RREF, polar / Fast Hadamard transform |
| `field/` | `FiniteField`, `ConstField` traits; axiom-test harness; `FieldVec` |
| `gf2m/` | GF(2^m) arithmetic generic over storage width (sealed `UintExt` trait); Barrett, Karatsuba, table and SIMD strategies |
| `gfp/` | `Fp<const P: u64>` Montgomery multiplication, plus a specialized module for Mersenne/Proth primes |
| `gfpn/` | `QuadraticExt<C>`, `CubicExt<C>` tower extensions over `ExtConfig` |
| `primitive_polys` | Static database of primitive polynomials for m = 2..16, plus verification and generation |
| `kernels/` | Runtime dispatch to scalar or SIMD backends |
| `compute/` | Parallel/batch operations (Rayon, feature-gated) |
| `io/` | Serde serialization (feature-gated) |
| `rng` | Deterministic random bit generators |

## Install

```toml
[dependencies]
gf2-core = { path = "...", features = ["simd"] }   # AVX2/AVX-512 on x86_64

# Minimal
gf2-core = { path = "...", default-features = false }
```

## Getting started

### Bit vectors and matrices

```rust
use gf2_core::{BitVec, BitMatrix};

let mut bv = BitVec::zeros(8);
bv.set(0, true); bv.set(7, true);
assert_eq!(bv.count_ones(), 2);
assert_eq!(bv.find_first_one(), Some(0));

let mut a = BitVec::from_bytes_le(&[0b1010]);
a.bit_xor_into(&BitVec::from_bytes_le(&[0b1100]));
assert_eq!(a.to_bytes_le(), vec![0b0110]);

let m = BitMatrix::identity(128);
let p = &m * &m;                // M4RM
assert_eq!(p, m);
```

### Sparse matrices (LDPC-scale)

```rust
use gf2_core::sparse::SpBitMatrix;

let coo = [(0, 1), (0, 3), (1, 2)];
let h   = SpBitMatrix::from_coo(2, 5, &coo);
for col in h.row_iter(0) {
    println!("nonzero at column {col}");
}
```

Use `SpBitMatrixDual` when you need both row and column traversal (syndrome computation, min-sum passes).

### GF(2^m) arithmetic

```rust
use gf2_core::gf2m::Gf2mField;

// AES field: GF(2^8) with x^8 + x^4 + x^3 + x + 1
let f = Gf2mField::new(8, 0b1_0001_1101).with_tables();
let a = f.element(0x53);
let b = f.element(0xCA);
let _ = &a * &b;                // O(1) log/antilog
```

Strategy selection (Barrett, Karatsuba, tables, SIMD) is transparent and depends on `m`, storage width, and available CPU features.

### Prime fields and tower extensions

```rust
use gf2_core::gfp::Fp;

// Fp<P> carries Montgomery constants as associated consts.
type F = Fp<1_000_000_007>;
let a = F::new(12345);
let b = F::new(67890);
let _ = a * b + a;               // Montgomery multiplication + modular add
```

`gfpn::QuadraticExt<C>` and `gfpn::CubicExt<C>` build tower extensions on top of any prime-field base through the `ExtConfig` trait. Both levels are covered by Lean4 proofs — see [`proofs/`](../../proofs/).

### Primitive polynomials

```rust
use gf2_core::primitive_polys;

let p = primitive_polys::lookup(8).unwrap();        // degree 8
assert!(primitive_polys::verify(8, p));
```

The static database covers m = 2..16. `generation` can search for primitive polynomials at larger degrees.

### Polar / Fast Hadamard transform

```rust
use gf2_core::BitVec;

let mut u = BitVec::zeros(1024);
u.set(0, true);
let x = u.polar_transform(1024);
assert_eq!(x.polar_transform_inverse(1024), u);
```

## SIMD and parallelism

- Enable `simd` (or rely on `gf2-coding`'s default) to route bitwise operations, matrix row XORs, and popcount through AVX2 / AVX-512 kernels. Runtime detection happens once per process via `OnceLock` in `lib.rs`.
- Enable `parallel` to opt in to Rayon-backed batch algorithms in `compute/`.

Validated speedups on large operands (>512 bytes): 3.4–3.6× for bulk logical ops and popcount; see [`docs/BENCHMARKS.md`](docs/BENCHMARKS.md) and [`docs/KERNEL_OPTIMIZATION.md`](docs/KERNEL_OPTIMIZATION.md) for measurements and methodology.

## Features

| Feature | Default | Effect |
|---|---|---|
| `rand` | ✅ | Random `BitVec` / `BitMatrix` / field elements |
| `io` | ✅ | Serde serialization of bit containers |
| `simd` | — | Route through `gf2-kernels-simd` (AVX2 / AVX-512) |
| `parallel` | — | Rayon batch algorithms |
| `visualization` | — | Save `BitMatrix` as PNG |

## Invariants

- **Tail masking**: padding bits past `len_bits` in the last `u64` word are always zero. Every mutating operation calls `mask_tail()`. Equality, `count_ones`, and `parity` depend on this.
- **Bit numbering**: bit `i` lives in `word = i >> 6`, `mask = 1u64 << (i & 63)`.
- **Dense vs sparse**: use `BitMatrix` when >5–10% of entries are nonzero; use `SpBitMatrix` / `SpBitMatrixDual` below that.

## Testing & docs

```bash
cargo test -p gf2-core --release                # full suite
cargo test -p gf2-core --release --doc          # doc examples
cargo bench -p gf2-core --bench fp_montgomery   # focused bench
cargo doc -p gf2-core --no-deps --open
```

Deep dives under `docs/`:

- [`BENCHMARKS.md`](docs/BENCHMARKS.md) — performance vs. M4RI / NTL / FLINT
- [`KERNEL_OPTIMIZATION.md`](docs/KERNEL_OPTIMIZATION.md) — SIMD architecture
- [`GF2M.md`](docs/GF2M.md) — GF(2^m) strategy selection
- [`PRIMITIVE_POLYNOMIALS.md`](docs/PRIMITIVE_POLYNOMIALS.md) — polynomial database and generation
- [`COMPUTE_BACKEND_DESIGN.md`](docs/COMPUTE_BACKEND_DESIGN.md), [`RREF_DESIGN_PLAN.md`](docs/RREF_DESIGN_PLAN.md), [`SPARSE_DEDUP_DESIGN.md`](docs/SPARSE_DEDUP_DESIGN.md), [`POLAR_IMPLEMENTATION_PLAN.md`](docs/POLAR_IMPLEMENTATION_PLAN.md)

## Safety

`#![deny(unsafe_code)]`. Unsafe SIMD lives in `gf2-kernels-simd`; GPU kernels in `gf2-kernels-hip`, consumed from `gf2-coding` (BCJR / Gray-QAM demap) and `gf2-algebra` (batch permanents over F_3 / F_5 / F_7) under each crate's `hip` feature.

## License

MIT — see [LICENSE-MIT](../../LICENSE-MIT).
