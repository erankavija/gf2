# gf2-kernels-simd

Backend crate holding the SIMD kernels of the gf2 workspace and the only
production `unsafe` code outside `gf2-kernels-hip`. It is an implementation
layer; application code calls the safe APIs of `gf2-core`, `gf2-algebra` and
`gf2-coding`.

## Role

The kernel families export safe `detect` functions that return bundles of
function pointers. Most return `Option` and yield `None` when the host lacks a
required CPU extension, and consuming crates run scalar code on `None`. The
`modem` and `transpose` detectors return a bundle on every host, with scalar
entries where no vector kernel applies. The families
include logical bit operations and population counts, $\mathrm{GF}(p)$ and
$\mathrm{GF}(2^m)$ arithmetic, BCH encoding, LLR and modem arithmetic, and
packed $\mathbb{F}_3$/$\mathbb{F}_5$/$\mathbb{F}_7$ operations. The
architecture is described in
[acceleration architecture](../../docs/concepts/acceleration-architecture.md).

## Supported targets

- x86 and x86_64; a bundle requires extensions drawn from AVX2, FMA, BMI2,
  PCLMULQDQ, VPCLMULQDQ, SSE4.1 and POPCNT.
- AVX-512 hosts run the AVX2 kernels.
- AArch64 and other architectures run scalar code.

## Enabling

The crate declares no enabling features: the `avx2` and `avx512` entries in
[`Cargo.toml`](Cargo.toml) gate no code, and backend selection is runtime
detection. Adopting crates gate most detection behind their own `simd` feature, which
`gf2-core`, `gf2-coding` and `gf2-algebra` declare; see
[supported configurations](../../docs/reference/supported-configurations.md#cargo-features)
and [installation](../../docs/reference/supported-configurations.md#installation).

## Documentation

```bash
cargo doc -p gf2-kernels-simd --no-deps
```

Rendered output: Rustdoc.
