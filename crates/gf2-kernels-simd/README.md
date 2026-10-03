# gf2-kernels-simd

Backend crate holding the SIMD kernels of the gf2 workspace and the only
production `unsafe` code outside `gf2-kernels-hip`. It is an implementation
layer; application code calls the safe APIs of `gf2-core`, `gf2-algebra` and
`gf2-coding`.

## Role

Each kernel family exports a safe `detect()` that returns a bundle of function
pointers, or `None` when the host lacks a required CPU extension. Consuming
crates cache the bundle and fall back to scalar code on `None`. The families
cover logical bit operations and population counts, $\mathrm{GF}(p)$ and
$\mathrm{GF}(2^m)$ arithmetic, BCH encoding, LLR and modem arithmetic, and
packed $\mathbb{F}_3$/$\mathbb{F}_5$/$\mathbb{F}_7$ operations. The
architecture is described in
[acceleration architecture](../../docs/concepts/acceleration-architecture.md).

## Supported targets

- x86 and x86_64 with AVX2; individual bundles also require FMA, BMI2,
  PCLMULQDQ, VPCLMULQDQ, SSE4.1 or POPCNT.
- AVX-512 hosts run the AVX2 kernels.
- AArch64 and other architectures expose no SIMD bundle and run scalar code.

## Enabling

The crate declares no enabling features: the `avx2` and `avx512` entries in
[`Cargo.toml`](Cargo.toml) gate no code, and backend selection is runtime
detection. Adopting crates enable it through their own `simd` feature, which
`gf2-core`, `gf2-coding` and `gf2-algebra` declare; see
[supported configurations](../../docs/reference/supported-configurations.md#cargo-features)
and [installation](../../docs/reference/supported-configurations.md#installation).

## Documentation

```bash
./scripts/cargo-budget.sh cargo doc -p gf2-kernels-simd --no-deps
```

Rendered output: [Rustdoc](../../target/doc/gf2_kernels_simd/index.html).
