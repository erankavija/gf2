# gf2-core

Safe-Rust storage and linear algebra over GF(2), and arithmetic over binary, prime and extension fields. `gf2-core` is the lowest layer of the gf2 workspace and has no production dependency on another workspace crate except the kernel crate `gf2-kernels-simd`.

## Contents

| Area | Provides | Modules |
|---|---|---|
| Dense GF(2) | `BitVec`, `BitSlice`, `BitSliceMut` and the row-major `BitMatrix`; M4RM multiplication, Gauss-Jordan inversion, RREF, polar transform | `bitvec`, `bitslice`, `matrix`, `alg` |
| Sparse GF(2) | `SpBitMatrix` (CSR), `SpBitMatrixDual` (row and column traversal), block-CSR and row permutations | `sparse` |
| Binary fields | GF(2^m) generic over storage width, with Barrett, Karatsuba, table and SIMD multiplication strategies; wide-degree fields; primitive-polynomial database and generation | `gf2m`, `primitive_polys` |
| Prime fields | `Fp<const P: u64>` with Montgomery form, and canonical-form reduction for Mersenne and Proth primes | `gfp` |
| Extension fields | `QuadraticExt` and `CubicExt` towers over `ExtConfig`; `QuotientField` for runtime irreducible moduli; field identity and extension witnesses | `gfpn`, `field::extension` |
| Field linear algebra | `FieldMatrix<F>` and sparse `SparseFieldMatrix<F>` over any `FiniteField`: GEMM (classical and Strassen-Winograd), PLE, RREF, nullspace, inverse, determinant, triangular solves, characteristic polynomial; `FieldPoly`, NTT, batch inversion | `field` |
| Execution | Runtime scalar/SIMD dispatch, Rayon batch backends, typed tuning profiles, serialization | `kernels`, `compute`, `tuning`, `io` |

Every field implementation passes the shared field-law suite exposed as `field::axiom_tests` under `test-support`.

## When to choose gf2-core

- Dense or sparse GF(2) matrices with elimination, inversion and multiplication on bit-packed storage.
- Generic algorithms over a `FiniteField` that must run unchanged over GF(2^m), `Fp<P>` and tower extensions.
- A math layer with `#![deny(unsafe_code)]`; unsafe SIMD lives in `gf2-kernels-simd`.

Codes, modems and channels are in `gf2-coding`; packed F_3/F_5/F_7 arithmetic and permanents are in `gf2-algebra`.

## Conventions

Bit `i` lives in word `i >> 6` under mask `1u64 << (i & 63)`; padding bits past the length are zero.

## Features

| Feature | Default | Effect |
|---|---|---|
| `rand` | yes | Random bit containers, matrices and field elements |
| `io` | yes | Serde serialization of bit containers; checksummed `FieldMatrix` files |
| `simd` | no | Enables runtime detection of the accelerated kernel bundles |
| `parallel` | no | Rayon batch algorithms in `compute::field` |
| `visualization` | no | `BitMatrix` export to PNG |
| `tuning-profile` | no | Strict format-2 tuning-envelope JSON codecs |
| `test-support` | no | Field axiom harness and test utilities for downstream crates |

```toml
[dependencies]
gf2-core = { path = "crates/gf2-core", features = ["simd", "parallel"] }
```

## Reference

- API reference: `cargo doc -p gf2-core --no-deps --open`; the crate-level page is [`src/lib.rs`](src/lib.rs).
- Documentation index: [`docs/index.md`](../../docs/index.md).

## License

MIT, see [LICENSE-MIT](../../LICENSE-MIT).
