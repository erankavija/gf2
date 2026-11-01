# Permanent statistics

`rare_event` estimates three-column permanental-rank deficiency over F₃, F₅,
and F₇ with nullspace importance trajectories and exact likelihood-exponent
histograms. `gf2-stats::weighted` reduces independent runs into likelihood
means, final-weight ESS, mean variance, and outward-rounded Student intervals.
The caller supplies the critical value appropriate to the independent-run
count and intended confidence level.

The runner reads its seed, field, matrix size, run allocation, prefix size,
and critical value from a JSON file. [config/small.json](config/small.json)
is a small invocation example. From the repository root:

```sh
CARGO_NET_OFFLINE=true CARGO_CI_NO_SCCACHE=1 ./scripts/cargo-budget.sh cargo run \
  --release -p permanent-statistics --bin permanent_rare_event -- \
  experiments/permanent-statistics/config/small.json /tmp/permanent-example 3
```

The optional final argument bounds the invocation to that many prefix
boundaries. Exit status 10 means progress remains. Repeat the command to
continue, or omit that argument to finish the configured allocation.

Each output directory has one writer and one append-only `execution.jsonl`,
opened before sampling. Synced progress records precede atomic publication
through `gf2-sim::checkpoint`; resume recovers a lagging checkpoint from the
log. A torn append is retained and the next append starts a separate line.
Completed trajectories remain in histograms. Only the active trajectory's
prefix is replayed to reconstruct its sampler and compressed state.

Resume compares `experiments_common::RunIdentity`: executable digest, build
flags, frozen configuration digest, seed, RNG implementation and version,
toolchain, and observed host facts. Configuration serialization determines
its digest, so JSON whitespace and key order are immaterial. Source revision
is recorded as a rebuild pointer. The binary observes the checkout containing
its working directory through `experiments_common::RepositoryRoot`.

On completion, `result.json` contains the run record, configuration, sample
and independent-run counts, exact decimal ratios, and interval endpoints.
The execution log records each invocation's terminal status. The sampler and
runner belong to `@/issue/e3370140`.
