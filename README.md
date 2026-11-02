# gf2

[![CI](https://github.com/erankavija/gf2/workflows/CI/badge.svg)](https://github.com/erankavija/gf2/actions)

gf2 is a Rust workspace for research in finite-field computing and coding
theory. It aims to pair composable APIs with correctness evidence from
standards test vectors and Lean 4 proofs, seeded and resumable simulation
campaigns, and SIMD and GPU backends whose results match their scalar and CPU
equivalents within a declared numerical contract.

## Crates

| Crate | Choose it for |
|---|---|
| [`gf2-core`](crates/gf2-core/README.md) | Bit-packed vectors and dense or sparse matrices over $\mathrm{GF}(2)$; arithmetic and linear algebra over $\mathrm{GF}(2^m)$, $\mathrm{GF}(p)$ and $\mathrm{GF}(p^m)$ behind the `FiniteField` trait ([field families](docs/concepts/finite-field-arithmetic.md)) |
| [`gf2-coding`](crates/gf2-coding/README.md) | Code constructions, decoders, modems and channel models, including the DVB-T2 (`@/citation/Etsi2015`) and 5G NR (`@/citation/ThreeGpp2017`) codes ([standards conformance](docs/reference/standards-conformance.md)) |
| [`gf2-algebra`](crates/gf2-algebra/README.md) | Packed arithmetic over $\mathbb{F}_3$, $\mathbb{F}_5$ and $\mathbb{F}_7$ and matrix permanents over those fields |
| [`gf2-sim`](crates/gf2-sim/README.md) | Seeded, checkpointed error-rate sweeps and campaigns over pipelines of `gf2-coding` stages, on CPU workers and, with the `hip` feature, a HIP device |
| [`gf2-stats`](crates/gf2-stats/README.md) | Binomial tests, confidence intervals and exact weighted-run reduction |

`gf2-coding`, `gf2-algebra` and `gf2-sim` depend on `gf2-core`; `gf2-sim`
also uses `gf2-stats` for interval estimates.

Two backend crates hold the workspace's production `unsafe` code and are
reached through the library crates' Cargo features
([acceleration architecture](docs/concepts/acceleration-architecture.md)):

- [`gf2-kernels-simd`](crates/gf2-kernels-simd/README.md) holds SIMD kernels
  for x86 and x86_64, selected by run-time CPU detection; a host lacking the
  extensions a kernel requires runs scalar code. The `simd` feature of
  `gf2-core`, `gf2-coding` and `gf2-algebra`, a default of the latter two,
  [gates most of that detection](docs/how-to/select-acceleration.md#select-the-instruction-set).
- `gf2-kernels-hip` holds HIP kernels for AMD GPUs, enabled by the `hip`
  feature, a default of no crate: BCH syndrome evaluation, BCJR decoding and
  Gray-QAM demapping in `gf2-coding`, batch permanents in `gf2-algebra`, and
  LDPC belief propagation, AWGN generation and demapping in `gf2-sim`
  pipelines.

[`proofs/`](proofs/README.md) holds Lean 4 translations of selected `gf2-core`
and `gf2-algebra` functions and the proofs over them; see
[Run and extend the Lean 4 proofs](docs/how-to/formal-verification.md).

## Installation

The crates are Git or path dependencies;
[installation](docs/reference/supported-configurations.md#installation) gives
the manifest form and
[Cargo features](docs/reference/supported-configurations.md#cargo-features)
the defaults. Constraints, each detailed in
[supported configurations](docs/reference/supported-configurations.md):

- Rust 1.95 or newer.
- `gf2-sim` builds on Unix targets only.
- The `hip` features require ROCm `hipcc`, and the linked GPU kernels are
  compiled for gfx1030. `gf2-kernels-hip` is outside the default Cargo
  workspace, and a build with `--workspace --all-features` requires ROCm.

## Documentation

The [documentation index](docs/index.md) lists the tutorials, how-to guides,
concept pages and reference pages, including the performance evidence. Two
tutorials run research workflows end to end:

- [Finite-field linear algebra at scale](docs/tutorials/field-linear-algebra-at-scale.md)
- [Coded-modulation link simulation](docs/tutorials/coded-modulation-link-simulation.md)

The crate READMEs linked above give the command that builds their Rustdoc.

## License

MIT, see [LICENSE-MIT](LICENSE-MIT).
