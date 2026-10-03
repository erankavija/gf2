# gf2-sim

Simulation orchestration layer of the gf2 workspace. `gf2-sim` composes the
codes and modems of [`gf2-coding`](../gf2-coding/README.md) into a `Pipeline`
of typed `Stage`s and runs seeded, checkpointed error-rate sweeps on CPU
workers and, with the `hip` feature, on a HIP device. It also holds the
checkpointed ordered-statistics-decoding and permanent-zero-fraction campaigns
built on `gf2-coding`, [`gf2-algebra`](../gf2-algebra/README.md) and
[`gf2-stats`](../gf2-stats/README.md).

## When to choose it

Choose `gf2-sim` when a study needs an error-rate sweep or a campaign that is
seeded, distributed over workers and resumable from checkpoints. The component
crates serve work below that layer: `gf2-coding` for codes, decoders, modems,
channel models and its `sim_runner` campaign files, `gf2-algebra` for
permanent computation, `gf2-stats` for sampling and interval statistics, and
[`gf2-core`](../gf2-core/README.md) for field arithmetic and linear algebra.

## Documentation

- [Documentation index](../../docs/index.md)
- [Coded-modulation link simulation](../../docs/tutorials/coded-modulation-link-simulation.md):
  a checkpointed sweep through a preset pipeline.
- [Run a simulation campaign](../../docs/how-to/run-simulation-campaigns.md)
  and [select SIMD, GPU and parallel execution](../../docs/how-to/select-acceleration.md).
- [Acceleration architecture](../../docs/concepts/acceleration-architecture.md):
  the [hybrid executor](../../docs/concepts/acceleration-architecture.md#hybrid-executor)
  and the determinism contract under
  [parallelism](../../docs/concepts/acceleration-architecture.md#parallelism).
- [Cargo features](../../docs/reference/supported-configurations.md#cargo-features),
  defined in the `[features]` table of [`Cargo.toml`](Cargo.toml), and
  [installation](../../docs/reference/supported-configurations.md#installation).
- Build and test commands: [`AGENTS.md`](../../AGENTS.md#supported-toolchain-and-commands).

```bash
./scripts/cargo-budget.sh cargo doc -p gf2-sim --no-deps
```

Rendered output: [Rustdoc](../../target/doc/gf2_sim/index.html).

## License

MIT, see [LICENSE-MIT](../../LICENSE-MIT).
