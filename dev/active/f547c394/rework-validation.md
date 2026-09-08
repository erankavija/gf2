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

The verbatim transcripts of every command in this report — baseline and
final suites, freeze-rule regression evidence, pilot red evidence, lint,
formatting, toolchain compatibility, acceptance exports, CI runs and the
workspace-wide sweep dumps — are preserved unedited in the companion log
[`rework-validation-raw-outputs.md`](rework-validation-raw-outputs.md).
They are moved out of this report, not summarized: this document carries
the audit, resolution table and results, and the companion carries the
evidence they cite.

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

The deferred-items sweep over every linked Markdown/JSON document and the two
`rg` sweeps over the support crate and the f547c394 artifacts are reproduced
verbatim in the companion log
[`rework-validation-raw-outputs.md`](rework-validation-raw-outputs.md).

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
