# gf2-algebra

Packed arithmetic over $\mathbb{F}_3$, $\mathbb{F}_5$ and $\mathbb{F}_7$ and
matrix-permanent algorithms built on it. The crate depends on
[`gf2-core`](../gf2-core/README.md) for `FiniteField`, `Fp<P>` and `BitVec`.

## When to choose it

Choose `gf2-algebra` to compute matrix permanents over these fields or to run
lane-parallel arithmetic over them through the `PackedField` traits.
`gf2-core` holds the scalar `Fp<P>` field and the field linear algebra beneath
it, and [`gf2-sim`](../gf2-sim/README.md) runs the checkpointed
permanent-zero-fraction campaigns over its permanents.

## Documentation

- [Documentation index](../../docs/index.md)
- [Finite-field arithmetic](../../docs/concepts/finite-field-arithmetic.md#choosing-a-field-type):
  the packed element types and their lane layouts.
- [Acceleration architecture](../../docs/concepts/acceleration-architecture.md):
  SIMD, HIP and rayon paths and their fallbacks.
- [Performance evidence](../../docs/reference/performance-evidence.md#f3-permanent-batched-avx2):
  the measured permanent kernel.
- [Cargo features](../../docs/reference/supported-configurations.md#cargo-features),
  defined in the `[features]` table of [`Cargo.toml`](Cargo.toml), and
  [installation](../../docs/reference/supported-configurations.md#installation).
- Build and test commands: [`AGENTS.md`](../../AGENTS.md#supported-toolchain-and-commands).

```bash
./scripts/cargo-budget.sh cargo doc -p gf2-algebra --no-deps
```

Rendered output: [Rustdoc](../../target/doc/gf2_algebra/index.html).

## License

MIT, see [LICENSE-MIT](../../LICENSE-MIT).
