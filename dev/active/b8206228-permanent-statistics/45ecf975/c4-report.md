Issue fd1ef344 — C4 of umbrella 45ecf975. Implementation handoff.

Worktree: `agent-fd1ef344`. Specification: [c4-design.md](/dev/active/b8206228-permanent-statistics/45ecf975/c4-design.md). Final checks completed before the 03:30 Helsinki cutoff; report generated at Mon Nov  2 02:34:09 EET 2026.

The permanent campaign, dataset, and validation binaries build from `experiments/permanent-statistics`. The gf2-sim campaign layer, continuation/frozen decoders, emission authorization, root_fs, and compiled paper data are removed. OSD retains its byte-compatible identity types in `osd_identity.rs`, with the named exception linked to @/issue/779d1926. The experiment uses one shared execution log, configured cell/shard expansion, exact log-scale decisions, Wilson intervals, per-cell run identities, explicit availability policy, and terminal failure propagation. Validation probes unsampled zero matrices before opening streams, preserves exhaustive backend disagreement, and resumes at terminal anchor boundaries.

No git commit or state-changing JIT call was made. AGENTS.md, .jit/, gf2-stats, experiments/common, and the benchmark output path string are untouched. The read-only issue lookup reported backlog/unassigned; implementation proceeded under the lead ownership stated in the request.

The literal criteria are covered as follows:

| Criterion | Evidence |
|---|---|
| REQ-01 | Workspace build passes. `git grep -n permanent_campaign -- crates` returns only `batched_f3_permanent.rs:455`, the preserved benchmark path. No permanent_* binary source remains in gf2-sim. |
| REQ-02 | The campaign directory and three binaries are deleted from gf2-sim. The analysis script loads the target CSV and literature claim through manifest reference_data paths. The moved target CSV retains identical bytes. |
| REQ-03 | Resume compares RunIdentity per cell. Source revision is a rebuild pointer; backend evidence paths are copied, never verified. Repository paths use the shared root abstraction. The root-discovery query is noted below. |
| REQ-04 | One shared campaign contract covers q=3,5,7, supporting CPU backends, workers 1/2/4, bounded resume, accepted-log/checkpoint interruption, explicit fallback, and a device-gated HIP path. The CPU fake dispatcher exercises the production accelerator batching loop. |
| REQ-05 | C3 and C4 total 4,825 Rust code lines, including source and tests; ceiling 6,000. |

Code lines by module follow. Counts use every Rust file in the experiment and the design method `awk 'NF && !/^\s*\/\//'` (nonblank, noncomment lines).

| Module | Code lines |
|---|---:|
| [src/bin/permanent_campaign.rs](/experiments/permanent-statistics/src/bin/permanent_campaign.rs) | 84 |
| [src/bin/permanent_dataset.rs](/experiments/permanent-statistics/src/bin/permanent_dataset.rs) | 20 |
| [src/bin/permanent_rare_event.rs](/experiments/permanent-statistics/src/bin/permanent_rare_event.rs) | 58 |
| [src/bin/permanent_validation.rs](/experiments/permanent-statistics/src/bin/permanent_validation.rs) | 54 |
| [src/bin/support/mod.rs](/experiments/permanent-statistics/src/bin/support/mod.rs) | 38 |
| [src/campaign/config.rs](/experiments/permanent-statistics/src/campaign/config.rs) | 203 |
| [src/campaign/dataset.rs](/experiments/permanent-statistics/src/campaign/dataset.rs) | 517 |
| [src/campaign/decide.rs](/experiments/permanent-statistics/src/campaign/decide.rs) | 87 |
| [src/campaign/evaluate.rs](/experiments/permanent-statistics/src/campaign/evaluate.rs) | 402 |
| [src/campaign/mod.rs](/experiments/permanent-statistics/src/campaign/mod.rs) | 67 |
| [src/campaign/runner.rs](/experiments/permanent-statistics/src/campaign/runner.rs) | 515 |
| [src/campaign/validation.rs](/experiments/permanent-statistics/src/campaign/validation.rs) | 399 |
| [src/execution_log.rs](/experiments/permanent-statistics/src/execution_log.rs) | 64 |
| [src/lib.rs](/experiments/permanent-statistics/src/lib.rs) | 11 |
| [src/rare_event/mod.rs](/experiments/permanent-statistics/src/rare_event/mod.rs) | 257 |
| [src/rare_event/runner.rs](/experiments/permanent-statistics/src/rare_event/runner.rs) | 346 |
| [tests/campaign_bins.rs](/experiments/permanent-statistics/tests/campaign_bins.rs) | 135 |
| [tests/campaign_dataset.rs](/experiments/permanent-statistics/tests/campaign_dataset.rs) | 128 |
| [tests/campaign_determinism.rs](/experiments/permanent-statistics/tests/campaign_determinism.rs) | 203 |
| [tests/campaign_evaluate.rs](/experiments/permanent-statistics/tests/campaign_evaluate.rs) | 286 |
| [tests/campaign_runner.rs](/experiments/permanent-statistics/tests/campaign_runner.rs) | 274 |
| [tests/campaign_validation.rs](/experiments/permanent-statistics/tests/campaign_validation.rs) | 222 |
| [tests/common/mod.rs](/experiments/permanent-statistics/tests/common/mod.rs) | 58 |
| [tests/permanent_rare_event.rs](/experiments/permanent-statistics/tests/permanent_rare_event.rs) | 194 |
| [tests/rare_event_runner.rs](/experiments/permanent-statistics/tests/rare_event_runner.rs) | 203 |
| **Total** | **4,825** |

Verification uses the repository commands: the workspace build, the
`permanent-statistics` and `gf2-sim` fast tier, clippy, fmt, `./scripts/cargo-ci.sh`,
the HIP feature build, the Rust 1.95 release binaries that regenerate the
analysis fixtures, and the analysis unit tests. Cargo commands set
`CARGO_NET_OFFLINE=true` and `CARGO_CI_NO_SCCACHE=1`.

Test-first evidence includes the missing experiment API, rejection replay after an interruption before halt propagation, and invalid validation preflight. Those cases failed before implementation/fix and pass in the final suites. Dispatch panic diagnostics are tested through the same guard used by campaign execution and validation.

Design differences and discrepancies are explicit:

- The required executed_backend observation prevents byte-identical shard JSONL across different executed backends. Tests compare complete bytes across workers and resume within a backend, identical summary.csv across backends, and identical shard mathematical payloads after checking/normalizing only executed_backend. Fallback records the backend that actually ran.
- The design allocates each family budget over K = all cells. The removed code divided the determinant family budget by the count of determinant-enabled cells. The implementation follows the design; allocations therefore differ for configurations with only some determinant companions. The configured Wilson z also follows the design, including its longer 95% value. The statistical kernels, tails, finite-n determinant null, and inclusive log-scale comparator are unchanged.
- The shared RepositoryRoot::containing API runs git rev-parse --show-toplevel before RunRecord::observe reads the rebuild pointer. This is an additional location query to the design sentence saying the only git read is observation. No git revision, dirty flag, receipt, or path location controls acceptance. experiments/common is unchanged.
- Data and fixture moves use filesystem operations because git metadata is read-only. The lead can stage and commit the resulting tree.

The Wilson-only default, compact manifest with no shard list, one JSONL per cell, and --on-unavailable defaults are implemented as written. No other design deviation is recorded.

The C4 implementation has no remaining functional blocker. HIP feature compilation succeeds, with the existing optional gfx940 compiler probe skipped by this host. No usable device is exposed, so real-device tests self-gate; GPU runtime equivalence is unverified here. The separate HIP-crate build resolves an uncached zerocopy-derive 0.8.63 and cannot download it while offline. Dependency pins and network settings are unchanged.

Changed files are listed completely below. `new` denotes an untracked prepared file; `M` and `D` are git diff statuses. Counts above include the whole experiment, including unchanged C3 modules.

| Status | Path |
|---|---|
| M | `.config/nextest.toml` |
| M | `Cargo.lock` |
| M | `crates/gf2-sim/Cargo.toml` |
| M | `crates/gf2-sim/README.md` |
| M | `ebch_osd_awgn_campaign.rs` |
| D | `permanent_campaign.rs` |
| D | `permanent_dataset.rs` |
| D | `permanent_validation.rs` |
| M | `crates/gf2-sim/src/checkpoint/mod.rs` |
| M | `crates/gf2-sim/src/lib.rs` |
| M | `osd_campaign.rs` |
| new | `osd_identity.rs` |
| D | `acceptance.rs` |
| D | `coordinator.rs` |
| D | `coordinator_tests.rs` |
| D | `fixture.rs` |
| D | `launch_cost.rs` |
| D | `mod.rs` |
| D | `provenance.rs` |
| D | `root_fs.rs` |
| D | `schedule.rs` |
| D | `schema.rs` |
| D | `validation.rs` |
| D | `no_public_field_sidecar_mutator.rs` |
| D | `no_public_field_sidecar_mutator.stderr` |
| M | `osd_campaign_protocol.rs` |
| D | `permanent_campaign_bin.rs` |
| D | `permanent_validation.rs` |
| M | `dev/index.md` |
| M | simulation-campaign execution support |
| M | simulation-campaign execution tests |
| D | `checksums.sha256` |
| D | `manifest.json` |
| D | `shard-000000.json` |
| D | `q3.json` |
| D | `summary.csv` |
| D | `cells.csv` |
| D | `curve-q3.svg` |
| D | `checksums.sha256` |
| D | `manifest.json` |
| D | `shard-000000.json` |
| D | `q9.json` |
| D | `summary.csv` |
| D | `checksums.sha256` |
| D | `manifest.json` |
| D | `shard-000000.json` |
| D | `shard-000001.json` |
| D | `q3.json` |
| D | `q5.json` |
| D | `summary.csv` |
| M | `protocol.md` |
| D | `scheinerman2024-q3-targets-v1.csv` |
| M | `experiments/permanent-statistics/Cargo.toml` |
| M | `experiments/permanent-statistics/README.md` |
| new | `experiments/permanent-statistics/config/small-campaign.json` |
| new | `experiments/permanent-statistics/data/literature-claim.json` |
| new | `experiments/permanent-statistics/data/scheinerman2024-q3-targets-v1.csv` |
| new | `experiments/permanent-statistics/src/bin/permanent_campaign.rs` |
| new | `experiments/permanent-statistics/src/bin/permanent_dataset.rs` |
| M | `experiments/permanent-statistics/src/bin/permanent_rare_event.rs` |
| new | `experiments/permanent-statistics/src/bin/permanent_validation.rs` |
| new | `experiments/permanent-statistics/src/bin/support/mod.rs` |
| new | `experiments/permanent-statistics/src/campaign/config.rs` |
| new | `experiments/permanent-statistics/src/campaign/dataset.rs` |
| new | `experiments/permanent-statistics/src/campaign/decide.rs` |
| new | `experiments/permanent-statistics/src/campaign/evaluate.rs` |
| new | `experiments/permanent-statistics/src/campaign/mod.rs` |
| new | `experiments/permanent-statistics/src/campaign/runner.rs` |
| new | `experiments/permanent-statistics/src/campaign/validation.rs` |
| new | `experiments/permanent-statistics/src/execution_log.rs` |
| M | `experiments/permanent-statistics/src/lib.rs` |
| M | `experiments/permanent-statistics/src/rare_event/runner.rs` |
| new | `experiments/permanent-statistics/tests/campaign_bins.rs` |
| new | `experiments/permanent-statistics/tests/campaign_dataset.rs` |
| new | `experiments/permanent-statistics/tests/campaign_determinism.rs` |
| new | `experiments/permanent-statistics/tests/campaign_evaluate.rs` |
| new | `experiments/permanent-statistics/tests/campaign_runner.rs` |
| new | `experiments/permanent-statistics/tests/campaign_validation.rs` |
| new | `experiments/permanent-statistics/tests/common/mod.rs` |
| new | `experiments/permanent-statistics/tests/fixtures/analysis/checksum-mismatch/checksums.sha256` |
| new | `experiments/permanent-statistics/tests/fixtures/analysis/checksum-mismatch/execution.jsonl` |
| new | `experiments/permanent-statistics/tests/fixtures/analysis/checksum-mismatch/manifest.json` |
| new | `experiments/permanent-statistics/tests/fixtures/analysis/checksum-mismatch/runs.json` |
| new | `experiments/permanent-statistics/tests/fixtures/analysis/checksum-mismatch/shards/q3-n02.jsonl` |
| new | `experiments/permanent-statistics/tests/fixtures/analysis/checksum-mismatch/shards/q5-n02.jsonl` |
| new | `experiments/permanent-statistics/tests/fixtures/analysis/checksum-mismatch/shards/q7-n02.jsonl` |
| new | `experiments/permanent-statistics/tests/fixtures/analysis/checksum-mismatch/summary.csv` |
| new | `experiments/permanent-statistics/tests/fixtures/analysis/golden/small-campaign/cells.csv` |
| new | `experiments/permanent-statistics/tests/fixtures/analysis/golden/small-campaign/curve-q3.svg` |
| new | `experiments/permanent-statistics/tests/fixtures/analysis/golden/small-campaign/curve-q5.svg` |
| new | `experiments/permanent-statistics/tests/fixtures/analysis/golden/small-campaign/curve-q7.svg` |
| new | `experiments/permanent-statistics/tests/fixtures/analysis/golden/small-campaign/literature-claim.json` |
| new | `experiments/permanent-statistics/tests/fixtures/analysis/halted-config.json` |
| new | `experiments/permanent-statistics/tests/fixtures/analysis/invalid-q/checksums.sha256` |
| new | `experiments/permanent-statistics/tests/fixtures/analysis/invalid-q/execution.jsonl` |
| new | `experiments/permanent-statistics/tests/fixtures/analysis/invalid-q/manifest.json` |
| new | `experiments/permanent-statistics/tests/fixtures/analysis/invalid-q/runs.json` |
| new | `experiments/permanent-statistics/tests/fixtures/analysis/invalid-q/shards/q3-n02.jsonl` |
| new | `experiments/permanent-statistics/tests/fixtures/analysis/invalid-q/shards/q5-n02.jsonl` |
| new | `experiments/permanent-statistics/tests/fixtures/analysis/invalid-q/shards/q7-n02.jsonl` |
| new | `experiments/permanent-statistics/tests/fixtures/analysis/invalid-q/summary.csv` |
| new | `experiments/permanent-statistics/tests/fixtures/analysis/small-campaign-halted/checksums.sha256` |
| new | `experiments/permanent-statistics/tests/fixtures/analysis/small-campaign-halted/execution.jsonl` |
| new | `experiments/permanent-statistics/tests/fixtures/analysis/small-campaign-halted/manifest.json` |
| new | `experiments/permanent-statistics/tests/fixtures/analysis/small-campaign-halted/runs.json` |
| new | `experiments/permanent-statistics/tests/fixtures/analysis/small-campaign-halted/shards/q3-n02.jsonl` |
| new | `experiments/permanent-statistics/tests/fixtures/analysis/small-campaign-halted/shards/q5-n02.jsonl` |
| new | `experiments/permanent-statistics/tests/fixtures/analysis/small-campaign-halted/shards/q7-n02.jsonl` |
| new | `experiments/permanent-statistics/tests/fixtures/analysis/small-campaign-halted/summary.csv` |
| new | `experiments/permanent-statistics/tests/fixtures/analysis/small-campaign/checksums.sha256` |
| new | `experiments/permanent-statistics/tests/fixtures/analysis/small-campaign/execution.jsonl` |
| new | `experiments/permanent-statistics/tests/fixtures/analysis/small-campaign/manifest.json` |
| new | `experiments/permanent-statistics/tests/fixtures/analysis/small-campaign/runs.json` |
| new | `experiments/permanent-statistics/tests/fixtures/analysis/small-campaign/shards/q3-n02.jsonl` |
| new | `experiments/permanent-statistics/tests/fixtures/analysis/small-campaign/shards/q5-n02.jsonl` |
| new | `experiments/permanent-statistics/tests/fixtures/analysis/small-campaign/shards/q7-n02.jsonl` |
| new | `experiments/permanent-statistics/tests/fixtures/analysis/small-campaign/summary.csv` |
| M | `scripts/permanent_zero_fraction_analysis.py` |
| M | `tests/test_permanent_zero_fraction_analysis.py` |
