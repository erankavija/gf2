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
CARGO_NET_OFFLINE=true cargo run \
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


`campaign` draws uniformly sampled square matrices over F₃, F₅, and F₇,
compares specialized, generic Ryser, and optional HIP implementations, and
records permanent residue histograms with optional paired determinants.
[config/small-campaign.json](config/small-campaign.json) demonstrates the
JSON configuration. Its cells and sample counts are test inputs.

```sh
CARGO_NET_OFFLINE=true cargo run \
  --release -p permanent-statistics --bin permanent_campaign -- \
  --config experiments/permanent-statistics/config/small-campaign.json \
  --output /tmp/small-campaign --q 3 --n 2 --workers 4 --max-shards 1
```

Repeat an invocation returning status 10 to finish its cell, then select the
remaining cells in configuration order. The configured first cell must
terminate before another starts; admission allows only one active cell. A
reported dispatch error spends a retry, while an interrupted draw restarts
at the same address. An exact rejection or exhausted retry allocation halts
remaining cells and preserves accepted counts. The default unavailable-backend
policy refuses without writing. `--on-unavailable halt` records a terminal
mechanical halt; `--on-unavailable fallback=batch_parallel` explicitly selects
a supporting CPU implementation. Each shard records its executed backend.

Schema 3 stores a compact `manifest.json`, one synced `execution.jsonl`, one
`q<q>-n<nn>.jsonl` under `shards` per cell, and a recoverable `checkpoint.json`. The
manifest contains no shard inventory. `permanent_dataset finalize DIR`
requires terminal cells and publishes `summary.csv`, `runs.json`, and
`checksums.sha256`; `permanent_dataset verify DIR` checks these projections
against the log and pooled shard evidence. Checkpoints are excluded from
integrity membership. Per-cell resume compares the closed run identity;
worker counts and fallback choices are invocation observations.

Permanent decisions use the one-sided exact binomial lower tail at `1/q`.
Determinants use the probability-ordering two-sided exact test at
`1 - product(1 - q^-i, i=1..n)`. The permanent budget divides across every
registered cell and the determinant budget across companion-enabled cells. Decisions compare log p-values inclusively;
reports use the configuration's Wilson critical value. The analysis script
reads target counts and the literature claim from the manifest's committed
reference-data paths.

The campaign `permanent-zero-fraction-20261102` is governed by
[protocol.md](protocol.md), with [config/campaign.json](config/campaign.json),
[preregistration.json](preregistration.json), and a pre-draw manifest under
`config/`. `permanent_validation --check PATH` parses a configuration and
reports it without opening a stream, a device, or a file.

`permanent_validation --config PATH --output DIR --workers W` runs independent
fixed-expansion exact anchors, determinant counts, fresh-stream regeneration,
and sampled exact-null tests on `StreamPurpose::Validation`. The optional
`--max-anchors K` bounds an invocation; terminal anchors are adopted on resume,
including failures. Status 0 means passed, 1 means failed or error, 10 means
anchors remain, and 2 means invalid arguments. The `hip` feature enables
accelerator dispatch and requires the ROCm toolchain. These campaign contracts
belong to `@/issue/fd1ef344`.
