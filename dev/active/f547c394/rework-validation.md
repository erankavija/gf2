```text
$ jit gate status f547c394 code-review --all
No matching gate runs for issue f547c394

$ python3 <the requested cumulative-finding extractor> f547c394 .jit/gate-runs/*/result.json
zsh:2: no matches found: .jit/gate-runs/*/result.json

$ jit doc list f547c394
Document references for issue f547c394-9f05-4207-8f7f-2dcbb9e56999:
  - dev/active/1a379447-zen3-cpu-performance/measurement-contract.md (Shared Zen 3 measurement contract) [HEAD] <specification>
  - dev/active/f547c394/protocol.md (Zen 3 benchmark protocol v1) [HEAD] <specification>
  - dev/active/f547c394/addendum.schema.json (Family addendum schema v1) [HEAD] <specification>
  - dev/active/f547c394/addendum-protocol-smoke.json (Frozen smoke family addendum) [HEAD] <specification>
  - dev/active/f547c394/design.md (Canonical homes and design decisions) [HEAD] <design>
  - dev/bench_results/f547c394/run-smoke.sh (Smoke receipt launcher) [HEAD] <benchmark>
  - dev/active/f547c394/provenance-clarification.md (Invoker provenance clarification) [HEAD] <review>
  - dev/active/f547c394/addendum-protocol-smoke-pilot.json (Exploratory smoke addendum) [HEAD] <specification>
  - dev/active/f547c394/producing-inputs.json (Producing input closure) [HEAD] <specification>
  - dev/bench_results/f547c394/2026-09-07-f547c394-protocol-pilot/receipt.json (Protocol pilot: receipt.json) [HEAD] <benchmark>
  - dev/bench_results/f547c394/2026-09-07-f547c394-protocol-pilot/plan.json (Protocol pilot: plan.json) [HEAD] <benchmark>
  - dev/bench_results/f547c394/2026-09-07-f547c394-protocol-pilot/execution.log (Protocol pilot: execution.log) [HEAD] <benchmark>
  - dev/bench_results/f547c394/2026-09-07-f547c394-protocol-pilot/launcher.log (Protocol pilot: launcher.log) [HEAD] <benchmark>
  - dev/bench_results/f547c394/2026-09-07-f547c394-protocol-pilot/acceptance-summary.json (Protocol pilot: acceptance-summary.json) [HEAD] <benchmark>
  - dev/bench_results/f547c394/2026-09-07-f547c394-protocol-pilot/acceptance-summary.md (Protocol pilot: acceptance-summary.md) [HEAD] <benchmark>
  - dev/bench_results/f547c394/2026-09-07-f547c394-protocol-confirmation/receipt.json (Protocol confirmation: receipt.json) [HEAD] <benchmark>
  - dev/bench_results/f547c394/2026-09-07-f547c394-protocol-confirmation/plan.json (Protocol confirmation: plan.json) [HEAD] <benchmark>
  - dev/bench_results/f547c394/2026-09-07-f547c394-protocol-confirmation/execution.log (Protocol confirmation: execution.log) [HEAD] <benchmark>
  - dev/bench_results/f547c394/2026-09-07-f547c394-protocol-confirmation/launcher.log (Protocol confirmation: launcher.log) [HEAD] <benchmark>
  - dev/bench_results/f547c394/2026-09-07-f547c394-protocol-confirmation/acceptance-summary.json (Protocol confirmation: acceptance-summary.json) [HEAD] <benchmark>
  - dev/bench_results/f547c394/2026-09-07-f547c394-protocol-confirmation/acceptance-summary.md (Protocol confirmation: acceptance-summary.md) [HEAD] <benchmark>
  - dev/active/1a379447-zen3-cpu-performance/reviews/f547c394-r1.md (Historical review round 1) [HEAD] <review>
  - dev/active/1a379447-zen3-cpu-performance/reviews/f547c394-r2.md (Review round 2: frozen-record binding) [HEAD] <review>

Total: 23

$ grep -inE deferred-pattern dev/active/1a379447-zen3-cpu-performance/measurement-contract.md
120:limit, preserved experiment or tracked falsifiable follow-up within the search
exit=0
$ grep -inE deferred-pattern dev/active/f547c394/protocol.md
exit=1
$ grep -inE deferred-pattern dev/active/f547c394/addendum.schema.json
exit=1
$ grep -inE deferred-pattern dev/active/f547c394/addendum-protocol-smoke.json
exit=1
$ grep -inE deferred-pattern dev/active/f547c394/design.md
exit=1
$ grep -inE deferred-pattern dev/active/f547c394/provenance-clarification.md
exit=1
$ grep -inE deferred-pattern dev/active/f547c394/addendum-protocol-smoke-pilot.json
exit=1
$ grep -inE deferred-pattern dev/active/f547c394/producing-inputs.json
exit=1
$ grep -inE deferred-pattern dev/bench_results/f547c394/2026-09-07-f547c394-protocol-pilot/receipt.json
exit=1
$ grep -inE deferred-pattern dev/bench_results/f547c394/2026-09-07-f547c394-protocol-pilot/plan.json
exit=1
$ grep -inE deferred-pattern dev/bench_results/f547c394/2026-09-07-f547c394-protocol-pilot/acceptance-summary.json
exit=1
$ grep -inE deferred-pattern dev/bench_results/f547c394/2026-09-07-f547c394-protocol-pilot/acceptance-summary.md
exit=1
$ grep -inE deferred-pattern dev/bench_results/f547c394/2026-09-07-f547c394-protocol-confirmation/receipt.json
exit=1
$ grep -inE deferred-pattern dev/bench_results/f547c394/2026-09-07-f547c394-protocol-confirmation/plan.json
exit=1
$ grep -inE deferred-pattern dev/bench_results/f547c394/2026-09-07-f547c394-protocol-confirmation/acceptance-summary.json
exit=1
$ grep -inE deferred-pattern dev/bench_results/f547c394/2026-09-07-f547c394-protocol-confirmation/acceptance-summary.md
exit=1
$ grep -inE deferred-pattern dev/active/1a379447-zen3-cpu-performance/reviews/f547c394-r1.md
23:### Deferred-items audit (Tier 2.75)
24:design.md "Pre-existing fragility observed" (a83583e0 composer lockfile) — legitimately out of scope, recorded in surfaced_pitfalls. design.md "Named exception: session lifecycle store" — tracked exception with a convergence condition; OK.
exit=0
$ grep -inE deferred-pattern dev/active/1a379447-zen3-cpu-performance/reviews/f547c394-r2.md
30:Formal criterion and holistic acceptance stop at the failed gate. The mandatory prior-findings and deferred-items audits are complete. R2 F1 violates the complete frozen-content obligation in REQ-01/REQ-04.
36:### Deferred-items audit (Tier 2.75)
38:The linked protocol/design/schema/addenda/clarification and measurement contract contain no in-scope deferred deliverable. `measurement-contract.md:120` requires future family work to track falsifiable residual gaps; the issue explicitly excludes family surveys and production optimization. `design.md:70-86` identifies pre-existing a835 Git-policy migration (`0ba493e1`) and its untracked composer lockfile fragility (`a83583e0`); these belong to the separate campaign contract, not this protocol task. `design.md:21-33` preserves the named session lifecycle-store exception and explicit convergence condition. The historical review records superseded findings rather than current unresolved implementation.
exit=0
```

The raw pre-edit audit appears above. Gate-run result files are absent from this
worktree, so the requested extractor cannot enumerate them. The committed
round-1 and round-2 review documents supply the cumulative findings below.
No primary-checkout paths are used to retrieve missing tracker data.

## Delivery status

The implementation and evidence changes were validated in the worker checkout.
The worker's sandbox mounted the Git metadata read-only, so the execution lead
committed the validated tree in final form on the worker branch; the closure
citations below name files and lines of that committed tree. Tracker state,
gate evaluation and formal review remain the lead's steps.

The worker's attempted early commit produced:

```text
fatal: Unable to create '/home/vkaskivuo/Projects/gf2/.git/worktrees/agent-f547c394/index.lock': Read-only file system
```

## Commits on the branch

```text
f0595bd6 Merge branch 'main' into worktree-agent-f547c394
cf2e7270 chore(jit:f547c394): preserve interrupted freeze-binding rework
41f1de43 test(jit:f547c394): expose incomplete campaign freeze binding
```

## Files changed

```text
dev/active/f547c394/addendum.schema.json
dev/active/f547c394/design.md
dev/active/f547c394/protocol.md
dev/active/f547c394/provenance-clarification.md
dev/bench_results/f547c394/2026-09-07-f547c394-protocol-pilot/acceptance-summary.json
dev/bench_results/f547c394/2026-09-07-f547c394-protocol-pilot/acceptance-summary.md
dev/bench_results/f547c394/run-smoke.sh
dev/tools/tuning-campaign-support/src/bin/benchmark-ab-runner.rs
dev/tools/tuning-campaign-support/src/journal.rs
dev/tools/tuning-campaign-support/src/process.rs
dev/tools/tuning-campaign-support/src/protocol.rs
dev/tools/tuning-campaign-support/src/receipt.rs
dev/tools/tuning-campaign-support/tests/checkpoint_provenance.rs
dev/tools/tuning-campaign-support/tests/protocol_contracts.rs
dev/active/f547c394/rework-validation.md
dev/bench_results/f547c394/revalidation.json
```

## Changes and behavioral evidence

`receipt::CampaignFacts` is the strict shared schema for opening campaign facts.
Acceptance constructs its receipt/plan/checkpoint projection and calls the same
`resume_equivalent` comparison used by runner resume. Rule P-23 rejects incomplete
or inconsistent frozen content, and semantic validation binds planned cell inputs,
arm pairing, role, label, seed, timing, toolchain, producing content and derived
checkpoint identities. Git locators remain informational.

`journal::CheckpointStore::inspect` shares canonical manifest and unit validation
with resume, reads typed results through `load`, and performs no recovery or writes.
Unavailable cells require their checkpoint. Fixture quality is set before checkpoint
publication. `PlanCell::pair_count` supplies the shared exploratory default and fixed
confirmation count. Production qualification requires non-exploratory evidence. The shared process
scanner also tolerates ESRCH when a procfs entry disappears during cleanup; full
CI supplied failing behavioral evidence and a direct procfs reproduction confirms
the kernel behavior.

Added tests:

- `inspection_validates_without_recovering_or_creating_files`
- `exploratory_only_receipts_never_qualify_for_production_selection`

Expanded or repaired tests:

- `acceptance_rejects_every_self_consistent_frozen_fact_replacement`: protocol,
  contract, schema, addendum, producing input, executable, arm descriptor,
  execution environment, plan, numeric settings, toolchain and checkpoint
  identity replacements; core replacement closures have valid digests and fail
  specifically with P-23. Missing plan, changed input and changed arm pairing
  also reject. These cases preserve the original opening log.
- `acceptance_preserves_unavailable_and_not_material_cells`: changed unavailable
  reason, missing unavailable checkpoint and omitted negative result reject;
  omitted exploratory count uses the frozen pilot minimum.
- `decoder_cells_require_quality_intervals_and_matched_settings`: quality is
  published before the checkpoint, and missing/divergent quality still rejects.
- `acceptance_rejects_an_addendum_not_frozen_before_measurement`: P-23 assertion.

The integration test
`runner_announces_the_log_before_work_and_resumes_without_repeating` retains actual
metadata commits and unrelated dirty edits between sessions. Shared checkpoint
metadata-independence tests remain passing.

All build/dependency caches used here are under `target/`; commands set
`CARGO_HOME=$PWD/target/rework/cargo-home`, `CARGO_NET_OFFLINE=true`, and
`CARGO_CI_NO_SCCACHE=1`. The compatibility check additionally selects the installed
Rust 1.95.0 toolchain. The repository wrapper supplies CPU and test locks.

## Resolution table

Source filenames below resolve under `dev/tools/tuning-campaign-support/` unless
a repository-relative path is given. Line numbers refer to the committed tree of the merge into `main`.

| # | Round | Source | Finding (verbatim) | Resolution (committed tree) |
|---|---|---|---|---|
| 1 | R1 | reviewer | F1 (protocol.rs:185-215, receipt.rs:463): a pinned artifact is verified only against the bytes at its pinned commit; an unavailable Git object or a digest mismatch is an error-severity finding that rejects the receipt. Remove the working-tree fallback and the note path. | Git-object requirement superseded by `dev/active/f547c394/provenance-clarification.md:13`; receipt-local verification with no repository fallback: `src/protocol.rs:203`, missing/mismatch regressions `tests/protocol_contracts.rs:1015` and `:1043`. |
| 2 | R1 | reviewer | F2 (addendum-protocol-smoke.json:11,18; protocol.rs:666): prove freezing rather than declaring it. The receipt's addendum pin commit must contain the addendum with the pinned digest and be an ancestor of or equal to the receipt's source revision; a self-referential or missing `frozen.at_commit` is not acceptable. `resolution_evidence` must name a committed pilot receipt (path and digest) distinct from the receipt under evaluation, verified by the tool. Regenerate the smoke evidence as a pilot receipt followed by a confirmatory receipt that cites it. | Git-ancestry and at_commit demands superseded by the clarification; `src/bin/benchmark-ab-runner.rs:301` opens with complete facts, `src/receipt.rs:1004` checks freeze, `:1623` checks distinct digest-pinned pilot. Both historical receipts independently pass: `dev/bench_results/f547c394/revalidation.json`. |
| 3 | R2 | reviewer | acceptance binds only the addendum to campaign-start. | Complete typed projection and shared comparator: `src/receipt.rs:67`, `:322`, `:338`, `:1004`; runner `src/bin/benchmark-ab-runner.rs:310`; P-23 and coverage: `dev/active/f547c394/protocol.md:35`, `:380`. |
| 4 | R2 | reviewer | Add failing behavioral evidence for self-consistent replacement pins/producing inputs, plan or execution settings that differ from frozen campaign facts. Preserve valid portable receipts and metadata-independent resume/acceptance. Audit all frozen fields together rather than adding only another single-field comparison. | `tests/protocol_contracts.rs:1153`; replacement closures rebuilt with canonical checkpoint publication, asserting P-23 specifically. Complete final test output below and portable export evaluations in revalidation record. |
| 5 | R2 | reviewer | Re-evaluate the committed pilot and confirmation with the corrected acceptance tool, record its identity and results, and update documents if needed. Preserve prior raw evidence; do not modify the frozen historical record to make validation pass. | `dev/bench_results/f547c394/revalidation.json`: tool/input identities, prior summaries, final summaries, all raw bytes checked against HEAD. No receipts regenerated. |
| 6 | Rework audit | reviewer | Resume omits frozen artifact identities | `src/receipt.rs:322` shared comparison; `src/bin/benchmark-ab-runner.rs:310`; integration test `tests/protocol_contracts.rs:1772`. |
| 7 | Rework audit | reviewer | Partial snapshot publication | Existing shared immutable publisher: `src/provenance.rs:124`; pin capture `src/protocol.rs:145`. Frozen snapshots digest-verify in both Git exports. |
| 8 | Rework audit | reviewer | Snapshot paths escape the receipt | `src/protocol.rs:215`; `tests/protocol_contracts.rs:990`. |
| 9 | Rework audit | reviewer | Git metadata controls checkpoint replay | `src/journal.rs:1369` semantic comparator; `:1471` replays persisted manifest; `tests/checkpoint_provenance.rs:70` and `:90`. |
| 10 | Rework audit | reviewer | Extraction omits a835 producing-input closure | Existing `dev/active/a83583e0/producing-build-inputs.json` includes shared `provenance.rs` in applicable maps; no a835 source or contract edits. |
| 11 | Publication audit | reviewer | Ignored nested Cargo.lock snapshots missing from committed receipts | Both exact `inputs/producing/Cargo.lock` paths are already tracked; raw `git ls-files` and Git export verification below. No force-add is needed for existing tracked bytes. |
| 12 | Lead criterion audit | reviewer | No actual unrelated commit/edit between runner sessions | `tests/protocol_contracts.rs:1772` creates actual unrelated metadata commit and dirty documentation between bounded sessions; final test log passes. |
| 13 | Research tier 1 | reviewer | Obsolete comparator citation labels on protocol task | Historical R2 review records lead-authorized closure; issue metadata carries no obsolete citation labels. No tracker mutation performed. |
| 14 | R2 resumption | reviewer | the in-progress acceptance evaluator explicitly parses unit JSON/schema/digests while `journal` owns checkpoint validation. | `src/journal.rs:1582` read-only inspection shares `open`/`validate_unit`; `src/receipt.rs:1128` consumes typed `load`. `tests/checkpoint_provenance.rs:158` verifies no recovery or file creation. |
| 15 | R2 resumption | reviewer | the decoder-quality fixture was changing results after checkpoint publication | `tests/protocol_contracts.rs:1627` sets CellSpec quality before canonical checkpoint publication; no republishing helper remains. |
| 16 | R2 resumption | reviewer | unavailable-cell/unavailable-checkpoint coverage with its regression test | `src/receipt.rs:1128` precedes status dispatch; `tests/protocol_contracts.rs:1453` covers missing unit and altered unavailable result. |
| 17 | Revalidation audit | acceptance run | pair count differs from the saved plan | Red fixture in pilot-default-red.log; `src/protocol.rs:992` supplies the canonical default to runner and evaluator. Historical pilot passes with unchanged plan bytes. |
| 18 | Revalidation audit | acceptance run | qualifies for production selection: **true** | Exploratory-only qualification regression red/green evidence below; `src/receipt.rs:1600` requires a non-exploratory cell. Pilot summary is corrected; prior summary retained in revalidation.json. |
| 19 | Full CI | CI test failure | called `Result::unwrap()` on an `Err` value: Os { code: 3, kind: Uncategorized, message: "No such process" } | Shared process scan tolerates a disappeared procfs entry at `src/process.rs:121`; procfs-race.log reproduces ESRCH after opening/reaping a process. Final support suite validates timeout/cleanup behavior. |
| 20 | Full CI | CI test failure | build dispatcher on gfx1030: Fatal(KernelLaunch { hip_code: 100, kernel: "GfxTarget::detect_device", args: "hip context: hipGetDeviceCount" }) | Unresolved environment limitation: the sandbox exposes no usable HIP device. GPU test source is outside this change and remains intact. Lead must run the full gate with the expected gfx1030 device available. |
| 21 | Final CI | CI test failure | TIMEOUT [   8.124s] gf2-sim::permanent_rare_event_artifacts rare_event_artifact_partial_publish_recovery | Unresolved full-suite timing failure outside this issue: initial CI passed this test; the focused rerun passes in 2.36 seconds. Both outcomes and the full-suite timeout are preserved below. Test budget and unrelated simulation code are unchanged. Lead owns any separate issue tracking. |
| 22 | Pre-edit | audit-step-1 | No matching gate runs for issue f547c394; zsh:2: no matches found: .jit/gate-runs/*/result.json | Raw records are unavailable in this worktree. Committed R1/R2 reviews supply findings; this absence remains an audit limitation, not an inferred passing gate. |
| 23 | Historical audit | audit-step-3: `dev/active/1a379447-zen3-cpu-performance/measurement-contract.md:120` | limit, preserved experiment or tracked falsifiable follow-up within the search | OUT-OF-SCOPE per issue text: "family comparator builds, baseline campaigns and consumer profiling are separate work." This clause governs later family research. |
| 24 | Historical audit | audit-step-3: `dev/active/1a379447-zen3-cpu-performance/reviews/f547c394-r1.md:23` | ### Deferred-items audit (Tier 2.75) | Historical review/audit marker, not a deferred deliverable; R2 freeze closure is `src/receipt.rs:338` and `:1004`. |
| 25 | Historical audit | audit-step-3: `dev/active/1a379447-zen3-cpu-performance/reviews/f547c394-r1.md:24` | design.md "Pre-existing fragility observed" (a83583e0 composer lockfile) — legitimately out of scope, recorded in surfaced_pitfalls. design.md "Named exception: session lifecycle store" — tracked exception with a convergence condition; OK. | Historical audit. `dev/active/f547c394/design.md:21` retains the session-store exception and convergence condition. OUT-OF-SCOPE per issue text: "family comparator builds, baseline campaigns and consumer profiling are separate work" covers separate a835 composer lockfile fragility. |
| 26 | Historical audit | audit-step-3: `dev/active/1a379447-zen3-cpu-performance/reviews/f547c394-r2.md:30` | Formal criterion and holistic acceptance stop at the failed gate. The mandatory prior-findings and deferred-items audits are complete. R2 F1 violates the complete frozen-content obligation in REQ-01/REQ-04. | Historical review/audit marker, not a deferred deliverable; R2 freeze closure is `src/receipt.rs:338` and `:1004`. |
| 27 | Historical audit | audit-step-3: `dev/active/1a379447-zen3-cpu-performance/reviews/f547c394-r2.md:36` | ### Deferred-items audit (Tier 2.75) | Historical review/audit marker, not a deferred deliverable; R2 freeze closure is `src/receipt.rs:338` and `:1004`. |
| 28 | Historical audit | audit-step-3: `dev/active/1a379447-zen3-cpu-performance/reviews/f547c394-r2.md:38` | The linked protocol/design/schema/addenda/clarification and measurement contract contain no in-scope deferred deliverable. `measurement-contract.md:120` requires future family work to track falsifiable residual gaps; the issue explicitly excludes family surveys and production optimization. `design.md:70-86` identifies pre-existing a835 Git-policy migration (`0ba493e1`) and its untracked composer lockfile fragility (`a83583e0`); these belong to the separate campaign contract, not this protocol task. `design.md:21-33` preserves the named session lifecycle-store exception and explicit convergence condition. The historical review records superseded findings rather than current unresolved implementation. | Historical audit preserves explicit scope decisions. Session-store convergence remains in design.md; separate a835 migration belongs to 0ba493e1. OUT-OF-SCOPE per issue text: "family comparator builds, baseline campaigns and consumer profiling are separate work." |

## Success criteria and scope

- REQ-01/REQ-04: shared typed freeze binding and strict portable validation have
  behavioral evidence and are committed in final form.
- REQ-02: paired/bootstrap/statistical rules, numeric settings, holdout and stop
  policies remain in protocol v1 and pass their behavioral suites.
- REQ-03: family margins and complexity fields remain explicit; pilots never
  qualify for production selection, and prior/negative evidence is retained.
- REQ-05: canonical locking, log announcement, resume and observed worker/host
  behavior remain covered by runner integration and host/process suites.
- REQ-06: decoder matched/quality-compatible schemas and quality evidence pass.
- REQ-07: protocol, schema, tooling validation and linked evidence are committed;
  formal gate evaluation and lead review are the tracker-side completion steps.

The known a835 composer-lockfile test fragility is reported in `design.md` and
is not patched. Its unit test passes in this pre-seeded worktree, where the
composer lockfile exists. No family benchmarking, performance claim, production
optimization or formal proof work is added.

## Command results and raw outputs

### Baseline

```text
$ ./scripts/cargo-budget.sh --test cargo nextest run -p tuning-campaign-support --all-features --cargo-profile ci-test --profile ci
cargo-budget: 24 jobs (1 live, 24 cpus)
   Compiling version_check v0.9.5
   Compiling proc-macro2 v1.0.103
   Compiling quote v1.0.42
   Compiling typenum v1.20.0
   Compiling unicode-ident v1.0.22
   Compiling serde_core v1.0.228
   Compiling serde v1.0.228
   Compiling rustix v1.1.2
   Compiling serde_json v1.0.145
   Compiling linux-raw-sys v0.11.0
   Compiling bitflags v2.10.0
   Compiling memchr v2.7.6
   Compiling cpufeatures v0.2.17
   Compiling ryu v1.0.20
   Compiling cfg-if v1.0.4
   Compiling itoa v1.0.15
   Compiling generic-array v0.14.7
   Compiling syn v2.0.111
   Compiling block-buffer v0.10.4
   Compiling crypto-common v0.1.7
   Compiling digest v0.10.7
   Compiling sha2 v0.10.9
   Compiling serde_derive v1.0.228
   Compiling tuning-campaign-support v0.1.0 (/home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-f547c394/dev/tools/tuning-campaign-support)
    Finished `ci-test` profile [optimized] target(s) in 20.17s
────────────
 Nextest run ID 7178783a-ad24-4b16-b381-1b261da50af2 with nextest profile: ci
    Starting 148 tests across 12 binaries
        PASS [   0.006s] (  1/148) tuning-campaign-support abtest::tests::xoshiro_matches_the_reference_first_output
        PASS [   0.007s] (  2/148) tuning-campaign-support abtest::tests::nearest_rank_percentiles_use_the_frozen_ranks
        PASS [   0.007s] (  3/148) tuning-campaign-support abtest::tests::wilson_interval_brackets_the_point_estimate
        PASS [   0.007s] (  4/148) tuning-campaign-support campaign::publication_tests::immutable_artifact_publication_preserves_ambiguous_bytes_and_excludes_writers
        PASS [   0.008s] (  5/148) tuning-campaign-support campaign::publication_tests::preparation_discovery_rejects_intent_under_another_session_root
        PASS [   0.008s] (  6/148) tuning-campaign-support campaign::publication_tests::uncommitted_mode_intent_never_claims_a_launch_occurred
        PASS [   0.009s] (  7/148) tuning-campaign-support campaign::publication_tests::prelock_evidence_binds_every_distinct_mode_writer_and_replay_checks_mutation
        PASS [   0.009s] (  8/148) tuning-campaign-support campaign::publication_tests::resume_identity_precedes_temporary_cleanup_and_namespace_is_exact
        PASS [   0.009s] (  9/148) tuning-campaign-support campaign::publication_tests::immutable_artifact_publication_replays_intent_and_target_boundaries
        PASS [   0.009s] ( 10/148) tuning-campaign-support campaign::publication_tests::synced_transition_intent_recovers_observation_and_journals_it_once
        PASS [   0.009s] ( 11/148) tuning-campaign-support campaign::publication_tests::preparation_replays_descriptor_prepare_and_active_claim_gaps
        PASS [   0.009s] ( 12/148) tuning-campaign-support campaign::publication_tests::tampered_or_cross_identity_publication_preserves_all_original_evidence
        PASS [   0.010s] ( 13/148) tuning-campaign-support campaign::publication_tests::preparation_recovers_every_uncommitted_initial_intent_boundary
        PASS [   0.010s] ( 14/148) tuning-campaign-support campaign::publication_tests::recovered_interruption_journals_cleanup_after_release_without_inventing_work
        PASS [   0.011s] ( 15/148) tuning-campaign-support journal::active_repair_tests::active_repair_preserves_complete_malformed_records_and_rejects_wrong_session
        PASS [   0.011s] ( 16/148) tuning-campaign-support campaign::publication_tests::replacement_preparation_rejects_byte_identical_checksum_alias
        PASS [   0.011s] ( 17/148) tuning-campaign-support campaign::publication_tests::preparation_intent_replays_initial_publication_and_rejects_changed_identity
        PASS [   0.005s] ( 18/148) tuning-campaign-support timing::tests::timed_calls_supply_the_exact_rotating_fixture_bank
        PASS [   0.012s] ( 19/148) tuning-campaign-support campaign::publication_tests::session_checksum_requires_strict_artifact_path_order
        PASS [   0.014s] ( 20/148) tuning-campaign-support campaign::publication_tests::mode_publication_replays_intent_sync_temporary_sync_and_destination_link
        PASS [   0.006s] ( 21/148) tuning-campaign-support::campaign_contracts candidate_blocks_require_exact_three_candidate_rotation_and_interleaving
        PASS [   0.006s] ( 22/148) tuning-campaign-support::campaign_contracts clean_lifecycle_cannot_publish_before_observed_release
        PASS [   0.008s] ( 23/148) tuning-campaign-support::campaign_contracts campaign_reconciliation_validates_the_complete_all_owner_universe
        PASS [   0.010s] ( 24/148) tuning-campaign-support::campaign_contracts acceptance_reconciliation_rejects_unknown_keys_wrong_cases_and_duplicates
        PASS [   0.008s] ( 25/148) tuning-campaign-support::campaign_contracts completed_exit_cannot_accept_after_its_retained_raw_artifact_changes
        PASS [   0.007s] ( 26/148) tuning-campaign-support::campaign_contracts configured_timing_emits_outside_intervals_and_stops_on_callback_error
        PASS [   0.009s] ( 27/148) tuning-campaign-support::campaign_contracts checkpoints_bind_progress_then_reconcile_missing_acceptance_once
        PASS [   0.008s] ( 28/148) tuning-campaign-support::campaign_contracts completion_records_missing_callback_progress_before_rejecting_exit
        PASS [   0.007s] ( 29/148) tuning-campaign-support::campaign_contracts derived_manifests_cannot_change_reserved_slots_or_input_digest
        PASS [   0.007s] ( 30/148) tuning-campaign-support::campaign_contracts durable_descriptors_reject_concurrent_writer_reuse_and_identity_change
        PASS [   0.006s] ( 31/148) tuning-campaign-support::campaign_contracts interleaved_manifest_accepts_rotated_candidate_blocks_and_rejects_mutations
        PASS [   0.014s] ( 32/148) tuning-campaign-support::campaign_contracts active_claim_and_torn_log_recover_in_place_before_release_and_replacement
        PASS [   0.007s] ( 33/148) tuning-campaign-support::campaign_contracts invalid_utf8_stdout_keeps_exact_exit_bytes_and_cannot_accept
        PASS [   0.014s] ( 34/148) tuning-campaign-support::campaign_contracts budget_terminal_checksum_gap_and_same_identity_resume_preserve_the_prefix
        PASS [   0.006s] ( 35/148) tuning-campaign-support::campaign_contracts missing_progress_or_validation_never_accepts_a_checkpoint
        PASS [   0.006s] ( 36/148) tuning-campaign-support::campaign_contracts opaque_cases_preserve_order_but_reject_noncanonical_and_duplicate_keys
        PASS [   0.021s] ( 37/148) tuning-campaign-support journal::active_repair_tests::active_claim_survives_every_same_session_torn_repair_boundary
        PASS [   0.012s] ( 38/148) tuning-campaign-support::campaign_contracts clean_exit_recovery_rejects_raw_stream_and_progress_mutations
        PASS [   0.012s] ( 39/148) tuning-campaign-support::campaign_contracts completed_raw_result_never_overrides_failure_or_timeout_observation
        PASS [   0.022s] ( 40/148) tuning-campaign-support campaign::publication_tests::retirement_and_reopen_reject_noncanonical_checksum_bindings
        PASS [   0.006s] ( 41/148) tuning-campaign-support::campaign_contracts pending_recovery_requires_synced_journal_handshake_and_is_idempotent
        PASS [   0.011s] ( 42/148) tuning-campaign-support::campaign_contracts every_bound_child_record_requires_its_exact_structured_case
        PASS [   0.006s] ( 43/148) tuning-campaign-support::campaign_contracts prelock_failure_does_not_invent_release_and_timeout_never_accepts
        PASS [   0.005s] ( 44/148) tuning-campaign-support::campaign_contracts process_paths_environment_and_staged_bytes_are_strict
        PASS [   0.005s] ( 45/148) tuning-campaign-support::campaign_contracts progress_rejects_probe_wrong_identity_and_out_of_order_events
        PASS [   0.006s] ( 46/148) tuning-campaign-support::campaign_contracts progress_framing_and_result_consistency_are_exact
        PASS [   0.007s] ( 47/148) tuning-campaign-support::campaign_contracts prefix_replay_rejects_overlapping_spawns_and_resampling_before_validation
        PASS [   0.006s] ( 48/148) tuning-campaign-support::campaign_contracts result_digest_excludes_stdout_framing_and_environment_is_canonical
        PASS [   0.006s] ( 49/148) tuning-campaign-support::campaign_contracts unclean_wrapper_needs_dead_tree_and_independent_lock_observation
        PASS [   0.006s] ( 50/148) tuning-campaign-support::campaign_contracts wrong_identity_incomplete_stream_and_probe_windows_reject
        PASS [   0.005s] ( 51/148) tuning-campaign-support::checkpoint_provenance changed_producing_digest_rejects_before_mutating_checkpoint_evidence
        PASS [   0.005s] ( 52/148) tuning-campaign-support::checkpoint_provenance metadata_only_resume_retains_completed_result_and_original_manifest
        PASS [   0.005s] ( 53/148) tuning-campaign-support::checkpoint_provenance metadata_only_revision_change_allows_resume_and_initialize_replay
        PASS [   0.005s] ( 54/148) tuning-campaign-support::contracts checkpoint_manifest_and_temp_namespace_fail_closed
        PASS [   0.006s] ( 55/148) tuning-campaign-support::contracts checkpoint_identity_is_validated_before_pending_recovery
        PASS [   0.008s] ( 56/148) tuning-campaign-support::campaign_contracts transition_projection_recovers_once_after_durable_state_commit
        PASS [   0.007s] ( 57/148) tuning-campaign-support::contracts call_calibration_and_window_rotation_preserve_the_protocol
        PASS [   0.007s] ( 58/148) tuning-campaign-support::contracts atomic_helpers_distinguish_immutable_evidence_from_mutable_pointers
        PASS [   0.013s] ( 59/148) tuning-campaign-support::campaign_contracts prelock_abort_rejects_any_held_or_fabricated_wrapper_evidence
        PASS [   0.006s] ( 60/148) tuning-campaign-support::contracts checkpoint_pending_namespace_requires_the_exact_generated_shape
        PASS [   0.013s] ( 61/148) tuning-campaign-support::campaign_contracts prepared_writer_death_closes_without_fictitious_wrapper_or_release
        PASS [   0.015s] ( 62/148) tuning-campaign-support::campaign_contracts pending_completion_rejects_wrong_outcome_case_artifacts_and_duplicate_intent
        PASS [   0.007s] ( 63/148) tuning-campaign-support::contracts checkpoint_pending_recovery_replays_every_deletion_boundary
        PASS [   0.012s] ( 64/148) tuning-campaign-support::campaign_contracts raw_stderr_validity_and_progress_equivalence_cannot_be_mutated_into_acceptance
        PASS [   0.032s] ( 65/148) tuning-campaign-support campaign::publication_tests::preparation_replacement_recovers_before_after_and_during_session_start
        PASS [   0.005s] ( 66/148) tuning-campaign-support::contracts child_v2_framing_is_canonical_and_fail_closed
        PASS [   0.012s] ( 67/148) tuning-campaign-support::campaign_contracts recovery_requires_release_then_censored_interruption_and_allows_resume
        PASS [   0.006s] ( 68/148) tuning-campaign-support::contracts checkpoint_units_are_immutable_and_resume_validates_identity_and_bytes
        PASS [   0.005s] ( 69/148) tuning-campaign-support::contracts coupled_gemm_requires_all_slices_and_one_strict_pair_winner
        PASS [   0.006s] ( 70/148) tuning-campaign-support::contracts coupled_gemm_ties_and_cross_stratum_regressions_keep_the_pair_default
        PASS [   0.006s] ( 71/148) tuning-campaign-support::contracts coupled_gemm_records_corner_edge_and_interior_grid_boundaries
        PASS [   0.005s] ( 72/148) tuning-campaign-support::contracts execution_log_is_durable_append_only_and_resumes_sequence
        PASS [   0.005s] ( 73/148) tuning-campaign-support::contracts extent_argmin_handles_unique_wins_plateaus_and_curve_reversals
        PASS [   0.006s] ( 74/148) tuning-campaign-support::contracts extent_statistics_cover_equal_weighting_quartiles_and_fail_closed_paths
        PASS [   0.027s] ( 75/148) tuning-campaign-support::campaign_contracts clean_exit_recovery_reuses_timing_before_and_after_owner_validation
        PASS [   0.005s] ( 76/148) tuning-campaign-support::contracts m4rm_joint_vector_rejects_any_stratum_regression
        PASS [   0.006s] ( 77/148) tuning-campaign-support::contracts journal_recovery_after_a_paused_prefix_records_recovery_without_interruption
        PASS [   0.006s] ( 78/148) tuning-campaign-support::contracts retained_threshold_rule_keeps_its_existing_direction_and_noise_semantics
        PASS [   0.036s] ( 79/148) tuning-campaign-support campaign::publication_tests::initial_preparation_replays_log_config_checkpoint_and_active_handoff_gaps
        PASS [   0.006s] ( 80/148) tuning-campaign-support::contracts journal_recovery_records_interruption_and_preserves_torn_tail
        PASS [   0.007s] ( 81/148) tuning-campaign-support::contracts journal_recovery_treats_a_complete_record_without_newline_as_torn
        PASS [   0.005s] ( 82/148) tuning-campaign-support::contracts threshold_fallbacks_cover_missing_no_win_and_reversal
        PASS [   0.006s] ( 83/148) tuning-campaign-support::contracts terminal_journal_lifecycle_fails_closed
        PASS [   0.005s] ( 84/148) tuning-campaign-support::host_process_contracts affinity_round_trips_serde_and_rejects_unordered_sets
        PASS [   0.007s] ( 85/148) tuning-campaign-support::contracts seed_mixer_matches_the_committed_vector_and_banked_roles
        PASS [   0.006s] ( 86/148) tuning-campaign-support::host_process_contracts core_arms_resolve_inside_the_mask_or_carry_a_reason
        PASS [   0.009s] ( 87/148) tuning-campaign-support::contracts journal_recovery_intent_is_idempotent_at_every_durable_boundary
        PASS [   0.018s] ( 88/148) tuning-campaign-support::campaign_contracts rejected_stderr_remains_durable_across_completion_and_exit_crashes
        PASS [   0.006s] ( 89/148) tuning-campaign-support::host_process_contracts lock_observation_requires_a_held_inherited_descriptor
        PASS [   0.010s] ( 90/148) tuning-campaign-support::driver_launcher launcher_rejects_arbitrary_stage_paths_before_creating_them
        PASS [   0.005s] ( 91/148) tuning-campaign-support::protocol_contracts bonferroni_level_widens_with_the_family_and_prior_trials
        PASS [   0.006s] ( 92/148) tuning-campaign-support::protocol_contracts decisions_follow_confidence_bound_margins_not_significance
        PASS [   0.007s] ( 93/148) tuning-campaign-support::protocol_contracts addendum_schema_accepts_the_frozen_smoke_addendum_and_rejects_unknown_fields
        PASS [   0.013s] ( 94/148) tuning-campaign-support::host_process_contracts topology_and_host_observation_report_runtime_facts
        PASS [   0.006s] ( 95/148) tuning-campaign-support::protocol_contracts pair_orders_are_counterbalanced_and_seed_determined
        PASS [   0.006s] ( 96/148) tuning-campaign-support::protocol_contracts protocol_document_pins_the_frozen_shared_settings
        PASS [   0.012s] ( 97/148) tuning-campaign-support::protocol_contracts paired_bootstrap_interval_is_deterministic_and_brackets_known_ratios
        PASS [   0.005s] ( 98/148) tuning-campaign-support::schema_subset arrays_refs_and_combinators_are_enforced
        PASS [   0.005s] ( 99/148) tuning-campaign-support::schema_subset nullable_type_arrays_accept_null_and_reject_other_types
        PASS [   0.005s] (100/148) tuning-campaign-support::schema_subset numeric_bounds_and_enums_are_enforced
        PASS [   0.005s] (101/148) tuning-campaign-support::schema_subset types_required_and_additional_properties_are_enforced
        PASS [   0.081s] (102/148) tuning-campaign-support campaign::publication_tests::prelock_interruption_checksum_and_retirement_recover_every_publication_gap
        PASS [   0.047s] (103/148) tuning-campaign-support::protocol_contracts acceptance_passes_a_confirmed_improvement_receipt
        PASS [   0.006s] (104/148) tuning-campaign-support::schema_subset unsupported_keywords_and_dangling_refs_are_reported_not_ignored
        PASS [   0.047s] (105/148) tuning-campaign-support::protocol_contracts acceptance_rejects_changed_producing_input_snapshot
        PASS [   0.049s] (106/148) tuning-campaign-support::protocol_contracts acceptance_fails_a_regression_beyond_the_equivalence_margin
        PASS [   0.049s] (107/148) tuning-campaign-support::protocol_contracts acceptance_rejects_a_digest_that_does_not_match_the_snapshot
        PASS [   0.049s] (108/148) tuning-campaign-support::protocol_contracts acceptance_marks_an_interval_spanning_the_margin_inconclusive
        PASS [   0.048s] (109/148) tuning-campaign-support::protocol_contracts acceptance_rejects_an_addendum_not_frozen_before_measurement
        PASS [   0.035s] (110/148) tuning-campaign-support::protocol_contracts unresolved_required_settings_make_a_cell_non_confirmatory
        PASS [   0.049s] (111/148) tuning-campaign-support::protocol_contracts acceptance_rejects_a_snapshot_path_that_escapes_the_receipt
        PASS [   0.050s] (112/148) tuning-campaign-support::protocol_contracts acceptance_rejects_a_missing_snapshot_despite_matching_source_bytes
        PASS [   0.007s] (113/148) tuning-campaign-support::schema_subset violations_are_addressed_by_json_pointer
        PASS [   0.049s] (114/148) tuning-campaign-support::protocol_contracts acceptance_summary_markdown_renders_from_the_summary_only
        PASS [   0.005s] (115/148) tuning-campaign-support::bin/tuning-extent-campaign-driver tests::campaign_stage_policy_rejects_non_tmp_and_mismatched_paths
        PASS [   0.006s] (116/148) tuning-campaign-support::bin/tuning-extent-campaign-driver tests::held_lock_requires_an_inherited_descriptor
        PASS [   0.005s] (117/148) tuning-campaign-support::bin/tuning-extent-campaign-driver tests::live_writer_and_reused_identity_are_distinguished
        PASS [   0.083s] (118/148) tuning-campaign-support::campaign_contracts completion_intent_recovers_every_raw_exit_validation_and_checkpoint_boundary
        PASS [   0.008s] (119/148) tuning-campaign-support::bin/tuning-extent-campaign-driver tests::interrupted_derived_projection_publication_is_replayed_and_bound
        PASS [   0.007s] (120/148) tuning-campaign-support::bin/tuning-extent-campaign-driver tests::producing_input_manifest_rejects_authority_and_path_mutations
        PASS [   0.005s] (121/148) tuning-campaign-support::bin/tuning-extent-campaign-driver tests::promotion_is_idempotent_and_rejects_different_occupied_bytes
        PASS [   0.009s] (122/148) tuning-campaign-support::bin/tuning-extent-campaign-driver tests::nonzero_exit_is_never_eligible
        PASS [   0.005s] (123/148) tuning-campaign-support::bin/tuning-extent-campaign-driver tests::relocated_build_source_observation_is_rejected_before_campaign_work
        PASS [   0.013s] (124/148) tuning-campaign-support::bin/tuning-extent-campaign-driver tests::incomplete_executable_staging_replays_before_campaign_config_exists
        PASS [   0.006s] (125/148) tuning-campaign-support::bin/tuning-extent-campaign-driver tests::relocated_preflight_report_is_rejected_before_campaign_work
        PASS [   0.006s] (126/148) tuning-campaign-support::bin/tuning-extent-campaign-driver tests::staged_process_receives_only_the_declared_measurement_environment
        PASS [   0.051s] (127/148) tuning-campaign-support::protocol_contracts source_control_metadata_does_not_change_acceptance
        PASS [   0.024s] (128/148) tuning-campaign-support::bin/tuning-extent-campaign-driver tests::affinity_is_observed_from_the_current_os_mask_and_binds_resume
        PASS [   0.011s] (129/148) tuning-campaign-support::bin/tuning-extent-campaign-driver tests::stderr_callback_failure_still_drains_and_reaps
        PASS [   0.019s] (130/148) tuning-campaign-support::bin/tuning-extent-campaign-driver tests::staged_reporting_preflights_use_their_actual_cli_flags_and_empty_stdin
        PASS [   0.024s] (131/148) tuning-campaign-support::bin/tuning-extent-campaign-driver tests::simultaneous_binary_streams_are_drained_without_loss
        PASS [   0.077s] (132/148) tuning-campaign-support::protocol_contracts decoder_cells_require_quality_intervals_and_matched_settings
        PASS [   0.084s] (133/148) tuning-campaign-support::protocol_contracts acceptance_rejects_a_receipt_with_missing_protocol_identity
        PASS [   0.086s] (134/148) tuning-campaign-support::protocol_contracts acceptance_preserves_unavailable_and_not_material_cells
        PASS [   0.085s] (135/148) tuning-campaign-support::protocol_contracts acceptance_rejects_missing_and_self_referential_resolution_evidence
        PASS [   0.041s] (136/148) tuning-campaign-support::bin/tuning-extent-campaign-driver tests::a_probe_streams_validates_and_commits_one_bound_checkpoint
        PASS [   0.095s] (137/148) tuning-campaign-support::driver_launcher launcher_discovers_publisher_intent_before_selecting_identity_or_building
        PASS [   0.096s] (138/148) tuning-campaign-support::driver_launcher launcher_discovers_complete_publisher_temporary_before_selecting_identity_or_building
        PASS [   0.052s] (139/148) tuning-campaign-support::bin/tuning-extent-campaign-driver tests::progress_is_delivered_before_exit
        PASS [   0.051s] (140/148) tuning-campaign-support::bin/tuning-extent-campaign-driver tests::rejected_raw_streams_cannot_recover_after_completion_crash
        PASS [   0.120s] (141/148) tuning-campaign-support::protocol_contracts acceptance_accepts_a_resumed_run_and_rejects_a_repeated_cell
        PASS [   0.125s] (142/148) tuning-campaign-support::protocol_contracts acceptance_rejects_every_self_consistent_frozen_fact_replacement
        PASS [   0.088s] (143/148) tuning-campaign-support::bin/tuning-extent-campaign-driver tests::clean_completion_crashes_recover_without_fresh_child_replay
        PASS [   0.087s] (144/148) tuning-campaign-support::bin/tuning-extent-campaign-driver tests::timeout_kills_descendant_holding_pipes
        PASS [   0.107s] (145/148) tuning-campaign-support::bin/tuning-extent-campaign-driver tests::timeout_reaps_an_escaped_descendant
        PASS [   0.139s] (146/148) tuning-campaign-support::bin/tuning-extent-campaign-driver tests::kill_grace_handles_a_child_ignoring_term
        PASS [   0.229s] (147/148) tuning-campaign-support::host_process_contracts run_process_drains_both_streams_and_times_out_cleanly
        PASS [   0.613s] (148/148) tuning-campaign-support::protocol_contracts runner_announces_the_log_before_work_and_resumes_without_repeating
────────────
     Summary [   0.661s] 148 tests run: 148 passed, 0 skipped
```

### Freeze-rule regression evidence

```text
$ ./scripts/cargo-budget.sh --test cargo nextest run -p tuning-campaign-support --all-features --cargo-profile ci-test --profile ci
cargo-budget: 24 jobs (1 live, 24 cpus)
   Compiling tuning-campaign-support v0.1.0 (/home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-f547c394/dev/tools/tuning-campaign-support)
    Finished `ci-test` profile [optimized] target(s) in 17.67s
────────────
 Nextest run ID ed86a28b-4070-4279-add6-b6cc99dee165 with nextest profile: ci
    Starting 149 tests across 12 binaries
        PASS [   0.005s] (  1/149) tuning-campaign-support abtest::tests::wilson_interval_brackets_the_point_estimate
        PASS [   0.005s] (  2/149) tuning-campaign-support abtest::tests::xoshiro_matches_the_reference_first_output
        PASS [   0.006s] (  3/149) tuning-campaign-support campaign::publication_tests::immutable_artifact_publication_preserves_ambiguous_bytes_and_excludes_writers
        PASS [   0.007s] (  4/149) tuning-campaign-support abtest::tests::nearest_rank_percentiles_use_the_frozen_ranks
        PASS [   0.008s] (  5/149) tuning-campaign-support campaign::publication_tests::resume_identity_precedes_temporary_cleanup_and_namespace_is_exact
        PASS [   0.008s] (  6/149) tuning-campaign-support campaign::publication_tests::tampered_or_cross_identity_publication_preserves_all_original_evidence
        PASS [   0.008s] (  7/149) tuning-campaign-support campaign::publication_tests::prelock_evidence_binds_every_distinct_mode_writer_and_replay_checks_mutation
        PASS [   0.008s] (  8/149) tuning-campaign-support campaign::publication_tests::uncommitted_mode_intent_never_claims_a_launch_occurred
        PASS [   0.008s] (  9/149) tuning-campaign-support campaign::publication_tests::preparation_discovery_rejects_intent_under_another_session_root
        PASS [   0.009s] ( 10/149) tuning-campaign-support campaign::publication_tests::preparation_replays_descriptor_prepare_and_active_claim_gaps
        PASS [   0.009s] ( 11/149) tuning-campaign-support campaign::publication_tests::preparation_recovers_every_uncommitted_initial_intent_boundary
        PASS [   0.010s] ( 12/149) tuning-campaign-support campaign::publication_tests::preparation_intent_replays_initial_publication_and_rejects_changed_identity
        PASS [   0.010s] ( 13/149) tuning-campaign-support campaign::publication_tests::immutable_artifact_publication_replays_intent_and_target_boundaries
        PASS [   0.005s] ( 14/149) tuning-campaign-support timing::tests::timed_calls_supply_the_exact_rotating_fixture_bank
        PASS [   0.010s] ( 15/149) tuning-campaign-support journal::active_repair_tests::active_repair_preserves_complete_malformed_records_and_rejects_wrong_session
        PASS [   0.010s] ( 16/149) tuning-campaign-support campaign::publication_tests::session_checksum_requires_strict_artifact_path_order
        PASS [   0.010s] ( 17/149) tuning-campaign-support campaign::publication_tests::recovered_interruption_journals_cleanup_after_release_without_inventing_work
        PASS [   0.011s] ( 18/149) tuning-campaign-support campaign::publication_tests::replacement_preparation_rejects_byte_identical_checksum_alias
        PASS [   0.012s] ( 19/149) tuning-campaign-support campaign::publication_tests::synced_transition_intent_recovers_observation_and_journals_it_once
        PASS [   0.006s] ( 20/149) tuning-campaign-support::campaign_contracts candidate_blocks_require_exact_three_candidate_rotation_and_interleaving
        PASS [   0.014s] ( 21/149) tuning-campaign-support campaign::publication_tests::mode_publication_replays_intent_sync_temporary_sync_and_destination_link
        PASS [   0.005s] ( 22/149) tuning-campaign-support::campaign_contracts clean_lifecycle_cannot_publish_before_observed_release
        PASS [   0.010s] ( 23/149) tuning-campaign-support::campaign_contracts acceptance_reconciliation_rejects_unknown_keys_wrong_cases_and_duplicates
        PASS [   0.008s] ( 24/149) tuning-campaign-support::campaign_contracts checkpoints_bind_progress_then_reconcile_missing_acceptance_once
        PASS [   0.005s] ( 25/149) tuning-campaign-support::campaign_contracts configured_timing_emits_outside_intervals_and_stops_on_callback_error
        PASS [   0.008s] ( 26/149) tuning-campaign-support::campaign_contracts campaign_reconciliation_validates_the_complete_all_owner_universe
        PASS [   0.007s] ( 27/149) tuning-campaign-support::campaign_contracts completed_exit_cannot_accept_after_its_retained_raw_artifact_changes
        PASS [   0.007s] ( 28/149) tuning-campaign-support::campaign_contracts completion_records_missing_callback_progress_before_rejecting_exit
        PASS [   0.007s] ( 29/149) tuning-campaign-support::campaign_contracts derived_manifests_cannot_change_reserved_slots_or_input_digest
        PASS [   0.006s] ( 30/149) tuning-campaign-support::campaign_contracts interleaved_manifest_accepts_rotated_candidate_blocks_and_rejects_mutations
        PASS [   0.008s] ( 31/149) tuning-campaign-support::campaign_contracts durable_descriptors_reject_concurrent_writer_reuse_and_identity_change
        PASS [   0.011s] ( 32/149) tuning-campaign-support::campaign_contracts clean_exit_recovery_rejects_raw_stream_and_progress_mutations
        PASS [   0.005s] ( 33/149) tuning-campaign-support::campaign_contracts opaque_cases_preserve_order_but_reject_noncanonical_and_duplicate_keys
        PASS [   0.006s] ( 34/149) tuning-campaign-support::campaign_contracts missing_progress_or_validation_never_accepts_a_checkpoint
        PASS [   0.007s] ( 35/149) tuning-campaign-support::campaign_contracts invalid_utf8_stdout_keeps_exact_exit_bytes_and_cannot_accept
        PASS [   0.014s] ( 36/149) tuning-campaign-support::campaign_contracts budget_terminal_checksum_gap_and_same_identity_resume_preserve_the_prefix
        PASS [   0.021s] ( 37/149) tuning-campaign-support journal::active_repair_tests::active_claim_survives_every_same_session_torn_repair_boundary
        PASS [   0.011s] ( 38/149) tuning-campaign-support::campaign_contracts completed_raw_result_never_overrides_failure_or_timeout_observation
        PASS [   0.015s] ( 39/149) tuning-campaign-support::campaign_contracts active_claim_and_torn_log_recover_in_place_before_release_and_replacement
        PASS [   0.006s] ( 40/149) tuning-campaign-support::campaign_contracts pending_recovery_requires_synced_journal_handshake_and_is_idempotent
        PASS [   0.007s] ( 41/149) tuning-campaign-support::campaign_contracts prefix_replay_rejects_overlapping_spawns_and_resampling_before_validation
        PASS [   0.006s] ( 42/149) tuning-campaign-support::campaign_contracts prelock_failure_does_not_invent_release_and_timeout_never_accepts
        PASS [   0.006s] ( 43/149) tuning-campaign-support::campaign_contracts process_paths_environment_and_staged_bytes_are_strict
        PASS [   0.006s] ( 44/149) tuning-campaign-support::campaign_contracts progress_framing_and_result_consistency_are_exact
        PASS [   0.013s] ( 45/149) tuning-campaign-support::campaign_contracts every_bound_child_record_requires_its_exact_structured_case
        PASS [   0.005s] ( 46/149) tuning-campaign-support::campaign_contracts progress_rejects_probe_wrong_identity_and_out_of_order_events
        PASS [   0.025s] ( 47/149) tuning-campaign-support campaign::publication_tests::retirement_and_reopen_reject_noncanonical_checksum_bindings
        PASS [   0.005s] ( 48/149) tuning-campaign-support::campaign_contracts result_digest_excludes_stdout_framing_and_environment_is_canonical
        PASS [   0.006s] ( 49/149) tuning-campaign-support::campaign_contracts wrong_identity_incomplete_stream_and_probe_windows_reject
        PASS [   0.006s] ( 50/149) tuning-campaign-support::campaign_contracts unclean_wrapper_needs_dead_tree_and_independent_lock_observation
        PASS [   0.006s] ( 51/149) tuning-campaign-support::checkpoint_provenance changed_producing_digest_rejects_before_mutating_checkpoint_evidence
        PASS [   0.006s] ( 52/149) tuning-campaign-support::checkpoint_provenance inspection_validates_without_recovering_or_creating_files
        PASS [   0.011s] ( 53/149) tuning-campaign-support::campaign_contracts prepared_writer_death_closes_without_fictitious_wrapper_or_release
        PASS [   0.005s] ( 54/149) tuning-campaign-support::contracts call_calibration_and_window_rotation_preserve_the_protocol
        PASS [   0.005s] ( 55/149) tuning-campaign-support::checkpoint_provenance metadata_only_resume_retains_completed_result_and_original_manifest
        PASS [   0.014s] ( 56/149) tuning-campaign-support::campaign_contracts prelock_abort_rejects_any_held_or_fabricated_wrapper_evidence
        PASS [   0.006s] ( 57/149) tuning-campaign-support::contracts checkpoint_manifest_and_temp_namespace_fail_closed
        PASS [   0.009s] ( 58/149) tuning-campaign-support::campaign_contracts transition_projection_recovers_once_after_durable_state_commit
        PASS [   0.006s] ( 59/149) tuning-campaign-support::checkpoint_provenance metadata_only_revision_change_allows_resume_and_initialize_replay
        PASS [   0.014s] ( 60/149) tuning-campaign-support::campaign_contracts pending_completion_rejects_wrong_outcome_case_artifacts_and_duplicate_intent
        PASS [   0.011s] ( 61/149) tuning-campaign-support::campaign_contracts raw_stderr_validity_and_progress_equivalence_cannot_be_mutated_into_acceptance
        PASS [   0.007s] ( 62/149) tuning-campaign-support::contracts atomic_helpers_distinguish_immutable_evidence_from_mutable_pointers
        PASS [   0.006s] ( 63/149) tuning-campaign-support::contracts checkpoint_identity_is_validated_before_pending_recovery
        PASS [   0.012s] ( 64/149) tuning-campaign-support::campaign_contracts recovery_requires_release_then_censored_interruption_and_allows_resume
        PASS [   0.006s] ( 65/149) tuning-campaign-support::contracts checkpoint_pending_namespace_requires_the_exact_generated_shape
        PASS [   0.005s] ( 66/149) tuning-campaign-support::contracts child_v2_framing_is_canonical_and_fail_closed
        PASS [   0.007s] ( 67/149) tuning-campaign-support::contracts checkpoint_pending_recovery_replays_every_deletion_boundary
        PASS [   0.005s] ( 68/149) tuning-campaign-support::contracts coupled_gemm_requires_all_slices_and_one_strict_pair_winner
        PASS [   0.005s] ( 69/149) tuning-campaign-support::contracts coupled_gemm_ties_and_cross_stratum_regressions_keep_the_pair_default
        PASS [   0.007s] ( 70/149) tuning-campaign-support::contracts checkpoint_units_are_immutable_and_resume_validates_identity_and_bytes
        PASS [   0.006s] ( 71/149) tuning-campaign-support::contracts coupled_gemm_records_corner_edge_and_interior_grid_boundaries
        PASS [   0.005s] ( 72/149) tuning-campaign-support::contracts execution_log_is_durable_append_only_and_resumes_sequence
        PASS [   0.034s] ( 73/149) tuning-campaign-support campaign::publication_tests::preparation_replacement_recovers_before_after_and_during_session_start
        PASS [   0.026s] ( 74/149) tuning-campaign-support::campaign_contracts clean_exit_recovery_reuses_timing_before_and_after_owner_validation
        PASS [   0.006s] ( 75/149) tuning-campaign-support::contracts extent_argmin_handles_unique_wins_plateaus_and_curve_reversals
        PASS [   0.005s] ( 76/149) tuning-campaign-support::contracts retained_threshold_rule_keeps_its_existing_direction_and_noise_semantics
        PASS [   0.006s] ( 77/149) tuning-campaign-support::contracts extent_statistics_cover_equal_weighting_quartiles_and_fail_closed_paths
        PASS [   0.006s] ( 78/149) tuning-campaign-support::contracts journal_recovery_treats_a_complete_record_without_newline_as_torn
        PASS [   0.036s] ( 79/149) tuning-campaign-support campaign::publication_tests::initial_preparation_replays_log_config_checkpoint_and_active_handoff_gaps
        PASS [   0.006s] ( 80/149) tuning-campaign-support::contracts seed_mixer_matches_the_committed_vector_and_banked_roles
        PASS [   0.007s] ( 81/149) tuning-campaign-support::contracts journal_recovery_after_a_paused_prefix_records_recovery_without_interruption
        PASS [   0.006s] ( 82/149) tuning-campaign-support::contracts m4rm_joint_vector_rejects_any_stratum_regression
        PASS [   0.007s] ( 83/149) tuning-campaign-support::contracts journal_recovery_records_interruption_and_preserves_torn_tail
        PASS [   0.006s] ( 84/149) tuning-campaign-support::contracts threshold_fallbacks_cover_missing_no_win_and_reversal
        PASS [   0.006s] ( 85/149) tuning-campaign-support::contracts terminal_journal_lifecycle_fails_closed
        PASS [   0.008s] ( 86/149) tuning-campaign-support::contracts journal_recovery_intent_is_idempotent_at_every_durable_boundary
        PASS [   0.005s] ( 87/149) tuning-campaign-support::host_process_contracts affinity_round_trips_serde_and_rejects_unordered_sets
        PASS [   0.019s] ( 88/149) tuning-campaign-support::campaign_contracts rejected_stderr_remains_durable_across_completion_and_exit_crashes
        PASS [   0.005s] ( 89/149) tuning-campaign-support::host_process_contracts core_arms_resolve_inside_the_mask_or_carry_a_reason
        PASS [   0.005s] ( 90/149) tuning-campaign-support::host_process_contracts lock_observation_requires_a_held_inherited_descriptor
        PASS [   0.010s] ( 91/149) tuning-campaign-support::driver_launcher launcher_rejects_arbitrary_stage_paths_before_creating_them
        PASS [   0.006s] ( 92/149) tuning-campaign-support::protocol_contracts bonferroni_level_widens_with_the_family_and_prior_trials
        PASS [   0.006s] ( 93/149) tuning-campaign-support::protocol_contracts decisions_follow_confidence_bound_margins_not_significance
        PASS [   0.007s] ( 94/149) tuning-campaign-support::protocol_contracts addendum_schema_accepts_the_frozen_smoke_addendum_and_rejects_unknown_fields
        PASS [   0.013s] ( 95/149) tuning-campaign-support::host_process_contracts topology_and_host_observation_report_runtime_facts
        PASS [   0.006s] ( 96/149) tuning-campaign-support::protocol_contracts pair_orders_are_counterbalanced_and_seed_determined
        PASS [   0.005s] ( 97/149) tuning-campaign-support::protocol_contracts protocol_document_pins_the_frozen_shared_settings
        PASS [   0.009s] ( 98/149) tuning-campaign-support::protocol_contracts paired_bootstrap_interval_is_deterministic_and_brackets_known_ratios
        PASS [   0.005s] ( 99/149) tuning-campaign-support::schema_subset arrays_refs_and_combinators_are_enforced
        PASS [   0.005s] (100/149) tuning-campaign-support::schema_subset nullable_type_arrays_accept_null_and_reject_other_types
        FAIL [   0.028s] (101/149) tuning-campaign-support::protocol_contracts acceptance_rejects_every_self_consistent_frozen_fact_replacement
  stdout ───

    running 1 test
    test acceptance_rejects_every_self_consistent_frozen_fact_replacement ... FAILED

    failures:

    failures:
        acceptance_rejects_every_self_consistent_frozen_fact_replacement

    test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 23 filtered out; finished in 0.02s

  stderr ───

    thread 'acceptance_rejects_every_self_consistent_frozen_fact_replacement' (1289) panicked at dev/tools/tuning-campaign-support/tests/protocol_contracts.rs:1233:9:
    protocol-pin: [Finding { rule: "P-03", severity: Error, cell: None, message: "campaign-start freeze: protocol pin differs from campaign-start" }]
    note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

        PASS [   0.005s] (102/149) tuning-campaign-support::schema_subset numeric_bounds_and_enums_are_enforced
        PASS [   0.005s] (103/149) tuning-campaign-support::schema_subset types_required_and_additional_properties_are_enforced
        PASS [   0.005s] (104/149) tuning-campaign-support::schema_subset unsupported_keywords_and_dangling_refs_are_reported_not_ignored
        PASS [   0.005s] (105/149) tuning-campaign-support::schema_subset violations_are_addressed_by_json_pointer
        PASS [   0.081s] (106/149) tuning-campaign-support campaign::publication_tests::prelock_interruption_checksum_and_retirement_recover_every_publication_gap
        PASS [   0.047s] (107/149) tuning-campaign-support::protocol_contracts acceptance_fails_a_regression_beyond_the_equivalence_margin
        PASS [   0.047s] (108/149) tuning-campaign-support::protocol_contracts acceptance_rejects_a_missing_snapshot_despite_matching_source_bytes
        PASS [   0.048s] (109/149) tuning-campaign-support::protocol_contracts acceptance_passes_a_confirmed_improvement_receipt
        PASS [   0.047s] (110/149) tuning-campaign-support::protocol_contracts acceptance_rejects_an_addendum_not_frozen_before_measurement
        PASS [   0.050s] (111/149) tuning-campaign-support::protocol_contracts acceptance_marks_an_interval_spanning_the_margin_inconclusive
        PASS [   0.034s] (112/149) tuning-campaign-support::protocol_contracts unresolved_required_settings_make_a_cell_non_confirmatory
        PASS [   0.050s] (113/149) tuning-campaign-support::protocol_contracts acceptance_rejects_a_digest_that_does_not_match_the_snapshot
        PASS [   0.047s] (114/149) tuning-campaign-support::protocol_contracts acceptance_summary_markdown_renders_from_the_summary_only
        PASS [   0.049s] (115/149) tuning-campaign-support::protocol_contracts acceptance_rejects_a_snapshot_path_that_escapes_the_receipt
        PASS [   0.005s] (116/149) tuning-campaign-support::bin/tuning-extent-campaign-driver tests::campaign_stage_policy_rejects_non_tmp_and_mismatched_paths
        PASS [   0.050s] (117/149) tuning-campaign-support::protocol_contracts acceptance_rejects_changed_producing_input_snapshot
        PASS [   0.005s] (118/149) tuning-campaign-support::bin/tuning-extent-campaign-driver tests::held_lock_requires_an_inherited_descriptor
        PASS [   0.006s] (119/149) tuning-campaign-support::bin/tuning-extent-campaign-driver tests::live_writer_and_reused_identity_are_distinguished
        PASS [   0.007s] (120/149) tuning-campaign-support::bin/tuning-extent-campaign-driver tests::interrupted_derived_projection_publication_is_replayed_and_bound
        PASS [   0.082s] (121/149) tuning-campaign-support::campaign_contracts completion_intent_recovers_every_raw_exit_validation_and_checkpoint_boundary
        PASS [   0.006s] (122/149) tuning-campaign-support::bin/tuning-extent-campaign-driver tests::promotion_is_idempotent_and_rejects_different_occupied_bytes
        PASS [   0.006s] (123/149) tuning-campaign-support::bin/tuning-extent-campaign-driver tests::producing_input_manifest_rejects_authority_and_path_mutations
        PASS [   0.005s] (124/149) tuning-campaign-support::bin/tuning-extent-campaign-driver tests::relocated_build_source_observation_is_rejected_before_campaign_work
        PASS [   0.012s] (125/149) tuning-campaign-support::bin/tuning-extent-campaign-driver tests::incomplete_executable_staging_replays_before_campaign_config_exists
        PASS [   0.011s] (126/149) tuning-campaign-support::bin/tuning-extent-campaign-driver tests::nonzero_exit_is_never_eligible
        PASS [   0.006s] (127/149) tuning-campaign-support::bin/tuning-extent-campaign-driver tests::relocated_preflight_report_is_rejected_before_campaign_work
        PASS [   0.047s] (128/149) tuning-campaign-support::protocol_contracts source_control_metadata_does_not_change_acceptance
        PASS [   0.006s] (129/149) tuning-campaign-support::bin/tuning-extent-campaign-driver tests::staged_process_receives_only_the_declared_measurement_environment
        PASS [   0.024s] (130/149) tuning-campaign-support::bin/tuning-extent-campaign-driver tests::affinity_is_observed_from_the_current_os_mask_and_binds_resume
        PASS [   0.010s] (131/149) tuning-campaign-support::bin/tuning-extent-campaign-driver tests::stderr_callback_failure_still_drains_and_reaps
        PASS [   0.018s] (132/149) tuning-campaign-support::bin/tuning-extent-campaign-driver tests::staged_reporting_preflights_use_their_actual_cli_flags_and_empty_stdin
        PASS [   0.041s] (133/149) tuning-campaign-support::bin/tuning-extent-campaign-driver tests::a_probe_streams_validates_and_commits_one_bound_checkpoint
        PASS [   0.024s] (134/149) tuning-campaign-support::bin/tuning-extent-campaign-driver tests::simultaneous_binary_streams_are_drained_without_loss
        PASS [   0.083s] (135/149) tuning-campaign-support::protocol_contracts acceptance_rejects_a_receipt_with_missing_protocol_identity
        PASS [   0.083s] (136/149) tuning-campaign-support::protocol_contracts acceptance_rejects_missing_and_self_referential_resolution_evidence
        PASS [   0.077s] (137/149) tuning-campaign-support::protocol_contracts decoder_cells_require_quality_intervals_and_matched_settings
        PASS [   0.085s] (138/149) tuning-campaign-support::protocol_contracts acceptance_preserves_unavailable_and_not_material_cells
        PASS [   0.091s] (139/149) tuning-campaign-support::driver_launcher launcher_discovers_publisher_intent_before_selecting_identity_or_building
        PASS [   0.096s] (140/149) tuning-campaign-support::driver_launcher launcher_discovers_complete_publisher_temporary_before_selecting_identity_or_building
        PASS [   0.048s] (141/149) tuning-campaign-support::bin/tuning-extent-campaign-driver tests::rejected_raw_streams_cannot_recover_after_completion_crash
        PASS [   0.050s] (142/149) tuning-campaign-support::bin/tuning-extent-campaign-driver tests::progress_is_delivered_before_exit
        PASS [   0.116s] (143/149) tuning-campaign-support::protocol_contracts acceptance_accepts_a_resumed_run_and_rejects_a_repeated_cell
        PASS [   0.086s] (144/149) tuning-campaign-support::bin/tuning-extent-campaign-driver tests::clean_completion_crashes_recover_without_fresh_child_replay
        PASS [   0.087s] (145/149) tuning-campaign-support::bin/tuning-extent-campaign-driver tests::timeout_kills_descendant_holding_pipes
        PASS [   0.106s] (146/149) tuning-campaign-support::bin/tuning-extent-campaign-driver tests::timeout_reaps_an_escaped_descendant
        PASS [   0.138s] (147/149) tuning-campaign-support::bin/tuning-extent-campaign-driver tests::kill_grace_handles_a_child_ignoring_term
        PASS [   0.231s] (148/149) tuning-campaign-support::host_process_contracts run_process_drains_both_streams_and_times_out_cleanly
        PASS [   0.613s] (149/149) tuning-campaign-support::protocol_contracts runner_announces_the_log_before_work_and_resumes_without_repeating
────────────
     Summary [   0.662s] 149 tests run: 148 passed, 1 failed, 0 skipped
        FAIL [   0.028s] (101/149) tuning-campaign-support::protocol_contracts acceptance_rejects_every_self_consistent_frozen_fact_replacement
error: test run failed
```

### Pilot default red evidence

```text
$ ./scripts/cargo-budget.sh --test cargo nextest run -p tuning-campaign-support --all-features --cargo-profile ci-test --profile ci -E 'test(acceptance_preserves_unavailable)'
cargo-budget: 24 jobs (1 live, 24 cpus)
   Compiling tuning-campaign-support v0.1.0 (/home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-f547c394/dev/tools/tuning-campaign-support)
    Finished `ci-test` profile [optimized] target(s) in 17.57s
────────────
 Nextest run ID c80c43f1-2074-492a-9b68-7adc8cc2bc2d with nextest profile: ci
    Starting 1 test across 12 binaries (148 tests skipped)
        FAIL [   0.037s] (1/1) tuning-campaign-support::protocol_contracts acceptance_preserves_unavailable_and_not_material_cells
  stdout ───

    running 1 test
    test acceptance_preserves_unavailable_and_not_material_cells ... FAILED

    failures:

    failures:
        acceptance_preserves_unavailable_and_not_material_cells

    test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 23 filtered out; finished in 0.03s

  stderr ───

    thread 'acceptance_preserves_unavailable_and_not_material_cells' (1053) panicked at dev/tools/tuning-campaign-support/tests/protocol_contracts.rs:1413:5:
    assertion `left == right` failed: [Finding { rule: "P-15", severity: Error, cell: Some("p"), message: "pair count differs from the saved plan" }]
      left: Rejected
     right: Accepted
    note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

────────────
     Summary [   0.038s] 1 test run: 0 passed, 1 failed, 148 skipped
        FAIL [   0.037s] (1/1) tuning-campaign-support::protocol_contracts acceptance_preserves_unavailable_and_not_material_cells
error: test run failed
```

### Pilot qualification red evidence

```text
$ ./scripts/cargo-budget.sh --test cargo nextest run -p tuning-campaign-support --all-features --cargo-profile ci-test --profile ci -E 'test(exploratory_only_receipts)'
cargo-budget: 24 jobs (1 live, 24 cpus)
   Compiling tuning-campaign-support v0.1.0 (/home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-f547c394/dev/tools/tuning-campaign-support)
    Finished `ci-test` profile [optimized] target(s) in 5.32s
────────────
 Nextest run ID a26c7046-8123-49da-b2a9-faaf8ac19e4d with nextest profile: ci
    Starting 1 test across 12 binaries (149 tests skipped)
        FAIL [   0.018s] (1/1) tuning-campaign-support::protocol_contracts exploratory_only_receipts_never_qualify_for_production_selection
  stdout ───

    running 1 test
    test exploratory_only_receipts_never_qualify_for_production_selection ... FAILED

    failures:

    failures:
        exploratory_only_receipts_never_qualify_for_production_selection

    test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 24 filtered out; finished in 0.01s

  stderr ───

    thread 'exploratory_only_receipts_never_qualify_for_production_selection' (209) panicked at dev/tools/tuning-campaign-support/tests/protocol_contracts.rs:1136:5:
    exploration supplies no confirmatory evidence
    note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

────────────
     Summary [   0.019s] 1 test run: 0 passed, 1 failed, 149 skipped
        FAIL [   0.018s] (1/1) tuning-campaign-support::protocol_contracts exploratory_only_receipts_never_qualify_for_production_selection
error: test run failed
```

### Final support suite

```text
$ ./scripts/cargo-budget.sh --test cargo nextest run -p tuning-campaign-support --all-features --cargo-profile ci-test --profile ci
cargo-budget: 12 jobs (2 live, 24 cpus)
    Blocking waiting for file lock on build directory
   Compiling tuning-campaign-support v0.1.0 (/home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-f547c394/dev/tools/tuning-campaign-support)
    Finished `ci-test` profile [optimized] target(s) in 1m 23s
────────────
 Nextest run ID 47fa4f18-5efd-4666-899e-c43cdeb59196 with nextest profile: ci
    Starting 150 tests across 12 binaries
        PASS [   0.006s] (  1/150) tuning-campaign-support abtest::tests::wilson_interval_brackets_the_point_estimate
        PASS [   0.007s] (  2/150) tuning-campaign-support abtest::tests::nearest_rank_percentiles_use_the_frozen_ranks
        PASS [   0.007s] (  3/150) tuning-campaign-support campaign::publication_tests::immutable_artifact_publication_preserves_ambiguous_bytes_and_excludes_writers
        PASS [   0.007s] (  4/150) tuning-campaign-support campaign::publication_tests::preparation_discovery_rejects_intent_under_another_session_root
        PASS [   0.007s] (  5/150) tuning-campaign-support campaign::publication_tests::resume_identity_precedes_temporary_cleanup_and_namespace_is_exact
        PASS [   0.008s] (  6/150) tuning-campaign-support campaign::publication_tests::uncommitted_mode_intent_never_claims_a_launch_occurred
        PASS [   0.008s] (  7/150) tuning-campaign-support campaign::publication_tests::tampered_or_cross_identity_publication_preserves_all_original_evidence
        PASS [   0.008s] (  8/150) tuning-campaign-support journal::active_repair_tests::active_repair_preserves_complete_malformed_records_and_rejects_wrong_session
        PASS [   0.009s] (  9/150) tuning-campaign-support abtest::tests::xoshiro_matches_the_reference_first_output
        PASS [   0.010s] ( 10/150) tuning-campaign-support campaign::publication_tests::prelock_evidence_binds_every_distinct_mode_writer_and_replay_checks_mutation
        PASS [   0.010s] ( 11/150) tuning-campaign-support campaign::publication_tests::preparation_recovers_every_uncommitted_initial_intent_boundary
        PASS [   0.011s] ( 12/150) tuning-campaign-support campaign::publication_tests::immutable_artifact_publication_replays_intent_and_target_boundaries
        PASS [   0.011s] ( 13/150) tuning-campaign-support campaign::publication_tests::preparation_intent_replays_initial_publication_and_rejects_changed_identity
        PASS [   0.010s] ( 14/150) tuning-campaign-support campaign::publication_tests::session_checksum_requires_strict_artifact_path_order
        PASS [   0.011s] ( 15/150) tuning-campaign-support campaign::publication_tests::replacement_preparation_rejects_byte_identical_checksum_alias
        PASS [   0.006s] ( 16/150) tuning-campaign-support timing::tests::timed_calls_supply_the_exact_rotating_fixture_bank
        PASS [   0.012s] ( 17/150) tuning-campaign-support campaign::publication_tests::recovered_interruption_journals_cleanup_after_release_without_inventing_work
        PASS [   0.013s] ( 18/150) tuning-campaign-support campaign::publication_tests::synced_transition_intent_recovers_observation_and_journals_it_once
        PASS [   0.014s] ( 19/150) tuning-campaign-support campaign::publication_tests::mode_publication_replays_intent_sync_temporary_sync_and_destination_link
        PASS [   0.014s] ( 20/150) tuning-campaign-support campaign::publication_tests::preparation_replays_descriptor_prepare_and_active_claim_gaps
        PASS [   0.007s] ( 21/150) tuning-campaign-support::campaign_contracts candidate_blocks_require_exact_three_candidate_rotation_and_interleaving
        PASS [   0.008s] ( 22/150) tuning-campaign-support::campaign_contracts campaign_reconciliation_validates_the_complete_all_owner_universe
        PASS [   0.005s] ( 23/150) tuning-campaign-support::campaign_contracts configured_timing_emits_outside_intervals_and_stops_on_callback_error
        PASS [   0.010s] ( 24/150) tuning-campaign-support::campaign_contracts acceptance_reconciliation_rejects_unknown_keys_wrong_cases_and_duplicates
        PASS [   0.008s] ( 25/150) tuning-campaign-support::campaign_contracts checkpoints_bind_progress_then_reconcile_missing_acceptance_once
        PASS [   0.008s] ( 26/150) tuning-campaign-support::campaign_contracts clean_lifecycle_cannot_publish_before_observed_release
        PASS [   0.007s] ( 27/150) tuning-campaign-support::campaign_contracts derived_manifests_cannot_change_reserved_slots_or_input_digest
        PASS [   0.008s] ( 28/150) tuning-campaign-support::campaign_contracts completed_exit_cannot_accept_after_its_retained_raw_artifact_changes
        PASS [   0.009s] ( 29/150) tuning-campaign-support::campaign_contracts completion_records_missing_callback_progress_before_rejecting_exit
        PASS [   0.014s] ( 30/150) tuning-campaign-support::campaign_contracts budget_terminal_checksum_gap_and_same_identity_resume_preserve_the_prefix
        PASS [   0.006s] ( 31/150) tuning-campaign-support::campaign_contracts missing_progress_or_validation_never_accepts_a_checkpoint
        PASS [   0.022s] ( 32/150) tuning-campaign-support campaign::publication_tests::retirement_and_reopen_reject_noncanonical_checksum_bindings
        PASS [   0.009s] ( 33/150) tuning-campaign-support::campaign_contracts interleaved_manifest_accepts_rotated_candidate_blocks_and_rejects_mutations
        PASS [   0.010s] ( 34/150) tuning-campaign-support::campaign_contracts durable_descriptors_reject_concurrent_writer_reuse_and_identity_change
        PASS [   0.007s] ( 35/150) tuning-campaign-support::campaign_contracts opaque_cases_preserve_order_but_reject_noncanonical_and_duplicate_keys
        PASS [   0.016s] ( 36/150) tuning-campaign-support::campaign_contracts active_claim_and_torn_log_recover_in_place_before_release_and_replacement
        PASS [   0.014s] ( 37/150) tuning-campaign-support::campaign_contracts clean_exit_recovery_rejects_raw_stream_and_progress_mutations
        PASS [   0.006s] ( 38/150) tuning-campaign-support::campaign_contracts pending_recovery_requires_synced_journal_handshake_and_is_idempotent
        PASS [   0.009s] ( 39/150) tuning-campaign-support::campaign_contracts invalid_utf8_stdout_keeps_exact_exit_bytes_and_cannot_accept
        PASS [   0.013s] ( 40/150) tuning-campaign-support::campaign_contracts completed_raw_result_never_overrides_failure_or_timeout_observation
        PASS [   0.006s] ( 41/150) tuning-campaign-support::campaign_contracts process_paths_environment_and_staged_bytes_are_strict
        PASS [   0.013s] ( 42/150) tuning-campaign-support::campaign_contracts every_bound_child_record_requires_its_exact_structured_case
        PASS [   0.007s] ( 43/150) tuning-campaign-support::campaign_contracts prelock_failure_does_not_invent_release_and_timeout_never_accepts
        PASS [   0.009s] ( 44/150) tuning-campaign-support::campaign_contracts prefix_replay_rejects_overlapping_spawns_and_resampling_before_validation
        PASS [   0.005s] ( 45/150) tuning-campaign-support::campaign_contracts progress_rejects_probe_wrong_identity_and_out_of_order_events
        PASS [   0.028s] ( 46/150) tuning-campaign-support journal::active_repair_tests::active_claim_survives_every_same_session_torn_repair_boundary
        PASS [   0.008s] ( 47/150) tuning-campaign-support::campaign_contracts progress_framing_and_result_consistency_are_exact
        PASS [   0.007s] ( 48/150) tuning-campaign-support::campaign_contracts unclean_wrapper_needs_dead_tree_and_independent_lock_observation
        PASS [   0.006s] ( 49/150) tuning-campaign-support::checkpoint_provenance changed_producing_digest_rejects_before_mutating_checkpoint_evidence
        PASS [   0.006s] ( 50/150) tuning-campaign-support::checkpoint_provenance inspection_validates_without_recovering_or_creating_files
        PASS [   0.013s] ( 51/150) tuning-campaign-support::campaign_contracts prepared_writer_death_closes_without_fictitious_wrapper_or_release
        PASS [   0.006s] ( 52/150) tuning-campaign-support::checkpoint_provenance metadata_only_resume_retains_completed_result_and_original_manifest
        PASS [   0.006s] ( 53/150) tuning-campaign-support::checkpoint_provenance metadata_only_revision_change_allows_resume_and_initialize_replay
        PASS [   0.015s] ( 54/150) tuning-campaign-support::campaign_contracts pending_completion_rejects_wrong_outcome_case_artifacts_and_duplicate_intent
        PASS [   0.009s] ( 55/150) tuning-campaign-support::campaign_contracts result_digest_excludes_stdout_framing_and_environment_is_canonical
        PASS [   0.032s] ( 56/150) tuning-campaign-support campaign::publication_tests::preparation_replacement_recovers_before_after_and_during_session_start
        PASS [   0.010s] ( 57/150) tuning-campaign-support::campaign_contracts transition_projection_recovers_once_after_durable_state_commit
        PASS [   0.006s] ( 58/150) tuning-campaign-support::contracts call_calibration_and_window_rotation_preserve_the_protocol
        PASS [   0.014s] ( 59/150) tuning-campaign-support::campaign_contracts prelock_abort_rejects_any_held_or_fabricated_wrapper_evidence
        PASS [   0.009s] ( 60/150) tuning-campaign-support::campaign_contracts wrong_identity_incomplete_stream_and_probe_windows_reject
        PASS [   0.012s] ( 61/150) tuning-campaign-support::campaign_contracts raw_stderr_validity_and_progress_equivalence_cannot_be_mutated_into_acceptance
        PASS [   0.007s] ( 62/150) tuning-campaign-support::contracts atomic_helpers_distinguish_immutable_evidence_from_mutable_pointers
        PASS [   0.007s] ( 63/150) tuning-campaign-support::contracts checkpoint_identity_is_validated_before_pending_recovery
        PASS [   0.006s] ( 64/150) tuning-campaign-support::contracts checkpoint_manifest_and_temp_namespace_fail_closed
        PASS [   0.026s] ( 65/150) tuning-campaign-support::campaign_contracts clean_exit_recovery_reuses_timing_before_and_after_owner_validation
        PASS [   0.007s] ( 66/150) tuning-campaign-support::contracts checkpoint_pending_namespace_requires_the_exact_generated_shape
        PASS [   0.037s] ( 67/150) tuning-campaign-support campaign::publication_tests::initial_preparation_replays_log_config_checkpoint_and_active_handoff_gaps
        PASS [   0.007s] ( 68/150) tuning-campaign-support::contracts checkpoint_pending_recovery_replays_every_deletion_boundary
        PASS [   0.005s] ( 69/150) tuning-campaign-support::contracts coupled_gemm_ties_and_cross_stratum_regressions_keep_the_pair_default
        PASS [   0.006s] ( 70/150) tuning-campaign-support::contracts coupled_gemm_requires_all_slices_and_one_strict_pair_winner
        PASS [   0.006s] ( 71/150) tuning-campaign-support::contracts child_v2_framing_is_canonical_and_fail_closed
        PASS [   0.006s] ( 72/150) tuning-campaign-support::contracts coupled_gemm_records_corner_edge_and_interior_grid_boundaries
        PASS [   0.005s] ( 73/150) tuning-campaign-support::contracts extent_statistics_cover_equal_weighting_quartiles_and_fail_closed_paths
        PASS [   0.006s] ( 74/150) tuning-campaign-support::contracts extent_argmin_handles_unique_wins_plateaus_and_curve_reversals
        PASS [   0.006s] ( 75/150) tuning-campaign-support::contracts execution_log_is_durable_append_only_and_resumes_sequence
        PASS [   0.016s] ( 76/150) tuning-campaign-support::campaign_contracts recovery_requires_release_then_censored_interruption_and_allows_resume
        PASS [   0.008s] ( 77/150) tuning-campaign-support::contracts checkpoint_units_are_immutable_and_resume_validates_identity_and_bytes
        PASS [   0.006s] ( 78/150) tuning-campaign-support::contracts journal_recovery_records_interruption_and_preserves_torn_tail
        PASS [   0.006s] ( 79/150) tuning-campaign-support::contracts retained_threshold_rule_keeps_its_existing_direction_and_noise_semantics
        PASS [   0.007s] ( 80/150) tuning-campaign-support::contracts journal_recovery_treats_a_complete_record_without_newline_as_torn
        PASS [   0.005s] ( 81/150) tuning-campaign-support::contracts threshold_fallbacks_cover_missing_no_win_and_reversal
        PASS [   0.007s] ( 82/150) tuning-campaign-support::contracts m4rm_joint_vector_rejects_any_stratum_regression
        PASS [   0.008s] ( 83/150) tuning-campaign-support::contracts journal_recovery_intent_is_idempotent_at_every_durable_boundary
        PASS [   0.007s] ( 84/150) tuning-campaign-support::contracts terminal_journal_lifecycle_fails_closed
        PASS [   0.005s] ( 85/150) tuning-campaign-support::host_process_contracts core_arms_resolve_inside_the_mask_or_carry_a_reason
        PASS [   0.020s] ( 86/150) tuning-campaign-support::campaign_contracts rejected_stderr_remains_durable_across_completion_and_exit_crashes
        PASS [   0.005s] ( 87/150) tuning-campaign-support::host_process_contracts lock_observation_requires_a_held_inherited_descriptor
        PASS [   0.005s] ( 88/150) tuning-campaign-support::host_process_contracts affinity_round_trips_serde_and_rejects_unordered_sets
        PASS [   0.010s] ( 89/150) tuning-campaign-support::contracts journal_recovery_after_a_paused_prefix_records_recovery_without_interruption
        PASS [   0.011s] ( 90/150) tuning-campaign-support::contracts seed_mixer_matches_the_committed_vector_and_banked_roles
        PASS [   0.011s] ( 91/150) tuning-campaign-support::driver_launcher launcher_rejects_arbitrary_stage_paths_before_creating_them
        PASS [   0.007s] ( 92/150) tuning-campaign-support::protocol_contracts addendum_schema_accepts_the_frozen_smoke_addendum_and_rejects_unknown_fields
        PASS [   0.006s] ( 93/150) tuning-campaign-support::protocol_contracts decisions_follow_confidence_bound_margins_not_significance
        PASS [   0.014s] ( 94/150) tuning-campaign-support::host_process_contracts topology_and_host_observation_report_runtime_facts
        PASS [   0.012s] ( 95/150) tuning-campaign-support::protocol_contracts bonferroni_level_widens_with_the_family_and_prior_trials
        PASS [   0.006s] ( 96/150) tuning-campaign-support::protocol_contracts pair_orders_are_counterbalanced_and_seed_determined
        PASS [   0.009s] ( 97/150) tuning-campaign-support::protocol_contracts paired_bootstrap_interval_is_deterministic_and_brackets_known_ratios
        PASS [   0.008s] ( 98/150) tuning-campaign-support::protocol_contracts protocol_document_pins_the_frozen_shared_settings
        PASS [   0.027s] ( 99/150) tuning-campaign-support::protocol_contracts exploratory_only_receipts_never_qualify_for_production_selection
        PASS [   0.005s] (100/150) tuning-campaign-support::schema_subset arrays_refs_and_combinators_are_enforced
        PASS [   0.083s] (101/150) tuning-campaign-support campaign::publication_tests::prelock_interruption_checksum_and_retirement_recover_every_publication_gap
        PASS [   0.049s] (102/150) tuning-campaign-support::protocol_contracts acceptance_fails_a_regression_beyond_the_equivalence_margin
        PASS [   0.005s] (103/150) tuning-campaign-support::schema_subset numeric_bounds_and_enums_are_enforced
        PASS [   0.050s] (104/150) tuning-campaign-support::protocol_contracts acceptance_passes_a_confirmed_improvement_receipt
        PASS [   0.008s] (105/150) tuning-campaign-support::schema_subset nullable_type_arrays_accept_null_and_reject_other_types
        PASS [   0.050s] (106/150) tuning-campaign-support::protocol_contracts acceptance_summary_markdown_renders_from_the_summary_only
        PASS [   0.053s] (107/150) tuning-campaign-support::protocol_contracts acceptance_rejects_a_missing_snapshot_despite_matching_source_bytes
        PASS [   0.051s] (108/150) tuning-campaign-support::protocol_contracts acceptance_rejects_changed_producing_input_snapshot
        PASS [   0.053s] (109/150) tuning-campaign-support::protocol_contracts acceptance_rejects_a_digest_that_does_not_match_the_snapshot
        PASS [   0.007s] (110/150) tuning-campaign-support::schema_subset types_required_and_additional_properties_are_enforced
        PASS [   0.006s] (111/150) tuning-campaign-support::schema_subset violations_are_addressed_by_json_pointer
        PASS [   0.057s] (112/150) tuning-campaign-support::protocol_contracts acceptance_marks_an_interval_spanning_the_margin_inconclusive
        PASS [   0.007s] (113/150) tuning-campaign-support::schema_subset unsupported_keywords_and_dangling_refs_are_reported_not_ignored
        PASS [   0.035s] (114/150) tuning-campaign-support::protocol_contracts unresolved_required_settings_make_a_cell_non_confirmatory
        PASS [   0.005s] (115/150) tuning-campaign-support::bin/tuning-extent-campaign-driver tests::campaign_stage_policy_rejects_non_tmp_and_mismatched_paths
        PASS [   0.088s] (116/150) tuning-campaign-support::campaign_contracts completion_intent_recovers_every_raw_exit_validation_and_checkpoint_boundary
        PASS [   0.060s] (117/150) tuning-campaign-support::protocol_contracts acceptance_rejects_a_snapshot_path_that_escapes_the_receipt
        PASS [   0.006s] (118/150) tuning-campaign-support::bin/tuning-extent-campaign-driver tests::live_writer_and_reused_identity_are_distinguished
        PASS [   0.009s] (119/150) tuning-campaign-support::bin/tuning-extent-campaign-driver tests::held_lock_requires_an_inherited_descriptor
        PASS [   0.064s] (120/150) tuning-campaign-support::protocol_contracts acceptance_rejects_an_addendum_not_frozen_before_measurement
        PASS [   0.011s] (121/150) tuning-campaign-support::bin/tuning-extent-campaign-driver tests::interrupted_derived_projection_publication_is_replayed_and_bound
        PASS [   0.007s] (122/150) tuning-campaign-support::bin/tuning-extent-campaign-driver tests::producing_input_manifest_rejects_authority_and_path_mutations
        PASS [   0.006s] (123/150) tuning-campaign-support::bin/tuning-extent-campaign-driver tests::promotion_is_idempotent_and_rejects_different_occupied_bytes
        PASS [   0.010s] (124/150) tuning-campaign-support::bin/tuning-extent-campaign-driver tests::nonzero_exit_is_never_eligible
        PASS [   0.006s] (125/150) tuning-campaign-support::bin/tuning-extent-campaign-driver tests::relocated_build_source_observation_is_rejected_before_campaign_work
        PASS [   0.015s] (126/150) tuning-campaign-support::bin/tuning-extent-campaign-driver tests::incomplete_executable_staging_replays_before_campaign_config_exists
        PASS [   0.049s] (127/150) tuning-campaign-support::protocol_contracts source_control_metadata_does_not_change_acceptance
        PASS [   0.005s] (128/150) tuning-campaign-support::bin/tuning-extent-campaign-driver tests::relocated_preflight_report_is_rejected_before_campaign_work
        PASS [   0.006s] (129/150) tuning-campaign-support::bin/tuning-extent-campaign-driver tests::staged_process_receives_only_the_declared_measurement_environment
        PASS [   0.011s] (130/150) tuning-campaign-support::bin/tuning-extent-campaign-driver tests::stderr_callback_failure_still_drains_and_reaps
        PASS [   0.018s] (131/150) tuning-campaign-support::bin/tuning-extent-campaign-driver tests::staged_reporting_preflights_use_their_actual_cli_flags_and_empty_stdin
        PASS [   0.087s] (132/150) tuning-campaign-support::protocol_contracts acceptance_rejects_a_receipt_with_missing_protocol_identity
        PASS [   0.035s] (133/150) tuning-campaign-support::bin/tuning-extent-campaign-driver tests::affinity_is_observed_from_the_current_os_mask_and_binds_resume
        PASS [   0.081s] (134/150) tuning-campaign-support::protocol_contracts decoder_cells_require_quality_intervals_and_matched_settings
        PASS [   0.024s] (135/150) tuning-campaign-support::bin/tuning-extent-campaign-driver tests::simultaneous_binary_streams_are_drained_without_loss
        PASS [   0.095s] (136/150) tuning-campaign-support::driver_launcher launcher_discovers_complete_publisher_temporary_before_selecting_identity_or_building
        PASS [   0.090s] (137/150) tuning-campaign-support::protocol_contracts acceptance_rejects_missing_and_self_referential_resolution_evidence
        PASS [   0.043s] (138/150) tuning-campaign-support::bin/tuning-extent-campaign-driver tests::a_probe_streams_validates_and_commits_one_bound_checkpoint
        PASS [   0.099s] (139/150) tuning-campaign-support::driver_launcher launcher_discovers_publisher_intent_before_selecting_identity_or_building
        PASS [   0.108s] (140/150) tuning-campaign-support::protocol_contracts acceptance_preserves_unavailable_and_not_material_cells
        PASS [   0.053s] (141/150) tuning-campaign-support::bin/tuning-extent-campaign-driver tests::progress_is_delivered_before_exit
        PASS [   0.053s] (142/150) tuning-campaign-support::bin/tuning-extent-campaign-driver tests::rejected_raw_streams_cannot_recover_after_completion_crash
        PASS [   0.127s] (143/150) tuning-campaign-support::protocol_contracts acceptance_accepts_a_resumed_run_and_rejects_a_repeated_cell
        PASS [   0.097s] (144/150) tuning-campaign-support::bin/tuning-extent-campaign-driver tests::clean_completion_crashes_recover_without_fresh_child_replay
        PASS [   0.087s] (145/150) tuning-campaign-support::bin/tuning-extent-campaign-driver tests::timeout_kills_descendant_holding_pipes
        PASS [   0.108s] (146/150) tuning-campaign-support::bin/tuning-extent-campaign-driver tests::timeout_reaps_an_escaped_descendant
        PASS [   0.138s] (147/150) tuning-campaign-support::bin/tuning-extent-campaign-driver tests::kill_grace_handles_a_child_ignoring_term
        PASS [   0.241s] (148/150) tuning-campaign-support::host_process_contracts run_process_drains_both_streams_and_times_out_cleanly
        PASS [   0.263s] (149/150) tuning-campaign-support::protocol_contracts acceptance_rejects_every_self_consistent_frozen_fact_replacement
        PASS [   0.613s] (150/150) tuning-campaign-support::protocol_contracts runner_announces_the_log_before_work_and_resumes_without_repeating
────────────
     Summary [   0.672s] 150 tests run: 150 passed, 0 skipped
```

### Workspace lint

```text
$ ./scripts/cargo-budget.sh cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo-budget: 6 jobs (4 live, 24 cpus)
    Checking tuning-campaign-support v0.1.0 (/home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-f547c394/dev/tools/tuning-campaign-support)
warning: gf2-kernels-hip@0.1.0: skip gfx940: hipcc --offload-arch=gfx940 on /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-f547c394/crates/gf2-kernels-hip/kernels/gfx940/probe.cpp exited with exit status: 1
    Checking gf2-core v0.1.0 (/home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-f547c394/crates/gf2-core)
    Checking gf2-algebra v0.1.0 (/home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-f547c394/crates/gf2-algebra)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 10.23s
```

### Formatting

```text
$ ./scripts/cargo-budget.sh cargo fmt --all -- --check
cargo-budget: 8 jobs (3 live, 24 cpus)
```

### Smoke launcher syntax

```text
$ bash -n dev/bench_results/f547c394/run-smoke.sh
exit=0
```

### Final staging attempt

```text
$ git add <explicit listed source, document and evidence paths>
fatal: Unable to create '/home/vkaskivuo/Projects/gf2/.git/worktrees/agent-f547c394/index.lock': Read-only file system
exit=128
```

### Rust 1.95.0 compatibility

```text
$ RUSTUP_TOOLCHAIN=1.95.0 ./scripts/cargo-budget.sh cargo check -p tuning-campaign-support --all-targets --all-features
cargo-budget: 4 jobs (5 live, 24 cpus)
    Blocking waiting for file lock on build directory
    Checking tuning-campaign-support v0.1.0 (/home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-f547c394/dev/tools/tuning-campaign-support)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 12.03s
```

### Release evaluator build

```text
$ ./scripts/cargo-budget.sh cargo build --release -p tuning-campaign-support --bin benchmark-acceptance
cargo-budget: 6 jobs (4 live, 24 cpus)
   Compiling tuning-campaign-support v0.1.0 (/home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-f547c394/dev/tools/tuning-campaign-support)
    Finished `release` profile [optimized] target(s) in 28.74s
```

### Independent acceptance and Git exports

```text
$ Commands printed below
$ target/release/benchmark-acceptance dev/bench_results/f547c394/2026-09-07-f547c394-protocol-confirmation
GF2_BENCHMARK_ACCEPTANCE=Accepted qualifies=false findings=0
exit=0
raw files verified unchanged from HEAD=40; producing Cargo.lock tracked=yes
$ target/release/benchmark-acceptance target/rework/export/dev/bench_results/f547c394/2026-09-07-f547c394-protocol-confirmation
GF2_BENCHMARK_ACCEPTANCE=Accepted qualifies=false findings=0
exit=0
raw files verified unchanged from HEAD=40; producing Cargo.lock tracked=yes
$ target/release/benchmark-acceptance dev/bench_results/f547c394/2026-09-07-f547c394-protocol-pilot
GF2_BENCHMARK_ACCEPTANCE=Accepted qualifies=false findings=0
exit=0
raw files verified unchanged from HEAD=37; producing Cargo.lock tracked=yes
$ target/release/benchmark-acceptance target/rework/export/dev/bench_results/f547c394/2026-09-07-f547c394-protocol-pilot
GF2_BENCHMARK_ACCEPTANCE=Accepted qualifies=false findings=0
exit=0
raw files verified unchanged from HEAD=37; producing Cargo.lock tracked=yes
acceptance executable SHA-256=0584560cf922589414e534fafb0b1a2164746d07d33c420dbeca3cdad3c9ce0d
behavior source map SHA-256=b96863ac52f36852f2f180cd5dd87fdaa548b3b6ec14331daece1c217ff5c907
```

### Procfs disappearance reproduction

```text
$ Open /proc/PID/stat, terminate and reap the child, then read the open descriptor
opened /proc/PID/stat; reaped PID; read returned errno=3: No such process
```

### Initial CI before the procfs fix

```text
$ ./scripts/cargo-ci.sh
--- test failures ---
        FAIL [   0.019s] (5265/5952) gf2-sim gpu::imp::tests::test_dispatcher_acquires_stream_and_allocates
        FAIL [   0.117s] (5944/5952) tuning-campaign-support::bin/tuning-extent-campaign-driver tests::kill_grace_handles_a_child_ignoring_term
        FAIL [   0.019s] (5265/5952) gf2-sim gpu::imp::tests::test_dispatcher_acquires_stream_and_allocates
        FAIL [   0.117s] (5944/5952) tuning-campaign-support::bin/tuning-extent-campaign-driver tests::kill_grace_handles_a_child_ignoring_term
  ✓ check: ok (9s)
  ✓ tuning-core-no-default: ok (0s)
  ✓ tuning-core-codec-only: ok (2s)
  ✓ tuning-algebra-no-default: ok (2s)
  ✓ tuning-algebra-codec-only: ok (1s)
  ✓ tuning-coding-no-default: ok (2s)
  ✓ tuning-coding-codec-only: ok (1s)
  ✓ test-build: ok (201s)
  ✗ test: FAILED (exit 100, 14s)
  ✓ tuning-profile-build: ok (20s)
  ✓ tuning-profile-nextest: ok (5s)
  ✓ tuning-algebra-calibration-build: ok (8s)
  ✓ tuning-algebra-calibration-nextest: ok (0s)
  ✓ tuning-campaign-support-build: ok (0s)
  ✓ tuning-campaign-support-nextest: ok (1s)
  ✓ tuning-campaign-validator: ok (0s)
  ✓ tuning-campaign-launcher-syntax: ok (0s)
  ✓ tuning-lifecycle-cargo: ok (3s)
  ✓ clippy: ok (9s)
  ✓ fmt: ok (3s)
  ✓ baked-core: ok (164s)
  ✓ tuning-core-artifact: ok (27s)
  ✓ tuning-algebra-artifacts: ok (1s)
  ✓ tuning-coding-codec: ok (9s)
  ✓ tuning-composer: ok (6s)

exit=1
```

### Final CI after the procfs fix

```text
$ ./scripts/cargo-ci.sh
--- test failures ---
        FAIL [   0.031s] (5264/5952) gf2-sim gpu::imp::tests::test_dispatcher_acquires_stream_and_allocates
     TIMEOUT [   8.124s] (5952/5952) gf2-sim::permanent_rare_event_artifacts rare_event_artifact_partial_publish_recovery
        FAIL [   0.031s] (5264/5952) gf2-sim gpu::imp::tests::test_dispatcher_acquires_stream_and_allocates
     TIMEOUT [   8.124s] (5952/5952) gf2-sim::permanent_rare_event_artifacts rare_event_artifact_partial_publish_recovery
  ✓ check: ok (1s)
  ✓ tuning-core-no-default: ok (0s)
  ✓ tuning-core-codec-only: ok (1s)
  ✓ tuning-algebra-no-default: ok (0s)
  ✓ tuning-algebra-codec-only: ok (0s)
  ✓ tuning-coding-no-default: ok (0s)
  ✓ tuning-coding-codec-only: ok (0s)
  ✓ test-build: ok (212s)
  ✗ test: FAILED (exit 100, 197s)
  ✓ tuning-profile-build: ok (21s)
  ✓ tuning-profile-nextest: ok (206s)
  ✓ tuning-algebra-calibration-build: ok (53s)
  ✓ tuning-algebra-calibration-nextest: ok (186s)
  ✓ tuning-campaign-support-build: ok (26s)
  ✓ tuning-campaign-support-nextest: ok (1s)
  ✓ tuning-campaign-validator: ok (0s)
  ✓ tuning-campaign-launcher-syntax: ok (0s)
  ✓ tuning-lifecycle-cargo: ok (13s)
  ✓ clippy: ok (26s)
  ✓ fmt: ok (2s)
  ✓ baked-core: ok (168s)
  ✓ tuning-core-artifact: ok (4s)
  ✓ tuning-algebra-artifacts: ok (7s)
  ✓ tuning-coding-codec: ok (13s)
  ✓ tuning-composer: ok (0s)

exit=1
```

### Initial CI detailed failures

```text
$ Extracted unchanged failure blocks and Summary from the raw nextest output
        FAIL [   0.019s] (5265/5952) gf2-sim gpu::imp::tests::test_dispatcher_acquires_stream_and_allocates
  stdout ───

    running 1 test
    test gpu::imp::tests::test_dispatcher_acquires_stream_and_allocates ... FAILED

    failures:

    failures:
        gpu::imp::tests::test_dispatcher_acquires_stream_and_allocates

    test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 354 filtered out; finished in 0.00s

  stderr ───

    thread 'gpu::imp::tests::test_dispatcher_acquires_stream_and_allocates' (35224) panicked at crates/gf2-sim/src/gpu/mod.rs:418:53:
    build dispatcher on gfx1030: Fatal(KernelLaunch { hip_code: 100, kernel: "GfxTarget::detect_device", args: "hip context: hipGetDeviceCount" })
    note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

        FAIL [   0.117s] (5944/5952) tuning-campaign-support::bin/tuning-extent-campaign-driver tests::kill_grace_handles_a_child_ignoring_term
  stdout ───

    running 1 test
    test tests::kill_grace_handles_a_child_ignoring_term ... FAILED

    failures:

    failures:
        tests::kill_grace_handles_a_child_ignoring_term

    test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 21 filtered out; finished in 0.11s

  stderr ───

    thread 'tests::kill_grace_handles_a_child_ignoring_term' (38858) panicked at dev/tools/tuning-campaign-support/src/bin/tuning-extent-campaign-driver.rs:2507:10:
    called `Result::unwrap()` on an `Err` value: Os { code: 3, kind: Uncategorized, message: "No such process" }
    note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

     Summary [  14.130s] 5952 tests run: 5950 passed, 2 failed, 248 skipped
```

### Final CI detailed failures

```text
$ Extracted unchanged failure blocks and Summary from the final raw nextest output
        FAIL [   0.031s] (5264/5952) gf2-sim gpu::imp::tests::test_dispatcher_acquires_stream_and_allocates
  stdout ───

    running 1 test
    test gpu::imp::tests::test_dispatcher_acquires_stream_and_allocates ... FAILED

    failures:

    failures:
        gpu::imp::tests::test_dispatcher_acquires_stream_and_allocates

    test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 354 filtered out; finished in 0.00s

  stderr ───

    thread 'gpu::imp::tests::test_dispatcher_acquires_stream_and_allocates' (37495) panicked at crates/gf2-sim/src/gpu/mod.rs:418:53:
    build dispatcher on gfx1030: Fatal(KernelLaunch { hip_code: 100, kernel: "GfxTarget::detect_device", args: "hip context: hipGetDeviceCount" })
    note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

     TIMEOUT [   8.124s] (5952/5952) gf2-sim::permanent_rare_event_artifacts rare_event_artifact_partial_publish_recovery
  stdout ───

    running 1 test

    (test timed out)

     Summary [  18.528s] 5952 tests run: 5950 passed, 1 failed, 1 timed out, 248 skipped
```

### Focused investigation of the unrelated timeout

```text
$ ./scripts/cargo-budget.sh --test cargo nextest run -p gf2-sim --all-features --cargo-profile ci-test --profile ci --test permanent_rare_event_artifacts -E 'test(rare_event_artifact_partial_publish_recovery)'
cargo-budget: 8 jobs (3 live, 24 cpus)
cargo-budget: another test run holds the lock; waiting up to 600s
    Blocking waiting for file lock on package cache
    Blocking waiting for file lock on package cache
    Blocking waiting for file lock on package cache
   Compiling zerocopy v0.8.31
   Compiling serde v1.0.228
   Compiling getrandom v0.2.16
   Compiling syn v2.0.111
   Compiling num-traits v0.2.19
   Compiling getrandom v0.3.4
   Compiling tracing-core v0.1.36
   Compiling either v1.15.0
   Compiling rand_core v0.6.4
   Compiling rayon v1.11.0
   Compiling rand_core v0.9.3
   Compiling rustix v1.1.2
   Compiling gf2-kernels-hip v0.1.0 (/home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-f547c394/crates/gf2-kernels-hip)
   Compiling linux-raw-sys v0.11.0
   Compiling regex-automata v0.4.13
   Compiling num-integer v0.1.47
   Compiling tracing-log v0.2.0
   Compiling num-bigint v0.4.8
   Compiling nix v0.31.3
   Compiling ppv-lite86 v0.2.21
   Compiling rand_chacha v0.3.1
   Compiling rand v0.8.5
   Compiling rand_chacha v0.9.0
   Compiling matchers v0.2.0
   Compiling thread_local v1.1.9
   Compiling rand_distr v0.4.3
   Compiling rand v0.9.2
   Compiling ctrlc v3.5.2
   Compiling tempfile v3.23.0
   Compiling toml_datetime v1.1.1+spec-1.1.0
   Compiling serde_spanned v1.1.1
   Compiling rusty-fork v0.3.1
   Compiling serde_derive v1.0.228
   Compiling tracing-attributes v0.1.31
   Compiling toml v1.1.2+spec-1.1.0
   Compiling rand_xorshift v0.4.0
   Compiling proptest v1.9.0
   Compiling tracing v0.1.44
   Compiling gf2-core v0.1.0 (/home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-f547c394/crates/gf2-core)
   Compiling serde_spanned v0.6.9
   Compiling toml_datetime v0.6.11
   Compiling tracing-serde v0.2.0
   Compiling trybuild v1.0.116
   Compiling tracing-subscriber v0.3.23
   Compiling toml_edit v0.22.27
   Compiling toml v0.8.23
warning: function `choose_k_block` is never used
   --> crates/gf2-core/src/alg/m4rm.rs:532:4
    |
532 | fn choose_k_block(m4rm: &M4rmSelectors, k: usize, n: usize) -> usize {
    |    ^^^^^^^^^^^^^^
    |
    = note: `#[warn(dead_code)]` (part of `#[warn(unused)]`) on by default

   Compiling gf2-stats v0.1.0 (/home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-f547c394/crates/gf2-stats)
warning: `gf2-core` (lib) generated 1 warning
warning: gf2-kernels-hip@0.1.0: skip gfx940: hipcc --offload-arch=gfx940 on /home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-f547c394/crates/gf2-kernels-hip/kernels/gfx940/probe.cpp exited with exit status: 1
   Compiling gf2-algebra v0.1.0 (/home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-f547c394/crates/gf2-algebra)
   Compiling gf2-coding v0.1.0 (/home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-f547c394/crates/gf2-coding)
   Compiling gf2-sim v0.1.0 (/home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-f547c394/crates/gf2-sim)
    Finished `ci-test` profile [optimized] target(s) in 38.06s
────────────
 Nextest run ID 83bfec22-4966-49d3-8359-0cafbf0df79c with nextest profile: ci
    Starting 1 test across 1 binary (15 tests skipped)
        PASS [   2.358s] (1/1) gf2-sim::permanent_rare_event_artifacts rare_event_artifact_partial_publish_recovery
────────────
     Summary [   2.360s] 1 test run: 1 passed, 15 skipped
```

## Workspace-wide sweeps

Active protocol/design/clarification and crate-rustdoc stale-claim sweep returns
no matches. The per-finding sweep includes immutable historical snapshot prose
and producing sources inside the committed receipts. Those matches are retained
as historical evidence under the explicit prohibition on editing frozen bytes;
the active protocol and clarification describe complete P-23 binding. Review
records remain history. No live addendum-only acceptance claim remains. Sweeps precede creation of this
report so its archived raw outputs do not recursively appear in their own results.

### Cumulative prior findings

```text
No matching gate runs for issue f547c394
```

### Deferred items over every linked Markdown/JSON document

```text
$ grep -inE deferred-pattern dev/active/1a379447-zen3-cpu-performance/measurement-contract.md
120:limit, preserved experiment or tracked falsifiable follow-up within the search
exit=0
$ grep -inE deferred-pattern dev/active/f547c394/protocol.md
exit=1
$ grep -inE deferred-pattern dev/active/f547c394/addendum.schema.json
exit=1
$ grep -inE deferred-pattern dev/active/f547c394/addendum-protocol-smoke.json
exit=1
$ grep -inE deferred-pattern dev/active/f547c394/design.md
exit=1
$ grep -inE deferred-pattern dev/active/f547c394/provenance-clarification.md
exit=1
$ grep -inE deferred-pattern dev/active/f547c394/addendum-protocol-smoke-pilot.json
exit=1
$ grep -inE deferred-pattern dev/active/f547c394/producing-inputs.json
exit=1
$ grep -inE deferred-pattern dev/bench_results/f547c394/2026-09-07-f547c394-protocol-pilot/receipt.json
exit=1
$ grep -inE deferred-pattern dev/bench_results/f547c394/2026-09-07-f547c394-protocol-pilot/plan.json
exit=1
$ grep -inE deferred-pattern dev/bench_results/f547c394/2026-09-07-f547c394-protocol-pilot/acceptance-summary.json
exit=1
$ grep -inE deferred-pattern dev/bench_results/f547c394/2026-09-07-f547c394-protocol-pilot/acceptance-summary.md
exit=1
$ grep -inE deferred-pattern dev/bench_results/f547c394/2026-09-07-f547c394-protocol-confirmation/receipt.json
exit=1
$ grep -inE deferred-pattern dev/bench_results/f547c394/2026-09-07-f547c394-protocol-confirmation/plan.json
exit=1
$ grep -inE deferred-pattern dev/bench_results/f547c394/2026-09-07-f547c394-protocol-confirmation/acceptance-summary.json
exit=1
$ grep -inE deferred-pattern dev/bench_results/f547c394/2026-09-07-f547c394-protocol-confirmation/acceptance-summary.md
exit=1
$ grep -inE deferred-pattern dev/active/1a379447-zen3-cpu-performance/reviews/f547c394-r1.md
23:### Deferred-items audit (Tier 2.75)
24:design.md "Pre-existing fragility observed" (a83583e0 composer lockfile) — legitimately out of scope, recorded in surfaced_pitfalls. design.md "Named exception: session lifecycle store" — tracked exception with a convergence condition; OK.
exit=0
$ grep -inE deferred-pattern dev/active/1a379447-zen3-cpu-performance/reviews/f547c394-r2.md
30:Formal criterion and holistic acceptance stop at the failed gate. The mandatory prior-findings and deferred-items audits are complete. R2 F1 violates the complete frozen-content obligation in REQ-01/REQ-04.
36:### Deferred-items audit (Tier 2.75)
38:The linked protocol/design/schema/addenda/clarification and measurement contract contain no in-scope deferred deliverable. `measurement-contract.md:120` requires future family work to track falsifiable residual gaps; the issue explicitly excludes family surveys and production optimization. `design.md:70-86` identifies pre-existing a835 Git-policy migration (`0ba493e1`) and its untracked composer lockfile fragility (`a83583e0`); these belong to the separate campaign contract, not this protocol task. `design.md:21-33` preserves the named session lifecycle-store exception and explicit convergence condition. The historical review records superseded findings rather than current unresolved implementation.
exit=0
```

### rg -n "campaign_start|CampaignFacts|frozen" dev/tools/tuning-campaign-support/src dev/tools/tuning-campaign-support/tests

```text
dev/tools/tuning-campaign-support/tests/protocol_contracts.rs:3://! fixtures, the protocol document's frozen settings, and the shared runner.
dev/tools/tuning-campaign-support/tests/protocol_contracts.rs:31:    evaluate, render_markdown, ArmQuality, ArmRecord, BenchmarkReceipt, CampaignFacts, CellClaim,
dev/tools/tuning-campaign-support/tests/protocol_contracts.rs:140:        frozen: Frozen {
dev/tools/tuning-campaign-support/tests/protocol_contracts.rs:141:            frozen_utc: Some("2026-09-06T00:00:00Z".into()),
dev/tools/tuning-campaign-support/tests/protocol_contracts.rs:486:    let facts = CampaignFacts {
dev/tools/tuning-campaign-support/tests/protocol_contracts.rs:692:fn addendum_schema_accepts_the_frozen_smoke_addendum_and_rejects_unknown_fields() {
dev/tools/tuning-campaign-support/tests/protocol_contracts.rs:732:    family.frozen = Frozen { frozen_utc: None };
dev/tools/tuning-campaign-support/tests/protocol_contracts.rs:1153:fn acceptance_rejects_every_self_consistent_frozen_fact_replacement() {
dev/tools/tuning-campaign-support/tests/protocol_contracts.rs:1335:fn acceptance_rejects_an_addendum_not_frozen_before_measurement() {
dev/tools/tuning-campaign-support/tests/protocol_contracts.rs:1343:        "unfrozen-addendum",
dev/tools/tuning-campaign-support/tests/protocol_contracts.rs:1496:    // alternate reason in the receipt must not replace that frozen result.
dev/tools/tuning-campaign-support/tests/protocol_contracts.rs:1708:fn protocol_document_pins_the_frozen_shared_settings() {
dev/tools/tuning-campaign-support/src/protocol.rs:1://! Zen 3 benchmark protocol identity, frozen shared settings, and the
dev/tools/tuning-campaign-support/src/protocol.rs:41:/// Shared numeric settings frozen by protocol version 1.
dev/tools/tuning-campaign-support/src/protocol.rs:73:/// The frozen version-1 settings.
dev/tools/tuning-campaign-support/src/protocol.rs:144:    /// campaign's frozen inputs.
dev/tools/tuning-campaign-support/src/protocol.rs:286:/// Human-readable time recorded when the addendum settings were frozen.
dev/tools/tuning-campaign-support/src/protocol.rs:290:    pub frozen_utc: Option<String>,
dev/tools/tuning-campaign-support/src/protocol.rs:587:/// One frozen cell.
dev/tools/tuning-campaign-support/src/protocol.rs:605:/// One family's independently frozen addendum.
dev/tools/tuning-campaign-support/src/protocol.rs:612:    pub frozen: Frozen,
dev/tools/tuning-campaign-support/src/protocol.rs:653:                "family alpha {} differs from the frozen shared alpha {}",
dev/tools/tuning-campaign-support/src/protocol.rs:740:                "max_confirmatory_attempts_per_candidate must equal the frozen {}",
dev/tools/tuning-campaign-support/src/protocol.rs:927:            "cell {cell_id} quality confidence differs from the frozen {}",
dev/tools/tuning-campaign-support/src/protocol.rs:984:    /// Pairs for an exploratory cell; confirmatory cells use the frozen count.
dev/tools/tuning-campaign-support/src/protocol.rs:990:    /// Exploratory cells without an explicit count use the frozen pilot minimum;
dev/tools/tuning-campaign-support/src/protocol.rs:1000:/// The runner's input: a bounded campaign over frozen addendum cells.
dev/tools/tuning-campaign-support/src/protocol.rs:1125:    /// frozen shared settings.
dev/tools/tuning-campaign-support/src/abtest.rs:14:/// Two-sided 95% normal quantile used by the frozen Wilson quality interval.
dev/tools/tuning-campaign-support/src/abtest.rs:268:/// Applies the frozen decision rule to an interval.
dev/tools/tuning-campaign-support/src/abtest.rs:336:    fn nearest_rank_percentiles_use_the_frozen_ranks() {
dev/tools/tuning-campaign-support/src/bin/benchmark-ab-runner.rs:38:    ArmQuality, ArmRecord, BenchmarkReceipt, CampaignFacts, CellClaim, CellRecord, CellStatus,
dev/tools/tuning-campaign-support/src/bin/benchmark-ab-runner.rs:177:) -> io::Result<CampaignFacts> {
dev/tools/tuning-campaign-support/src/bin/benchmark-ab-runner.rs:217:    Ok(CampaignFacts {
dev/tools/tuning-campaign-support/src/bin/benchmark-ab-runner.rs:266:    facts: CampaignFacts,
dev/tools/tuning-campaign-support/src/bin/benchmark-ab-runner.rs:308:        let first: CampaignFacts = serde_json::from_value(records[0].details.clone())
dev/tools/tuning-campaign-support/src/bin/benchmark-ab-runner.rs:725:    let facts: CampaignFacts = serde_json::from_value(
dev/tools/tuning-campaign-support/src/receipt.rs:59:/// Complete semantic inputs frozen in the opening campaign journal record.
dev/tools/tuning-campaign-support/src/receipt.rs:67:pub struct CampaignFacts {
dev/tools/tuning-campaign-support/src/receipt.rs:319:impl CampaignFacts {
dev/tools/tuning-campaign-support/src/receipt.rs:418:            let Some(frozen) = self.arms.get(name) else {
dev/tools/tuning-campaign-support/src/receipt.rs:424:            let profile_matches = match (&planned.tuning_profile, &frozen.tuning_profile) {
dev/tools/tuning-campaign-support/src/receipt.rs:426:                (Some(planned), Some(frozen)) => {
dev/tools/tuning-campaign-support/src/receipt.rs:427:                    planned.id == frozen.id && planned.sha256 == frozen.sha256
dev/tools/tuning-campaign-support/src/receipt.rs:431:            if planned.build != frozen.build
dev/tools/tuning-campaign-support/src/receipt.rs:432:                || planned.description != frozen.description
dev/tools/tuning-campaign-support/src/receipt.rs:433:                || planned.executable != frozen.executable_path
dev/tools/tuning-campaign-support/src/receipt.rs:434:                || planned.arguments != frozen.arguments
dev/tools/tuning-campaign-support/src/receipt.rs:435:                || planned.environment != frozen.environment
dev/tools/tuning-campaign-support/src/receipt.rs:436:                || planned.rustflags != frozen.rustflags
dev/tools/tuning-campaign-support/src/receipt.rs:568:    /// Flagged-window fraction above the frozen bound; re-run required.
dev/tools/tuning-campaign-support/src/receipt.rs:720:            "receipt settings differ from the frozen shared settings without declaring a deviation",
dev/tools/tuning-campaign-support/src/receipt.rs:867:                            match serde_json::from_value::<CampaignFacts>(record.details.clone()) {
dev/tools/tuning-campaign-support/src/provenance.rs:123:    /// the frozen snapshot is reopened without consulting `source_root`.
dev/tools/tuning-campaign-support/src/journal.rs:150:    campaign_started: bool,
dev/tools/tuning-campaign-support/src/journal.rs:229:            campaign_started: false,
dev/tools/tuning-campaign-support/src/journal.rs:300:            campaign_started: true,
dev/tools/tuning-campaign-support/src/journal.rs:389:            campaign_started: true,
dev/tools/tuning-campaign-support/src/journal.rs:468:        if !self.campaign_started && event != JournalEvent::CampaignStart {
dev/tools/tuning-campaign-support/src/journal.rs:471:        if self.campaign_started && event == JournalEvent::CampaignStart {
dev/tools/tuning-campaign-support/src/journal.rs:474:        self.campaign_started = true;
dev/tools/tuning-campaign-support/src/journal.rs:483:        if !self.campaign_started {
dev/tools/tuning-campaign-support/src/journal.rs:1083:        campaign_started: true,
```

### rg -n "addendum pin|only the addendum|campaign-start" dev/active/f547c394 dev/bench_results/f547c394

```text
dev/active/f547c394/protocol.md:78:The opening `campaign-start` record freezes `receipt::CampaignFacts` before
dev/active/f547c394/protocol.md:82:addendum pins, producing-input closure, toolchain, numerical settings and
dev/active/f547c394/protocol.md:87:The addendum pin names immutable receipt-local bytes. Resolution evidence names a
dev/active/f547c394/protocol.md:299:first `cell-start` (P-10). The campaign-start record binds the exact plan,
dev/active/f547c394/protocol.md:367:| P-10 | The execution log matches its digest, replays under the journal rules, opens with `campaign-start`, and was announced before the first cell. |
dev/active/f547c394/addendum.schema.json:34:      "description": "Human-readable freeze time. The receipt-local addendum pin is one field of receipt::CampaignFacts in the premeasurement campaign-start record. The shared typed comparison binds all receipt and plan provenance, including these exact addendum bytes, before measurement (protocol P-23).",
dev/bench_results/f547c394/2026-09-07-f547c394-protocol-confirmation/inputs/protocol.md:78:The receipt's addendum pin and first `campaign-start` record are the freeze
dev/bench_results/f547c394/2026-09-07-f547c394-protocol-confirmation/inputs/protocol.md:293:first `cell-start` (P-10). The campaign-start record binds the exact plan,
dev/bench_results/f547c394/2026-09-07-f547c394-protocol-confirmation/inputs/protocol.md:344:| P-03 | The addendum matches its receipt-local snapshot, its exact pin appears in campaign-start before any cell, it validates against the schema and semantic rules, and resolution evidence is a distinct digest-matched pilot receipt snapshot. |
dev/bench_results/f547c394/2026-09-07-f547c394-protocol-confirmation/inputs/protocol.md:351:| P-10 | The execution log matches its digest, replays under the journal rules, opens with `campaign-start`, and was announced before the first cell. |
dev/bench_results/f547c394/2026-09-07-f547c394-protocol-confirmation/execution.log:1:{"schema":"tuning-campaign-journal-v1","timestamp_utc":"2026-09-06T23:09:20.074753736Z","campaign_id":"confirmation-f547c394-20260906t230920z","session_id":"session-1788736160-4139265","sequence":0,"event":"campaign-start","details":{"addendum":{"path":"dev/active/f547c394/addendum-protocol-smoke.json","sha256":"307b6a2baf2da5025a26820ed44be55a212747440f3d9d00e09bdaf8b1d8a69c","snapshot":"inputs/family-addendum.json"},"addendum_schema":{"path":"dev/active/f547c394/addendum.schema.json","sha256":"5eab7106aaae753555a21177e93be8368bdb463eb638b06d288c4471a3e8d240","snapshot":"inputs/addendum.schema.json"},"arms":{"baseline":{"arguments":[],"build":"conservative-portable","description":"xor-fold, one pass per call","environment":{"GF2_SMOKE_PASSES":"1"},"executable_path":"/home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-f547c394/target/release/ab-smoke-workload","executable_sha256":"b9444479f89c3bb572f68609f364720fad8bd8dc4dbfa3f7d2d873afd895f1df","rustflags":null,"tuning_profile":null},"baseline-two-pass":{"arguments":[],"build":"conservative-portable","description":"xor-fold, two passes per call","environment":{"GF2_SMOKE_PASSES":"2"},"executable_path":"/home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-f547c394/target/release/ab-smoke-workload","executable_sha256":"b9444479f89c3bb572f68609f364720fad8bd8dc4dbfa3f7d2d873afd895f1df","rustflags":null,"tuning_profile":null},"candidate":{"arguments":[],"build":"conservative-portable","description":"xor-fold, one pass per call","environment":{"GF2_SMOKE_PASSES":"1"},"executable_path":"/home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-f547c394/target/release/ab-smoke-workload","executable_sha256":"b9444479f89c3bb572f68609f364720fad8bd8dc4dbfa3f7d2d873afd895f1df","rustflags":null,"tuning_profile":null}},"contract":{"path":"dev/active/1a379447-zen3-cpu-performance/measurement-contract.md","sha256":"9f3c7563580b7de0d3240f00a48f4c711f78cf7324e4c869d45166aa2cdcebb3","snapshot":"inputs/measurement-contract.md"},"identity":{"behavior_sha256":{"dev/bench_results/f547c394/run-smoke.sh":"44be6611988a99be006ed043030b8ba6e067ac427c7829806c2c36a497f414db","dev/scripts/ccx1-bench-flock.sh":"c209cd8a1d518884e87485b4d4fcd750f2bed615609e4535ae80e82fea66c39f","dev/tools/tuning-campaign-support/src/abtest.rs":"5a3449ea1ca0a876ddb460eb3536bdeb2ce4932fd66b0eff0f1e92057a739c81","dev/tools/tuning-campaign-support/src/bin/ab-smoke-workload.rs":"48d5d83d4ecdb5efee810a20fe5eb5c309062c72d23c050e78553353c8603a51","dev/tools/tuning-campaign-support/src/bin/benchmark-ab-runner.rs":"e02ebf7eab54e6fee16989c295a6d80a38fe5542867af8523a189c255295e160","dev/tools/tuning-campaign-support/src/bin/benchmark-acceptance.rs":"6156da414049261b3348bebfa02e97af85570050b4d12b3fc95fa8c372254fe3","dev/tools/tuning-campaign-support/src/campaign.rs":"6c3088e8f197cf5e271332bb439687cbeb654879874c1e2642280553df2e4a36","dev/tools/tuning-campaign-support/src/host.rs":"3f989e5461db417eb705e34c41c803b4e8eb06da6c38089a4553c06865045507","dev/tools/tuning-campaign-support/src/journal.rs":"9edd52e94bc084560ee951703caccad98341cae46d657150bd76e134156f7b85","dev/tools/tuning-campaign-support/src/lib.rs":"9b1790f86e67ed3e442963c29912c6829b8c411d077c07e7bc95aa08531416d8","dev/tools/tuning-campaign-support/src/process.rs":"be4d88682ab62e8a70333892e1ebdcb9eb44f643194c21fd594549b062d1be05","dev/tools/tuning-campaign-support/src/protocol.rs":"b3b45cd56d1a22c5897907677dd1974b81dca3740c0da6c0f8c6184eee6d8feb","dev/tools/tuning-campaign-support/src/provenance.rs":"b559664e0c7173bc6599b43a4543a617bba853e1f2385abb7da015a742cc77ed","dev/tools/tuning-campaign-support/src/receipt.rs":"4eae42ddc8d81cddc8ed4c9b6416b99dab6e9b5d94a017d36743f7aa428130e8","dev/tools/tuning-campaign-support/src/schema.rs":"d7f4001656c265abb7fb960cdba5eacf3739d0c76ab112d6a082e9eab12bec5d","dev/tools/tuning-campaign-support/src/seed.rs":"7be49c66c0917b44e748a90e668ad16ad5fef9e8a5e76efe173bb1b602539036","dev/tools/tuning-campaign-support/src/statistics.rs":"6c574b5ecca3aae79d760325117ccee816f10980b50a559234c2fd3b00347fa7","dev/tools/tuning-campaign-support/src/timing.rs":"9d15dfe5e8400def4dca53713bc0ff89926ae43f0f2a87cbf4f0e40f55de60c8","dev/tools/tuning-campaign-support/src/transport.rs":"883f903e4c0b5d3f651347f4695a4139504856aadb8afcb77a5942435cc8995a"},"executable_sha256":{"baseline":"b9444479f89c3bb572f68609f364720fad8bd8dc4dbfa3f7d2d873afd895f1df","baseline-two-pass":"b9444479f89c3bb572f68609f364720fad8bd8dc4dbfa3f7d2d873afd895f1df","candidate":"b9444479f89c3bb572f68609f364720fad8bd8dc4dbfa3f7d2d873afd895f1df"},"feature_contract":"release","host_identity":"fraktaali;cpus=[6,7,8,9,10,11]","lifecycle_behavior_sha256":"10c80d394eefcebf18aaeb6f400f1b43cafc33dfa233055e253ba36ae697bb6f","lifecycle_schema":"zen3-benchmark-runner-session-v1","ordered_work_manifest_sha256":"9d36a4786ac730b2600008090ba5f969f2ee9fc3f663d037a73be56a1768e7b1","process_descriptors_sha256":"6dc064293cd0d1ecf896146009a7d5bddd00cc7135ceeaf61ca7d5cb5c66bfa1","protocol_digest":"a7a1db9840d51c5860e1dbd57d741da345fcf2fb9e6f112e2085855aa2372e48","source_revision":"","source_sha256":"11330c8b0adecbffee0610f4c42e28244690d34f6d49244b9a5c925f495e3ec9","thread_contract":"RAYON_NUM_THREADS=unset"},"plan_sha256":"5c2b1f5a2b8ff0384a5cd0d5baf00cd19659d53df3fc6606edcfe6f166eb899d","protocol":{"path":"dev/active/f547c394/protocol.md","sha256":"a7a1db9840d51c5860e1dbd57d741da345fcf2fb9e6f112e2085855aa2372e48","snapshot":"inputs/protocol.md"},"settings":{"bootstrap_resamples":10000,"child_timeout_seconds":120,"confirmatory_pairs":24,"family_alpha":0.05,"flagged_window_factor":2.0,"max_confirmatory_attempts_per_candidate":1,"max_flagged_fraction":0.1,"max_pilot_trials_per_cell":8,"pilot_max_pairs":24,"pilot_min_pairs":6,"quality_confidence":0.95,"window_target_ms":100,"windows_per_execution":5},"settings_deviation":false,"source":{"producing":{"behavior_sha256":{"dev/bench_results/f547c394/run-smoke.sh":"44be6611988a99be006ed043030b8ba6e067ac427c7829806c2c36a497f414db","dev/scripts/ccx1-bench-flock.sh":"c209cd8a1d518884e87485b4d4fcd750f2bed615609e4535ae80e82fea66c39f","dev/tools/tuning-campaign-support/src/abtest.rs":"5a3449ea1ca0a876ddb460eb3536bdeb2ce4932fd66b0eff0f1e92057a739c81","dev/tools/tuning-campaign-support/src/bin/ab-smoke-workload.rs":"48d5d83d4ecdb5efee810a20fe5eb5c309062c72d23c050e78553353c8603a51","dev/tools/tuning-campaign-support/src/bin/benchmark-ab-runner.rs":"e02ebf7eab54e6fee16989c295a6d80a38fe5542867af8523a189c255295e160","dev/tools/tuning-campaign-support/src/bin/benchmark-acceptance.rs":"6156da414049261b3348bebfa02e97af85570050b4d12b3fc95fa8c372254fe3","dev/tools/tuning-campaign-support/src/campaign.rs":"6c3088e8f197cf5e271332bb439687cbeb654879874c1e2642280553df2e4a36","dev/tools/tuning-campaign-support/src/host.rs":"3f989e5461db417eb705e34c41c803b4e8eb06da6c38089a4553c06865045507","dev/tools/tuning-campaign-support/src/journal.rs":"9edd52e94bc084560ee951703caccad98341cae46d657150bd76e134156f7b85","dev/tools/tuning-campaign-support/src/lib.rs":"9b1790f86e67ed3e442963c29912c6829b8c411d077c07e7bc95aa08531416d8","dev/tools/tuning-campaign-support/src/process.rs":"be4d88682ab62e8a70333892e1ebdcb9eb44f643194c21fd594549b062d1be05","dev/tools/tuning-campaign-support/src/protocol.rs":"b3b45cd56d1a22c5897907677dd1974b81dca3740c0da6c0f8c6184eee6d8feb","dev/tools/tuning-campaign-support/src/provenance.rs":"b559664e0c7173bc6599b43a4543a617bba853e1f2385abb7da015a742cc77ed","dev/tools/tuning-campaign-support/src/receipt.rs":"4eae42ddc8d81cddc8ed4c9b6416b99dab6e9b5d94a017d36743f7aa428130e8","dev/tools/tuning-campaign-support/src/schema.rs":"d7f4001656c265abb7fb960cdba5eacf3739d0c76ab112d6a082e9eab12bec5d","dev/tools/tuning-campaign-support/src/seed.rs":"7be49c66c0917b44e748a90e668ad16ad5fef9e8a5e76efe173bb1b602539036","dev/tools/tuning-campaign-support/src/statistics.rs":"6c574b5ecca3aae79d760325117ccee816f10980b50a559234c2fd3b00347fa7","dev/tools/tuning-campaign-support/src/timing.rs":"9d15dfe5e8400def4dca53713bc0ff89926ae43f0f2a87cbf4f0e40f55de60c8","dev/tools/tuning-campaign-support/src/transport.rs":"883f903e4c0b5d3f651347f4695a4139504856aadb8afcb77a5942435cc8995a"},"build_inputs_sha256":{".cargo/config.toml":"a41ec65f4e6322d990fb5d9afcb668029958f7e57bf7e2623211a638ec82dffb","Cargo.lock":"541e7dcc0407f6471e64540d6c2edc33286653c771d9fd940ae1b1065075e7bc","Cargo.toml":"5fa0e5e66bc896c1b009192b2e76222b9c0c2295b44b25a5ad6fd9a6564f6cf5","dev/bench_results/f547c394/run-smoke.sh":"44be6611988a99be006ed043030b8ba6e067ac427c7829806c2c36a497f414db","dev/scripts/ccx1-bench-flock.sh":"c209cd8a1d518884e87485b4d4fcd750f2bed615609e4535ae80e82fea66c39f","dev/tools/tuning-campaign-support/Cargo.toml":"4e2a5ddb0cfbf25a3d3068804c1d6bf21a3ca28eecebfbc22ed351750110b8e8","dev/tools/tuning-campaign-support/src/abtest.rs":"5a3449ea1ca0a876ddb460eb3536bdeb2ce4932fd66b0eff0f1e92057a739c81","dev/tools/tuning-campaign-support/src/bin/ab-smoke-workload.rs":"48d5d83d4ecdb5efee810a20fe5eb5c309062c72d23c050e78553353c8603a51","dev/tools/tuning-campaign-support/src/bin/benchmark-ab-runner.rs":"e02ebf7eab54e6fee16989c295a6d80a38fe5542867af8523a189c255295e160","dev/tools/tuning-campaign-support/src/bin/benchmark-acceptance.rs":"6156da414049261b3348bebfa02e97af85570050b4d12b3fc95fa8c372254fe3","dev/tools/tuning-campaign-support/src/campaign.rs":"6c3088e8f197cf5e271332bb439687cbeb654879874c1e2642280553df2e4a36","dev/tools/tuning-campaign-support/src/host.rs":"3f989e5461db417eb705e34c41c803b4e8eb06da6c38089a4553c06865045507","dev/tools/tuning-campaign-support/src/journal.rs":"9edd52e94bc084560ee951703caccad98341cae46d657150bd76e134156f7b85","dev/tools/tuning-campaign-support/src/lib.rs":"9b1790f86e67ed3e442963c29912c6829b8c411d077c07e7bc95aa08531416d8","dev/tools/tuning-campaign-support/src/process.rs":"be4d88682ab62e8a70333892e1ebdcb9eb44f643194c21fd594549b062d1be05","dev/tools/tuning-campaign-support/src/protocol.rs":"b3b45cd56d1a22c5897907677dd1974b81dca3740c0da6c0f8c6184eee6d8feb","dev/tools/tuning-campaign-support/src/provenance.rs":"b559664e0c7173bc6599b43a4543a617bba853e1f2385abb7da015a742cc77ed","dev/tools/tuning-campaign-support/src/receipt.rs":"4eae42ddc8d81cddc8ed4c9b6416b99dab6e9b5d94a017d36743f7aa428130e8","dev/tools/tuning-campaign-support/src/schema.rs":"d7f4001656c265abb7fb960cdba5eacf3739d0c76ab112d6a082e9eab12bec5d","dev/tools/tuning-campaign-support/src/seed.rs":"7be49c66c0917b44e748a90e668ad16ad5fef9e8a5e76efe173bb1b602539036","dev/tools/tuning-campaign-support/src/statistics.rs":"6c574b5ecca3aae79d760325117ccee816f10980b50a559234c2fd3b00347fa7","dev/tools/tuning-campaign-support/src/timing.rs":"9d15dfe5e8400def4dca53713bc0ff89926ae43f0f2a87cbf4f0e40f55de60c8","dev/tools/tuning-campaign-support/src/transport.rs":"883f903e4c0b5d3f651347f4695a4139504856aadb8afcb77a5942435cc8995a"},"lifecycle_sha256":{"dev/bench_results/f547c394/run-smoke.sh":"44be6611988a99be006ed043030b8ba6e067ac427c7829806c2c36a497f414db","dev/scripts/ccx1-bench-flock.sh":"c209cd8a1d518884e87485b4d4fcd750f2bed615609e4535ae80e82fea66c39f","dev/tools/tuning-campaign-support/src/bin/benchmark-ab-runner.rs":"e02ebf7eab54e6fee16989c295a6d80a38fe5542867af8523a189c255295e160","dev/tools/tuning-campaign-support/src/journal.rs":"9edd52e94bc084560ee951703caccad98341cae46d657150bd76e134156f7b85","dev/tools/tuning-campaign-support/src/process.rs":"be4d88682ab62e8a70333892e1ebdcb9eb44f643194c21fd594549b062d1be05","dev/tools/tuning-campaign-support/src/protocol.rs":"b3b45cd56d1a22c5897907677dd1974b81dca3740c0da6c0f8c6184eee6d8feb","dev/tools/tuning-campaign-support/src/provenance.rs":"b559664e0c7173bc6599b43a4543a617bba853e1f2385abb7da015a742cc77ed","dev/tools/tuning-campaign-support/src/receipt.rs":"4eae42ddc8d81cddc8ed4c9b6416b99dab6e9b5d94a017d36743f7aa428130e8"},"manifest_path":"dev/active/f547c394/producing-inputs.json","manifest_sha256":"1d046407be24951ea54840face6073f95bc257c67b83e6f97c86e0977ddd716b"}},"toolchain":"rustc 1.97.0 (2d8144b78 2026-07-07)"}}
dev/bench_results/f547c394/2026-09-07-f547c394-protocol-confirmation/inputs/addendum.schema.json:34:      "description": "Human-readable freeze time. The receipt-local addendum pin and premeasurement campaign-start record prove the exact bytes and ordering of the freeze.",
dev/bench_results/f547c394/2026-09-07-f547c394-protocol-pilot/inputs/protocol.md:78:The receipt's addendum pin and first `campaign-start` record are the freeze
dev/bench_results/f547c394/2026-09-07-f547c394-protocol-pilot/inputs/protocol.md:293:first `cell-start` (P-10). The campaign-start record binds the exact plan,
dev/bench_results/f547c394/2026-09-07-f547c394-protocol-pilot/inputs/protocol.md:344:| P-03 | The addendum matches its receipt-local snapshot, its exact pin appears in campaign-start before any cell, it validates against the schema and semantic rules, and resolution evidence is a distinct digest-matched pilot receipt snapshot. |
dev/bench_results/f547c394/2026-09-07-f547c394-protocol-pilot/inputs/protocol.md:351:| P-10 | The execution log matches its digest, replays under the journal rules, opens with `campaign-start`, and was announced before the first cell. |
dev/bench_results/f547c394/2026-09-07-f547c394-protocol-pilot/inputs/addendum.schema.json:34:      "description": "Human-readable freeze time. The receipt-local addendum pin and premeasurement campaign-start record prove the exact bytes and ordering of the freeze.",
dev/bench_results/f547c394/2026-09-07-f547c394-protocol-pilot/execution.log:1:{"schema":"tuning-campaign-journal-v1","timestamp_utc":"2026-09-06T23:07:25.634777062Z","campaign_id":"pilot-f547c394-20260906t230725z","session_id":"session-1788736045-4133534","sequence":0,"event":"campaign-start","details":{"addendum":{"path":"dev/active/f547c394/addendum-protocol-smoke-pilot.json","sha256":"6f762913988f8cb517f5e17c2832fff06d22359a7f1eb8c59d2f47a7ffe0feb4","snapshot":"inputs/family-addendum.json"},"addendum_schema":{"path":"dev/active/f547c394/addendum.schema.json","sha256":"5eab7106aaae753555a21177e93be8368bdb463eb638b06d288c4471a3e8d240","snapshot":"inputs/addendum.schema.json"},"arms":{"baseline":{"arguments":[],"build":"conservative-portable","description":"xor-fold, one pass per call","environment":{"GF2_SMOKE_PASSES":"1"},"executable_path":"/home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-f547c394/target/release/ab-smoke-workload","executable_sha256":"b9444479f89c3bb572f68609f364720fad8bd8dc4dbfa3f7d2d873afd895f1df","rustflags":null,"tuning_profile":null},"baseline-two-pass":{"arguments":[],"build":"conservative-portable","description":"xor-fold, two passes per call","environment":{"GF2_SMOKE_PASSES":"2"},"executable_path":"/home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-f547c394/target/release/ab-smoke-workload","executable_sha256":"b9444479f89c3bb572f68609f364720fad8bd8dc4dbfa3f7d2d873afd895f1df","rustflags":null,"tuning_profile":null},"candidate":{"arguments":[],"build":"conservative-portable","description":"xor-fold, one pass per call","environment":{"GF2_SMOKE_PASSES":"1"},"executable_path":"/home/vkaskivuo/Projects/gf2/.agents/worktrees/agent-f547c394/target/release/ab-smoke-workload","executable_sha256":"b9444479f89c3bb572f68609f364720fad8bd8dc4dbfa3f7d2d873afd895f1df","rustflags":null,"tuning_profile":null}},"contract":{"path":"dev/active/1a379447-zen3-cpu-performance/measurement-contract.md","sha256":"9f3c7563580b7de0d3240f00a48f4c711f78cf7324e4c869d45166aa2cdcebb3","snapshot":"inputs/measurement-contract.md"},"identity":{"behavior_sha256":{"dev/bench_results/f547c394/run-smoke.sh":"44be6611988a99be006ed043030b8ba6e067ac427c7829806c2c36a497f414db","dev/scripts/ccx1-bench-flock.sh":"c209cd8a1d518884e87485b4d4fcd750f2bed615609e4535ae80e82fea66c39f","dev/tools/tuning-campaign-support/src/abtest.rs":"5a3449ea1ca0a876ddb460eb3536bdeb2ce4932fd66b0eff0f1e92057a739c81","dev/tools/tuning-campaign-support/src/bin/ab-smoke-workload.rs":"48d5d83d4ecdb5efee810a20fe5eb5c309062c72d23c050e78553353c8603a51","dev/tools/tuning-campaign-support/src/bin/benchmark-ab-runner.rs":"e02ebf7eab54e6fee16989c295a6d80a38fe5542867af8523a189c255295e160","dev/tools/tuning-campaign-support/src/bin/benchmark-acceptance.rs":"6156da414049261b3348bebfa02e97af85570050b4d12b3fc95fa8c372254fe3","dev/tools/tuning-campaign-support/src/campaign.rs":"6c3088e8f197cf5e271332bb439687cbeb654879874c1e2642280553df2e4a36","dev/tools/tuning-campaign-support/src/host.rs":"3f989e5461db417eb705e34c41c803b4e8eb06da6c38089a4553c06865045507","dev/tools/tuning-campaign-support/src/journal.rs":"9edd52e94bc084560ee951703caccad98341cae46d657150bd76e134156f7b85","dev/tools/tuning-campaign-support/src/lib.rs":"9b1790f86e67ed3e442963c29912c6829b8c411d077c07e7bc95aa08531416d8","dev/tools/tuning-campaign-support/src/process.rs":"be4d88682ab62e8a70333892e1ebdcb9eb44f643194c21fd594549b062d1be05","dev/tools/tuning-campaign-support/src/protocol.rs":"b3b45cd56d1a22c5897907677dd1974b81dca3740c0da6c0f8c6184eee6d8feb","dev/tools/tuning-campaign-support/src/provenance.rs":"b559664e0c7173bc6599b43a4543a617bba853e1f2385abb7da015a742cc77ed","dev/tools/tuning-campaign-support/src/receipt.rs":"4eae42ddc8d81cddc8ed4c9b6416b99dab6e9b5d94a017d36743f7aa428130e8","dev/tools/tuning-campaign-support/src/schema.rs":"d7f4001656c265abb7fb960cdba5eacf3739d0c76ab112d6a082e9eab12bec5d","dev/tools/tuning-campaign-support/src/seed.rs":"7be49c66c0917b44e748a90e668ad16ad5fef9e8a5e76efe173bb1b602539036","dev/tools/tuning-campaign-support/src/statistics.rs":"6c574b5ecca3aae79d760325117ccee816f10980b50a559234c2fd3b00347fa7","dev/tools/tuning-campaign-support/src/timing.rs":"9d15dfe5e8400def4dca53713bc0ff89926ae43f0f2a87cbf4f0e40f55de60c8","dev/tools/tuning-campaign-support/src/transport.rs":"883f903e4c0b5d3f651347f4695a4139504856aadb8afcb77a5942435cc8995a"},"executable_sha256":{"baseline":"b9444479f89c3bb572f68609f364720fad8bd8dc4dbfa3f7d2d873afd895f1df","baseline-two-pass":"b9444479f89c3bb572f68609f364720fad8bd8dc4dbfa3f7d2d873afd895f1df","candidate":"b9444479f89c3bb572f68609f364720fad8bd8dc4dbfa3f7d2d873afd895f1df"},"feature_contract":"release","host_identity":"fraktaali;cpus=[6,7,8,9,10,11]","lifecycle_behavior_sha256":"10c80d394eefcebf18aaeb6f400f1b43cafc33dfa233055e253ba36ae697bb6f","lifecycle_schema":"zen3-benchmark-runner-session-v1","ordered_work_manifest_sha256":"e0c37cbbebb97d6fbe69f66cf64f285b8356d5960dd3111c63c1c206a35b4719","process_descriptors_sha256":"6dc064293cd0d1ecf896146009a7d5bddd00cc7135ceeaf61ca7d5cb5c66bfa1","protocol_digest":"a7a1db9840d51c5860e1dbd57d741da345fcf2fb9e6f112e2085855aa2372e48","source_revision":"","source_sha256":"11330c8b0adecbffee0610f4c42e28244690d34f6d49244b9a5c925f495e3ec9","thread_contract":"RAYON_NUM_THREADS=unset"},"plan_sha256":"53a0f2d66709dc7764735ed743acf036977d0275c14d0e40da800e035bc130d9","protocol":{"path":"dev/active/f547c394/protocol.md","sha256":"a7a1db9840d51c5860e1dbd57d741da345fcf2fb9e6f112e2085855aa2372e48","snapshot":"inputs/protocol.md"},"settings":{"bootstrap_resamples":10000,"child_timeout_seconds":120,"confirmatory_pairs":24,"family_alpha":0.05,"flagged_window_factor":2.0,"max_confirmatory_attempts_per_candidate":1,"max_flagged_fraction":0.1,"max_pilot_trials_per_cell":8,"pilot_max_pairs":24,"pilot_min_pairs":6,"quality_confidence":0.95,"window_target_ms":100,"windows_per_execution":5},"settings_deviation":false,"source":{"producing":{"behavior_sha256":{"dev/bench_results/f547c394/run-smoke.sh":"44be6611988a99be006ed043030b8ba6e067ac427c7829806c2c36a497f414db","dev/scripts/ccx1-bench-flock.sh":"c209cd8a1d518884e87485b4d4fcd750f2bed615609e4535ae80e82fea66c39f","dev/tools/tuning-campaign-support/src/abtest.rs":"5a3449ea1ca0a876ddb460eb3536bdeb2ce4932fd66b0eff0f1e92057a739c81","dev/tools/tuning-campaign-support/src/bin/ab-smoke-workload.rs":"48d5d83d4ecdb5efee810a20fe5eb5c309062c72d23c050e78553353c8603a51","dev/tools/tuning-campaign-support/src/bin/benchmark-ab-runner.rs":"e02ebf7eab54e6fee16989c295a6d80a38fe5542867af8523a189c255295e160","dev/tools/tuning-campaign-support/src/bin/benchmark-acceptance.rs":"6156da414049261b3348bebfa02e97af85570050b4d12b3fc95fa8c372254fe3","dev/tools/tuning-campaign-support/src/campaign.rs":"6c3088e8f197cf5e271332bb439687cbeb654879874c1e2642280553df2e4a36","dev/tools/tuning-campaign-support/src/host.rs":"3f989e5461db417eb705e34c41c803b4e8eb06da6c38089a4553c06865045507","dev/tools/tuning-campaign-support/src/journal.rs":"9edd52e94bc084560ee951703caccad98341cae46d657150bd76e134156f7b85","dev/tools/tuning-campaign-support/src/lib.rs":"9b1790f86e67ed3e442963c29912c6829b8c411d077c07e7bc95aa08531416d8","dev/tools/tuning-campaign-support/src/process.rs":"be4d88682ab62e8a70333892e1ebdcb9eb44f643194c21fd594549b062d1be05","dev/tools/tuning-campaign-support/src/protocol.rs":"b3b45cd56d1a22c5897907677dd1974b81dca3740c0da6c0f8c6184eee6d8feb","dev/tools/tuning-campaign-support/src/provenance.rs":"b559664e0c7173bc6599b43a4543a617bba853e1f2385abb7da015a742cc77ed","dev/tools/tuning-campaign-support/src/receipt.rs":"4eae42ddc8d81cddc8ed4c9b6416b99dab6e9b5d94a017d36743f7aa428130e8","dev/tools/tuning-campaign-support/src/schema.rs":"d7f4001656c265abb7fb960cdba5eacf3739d0c76ab112d6a082e9eab12bec5d","dev/tools/tuning-campaign-support/src/seed.rs":"7be49c66c0917b44e748a90e668ad16ad5fef9e8a5e76efe173bb1b602539036","dev/tools/tuning-campaign-support/src/statistics.rs":"6c574b5ecca3aae79d760325117ccee816f10980b50a559234c2fd3b00347fa7","dev/tools/tuning-campaign-support/src/timing.rs":"9d15dfe5e8400def4dca53713bc0ff89926ae43f0f2a87cbf4f0e40f55de60c8","dev/tools/tuning-campaign-support/src/transport.rs":"883f903e4c0b5d3f651347f4695a4139504856aadb8afcb77a5942435cc8995a"},"build_inputs_sha256":{".cargo/config.toml":"a41ec65f4e6322d990fb5d9afcb668029958f7e57bf7e2623211a638ec82dffb","Cargo.lock":"541e7dcc0407f6471e64540d6c2edc33286653c771d9fd940ae1b1065075e7bc","Cargo.toml":"5fa0e5e66bc896c1b009192b2e76222b9c0c2295b44b25a5ad6fd9a6564f6cf5","dev/bench_results/f547c394/run-smoke.sh":"44be6611988a99be006ed043030b8ba6e067ac427c7829806c2c36a497f414db","dev/scripts/ccx1-bench-flock.sh":"c209cd8a1d518884e87485b4d4fcd750f2bed615609e4535ae80e82fea66c39f","dev/tools/tuning-campaign-support/Cargo.toml":"4e2a5ddb0cfbf25a3d3068804c1d6bf21a3ca28eecebfbc22ed351750110b8e8","dev/tools/tuning-campaign-support/src/abtest.rs":"5a3449ea1ca0a876ddb460eb3536bdeb2ce4932fd66b0eff0f1e92057a739c81","dev/tools/tuning-campaign-support/src/bin/ab-smoke-workload.rs":"48d5d83d4ecdb5efee810a20fe5eb5c309062c72d23c050e78553353c8603a51","dev/tools/tuning-campaign-support/src/bin/benchmark-ab-runner.rs":"e02ebf7eab54e6fee16989c295a6d80a38fe5542867af8523a189c255295e160","dev/tools/tuning-campaign-support/src/bin/benchmark-acceptance.rs":"6156da414049261b3348bebfa02e97af85570050b4d12b3fc95fa8c372254fe3","dev/tools/tuning-campaign-support/src/campaign.rs":"6c3088e8f197cf5e271332bb439687cbeb654879874c1e2642280553df2e4a36","dev/tools/tuning-campaign-support/src/host.rs":"3f989e5461db417eb705e34c41c803b4e8eb06da6c38089a4553c06865045507","dev/tools/tuning-campaign-support/src/journal.rs":"9edd52e94bc084560ee951703caccad98341cae46d657150bd76e134156f7b85","dev/tools/tuning-campaign-support/src/lib.rs":"9b1790f86e67ed3e442963c29912c6829b8c411d077c07e7bc95aa08531416d8","dev/tools/tuning-campaign-support/src/process.rs":"be4d88682ab62e8a70333892e1ebdcb9eb44f643194c21fd594549b062d1be05","dev/tools/tuning-campaign-support/src/protocol.rs":"b3b45cd56d1a22c5897907677dd1974b81dca3740c0da6c0f8c6184eee6d8feb","dev/tools/tuning-campaign-support/src/provenance.rs":"b559664e0c7173bc6599b43a4543a617bba853e1f2385abb7da015a742cc77ed","dev/tools/tuning-campaign-support/src/receipt.rs":"4eae42ddc8d81cddc8ed4c9b6416b99dab6e9b5d94a017d36743f7aa428130e8","dev/tools/tuning-campaign-support/src/schema.rs":"d7f4001656c265abb7fb960cdba5eacf3739d0c76ab112d6a082e9eab12bec5d","dev/tools/tuning-campaign-support/src/seed.rs":"7be49c66c0917b44e748a90e668ad16ad5fef9e8a5e76efe173bb1b602539036","dev/tools/tuning-campaign-support/src/statistics.rs":"6c574b5ecca3aae79d760325117ccee816f10980b50a559234c2fd3b00347fa7","dev/tools/tuning-campaign-support/src/timing.rs":"9d15dfe5e8400def4dca53713bc0ff89926ae43f0f2a87cbf4f0e40f55de60c8","dev/tools/tuning-campaign-support/src/transport.rs":"883f903e4c0b5d3f651347f4695a4139504856aadb8afcb77a5942435cc8995a"},"lifecycle_sha256":{"dev/bench_results/f547c394/run-smoke.sh":"44be6611988a99be006ed043030b8ba6e067ac427c7829806c2c36a497f414db","dev/scripts/ccx1-bench-flock.sh":"c209cd8a1d518884e87485b4d4fcd750f2bed615609e4535ae80e82fea66c39f","dev/tools/tuning-campaign-support/src/bin/benchmark-ab-runner.rs":"e02ebf7eab54e6fee16989c295a6d80a38fe5542867af8523a189c255295e160","dev/tools/tuning-campaign-support/src/journal.rs":"9edd52e94bc084560ee951703caccad98341cae46d657150bd76e134156f7b85","dev/tools/tuning-campaign-support/src/process.rs":"be4d88682ab62e8a70333892e1ebdcb9eb44f643194c21fd594549b062d1be05","dev/tools/tuning-campaign-support/src/protocol.rs":"b3b45cd56d1a22c5897907677dd1974b81dca3740c0da6c0f8c6184eee6d8feb","dev/tools/tuning-campaign-support/src/provenance.rs":"b559664e0c7173bc6599b43a4543a617bba853e1f2385abb7da015a742cc77ed","dev/tools/tuning-campaign-support/src/receipt.rs":"4eae42ddc8d81cddc8ed4c9b6416b99dab6e9b5d94a017d36743f7aa428130e8"},"manifest_path":"dev/active/f547c394/producing-inputs.json","manifest_sha256":"1d046407be24951ea54840face6073f95bc257c67b83e6f97c86e0977ddd716b"}},"toolchain":"rustc 1.97.0 (2d8144b78 2026-07-07)"}}
dev/bench_results/f547c394/2026-09-07-f547c394-protocol-pilot/inputs/producing/dev/tools/tuning-campaign-support/src/receipt.rs:619:                            "campaign-start does not freeze the receipt addendum content pin",
dev/bench_results/f547c394/2026-09-07-f547c394-protocol-pilot/inputs/producing/dev/tools/tuning-campaign-support/src/receipt.rs:653:                            "execution log does not open with campaign-start",
dev/bench_results/f547c394/2026-09-07-f547c394-protocol-pilot/inputs/producing/dev/tools/tuning-campaign-support/src/journal.rs:469:            return Err(invalid("the first journal record must be campaign-start"));
dev/bench_results/f547c394/2026-09-07-f547c394-protocol-pilot/inputs/producing/dev/tools/tuning-campaign-support/src/journal.rs:472:            return Err(invalid("campaign-start may occur exactly once"));
dev/bench_results/f547c394/2026-09-07-f547c394-protocol-pilot/inputs/producing/dev/tools/tuning-campaign-support/src/journal.rs:484:            return Err(invalid("campaign-start must precede a terminal record"));
dev/bench_results/f547c394/2026-09-07-f547c394-protocol-pilot/inputs/producing/dev/tools/tuning-campaign-support/src/journal.rs:1121:                return Err(invalid("journal does not begin with campaign-start"));
dev/bench_results/f547c394/2026-09-07-f547c394-protocol-pilot/inputs/producing/dev/tools/tuning-campaign-support/src/journal.rs:1153:                return Err(invalid("journal contains duplicate campaign-start"));
dev/bench_results/f547c394/2026-09-07-f547c394-protocol-pilot/inputs/producing/dev/tools/tuning-campaign-support/src/bin/benchmark-ab-runner.rs:87:/// Immutable facts the campaign-start record carries for finalization.
dev/bench_results/f547c394/2026-09-07-f547c394-protocol-pilot/inputs/producing/dev/tools/tuning-campaign-support/src/bin/benchmark-ab-runner.rs:342:            .map_err(|error| invalid(format!("campaign-start facts do not decode: {error}")))?;
dev/bench_results/f547c394/2026-09-07-f547c394-protocol-pilot/inputs/producing/dev/tools/tuning-campaign-support/src/bin/benchmark-ab-runner.rs:345:                "resume identity differs from the campaign-start facts",
dev/bench_results/f547c394/2026-09-07-f547c394-protocol-pilot/inputs/producing/dev/tools/tuning-campaign-support/src/bin/benchmark-ab-runner.rs:765:            .ok_or_else(|| invalid("log lacks campaign-start"))?
dev/bench_results/f547c394/2026-09-07-f547c394-protocol-pilot/inputs/producing/dev/tools/tuning-campaign-support/src/bin/benchmark-ab-runner.rs:772:            "staged plan differs from the campaign-start plan digest",
dev/bench_results/f547c394/2026-09-07-f547c394-protocol-confirmation/inputs/producing/dev/tools/tuning-campaign-support/src/bin/benchmark-ab-runner.rs:87:/// Immutable facts the campaign-start record carries for finalization.
dev/bench_results/f547c394/2026-09-07-f547c394-protocol-confirmation/inputs/producing/dev/tools/tuning-campaign-support/src/bin/benchmark-ab-runner.rs:342:            .map_err(|error| invalid(format!("campaign-start facts do not decode: {error}")))?;
dev/bench_results/f547c394/2026-09-07-f547c394-protocol-confirmation/inputs/producing/dev/tools/tuning-campaign-support/src/bin/benchmark-ab-runner.rs:345:                "resume identity differs from the campaign-start facts",
dev/bench_results/f547c394/2026-09-07-f547c394-protocol-confirmation/inputs/producing/dev/tools/tuning-campaign-support/src/bin/benchmark-ab-runner.rs:765:            .ok_or_else(|| invalid("log lacks campaign-start"))?
dev/bench_results/f547c394/2026-09-07-f547c394-protocol-confirmation/inputs/producing/dev/tools/tuning-campaign-support/src/bin/benchmark-ab-runner.rs:772:            "staged plan differs from the campaign-start plan digest",
dev/bench_results/f547c394/2026-09-07-f547c394-protocol-confirmation/inputs/producing/dev/tools/tuning-campaign-support/src/receipt.rs:619:                            "campaign-start does not freeze the receipt addendum content pin",
dev/bench_results/f547c394/2026-09-07-f547c394-protocol-confirmation/inputs/producing/dev/tools/tuning-campaign-support/src/receipt.rs:653:                            "execution log does not open with campaign-start",
dev/bench_results/f547c394/2026-09-07-f547c394-protocol-confirmation/inputs/producing/dev/tools/tuning-campaign-support/src/journal.rs:469:            return Err(invalid("the first journal record must be campaign-start"));
dev/bench_results/f547c394/2026-09-07-f547c394-protocol-confirmation/inputs/producing/dev/tools/tuning-campaign-support/src/journal.rs:472:            return Err(invalid("campaign-start may occur exactly once"));
dev/bench_results/f547c394/2026-09-07-f547c394-protocol-confirmation/inputs/producing/dev/tools/tuning-campaign-support/src/journal.rs:484:            return Err(invalid("campaign-start must precede a terminal record"));
dev/bench_results/f547c394/2026-09-07-f547c394-protocol-confirmation/inputs/producing/dev/tools/tuning-campaign-support/src/journal.rs:1121:                return Err(invalid("journal does not begin with campaign-start"));
dev/bench_results/f547c394/2026-09-07-f547c394-protocol-confirmation/inputs/producing/dev/tools/tuning-campaign-support/src/journal.rs:1153:                return Err(invalid("journal contains duplicate campaign-start"));
```

## Acceptance identity and evidence publication

The executable SHA-256 is `0584560cf922589414e534fafb0b1a2164746d07d33c420dbeca3cdad3c9ce0d`.
The behavioral source-map SHA-256 is `b96863ac52f36852f2f180cd5dd87fdaa548b3b6ec14331daece1c217ff5c907`.
The map encoding, every selected source/build digest, prior summaries, resulting
summaries, commands and unchanged raw-file identities are recorded in
`dev/bench_results/f547c394/revalidation.json`.

Both receipts and both clean exports are accepted with zero findings. Neither
qualifies for production selection. The pilot is exploratory; confirmation retains
its negative and unavailable outcomes. No receipts are regenerated or superseded.
The pilot acceptance summaries change only the erroneous qualification flag;
the preceding summaries remain in the revalidation record. The confirmation
summaries are recomputed and remain byte-identical.

Tracked nested lockfiles:

```text
dev/bench_results/f547c394/2026-09-07-f547c394-protocol-confirmation/inputs/producing/Cargo.lock
dev/bench_results/f547c394/2026-09-07-f547c394-protocol-pilot/inputs/producing/Cargo.lock
```

## Artifacts for the lead

Created:

- `dev/bench_results/f547c394/revalidation.json`
- `dev/active/f547c394/rework-validation.md`

Updated active protocol/design/schema/clarification, the launcher comment, library
implementation and tests, and the pilot's two acceptance summaries. Raw receipt,
plan, log, sample and snapshot files are unchanged. No artifact is removed or
superseded. The lead owns linking these two created artifacts and any tracker
updates. `producing-inputs.json` already covers every affected producing source;
it needs no inventory change.

## Remaining limitations

- Full CI is not passing: the sandbox exposes no usable HIP device, and the
  final workspace run hit an eight-second timeout in unrelated rare-event
  artifact publication recovery.
  The initial workspace run also found the process-scan race; that race is fixed
  and final support and workspace test results are preserved below. Both full CI invocations are preserved. The final invocation follows the
  source correction required by the first failure; the HIP test still requires
  a usable device.
- The worker could not commit; the execution lead committed the validated tree
  in final form on the worker branch.
- Historical gate-run JSON is absent. The cumulative findings audit uses both
  committed review verdicts, with the raw audit failure preserved above.
- Formal code/doc/research gate judgments and tracker writes belong to the lead
  and were not invoked. Passing implementation checks do not assert gate approval.
