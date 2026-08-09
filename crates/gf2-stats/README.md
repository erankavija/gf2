# gf2-stats

`gf2-stats` is the narrow statistics layer for reproducible finite-field
campaigns. It depends on [`gf2-core`](../gf2-core/README.md) for the shared
mathematical primitives while keeping campaign statistics out of the algebra
and simulation layers.

The crate is intentionally an empty foundation. Later changes add these
surfaces:

- a reproducible matrix sampler;
- interval estimators;
- exact statistical tests; and
- a streaming shard accumulator.
