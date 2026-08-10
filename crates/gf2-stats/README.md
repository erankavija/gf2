# gf2-stats

`gf2-stats` is the narrow statistics layer for reproducible finite-field
campaigns. It depends on [`gf2-core`](../gf2-core/README.md) for the shared
mathematical primitives while keeping campaign statistics out of the algebra
and simulation layers.

The crate provides a reproducible matrix sampler, count-based binomial
confidence intervals, and exact binomial hypothesis tests. Use the Wilson score
interval for larger-count normal-score approximations and the conservative
Clopper-Pearson interval when rare-event cells have observed or expected counts
in the tens. Campaign acceptance decisions use the exact tests, whose log-scale
results retain a decision even when a p-value would underflow as a direct
probability.

Later changes add a streaming shard accumulator.
