# gf2 development directory

`dev/` is gf2's developer documentation, with the working documents,
measurement evidence and tooling of its development. An experiment's code,
protocol, configuration, results and small receipts live at
[`experiments/<name>/`](../experiments/). Permanent documentation is indexed at
[`docs/index.md`](../docs/index.md), and [`AGENTS.md`](../AGENTS.md) is the
engineering contract for all of them.

The [eBCH OSD](../experiments/osd-ebch/README.md) and
[DVB-T2 AWGN](../experiments/dvb-t2-awgn/README.md) campaign crates live under
`experiments/`. Their published records remain under
[`simulation_results/osd-ebch-128-64/`](simulation_results/osd-ebch-128-64/) and
[`benchmarks/dvb_t2_awgn/`](benchmarks/dvb_t2_awgn/), respectively.

The `[documentation]` table of [`.jit/config.toml`](../.jit/config.toml)
declares the development root, the archive root, the paths archival relocates
or retains, and the issue-scoped areas. `jit config get documentation` prints
it.

## Epic documents: `active/`

`active/<epic-short-id>-<slug>/` holds the documents of one non-terminal epic,
per `@/inv/active-document-layout`. The epic id resolves the directory, and
`jit doc add` links each document to its owning issue:

```bash
mkdir -p "$(jit doc dir <epic-id> dev/active)"
jit doc add <issue-id> <path>
```

`active/` holds four further forms of entry, per
`@/issue/fa787f85/decision/DEC-01` and `@/issue/fa787f85/decision/DEC-02`:

| Entry | Held at its path by |
|---|---|
| Flat entry `active/<issue-short-id>/` of an open issue | The tracked move issue that names it |
| Flat entry of a done task that tooling reads at its path | That tooling; a tracked move issue names the entry |
| Files of an archived epic, in its directory or in a flat entry of one of its done tasks | A commit-pinned tracker document reference, or a link from a byte-fixed receipt or protocol |
| Directory that the session of another running epic places | That epic, which links its files to their owning issues |

`jit doc list <issue-id>` prints the document references of an entry's owning
issue, and `jit search <issue-id>` lists the issues that name the entry, the
move issue among them.

[authoring-conventions.md](authoring-conventions.md) gives the asset and link
patterns that keep a linked document resolvable through archival.

## Archives: `archive/`

| Entry | Holds | Written by |
|---|---|---|
| `archive/<short-id>-<slug>/` | Issue-linked artifacts of one archived container at their source paths relative to `dev/`, beside a `.jit-container` marker | `jit archive container <id> --execute` |
| `archive/legacy/<original-path>` | Material without an owning container, at its original repository-relative path | `git mv` |

## Operational directories

Place material by the first column. The last column names code that reads or
writes the directory.

| Material | Directory | Readers and writers |
|---|---|---|
| Benchmark results, configuration and input digests | `bench_results/` | `experiments/permanent-statistics/tests/rank_event_rates.rs` |
| Benchmark output tables, campaign measurement plans, tuning-profile receipts | `benchmarks/` | Read by `dev/scripts/permanent-campaign-runner.sh`; written by `scripts/perm-uniformity-repro.sh` and `dev/tools/tuning-campaign-support` |
| `sim_runner` campaign definitions | `campaigns/` | `crates/gf2-coding/src/bin/sim_runner.rs`, `crates/gf2-coding/tests/grand_phase1_smoke.rs` |
| Reference curves, their comparison scripts, complete tuning profiles | `reference_data/` | `crates/gf2-coding/tests/ebch_128_64_reference.rs`, `crates/gf2-algebra/tests/tuning_repository_envelopes.rs`, `dev/tools/tuning-profile-compose/src/main.rs`, `dev/scripts/gen_fig7_report.sh` |
| Standalone prototype crates | `research/` | `crates/gf2-kernels-simd/Cargo.toml` (dev-dependency), `.jit/gates.toml`, `experiments/permanent-benchmarks/scripts/permanent-repro.sh`, `dev/scripts/permanent-campaign-runner.sh` |
| CI checks, the benchmark lock wrapper, campaign runners | `scripts/` | `scripts/cargo-ci.sh`, [`AGENTS.md`](../AGENTS.md) |
| Campaign output and the permanent zero-fraction datasets | `simulation_results/` | `output_dir` of the `campaigns/` definitions, historical permanent receipts, `crates/gf2-coding/tests/grand_phase1_smoke.rs` |
| Study inputs and measurement logs, one directory per issue | `studies/` | Written by `dev/scripts/permanent-campaign-runner.sh` |
| Tuning-campaign crates | `tools/` | Root `Cargo.toml` (workspace member), dev-dependency of `crates/gf2-core` and `crates/gf2-algebra`, `scripts/cargo-ci.sh` step `tuning-composer` |
