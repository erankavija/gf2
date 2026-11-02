# gf2 development directory

`dev/` holds gf2's developer documentation, working documents and
development tooling. An experiment's code,
protocol and configuration live at
[`experiments/<name>/`](../experiments/). Permanent documentation is indexed at
[`docs/index.md`](../docs/index.md), and [`AGENTS.md`](../AGENTS.md) is the
engineering contract for all of them.

The [eBCH OSD](../experiments/osd-ebch/README.md) and
[DVB-T2 AWGN](../experiments/dvb-t2-awgn/README.md) campaign crates live under
`experiments/`.

The `[documentation]` table of [`.jit/config.toml`](../.jit/config.toml)
declares the development root, the issue-scoped area and the permanent paths.
`jit config get documentation` prints it.

## Epic documents: `active/`

`active/<epic-short-id>-<slug>/` holds the prose and code of one open epic, per
`@/inv/active-document-layout`, and `active/<issue-short-id>/` those of one open
issue outside an epic. The epic id resolves the directory, and `jit doc add`
links each document to its owning issue:

```bash
mkdir -p "$(jit doc dir <epic-id> dev/active)"
jit doc add <issue-id> <path>
```

`scripts/check-tracked-budget.py` refuses other file kinds there.

## Operational directories

Place material by the first column. The last column names code that reads or
writes the directory.

| Material | Directory | Readers and writers |
|---|---|---|
| `sim_runner` campaign definitions | `campaigns/` | `crates/gf2-coding/src/bin/sim_runner.rs`, `crates/gf2-coding/tests/grand_phase1_smoke.rs` |
| Reference curves, their comparison scripts, complete tuning profiles | `reference_data/` | `crates/gf2-coding/tests/ebch_128_64_reference.rs`, `crates/gf2-algebra/tests/tuning_repository_envelopes.rs`, `dev/tools/tuning-profile-compose/src/main.rs`, `dev/scripts/gen_fig7_report.sh` |
| Standalone prototype crates | `research/` | `crates/gf2-kernels-simd/Cargo.toml` (dev-dependency), `.jit/gates.toml`, `experiments/permanent-benchmarks/scripts/permanent-repro.sh` |
| Benchmark lock wrapper, benchmark window and its queue, campaign launchers | `scripts/` | `scripts/cargo-ci.sh`, [`AGENTS.md`](../AGENTS.md) |
| Tuning-campaign crates | `tools/` | Root `Cargo.toml` (workspace member), dev-dependency of `crates/gf2-core` and `crates/gf2-algebra`, `scripts/cargo-ci.sh` step `tuning-composer` |

Harness output is written below the gitignored `target/` directory.
 