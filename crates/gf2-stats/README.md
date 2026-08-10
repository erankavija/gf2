# gf2-stats

`gf2-stats` is the narrow statistics layer for reproducible finite-field
campaigns. It depends on [`gf2-core`](../gf2-core/README.md) for the shared
mathematical primitives while keeping campaign statistics out of the algebra
and simulation layers.

The crate provides a reproducible matrix sampler and count-based binomial
confidence intervals. Use the Wilson score interval for larger-count,
normal-score approximations; use the conservative Clopper-Pearson interval
when rare-event cells have observed or expected counts in the tens.

Later changes add exact statistical tests and a streaming shard accumulator.
