# C4 design: the small permanent campaign (`@/issue/fd1ef344`)

Goal: replace `permanent_campaign/` and three bins with `experiments/permanent-statistics/src/campaign/`. Budget: src about 2,760 new, tests about 1,580 new, rare_event 1,104 (src 707, tests 397), total about 5,450 code lines against the 6,000 ceiling. Run no timed work; every run below is a test or a queue line.

## Decisions

1. **Statistics are unchanged.** Same estimate, Wilson 95% report interval, one-sided exact floor test at `1/q`, two-sided probability-ordering determinant test at the finite-`n` null `1-prod(1-q^-i)`, Bonferroni level $0.025/K$ within each family, `rejects_at` inclusive, comparison on log scale. Use `gf2_stats::binomial::lower_tail_test(z, n, 1.0/q)`: `permanent_zero_floor_test` is deleted by `@/issue/c251bf43`.
2. **One Wilson z.** Today `summary.csv` uses `gf2_stats::intervals::Z_95` (1.959964) while the sidecars and analysis script use 1.959963984540054. The config's `wilson_z` (the longer value, matching the target table) is the only one the tool uses.
3. **Determinant companion stays**, on the same matrices: it holds half the error budget and feeds epic REQ-05 (`@/issue/0665e7be`). Residue histogram stays: it supplies the record consistency check.
4. **Protocol rules that stay, driven by config:** first-cell gate, serial cell admission, at most `max_shard_attempts` reported failures per shard, every terminal failure halts all other non-terminal cells with the same reason (`Coordinator::terminalize_mechanical`, `halt_other_cells`). These protect `@/inv/falsification-preserved`.
5. **An interruption is not a failure.** A killed process re-runs the same address and redraws identical matrices, so it spends no retry. Panics from a dispatch are caught with `catch_unwind` and count as reported failures. C5 restates the "two executions" clause accordingly.
6. **Comparison lives only in the Python script.** `scripts/permanent_zero_fraction_analysis.py` already computes intervals, relation and precision class from the CSV. All Rust `Q3*`, `Coordinator*Sidecar` and `LiteratureSearchClaim` types go, so the Rust tool reads no paper data.
7. **Dataset size fits `scripts/tracked-budget.toml`** (1 MiB per file, 5 MiB per commit). The draft manifest is 1.25 MB and 14,229 shards at `shard_size` 50,000 cost about 6 MB of files and about 2.8 MB of log. So the manifest stores no shard list, shard records are one JSONL per cell, and `shard_size` is per cell group. C5 chooses sizes for at most 4,500 shards, for example 200,000 in the precision tiers and 50,000 in the frontier tiers (3,630 shards, an estimated 730 KB log).
8. **Accelerator fallback** is an explicit per-invocation choice, default refusal (see CLI). The cell's configured backend never changes silently, and the shard records which backend ran. `gf2_algebra::gpu` has no internal fallback and panics on dispatch failure.
9. **Closed identity only (REQ-03).** Resume and acceptance compare `experiments_common::RunIdentity`. The only git read is `RunRecord::observe` taking the rebuild pointer. Backend evidence is a cited path, never verified. Worker count and fallback are logged run-time observations outside identity.
10. **Identity binds per cell.** A cell's first `start` fixes its identity; resuming it under a different identity refuses. Other cells may use another build (hip and non-hip) or host, but `configuration_sha256` is campaign-wide.
11. **No time ceiling in the tool.** None exists today (no timer in `coordinator.rs`); bounded runs replace it, and C5 states the 12-hour clause as an operator rule or removes it.
12. **Sampler import** within the experiment crate is `crate::sampler`; integration tests use `permanent_statistics::sampler`.
13. **Advisory lock:** `File::try_lock` on the log, as `acquire_execution_lock` does today, catches a double launch in three lines. No symlink or fd-anchored defence.

## Modules

| File under `experiments/permanent-statistics/src/` | Code lines | Content |
|---|---:|---|
| `lib.rs` | 10 | adds `campaign`, `execution_log`, `sampler_rng()` (the `rand_chacha 0.9.0` `RngIdentity`, also used by `permanent_rare_event`) |
| `execution_log.rs` | 70 | `open`, `append<E>`, `read<E>` with torn-line tolerance, lifted from `rare_event/runner.rs` `ExecutionLog` and `logged_progress`; rare_event converts to it, as does its `field_order` serde module |
| `campaign/mod.rs` | 60 | orientation paragraph, `CampaignError`, re-exports |
| `campaign/config.rs` | 300 | `CampaignConfig`, `load`, `validate`, `digest`, group expansion to `Cell` and `ShardSpec` |
| `campaign/evaluate.rs` | 480 | `Backend`, `Kernel`, `evaluate_shard`, launch sizing, hip dispatch |
| `campaign/decide.rs` | 130 | `AcceptancePlan`, `assess_completed_cell` |
| `campaign/dataset.rs` | 520 | records, `manifest.json`, shard JSONL, CSV, `runs.json`, checksums, `verify` |
| `campaign/runner.rs` | 520 | events, state replay, checkpoint, `run_cell`, halt propagation |
| `campaign/validation.rs` | 420 | preregistered checks, resumable at anchor boundaries, `receipt.json` |
| `bin/permanent_campaign.rs`, `permanent_dataset.rs`, `permanent_validation.rs` | 110, 70, 70 | thin; exit codes as `permanent_rare_event` |

`Cargo.toml`: add `rayon` and `hip = ["gf2-algebra/hip"]`; `gf2-algebra` default features cover the kernels, as they do for `gf2-sim`. Digests go through `Sha256Digest::of`, so no `sha2`.

## Existing files

| File | Action | What survives |
|---|---|---|
| `schedule.rs` | shrink to `evaluate.rs` | `launch_size`; `backend_supports_cell`; `resolve_processor_path`; `PackedMatrix`; `evaluate_permanent`; `evaluate_determinant`; `dispatch_accelerator` with `permanent_batch_bipedal{3,5,7}` and `has_usable_device`; `ProductionBackendEvaluator` becomes `Kernel`. `run_shard_for_{serial,batch,accelerator}` merge into one `evaluate_shard<Q>` loop: fill a reused buffer of `batch_len` matrices (1, the chunk rule, or `launch_size`), `Kernel::permanents`, determinants, observer, tally. Drop `PhaseDurations`, `EvaluatedShard`, `AcceleratorCostTable`, `FieldRun`, every `#[cfg(test)]` run or emit helper. The observer becomes production API, since validation uses it |
| `schema.rs` | shrink to `dataset.rs` | `ShardRecord` (flat), `DeterminantPlan` as bool, `SummaryRow`, `CellTerminalState`, `HaltReason`, `AcceptanceVerdict`, `ProportionEstimate`, `encode_summary_csv`, `parse_summary_row`, `validate_summary_aggregate`. Drop `READABLE_SCHEMA_VERSIONS`, `read_json_at_version`, optional v2 fields, `Availability`, `Provenance`, `DATASET_HOME`, `FieldSummary`, `QuarantinedShard`, `DatasetLayout`, `WriterRole`, token types (use `experiments_common`) |
| `acceptance.rs` | shrink to `decide.rs` | `AcceptancePlan` reading budgets from config, `assess_completed_cell`, `determinant_null_probability`. Evidence structs collapse to `log_p_value` plus verdict |
| `coordinator.rs` | delete; intent to `runner.rs` | Lift only `halt_other_cells` logic, `halted_row`, `pooled_accepted_from_receipt` as tally replay, `validate_raw_record` checks. All receipts, authorization persistence, sidecars, Q3 parsing, monotonic and forged-receipt validation go |
| `validation.rs` | shrink to `validation.rs` | `exact_anchor`, `evaluate_exact_batch`, `backend_agreements_pass`, `sample`, `probe_backend`, `determinant_singular_count`. Delete the continuation machinery, every `FROZEN_*` and `AUTHORIZED_*`, `snapshot_frozen_campaign`, journal verifiers, `read_exact_anchor_authority`, `publish_or_adopt` |
| `provenance.rs` | delete | `generate_integrity_file`, `encode_integrity_file`, `decode_integrity_file` logic move to `dataset.rs`; identity observation already lives in `experiments_common`; OSD parts go to `osd_identity.rs` |
| `root_fs.rs`, `launch_cost.rs`, `fixture.rs`, `mod.rs`, `coordinator_tests.rs` | delete | Costs move to config. New tests build configs in a `tests/common/mod.rs` of about 60 lines |
| `bin/permanent_{campaign,dataset,validation}.rs`, `tests/permanent_{campaign_bin,validation}.rs`, `tests/compile_fail/no_public_field_sidecar_mutator.{rs,stderr}` | delete from `gf2-sim` | Rewritten as below |

## Config (`config/*.json`, `deny_unknown_fields`, digest as `rare_event::Configuration::digest`)

```json
{"campaign_id": "...", "root_seed": 1,
 "acceptance": {"permanent_family_error": 0.025, "determinant_family_error": 0.025, "wilson_z": 1.959963984540054},
 "execution": {"first_cell": [7, 20], "max_shard_attempts": 2, "accelerator_launch_cap_ms": 500},
 "cell_groups": [{"q": 3, "n_first": 4, "n_last": 8, "matrix_count": 20000000, "shard_size": 200000,
   "backend": "batch_parallel", "determinant_companion": true,
   "backend_evidence": "...", "accelerator_per_matrix_us": null}],
 "reference_data": {"reproduction_targets": "experiments/permanent-statistics/data/scheinerman2024-q3-targets-v1.csv",
   "literature_claim": "experiments/permanent-statistics/data/literature-claim.json"},
 "validation": {"root_seed": 2, "regeneration_matrix_count": 1024, "sample_matrix_count": 400000,
   "level": 0.001, "exact_batch_matrix_count": 8192, "sample_backend": "batch_parallel",
   "selectable_backends": ["batch_parallel", "intra_matrix_parallel", "generic_ryser", "accelerator"],
   "anchors": [{"q": 3, "n": 1, "stream_index": 0, "permanent_zero_count": 1}]}}
```

Cells expand in group order; groups must be sorted and non-overlapping. `stream_index = (cell_ordinal << 32) | shard_id` and shard count is `ceil(matrix_count / shard_size)`, as `generate_manifest_draft.py` does. `accelerator_per_matrix_us` is required exactly for `accelerator` groups. $K$ is the number of tests in that family: all cells for permanents and only determinant-enabled cells for determinants. `validate` also checks `backend_supports_cell`, and `first_cell` names a cell. C4 ships `config/small-campaign.json` (cheap cells, CPU backends) for tests; C5 writes the real protocol. `backend_evidence` paths are `ArtifactPath` and are only copied to the manifest.

## Data formats (dataset directory, `schema_version` 3, exact equality only)

- `manifest.json`: `{schema_version, campaign_id, root_seed, configuration_sha256, purpose_tag: 3, acceptance, reference_data, cells: [{q, n, matrix_count, shard_size, backend, backend_evidence, determinant_companion}]}`. A pure function of the config, written once before the first draw; a differing existing file refuses.
- `execution.jsonl`: one log, leading-newline appends, synced. Events: `start {q, n, record: RunRecord, workers, argv, on_unavailable}`, `shard_accepted {q, n, shard}`, `shard_failed {q, n, shard_id, attempt, error}`, `cell_terminal {q, n, row}`, `campaign_halted {q, n, reason}`, `invocation_end {status}`.
- `checkpoint.json`: `gf2_sim::checkpoint` payload `permanent-statistics/campaign`, one tally per cell (`next_shard`, counts, failure count, terminal). Hash is `sha256:` of campaign-wide config digest. Logged state wins over a lagging checkpoint and a checkpoint without log progress refuses, exactly as rare_event. Excluded from checksums and not committed.
- `shards/q{q}-n{n:02}.jsonl`: compact lines `{shard_id, stream_index, matrix_count, permanent_zero_count, permanent_histogram, determinant_zero_count: u64|null, executed_backend}`. A projection of the log, published with `publish_atomic` at cell terminal. Partial cells publish what they accepted.
- `summary.csv`: the current 18 columns of the script's `SUMMARY_FIELDS` plus `permanent_log_p_value`, `determinant_log_p_value`. One row per cell; halted rows pool accepted shards and leave estimates empty.
- `runs.json`: `[{record: RunRecord, cells: [[q, n]]}]` from the log, for `@/inv/claims-trace-to-artifacts`. `checksums.sha256`: `sha256sum` text over everything above except the checkpoint.
- `data/literature-claim.json` (committed): `{"fields": [5, 7], "claim": "...", "source": "dev/studies/b488f02c/literature-search-2026-08-08.md", "limitations": "..."}`; text taken from that source. `git mv` the target CSV to `data/`, fixing the link in `protocol.md`.

Validation directory: `execution.jsonl` (`start`, `anchor_terminal {anchor}`, `invocation_end`) and `receipt.json` `{record, configuration, anchors: [{q, n, enumerated, oracle_zero_count, determinant_zero_count, backends: [{backend, status, compared, mismatches}], regeneration: {workers: [1, W], sha256: [a, b], identical}, sample: {matrix_count, zero_count, log_p_value}, verdict}], verdict}`.

## CLI

- `permanent_campaign --config PATH --output DIR --q Q --n N [--workers W=1] [--max-shards K] [--on-unavailable refuse|halt|fallback=BACKEND]`. Exit 0: the cell is terminal, 10: progress remains, 1: failure or refusal, 2: bad arguments. Default `refuse` writes nothing when the cell's backend is unusable in this build or host, because that is an operator mistake. `halt` records terminal `backend_unavailable`. `fallback=` runs remaining shards on that backend, which must satisfy `backend_supports_cell`.
- `permanent_dataset finalize DIR` needs every cell terminal; writes `summary.csv`, `runs.json`, `checksums.sha256`. `permanent_dataset verify DIR` checks checksums, membership, per-shard consistency and that the summary equals the pooled shards.
- `permanent_validation --config PATH --output DIR [--workers W=1] [--max-anchors K]`. Exit 0 passed, 1 failed or error, 10 progress remains, 2 bad arguments.

## Runner (`run_cell`)

1. Write or compare the manifest; open and lock the log; replay state; check identity.
2. Refuse if halted, if `first_cell` is not terminal and the scope differs, or if another cell is started but not terminal. A terminal cell returns without sampling.
3. Preflight the backend and apply `--on-unavailable`; append `start`.
4. For each remaining shard within the budget: `catch_unwind(evaluate_shard)`; check histogram sum, `hist[0] == zeros`, determinant presence; append `shard_accepted`, then write the checkpoint. On error append `shard_failed`; at `max_shard_attempts` halt with `execution_failure`. A device-unavailable error halts with `backend_unavailable`.
5. All shards done: `assess_completed_cell`, publish shards, `cell_terminal`. On rejection, `campaign_halted` and terminal rows for all others with `acceptance_failure`; a mechanical halt does the same with its own reason.
6. Append `invocation_end` in every path, as `execute_frozen_run` does.

## Validation (`StreamPurpose::Validation`, no campaign address is opened)

For each anchor: (a) enumerate with `gf2_algebra::permanent::try_visit_permanent_anchor_matrices`, comparing the zero count with config, every `selectable_backends` entry per matrix through `Kernel` (`backend_supports_cell` false means unsupported; anything else not identical fails), and determinants with `determinant_singular_probability`. (b) Regenerate `regeneration_matrix_count` matrices with fresh samplers through `evaluate_shard`'s observer at workers 1 and W, comparing a digest of address-ordered entry bytes and permanent values; W = 1 is recorded as two fresh serial runs. (c) Draw `sample_matrix_count` from the anchor's address; null is enumerated zeros over enumerated count; `two_sided_test(...).rejects_at(level)` fails, so equality fails. Exact failure skips (b) and (c). Terminal anchors are adopted and never redrawn; a failed anchor stays failed and the overall verdict fails. An interrupted anchor re-runs from its start at the same address. `preflight_required_backends` runs first.

## Tests (`experiments/permanent-statistics/tests/`)

| New file | Lines | Content, from |
|---|---:|---|
| `campaign_determinism.rs` | 300 | Shared contract for REQ-04: one `contract(path)` asserts byte-identical shard JSONL and `summary.csv` for a small config over q = 3, 5, 7 across workers {1, 2, 4}, every supporting backend, `--max-shards 1` repeats, a kill between log append and checkpoint, and `fallback=` from `accelerator`; on `hip`, also a real device run, self-gated by `has_usable_device`. From `batch_records_are_identical_across_configured_thread_counts`, `identical_runs_emit_byte_identical_files`, `campaign_selectable_backends_match_generic_ryser_per_matrix` |
| `campaign_evaluate.rs` | 420 | `schedule.rs` tests: `batch_results_match_ryser_...`, `batch_determinant_counts_match_scalar_...`, `exhaustive_small_field_determinant_count_...`, `companion_does_not_change_permanent_counts`, `companion_observer_...`, `composite_loop_matches_direct_oracle` without timings, `launch_size` tests, `test_accelerator_launch_split_...` on a CPU fake dispatch so it runs everywhere, processor-path resolution as one table test including the F7 ceiling |
| `campaign_runner.rs` | 320 | `coordinator_tests.rs` intents kept: `acceptance_uses_one_budget_...`, `coordinator_persists_exact_underflow_...`, `coordinator_enforces_first_cell_retry_and_contradiction_preservation`, `zero_evidence_halt_...`. New: interruption spends no retry; a changed `source_revision` still resumes; a changed `RunIdentity` field refuses; lock contention refuses before sampling |
| `campaign_dataset.rs` | 200 | CSV round trip, `not_evaluated_plan_...`, `determinant_counts_pool_across_shards_...`, `summary_uses_the_canonical_completed_cell_assessment`, `verify` refusals, per-cell JSONL line consistency |
| `campaign_validation.rs` | 220 | `permanent_validation.rs` intents kept: `every_required_backend_...`, `replay_...` as regeneration, `sampled_draws_come_from_the_validation_stream_...`, `the_sampled_null_derives_...`, `the_exact_decision_is_strict_...`, `an_exact_component_failure_...`, plus no-redraw on resume and a table test of anchor counts against `gf2_algebra`. Use anchors with n <= 3 and small samples for the fast tier |
| `campaign_bins.rs` | 120 | exit codes, default refusal writes nothing, invalid selector, `--max-shards` gives 10 |

Deleted without replacement: receipt forgery and mutation, symlink refusals, emission and git-revision tests, frozen preregistration and toolchain pins, journal and continuation tests, v1 and v2 schema tolerance, cost-table CSV tests, Q3 and literature sidecar tests, `DatasetLayout` tests. gf2-sim's shared determinism tests (`determinism.rs`, `parallel_determinism.rs`, etc.) never touched the campaign and stay. Word-boundary cases do not apply: the tool holds no bit-packed state.

## `osd_campaign.rs` during transition

Add `osd_identity.rs`, about 420 code lines, carrying only what `osd_campaign.rs`, `ebch_osd_awgn_campaign.rs` and `tests/osd_campaign_protocol.rs` import: `ArtifactPath`, `Sha256Digest`, `GitRevision`, their error types, `ArtifactIdentity`, `Availability`, `RngAlgorithm`, `Provenance` (all moved verbatim from `schema.rs`, so OSD checkpoints and receipts keep their bytes), and `observe_cpu_identity`, `observe_provenance`, `repository_top_level` with their private git helpers from `provenance.rs`, returning `io::Error`. No `crates/*` may depend on `experiments/`, so this is a named exception to `@/inv/convention-convergence`: its module comment cites `@/issue/779d1926`, which deletes it. Update the three import sites and the rustdoc at `osd_campaign.rs:407`.

## Other changes

- `crates/gf2-sim`: drop `pub mod permanent_campaign` from `lib.rs`; drop deps `gf2-algebra` (no other user), `rustix`; `hip` loses `gf2-algebra/hip`; `sha2` stays for the OSD bin, rewrite its comment. Fix the example identity in `checkpoint/mod.rs:18` and `README.md:7`.
- `.config/nextest.toml`: remove `binary(permanent_campaign_bin)` from the gf2-sim override; add an override `package(permanent-statistics) and (binary(campaign_determinism) or binary(campaign_bins))` with `threads-required = 4`. `@/issue/1634c185` REQ-04 then only confirms this.
- `scripts/permanent_zero_fraction_analysis.py`: `SCHEMA_VERSION = 3`; `SUMMARY_FIELDS` gains two columns; `_manifest_raw_paths` derives `shards/q{q}-n{n:02}.jsonl` and adds `execution.jsonl`, `runs.json`; delete `_field_summary_terminal_states` and its parameters; `_require_canonical_conformance` runs `-p permanent-statistics --bin permanent_dataset -- verify`; targets and claim paths come from `manifest["reference_data"]`, resolved against `git rev-parse --show-toplevel`; write the claim with its source to `derived/.../literature-claim.json`; update the module docstring.
- `tests/test_permanent_zero_fraction_analysis.py`: `git mv` fixtures to `experiments/permanent-statistics/tests/fixtures/analysis/`, regenerate from `small-campaign.json` by the tool, refresh golden bytes.
- `dev/scripts/permanent-campaign-runner.sh`: delete `run_simulation_campaign` and `CAMPAIGN_SIM_*`, with their cases in `permanent-campaign-runner.test.sh` (adjust the pass count). `dev/index.md` rows for `benchmarks/`, `simulation_results/`, `studies/`. Extend `experiments/permanent-statistics/README.md` and `Cargo.toml` description. `permanent-zero-fraction/` otherwise stays for C5 to retire.
- `batched_f3_permanent.rs:455` is a benchmark output path, outside REQ-01; the plan's `git grep permanent_campaign -- crates` check needs that exclusion or a later fix in `@/issue/39e5f643`.

## Implementation order

1. `execution_log.rs`, converting rare_event; its tests stay green.
2. `osd_identity.rs` in gf2-sim with the three import sites; build OSD tests before anything is deleted.
3. `config.rs`, `evaluate.rs`, `campaign_evaluate.rs`.
4. `decide.rs`, `dataset.rs`, `campaign_dataset.rs`.
5. `runner.rs`, `permanent_campaign`, `campaign_runner.rs`, `campaign_determinism.rs`.
6. `validation.rs`, `permanent_validation`, `permanent_dataset`, remaining tests.
7. Script, fixtures, nextest, runner shell, data moves.
8. Delete `permanent_campaign/`, bins, tests, compile-fail case, deps; sweep prose; run `cargo-ci`; count code lines with the method `awk 'NF && !/^\s*\/\//'`.

## Blocked on owner

Neither blocks implementation; each proceeds on the default and C5 records it.

- **"Exact intervals".** The brief says exact intervals; the protocol reports Wilson and uses exact tests for decisions, and the brief also keeps statistics unchanged. Default: Wilson only, no Clopper-Pearson column.
- **`shard_size`.** Decision 7 changes the draft's 50,000 in precision tiers to fit the budget, without touching cells, `N` or tests. Default: C5 raises it. The alternative is a budget exception for 14,229 shards.
