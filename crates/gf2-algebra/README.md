# gf2-algebra

Packed arithmetic over F_3, F_5 and F_7 and matrix-permanent algorithms built on it. The crate depends on [`gf2-core`](../gf2-core/README.md) for `FiniteField`, `Fp<P>` and `BitVec`, and denies `unsafe` code; SIMD and GPU kernels live in `gf2-kernels-simd` and `gf2-kernels-hip`.

## When to choose it

- Permanents over F_3, F_5 or F_7 for matrices too large for generic field arithmetic, using Ryser's formula in Gray-code order over bit-plane encodings of field elements (Scheinerman, arXiv 2407.20205).
- Permanental rank deficiency of rectangular matrices (`permanent::rank`) and exact small-order permanent-zero probabilities (`permanent::exact`).
- Lane-parallel F_3, F_5 and F_7 vectors through the `PackedField` traits: 64 lanes for F_3 and F_5, 16 lanes for F_7.

`permanent_ryser` accepts any `FiniteField` and serves as the correctness oracle for the packed kernels.

## Computation

| Capability | Entry points |
|---|---|
| Packed fields | `packed::{Bipedal3, Packed5, Packed7}` with `*Vec` and `*Matrix` companions |
| Permanent, any field | `permanent::permanent_ryser`, `permanent_mod3_reference` |
| Permanent, packed | `permanent_bipedal3` (`n <= 255`; single-word kernel through `n = 63`, multi-word above), `permanent_bipedal5`, `permanent_bipedal7` (single-word; `n` bounded by the lane count) |
| Permanental rank | `permanental_rank_status`, `exact_permanental_rank_deficiency` |
| Exact enumeration | `enumerate_permanent_zero_probability`, `determinant_singular_probability` |

## Backends

- **Scalar**: always available and the dispatch target of `permanent_bipedal3`.
- **AVX2** (`simd`): `permanent_bipedal3_batch` evaluates up to four F_3 matrices together and `permanent_bipedal3_singleword_simd` exposes the single-matrix kernel; both fall back to scalar when the host lacks AVX2.
- **Rayon** (`parallel`): `permanent_bipedal3_parallel` partitions the Gray-code walk into chunks; chunk size comes from `tuning::AlgebraTuning`.
- **HIP/ROCm** (`hip`): `gpu::permanent_batch_bipedal{3,5,7}` evaluates a batch on the device.

## Features

The [manifest](Cargo.toml) lists every feature. Defaults are `simd`, `parallel`, `f5` and `f7`.

| Feature | Effect |
|---|---|
| `simd` | AVX2 batch and single-matrix kernels |
| `parallel` | Rayon permanent |
| `f5`, `f7` | `Packed5`/`Packed7` types and `permanent_bipedal5`/`permanent_bipedal7` |
| `hip` | `gpu` module; requires ROCm and `hipcc` |
| `serde` | `Serialize`/`Deserialize` on packed types |
| `tuning-profile` | `AlgebraTuningCodec` |
| `test-support` | `testutil` matrix generators and oracles |

```toml
[dependencies]
gf2-algebra = { path = "crates/gf2-algebra" }

# Scalar, F_3 only
# gf2-algebra = { path = "crates/gf2-algebra", default-features = false }

# GPU
# gf2-algebra = { path = "crates/gf2-algebra", features = ["hip"] }
```

## Reference

API documentation is generated from the crate:

```bash
cargo doc -p gf2-algebra --no-deps --open
```

The workspace [documentation index](../../docs/index.md) links the remaining reference material.

## License

MIT; see [`LICENSE-MIT`](../../LICENSE-MIT).
