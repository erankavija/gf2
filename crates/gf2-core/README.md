# gf2-core

Bit-packed storage and linear algebra over $\mathrm{GF}(2)$, and arithmetic
and linear algebra over binary, prime and extension fields behind the
`FiniteField` trait. `gf2-core` is the lowest library layer of the gf2
workspace and has no production dependency on another workspace crate except
the kernel crate [`gf2-kernels-simd`](../gf2-kernels-simd/README.md).

## When to choose it

Choose `gf2-core` for dense or sparse $\mathrm{GF}(2)$ matrices, or for an
algorithm written once over `FiniteField` and run over several field
families. The crates above it add domain layers:
[`gf2-coding`](../gf2-coding/README.md) for codes, modems and channels,
[`gf2-algebra`](../gf2-algebra/README.md) for packed $\mathbb{F}_3$,
$\mathbb{F}_5$ and $\mathbb{F}_7$ arithmetic and permanents, and
[`gf2-sim`](../gf2-sim/README.md) for simulation orchestration. Statistical
estimators are available from [`gf2-stats`](../gf2-stats/README.md).

## Documentation

- [Documentation index](../../docs/index.md)
- [Finite-field arithmetic](../../docs/concepts/finite-field-arithmetic.md):
  field families, representations and the bit-indexing convention.
- [Finite-field linear algebra at scale](../../docs/tutorials/field-linear-algebra-at-scale.md):
  multiplication, decomposition and solving over `FieldMatrix`.
- [Acceleration architecture](../../docs/concepts/acceleration-architecture.md)
  and [select SIMD, GPU and parallel execution](../../docs/how-to/select-acceleration.md).
- [Cargo features](../../docs/reference/supported-configurations.md#cargo-features),
  defined in the `[features]` table of [`Cargo.toml`](Cargo.toml), and
  [installation](../../docs/reference/supported-configurations.md#installation).
- Build and test commands: [`AGENTS.md`](../../AGENTS.md#supported-toolchain-and-commands).

```bash
cargo doc -p gf2-core --no-deps
```

Rendered output: Rustdoc.

## License

MIT, see [LICENSE-MIT](../../LICENSE-MIT).
