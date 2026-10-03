# gf2-stats

Statistics library for reproducible finite-field campaigns, built on
[`gf2-core`](../gf2-core/README.md). Public modules: `sampler` (seeded matrix
sampling), `intervals` (Wilson and Clopper-Pearson binomial intervals),
`binomial` (exact binomial tests with log-scale results), `accumulator`
(streaming shard accumulator with JSON snapshots) and `weighted` (exact
weighted-run reduction from exponent histograms).

```bash
./scripts/cargo-budget.sh cargo doc -p gf2-stats --no-deps
```

Rendered output: [Rustdoc](../../target/doc/gf2_stats/index.html).
