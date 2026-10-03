# gf2-coding

Error-correcting codes, soft decoders, modems, channels and Monte Carlo
simulation over the finite-field and bit-matrix primitives of
[`gf2-core`](../gf2-core/README.md).

## When to choose it

Choose `gf2-coding` for code constructions, decoders, modem and channel
models, including the DVB-T2 (`@/citation/Etsi2015`) and 5G NR
(`@/citation/ThreeGpp2017`) codes. `gf2-core` holds the field arithmetic, bit
storage and linear algebra beneath it, and
[`gf2-sim`](../gf2-sim/README.md) composes its components into checkpointed
simulation pipelines and campaigns.

## Documentation

- [Documentation index](../../docs/index.md)
- [Standards conformance](../../docs/reference/standards-conformance.md):
  supported DVB-T2 and 5G NR configurations, bit order and external evidence.
- [Run a simulation campaign](../../docs/how-to/run-simulation-campaigns.md)
  and [coded-modulation link simulation](../../docs/tutorials/coded-modulation-link-simulation.md).
- [Acceleration architecture](../../docs/concepts/acceleration-architecture.md)
  and [select SIMD, GPU and parallel execution](../../docs/how-to/select-acceleration.md).
- [Cargo features](../../docs/reference/supported-configurations.md#cargo-features),
  defined in the `[features]` table of [`Cargo.toml`](Cargo.toml), and
  [installation](../../docs/reference/supported-configurations.md#installation).
- Build and test commands: [`AGENTS.md`](../../AGENTS.md#supported-toolchain-and-commands).

```bash
./scripts/cargo-budget.sh cargo doc -p gf2-coding --no-deps
```

Rendered output: [Rustdoc](../../target/doc/gf2_coding/index.html).

## License

MIT, see [LICENSE-MIT](../../LICENSE-MIT).
