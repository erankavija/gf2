# Research rework findings: protocol v3

## Question and method

Can the shared Zen 3 protocol preserve scientific evidence across resume,
failed attempts, decoder-quality comparison and cache-state declarations?
The method is independent acceptance recomputation over frozen inputs, behavioral
fixtures, workspace sweeps, and bounded release pipeline receipts. Timing uses
paired fresh child executions; quality uses independent frame slots paired
between arms. Synthetic smoke quality is deterministic fixture data, not a
Monte Carlo estimate of a decoder or a scientific performance claim.

## Current protocol v3 state

Protocol v3 applies the bounded convergence rules for margins, bootstrap alpha,
outlier boundaries, document guards, legacy receipt paths, pilot-derived
resolution, and lock observations. P-20 independently binds the stored v3
alpha, and pilot resolution derives its corrected alpha from frozen pilot
addendum and ledger snapshots. The active runner produces fresh r2 pilot and
confirmation smoke evidence at
`dev/bench_results/f547c394/v3-r2-pilot/` and
`dev/bench_results/f547c394/v3-r2-confirmation/`; both independently evaluate
as accepted with zero findings. The smoke receipts are pipeline evidence and
make no gf2 performance or decoder-quality claim.

The v1 and v2 receipt collections remain byte-for-byte equal to baseline
`c01be44e`. The v3 r1 evidence collection remains byte-for-byte equal to
`7756e1fd` as falsified incomplete-provenance evidence: its closure captures
`run-smoke-v2.sh` instead of the launcher that ran. The machine-readable
preservation proofs are
[`research-r4-v1-v2-preservation.json`](research-r4-v1-v2-preservation.json)
and
[`research-r5-v3-r1-preservation.json`](research-r5-v3-r1-preservation.json).
Historical rows below retain their reviewed wording and evidence references.

[The raw pre-work audit](research-r3-prework.txt) precedes source edits.
The local tracker reports no matching research/code review runs. The linked
R1/R2 reviews, prior rework table and lead’s subsequently linked
[R3 prior-findings ledger](../1a379447-zen3-cpu-performance/reviews/f547c394-r3.md#prior-findings-regression-table-tier-15)
supply the historical findings below. That ledger identifies research R1 as
citation-label drift, records its authorized repair, and reopens code R2 F1
as the current host-provenance finding. The epic handoffs corroborate the
citation-tier history and record the later checker exhaustion without output.
Raw gate stdout remains unavailable locally; no additional methodology finding
is invented or represented as reviewed.

## Pinned arms and mappings

The v3 launcher pins the release executable, toolchain, producing/build closure,
protocol, schema, contract, addendum and exact plan. Arm descriptors use
repository-relative executable paths. `GF2_SMOKE_PASSES` selects one or two XOR
passes; the second pass is the deliberately losing timing arm. The decoder
cell uses the same deterministic ordered quality slots for both arms and a
frozen information-bit denominator. This validates transport and acceptance
mapping only, not a real decoder algorithm or external standard conformance.
Host observations are recorded per session; the protocol lists material and
informational fields. Results and sample counts are projected below from the
receipts, not hand-maintained estimates.

## Resolution table

Source line numbers in the review columns refer to reviewed commit `259c1b9a`.
Resolution citations refer to the final submitted file contents (the lead makes
HEAD by committing them). Each sweep match has its own row in
[the sweep resolution table](research-r3-sweep-resolutions.md), which is part of
this table. The accompanying JSON preserves every exact match and command.

| # | Round | Source | Finding (verbatim) | Resolution (file:line at HEAD) |
|---|-------|--------|--------------------|--------------------------------|
| H01 | R1 | reviewer | F1 (protocol.rs:185-215, receipt.rs:463): a pinned artifact is verified only against the bytes at its pinned commit; an unavailable Git object or a digest mismatch is an error-severity finding that rejects the receipt. Remove the working-tree fallback and the note path. | Git-object requirements are superseded by `dev/active/f547c394/provenance-clarification.md:13`; receipt-local verification has no working-tree fallback: `dev/tools/tuning-campaign-support/src/protocol.rs:203`, `dev/tools/tuning-campaign-support/tests/protocol_contracts.rs:1142`, `dev/tools/tuning-campaign-support/tests/protocol_contracts.rs:1170` |
| H02 | R1 | reviewer | F2 (addendum-protocol-smoke.json:11,18; protocol.rs:666): prove freezing rather than declaring it. The receipt's addendum pin commit must contain the addendum with the pinned digest and be an ancestor of or equal to the receipt's source revision; a self-referential or missing `frozen.at_commit` is not acceptable. `resolution_evidence` must name a committed pilot receipt (path and digest) distinct from the receipt under evaluation, verified by the tool. Regenerate the smoke evidence as a pilot receipt followed by a confirmatory receipt that cites it. | Git-ancestry requirements are superseded by `dev/active/f547c394/provenance-clarification.md:13`. Campaign facts prove the content freeze; distinct pilot enforcement: `dev/tools/tuning-campaign-support/src/receipt.rs:1865`, `dev/tools/tuning-campaign-support/tests/protocol_contracts.rs:1497` |
| H03 | R2 | reviewer | acceptance binds only the addendum to campaign-start. | `dev/tools/tuning-campaign-support/src/receipt.rs:365`, `dev/tools/tuning-campaign-support/tests/protocol_contracts.rs:1233`, `dev/tools/tuning-campaign-support/src/host.rs:452`, `dev/tools/tuning-campaign-support/src/receipt.rs:343`, `dev/tools/tuning-campaign-support/tests/protocol_contracts.rs:2147` |
| H04 | R2 | reviewer | Add failing behavioral evidence for self-consistent replacement pins/producing inputs, plan or execution settings that differ from frozen campaign facts. Preserve valid portable receipts and metadata-independent resume/acceptance. Audit all frozen fields together rather than adding only another single-field comparison. | `dev/tools/tuning-campaign-support/tests/protocol_contracts.rs:1280` rebuilds internally consistent replacement inputs and requires P-23 rejection; `dev/tools/tuning-campaign-support/src/host.rs:452`, `dev/tools/tuning-campaign-support/src/receipt.rs:343`, `dev/tools/tuning-campaign-support/tests/protocol_contracts.rs:2147` |
| H05 | R2 | reviewer | Re-evaluate the committed pilot and confirmation with the corrected acceptance tool, record its identity and results, and update documents if needed. Preserve prior raw evidence; do not modify the frozen historical record to make validation pass. | `dev/active/f547c394/research-r3-v1-preservation.json:1` records evaluator identity and preservation of every published v1 byte; `dev/tools/tuning-campaign-support/tests/protocol_contracts.rs:2133` evaluates both original receipts read-only. |
| H06 | Rework audit | reviewer | Resume omits frozen artifact identities | `dev/tools/tuning-campaign-support/src/host.rs:452`, `dev/tools/tuning-campaign-support/src/receipt.rs:343`, `dev/tools/tuning-campaign-support/tests/protocol_contracts.rs:2147`, `dev/tools/tuning-campaign-support/tests/protocol_contracts.rs:1900` |
| H07 | Rework audit | reviewer | Partial snapshot publication | `dev/tools/tuning-campaign-support/src/protocol.rs:145` uses `dev/tools/tuning-campaign-support/src/journal.rs:1800`; receipt-local snapshots are complete immutable publications. |
| H08 | Rework audit | reviewer | Snapshot paths escape the receipt | `dev/tools/tuning-campaign-support/src/protocol.rs:215`, `dev/tools/tuning-campaign-support/tests/protocol_contracts.rs:1117` |
| H09 | Rework audit | reviewer | Git metadata controls checkpoint replay | `dev/tools/tuning-campaign-support/tests/checkpoint_provenance.rs:70`, `dev/tools/tuning-campaign-support/tests/checkpoint_provenance.rs:90` verify preserved manifests/results across metadata changes. |
| H10 | Rework audit | reviewer | Extraction omits a835 producing-input closure | `dev/active/a83583e0/producing-build-inputs.json:110`, `:123`, `:297` include shared provenance.rs in applicable maps; verified read-only, no separate campaign edits. |
| H11 | Publication audit | reviewer | Ignored nested Cargo.lock snapshots missing from committed receipts | Both historical lock snapshots are tracked and byte-verified in `dev/active/f547c394/research-r3-v1-preservation.json:1`. New receipt snapshots require the exact force-add paths in the lead commit plan. |
| H12 | Lead criterion audit | reviewer | No actual unrelated commit/edit between runner sessions | `dev/tools/tuning-campaign-support/tests/protocol_contracts.rs:1900` creates a real unrelated Git commit and a dirty documentation edit between three bounded runner sessions. |
| H13 | Research tier 1 | reviewer | Obsolete comparator citation labels on protocol task | `dev/active/1a379447-zen3-cpu-performance/reviews/f547c394-r3.md:40` records the authorized label repair and research tier-1 PASS; `dev/active/f547c394/validation-r3/issue-status.txt:1` preserves current read-only issue status. |
| H14 | R2 resumption | reviewer | the in-progress acceptance evaluator explicitly parses unit JSON/schema/digests while `journal` owns checkpoint validation. | `dev/tools/tuning-campaign-support/src/journal.rs:1582`, `dev/tools/tuning-campaign-support/src/receipt.rs:1100`, `dev/tools/tuning-campaign-support/tests/checkpoint_provenance.rs:158` |
| H15 | R2 resumption | reviewer | the decoder-quality fixture was changing results after checkpoint publication | `dev/tools/tuning-campaign-support/tests/protocol_contracts.rs:377` publishes supplied quality through canonical checkpoint construction; `dev/tools/tuning-campaign-support/tests/protocol_contracts.rs:1755` and `dev/tools/tuning-campaign-support/src/bin/benchmark-ab-runner.rs:540`, `dev/tools/tuning-campaign-support/src/bin/benchmark-ab-runner.rs:676`, `dev/tools/tuning-campaign-support/tests/protocol_contracts.rs:1900` |
| H16 | R2 resumption | reviewer | unavailable-cell/unavailable-checkpoint coverage with its regression test | `dev/tools/tuning-campaign-support/src/receipt.rs:1100` precedes status dispatch; `dev/tools/tuning-campaign-support/tests/protocol_contracts.rs:1580` rejects missing and altered unavailable checkpoints. |
| H17 | Revalidation audit | acceptance run | pair count differs from the saved plan | `dev/tools/tuning-campaign-support/src/protocol.rs:1030` supplies one default to runner/evaluator; `dev/tools/tuning-campaign-support/tests/protocol_contracts.rs:2133` accepts the unchanged historical pilot. |
| H18 | Revalidation audit | acceptance run | qualifies for production selection: **true** | `dev/tools/tuning-campaign-support/src/receipt.rs:1842` requires non-exploratory evidence; `dev/tools/tuning-campaign-support/tests/protocol_contracts.rs:1255`. Historical false summary is preserved in `dev/bench_results/f547c394/revalidation.json:1`. |
| H19 | Full CI | CI test failure | called `Result::unwrap()` on an `Err` value: Os { code: 3, kind: Uncategorized, message: "No such process" } | `dev/tools/tuning-campaign-support/src/process.rs:121`; retained ESRCH tolerance is exercised by the full support suite. |
| H20 | Full CI | CI test failure | build dispatcher on gfx1030: Fatal(KernelLaunch { hip_code: 100, kernel: "GfxTarget::detect_device", args: "hip context: hipGetDeviceCount" }) | Environment limitation reproduced in `dev/active/f547c394/validation-r3/gpu-diagnosis.log:1`; full CI fails this HIP-device test in `validation-r3/cargo-ci.log`. No GPU implementation change; lead must validate with the required device. |
| H21 | Final CI | CI test failure | TIMEOUT [   8.124s] gf2-sim::permanent_rare_event_artifacts rare_event_artifact_partial_publish_recovery | Historical timeout is preserved in `dev/active/f547c394/rework-validation.md:203`; this round’s full CI has only the GPU failure (`dev/active/f547c394/validation-r3/cargo-ci.log:1`). No wall-clock assertion or test budget was widened by this worker. |
| H22 | Pre-edit | audit-step-1 | No matching gate runs for issue f547c394; zsh:2: no matches found: .jit/gate-runs/*/result.json | `dev/active/f547c394/research-r3-prework.txt:1` records the same absence; local gate history remains unavailable, not a passing gate. |
| H23 | Historical audit | audit-step-3: `dev/active/1a379447-zen3-cpu-performance/measurement-contract.md:120` | limit, preserved experiment or tracked falsifiable follow-up within the search | Outside this issue: the literal criterion scope assigns comparator builds, baseline campaigns and consumer profiling to later family surveys. `dev/active/1a379447-zen3-cpu-performance/measurement-contract.md:120` requires those surveys to track residual gaps. |
| H24 | Historical audit | audit-step-3: `dev/active/1a379447-zen3-cpu-performance/reviews/f547c394-r1.md:23` | ### Deferred-items audit (Tier 2.75) | `dev/tools/tuning-campaign-support/src/receipt.rs:365`, `dev/tools/tuning-campaign-support/tests/protocol_contracts.rs:1233`, `dev/tools/tuning-campaign-support/src/host.rs:452`, `dev/tools/tuning-campaign-support/src/receipt.rs:343`, `dev/tools/tuning-campaign-support/tests/protocol_contracts.rs:2147` |
| H25 | Historical audit | audit-step-3: `dev/active/1a379447-zen3-cpu-performance/reviews/f547c394-r1.md:24` | design.md "Pre-existing fragility observed" (a83583e0 composer lockfile) — legitimately out of scope, recorded in surfaced_pitfalls. design.md "Named exception: session lifecycle store" — tracked exception with a convergence condition; OK. | `dev/active/f547c394/design.md:86`; outside this protocol issue: separate a835 family producing closure and family surveys. The named lifecycle exception and convergence remain in design.md. |
| H26 | Historical audit | audit-step-3: `dev/active/1a379447-zen3-cpu-performance/reviews/f547c394-r2.md:30` | Formal criterion and holistic acceptance stop at the failed gate. The mandatory prior-findings and deferred-items audits are complete. R2 F1 violates the complete frozen-content obligation in REQ-01/REQ-04. | `dev/tools/tuning-campaign-support/src/receipt.rs:365`, `dev/tools/tuning-campaign-support/tests/protocol_contracts.rs:1233`, `dev/tools/tuning-campaign-support/src/host.rs:452`, `dev/tools/tuning-campaign-support/src/receipt.rs:343`, `dev/tools/tuning-campaign-support/tests/protocol_contracts.rs:2147` |
| H27 | Historical audit | audit-step-3: `dev/active/1a379447-zen3-cpu-performance/reviews/f547c394-r2.md:36` | ### Deferred-items audit (Tier 2.75) | `dev/tools/tuning-campaign-support/src/receipt.rs:365`, `dev/tools/tuning-campaign-support/tests/protocol_contracts.rs:1233`, `dev/tools/tuning-campaign-support/src/host.rs:452`, `dev/tools/tuning-campaign-support/src/receipt.rs:343`, `dev/tools/tuning-campaign-support/tests/protocol_contracts.rs:2147` |
| H28 | Historical audit | audit-step-3: `dev/active/1a379447-zen3-cpu-performance/reviews/f547c394-r2.md:38` | The linked protocol/design/schema/addenda/clarification and measurement contract contain no in-scope deferred deliverable. `measurement-contract.md:120` requires future family work to track falsifiable residual gaps; the issue explicitly excludes family surveys and production optimization. `design.md:70-86` identifies pre-existing a835 Git-policy migration (`0ba493e1`) and its untracked composer lockfile fragility (`a83583e0`); these belong to the separate campaign contract, not this protocol task. `design.md:21-33` preserves the named session lifecycle-store exception and explicit convergence condition. The historical review records superseded findings rather than current unresolved implementation. | `dev/active/f547c394/design.md:86`; outside this protocol issue: separate a835 family producing closure and family surveys. The named lifecycle exception and convergence remain in design.md. |
| F1 | Research R3 | `benchmark-ab-runner.rs:215` | incomplete host provenance across resume. freezes only hostname and CPU IDs. Kernel, CPU model, governors, SMT state, topology and affinity can change between sessions, and finalization keeps only the last session's host observation. A campaign whose sessions ran under materially different host conditions can pass acceptance and publish a single hardware description. This is R2 F1 still open. | All material fields bind resume; complete observations are retained per session. Informational timestamp/load/memory differences are explicitly tested. `dev/tools/tuning-campaign-support/src/host.rs:452`, `dev/tools/tuning-campaign-support/src/receipt.rs:343`, `dev/tools/tuning-campaign-support/tests/protocol_contracts.rs:2147`. |
| F2 | Research R3 | `receipt.rs:1565` | failed confirmations can be omitted from the trial ledger. validates only the entries the addendum supplies. There is no append-only chain or independently authoritative family ledger proving the ledger is complete, so dropping a failed trial reduces the Bonferroni comparison count and can narrow intervals enough to change an adoption decision. This is the mechanism REQ-03 exists to prevent ("prevent failed confirmation data from being silently relabeled or discarded") and it violates `@/inv/falsification-preserved`. | Reservations precede measurements, bind predecessor bytes and include unfinished/failed attempts. Complete prefixes, not supplied counters, determine comparison count and sequential alpha spending. `dev/tools/tuning-campaign-support/src/trial_ledger.rs:87`, `dev/tools/tuning-campaign-support/src/trial_ledger.rs:178`, `dev/tools/tuning-campaign-support/tests/protocol_contracts.rs:2346`. |
| F3 | Research R3 | `benchmark-ab-runner.rs:508` | the shared runner cannot produce an accepted decoder receipt. parses decoder quality and journals it but never transfers it into the receipt; `measure_cell` leaves `decoder_quality` as `None`, and acceptance rejects a decoder cell without that field. The tests construct quality receipts directly, so they never exercise the advertised runner path. REQ-06's executable decoder protocol is therefore unavailable — which the LDPC survey (`c077a88b`) needs *this week*. | Child quality is retained per execution and copied to the cell. Repeated timing executions must agree on the same frozen frame evidence. Runner integration and release smoke exercise the path. `dev/tools/tuning-campaign-support/src/bin/benchmark-ab-runner.rs:540`, `dev/tools/tuning-campaign-support/src/bin/benchmark-ab-runner.rs:676`, `dev/tools/tuning-campaign-support/tests/protocol_contracts.rs:1900`. |
| F4 | Research R3 | `receipt.rs:1411` | decoder sample counts and BER are not bound to the frozen plan. recomputes intervals from receipt-supplied counts without requiring frames to match the addendum, without binding bits to the input/code identity, and without checking that the reported BER equals `bit_errors / bits`. A receipt can change its denominator or its BER point and still be accepted. | Frames equal input.frames, bits equal frames times code.k, and frame error vectors recompute aggregates and BER points. Separate rejection fixtures cover frames, bits and point alteration. `dev/tools/tuning-campaign-support/src/receipt.rs:2044`, `dev/tools/tuning-campaign-support/tests/protocol_contracts.rs:2239`. |
| F5 | Research R3 | `receipt.rs:1412` | BER uncertainty assumes independent bits. applies a binomial Wilson interval to aggregate bit errors. Errors cluster within a decoded frame or codeword, so bits are not independent Bernoulli trials and the aggregate interval understates uncertainty, possibly by a lot. | The bounded-frame interval assumes independent frames and unrestricted dependence inside each frame. Clustered fixture demonstrates a wider interval than aggregate-bit Wilson. `dev/tools/tuning-campaign-support/src/abtest.rs:349`, `dev/tools/tuning-campaign-support/tests/protocol_contracts.rs:2305`. |
| F6 | Research R3 | `receipt.rs:1456` | FER quality compatibility is not established by the implemented bound. compares the candidate's marginal FER upper bound against a tolerance times the baseline's marginal upper bound. Two marginal upper bounds do not bound their ratio; using the baseline's *upper* bound makes the test permissive, and the paired same-frame structure is discarded. A degraded decoder can be certified quality-compatible. | A paired confidence bound on candidate FER minus tolerance times baseline FER certifies non-inferiority. At the declared 1000 synthetic frame slots, equal arms pass and the degraded candidate fails compatibility. `dev/tools/tuning-campaign-support/src/abtest.rs:368`, `dev/tools/tuning-campaign-support/tests/protocol_contracts.rs:2239`. |
| F7 | Research R3 | `timing.rs:176` | the declared cold-cache condition is not cold. calibrates the call count by repeatedly invoking the workload before the first timed window, including for cells declaring `cache_state: cold`. A fresh process does not reset CPU caches or frequency state either. | Frozen positive calls bypass workload calibration. Protocol defines first-use workload precisely and makes no L1/L2/L3, TLB or frequency reset claim. `dev/tools/tuning-campaign-support/src/timing.rs:182`, `dev/tools/tuning-campaign-support/src/protocol.rs:703`, `dev/tools/tuning-campaign-support/tests/protocol_contracts.rs:2313`. |
| F8 | Research R3 | `receipt.rs:1315` | outlier detection pools baseline and candidate windows under one absolute median threshold, so sensitivity depends on effect size. | Fixed: flag within each execution; the large-effect regression fixture shows the pooled false flags disappear. `dev/tools/tuning-campaign-support/src/receipt.rs:1474`, `dev/tools/tuning-campaign-support/tests/protocol_contracts.rs:2452`. |
| F9 | Research R3 | `protocol.md:203` | the "10 000 resamples are well below every declared measurement resolution" claim has no derivation or stability analysis, while the schema admits arbitrary resolutions. | Removed the universal claim. A second seeded endpoint calculation must fit declared resolution and each tail needs twenty expected replicates; otherwise the cell is not-confirmatory. This is explicitly a diagnostic, not a coverage theorem. `dev/tools/tuning-campaign-support/src/receipt.rs:1758`, `dev/tools/tuning-campaign-support/tests/protocol_contracts.rs:2500`. |
| F10 | Research R3 | `protocol.md:176` | a one-sided non-inferiority decision is attributed to TOST equivalence; Schuirmann's procedure is two one-sided tests. | Fixed the active protocol and decision rustdoc. Historical v1 text remains immutable and is identified in the amendment record. `dev/active/f547c394/protocol.md:220`, `dev/tools/tuning-campaign-support/src/abtest.rs:260`. |
| F11 | Research R3 | `addendum-protocol-smoke-pilot.json:11` | the declared freeze timestamp `2026-09-07T00:00:00Z` is later than the launcher start `2026-09-06T23:07:25Z`. | Recorded the v1 metadata contradiction without editing evidence. V2 preparation records actual UTC time before launch and both runner and acceptance reject a later declared freeze. `dev/active/f547c394/amendment-v2.md:18`, `dev/tools/tuning-campaign-support/src/protocol.rs:1215`, `dev/tools/tuning-campaign-support/tests/protocol_contracts.rs:2480`. |
| F12 | Research R3 | `protocol.md:27` | the current protocol and the receipt-pinned protocol have different digests and different acceptance semantics while both identify as version 1, which the protocol's own versioning rule forbids. | Published v2 semantics and schema, preserved v1 evidence and its evaluator branch, and reject version mismatches in both directions. `dev/active/f547c394/amendment-v2.md:27`, `dev/tools/tuning-campaign-support/src/receipt.rs:700`, `dev/tools/tuning-campaign-support/tests/protocol_contracts.rs:2133`. |
| F13 | Research R4 | Declared margins accept equality with resolution or reconstruct decimal deltas. | V3 compares each declared margin directly with `1 + measurement_resolution` and rejects equality. `src/protocol.rs`; equality and one-percent-above coverage: `tests/protocol_contracts.rs`. |
| F14 | Research R4 | Bootstrap rank selection reconstructs alpha from displayed confidence. | V3 passes declared corrected alpha to bootstrap rank selection and records it in the interval. `src/abtest.rs`, `src/receipt.rs`, and `tests/protocol_contracts.rs`. |
| F15 | Research R4 | The flagged-window equality boundary differs between the protocol and evaluator. | V3 flags windows at or above the factor while retaining strict `max_flagged_fraction`. `src/abtest.rs`, `src/receipt.rs`, and `tests/protocol_contracts.rs`. |
| F16 | Research R4 | The protocol guard omits semantic justifications and assumes a bounded P-rule range. | The guard compares full table rows and derives the P-rule set from the evaluator source. `src/protocol.rs` and `tests/protocol_contracts.rs`. |
| F17 | Research R4 | V1 prior-trial receipt paths bypass the repository-relative path rule. | Every declared prior-trial path uses `validate_relative`. `src/protocol.rs` and `tests/protocol_contracts.rs`. |
| F18 | Research R4 | Resolution evidence does not establish its derived width or family identity. | V3 recomputes the widest relative pilot interval half-width and checks family and issue identity. `src/receipt.rs` and `tests/protocol_contracts.rs`. |
| F19 | Research R4 | P-07 named facts beyond the runner's observations. | V3 requires only the inherited held descriptor and independent conflicting-lock observation, with path and PID. `src/receipt.rs`, `src/bin/benchmark-ab-runner.rs`, and `tests/protocol_contracts.rs`. |
| A01 | Sweep consequence | Sequential alpha/m across attempts | Repeated alpha/m decisions need an overall error budget. | `dev/tools/tuning-campaign-support/src/trial_ledger.rs:214` derives alpha/[t(t+1)] from the chain; the summable allocation is stated in protocol.md and shared by runner/evaluator. |
| A03 | Ledger consequence | Candidate attempts | The frozen per-candidate attempt cap needs enforcement across failed trials. | `dev/tools/tuning-campaign-support/src/trial_ledger.rs:224`, `dev/tools/tuning-campaign-support/src/trial_ledger.rs:87` and `dev/tools/tuning-campaign-support/src/trial_ledger.rs:37` bind behavioral arm identities and reject repeat reservations within one protocol version; the ledger fixture covers a failed candidate retry. |
| A02 | Prior ledger audit | research-review R1 ledger | research-review R1's closure ledger | `dev/active/1a379447-zen3-cpu-performance/reviews/f547c394-r3.md:34` is the lead’s explicit prior-findings ledger: code R1 F1/F2 and research R1 citation-label drift are closed, code R2 F1 is reopened as current F1. H01/H02/H13 and F1 retain those closures. Raw gate stdout is unavailable locally. |
| A04 | Release preflight | `dev/bench_results/f547c394/v2-pilot-preflight-rejected/rejection.json:1` | addendum invalid: decoder-family cell cold-first-use lacks decoder fields | The launcher supplies the frozen decoder contract to both decoder-family cells (`dev/bench_results/f547c394/run-smoke-v2.sh:55`). Original rejected frozen inputs and producing snapshots remain in the preflight archive; fresh r1 inputs govern measured evidence. No failed confirmation or timed sample was discarded. |
| A06 | Cache/timing sweep consequence | `dev/bench_results/f547c394/v2-pilot/inputs/protocol.md:160` | so successive calls do not reuse cache-resident data; bounding a confirmatory cell at 24 seconds of timed windows by construction; means the host was not quiet | The live v2 prose states only bank rotation, a nominal warm-window target and an instability classification: `dev/active/f547c394/protocol.md:160`, `dev/active/f547c394/protocol.md:248`, `dev/active/f547c394/protocol.md:254`. Numerical rules and evaluator are unchanged; `dev/active/f547c394/amendment-v2.md:41` preserves the earlier pilot pin. |
| A05 | Publication prose audit | `dev/active/f547c394/addendum-smoke-v2-confirmation-r1.json:28` | Unused by the exploratory smoke cells. | The generator uses role-neutral rationales (`dev/bench_results/f547c394/run-smoke-v2.sh:36`). The frozen confirmation retains its inherited unused comparator-gap rationale and pilot wording in the maintenance rationale; neither changes a setting or decision. Published bytes are immutable. |
| A07 | Attempt-policy consequence | `dev/bench_results/f547c394/v2-confirmation/inputs/protocol.md:166` | is `unstable` and must be re-run. | The protocol keeps the frozen cap for unstable/inconclusive outcomes: `dev/active/f547c394/protocol.md:167`. The unchanged ledger enforces it: `dev/tools/tuning-campaign-support/src/trial_ledger.rs:37`, `dev/tools/tuning-campaign-support/src/trial_ledger.rs:87`. Frozen v2 wording is preserved. |

## Evidence and criterion status

The focused support suite passes all 158 tests, including a complete chain with
a failed prior reservation, candidate retry rejection, frozen decoder count
rejections, clustered uncertainty, fixed cold calls and both published v1
receipts. Rust 1.95 compatibility, focused clippy and the release build pass.
The [command record](validation-r3/commands.md) cites every final raw output and
preserves intermediate failed checks without treating them as measurements.

`CARGO_CI_NO_SCCACHE=1 ./scripts/cargo-ci.sh` completed with exit 1. Its only
failed test is the GPU dispatcher’s HIP device detection. The isolated rerun
reproduces `hipGetDeviceCount`, code 100; the worker changes no GPU code or
wall-clock assertion. The complete verdict is in
[the raw CI log](validation-r3/cargo-ci.log); the failing step is verbatim:

```text
  ✗ test: FAILED (exit 100, 576s)
```

[The v1 preservation record](research-r3-v1-preservation.json) verifies all
published v1 files against the assignment’s base and records the release
evaluator identity. Their receipt-local bytes and original summaries are intact.
The preflight rejection is preserved in
`dev/bench_results/f547c394/v2-pilot-preflight-rejected/rejection.json`; it has no
measurement or ledger reservation. The independent r1 pilot inputs replace it
for measurement without overwriting it.

The per-finding sweep and the
[linked-document audit](research-r3-deferred-resolutions.md) are integral parts
of the resolution table. Generated raw-output and sweep companions should be
retained as repository artifacts; the concise findings report is the tracker’s
review entry point.

## Reproducible v2 smoke results

[The evidence projection](smoke-v2-evidence.json) is generated from the two
receipts and their independently recomputed acceptance summaries. It records
exact receipt, protocol, addendum, executable and producing-input digests.
The raw receipt directories are `dev/bench_results/f547c394/v2-pilot/` and
`dev/bench_results/f547c394/v2-confirmation/`. Both are accepted with zero
findings, two retained host observations and two bounded sessions; neither
qualifies for production selection. Portable evaluation reproduces each summary
byte for byte ([portability record](research-r3-portability.json)).

The observed host is `fraktaali`, AMD Ryzen 9 5900X, running Linux
7.2.2-arch1-1. The release executables were built with recorded
`rustc 1.97.0 (2d8144b78 2026-07-07)`; Rust 1.95 compatibility was checked
separately. All arms are conservative-portable builds of the pinned synthetic
XOR-fold workload, with one pass for baseline/candidate and two for the losing
arm. The current package version and complete build closure are in the pinned
Cargo inputs. Each timed child uses resolved CPU 0 with one worker. These are
pipeline results, not claims about gf2 kernels or a decoder implementation.

| Run | Cell | Paired executions | Timing ratio | Interval | Confidence | Outcome |
|---|---|---:|---:|---|---:|---|
| pilot | `decoder-pipeline` | 6 | 1.000790 | [0.996410, 1.003035] | 97.5% | pilot |
| pilot | `cold-first-use` | 6 | 0.502121 | [0.498147, 0.503608] | 97.5% | pilot |
| confirmation | `decoder-pipeline` | 24 | 1.001121 | [0.999084, 1.003638] | 98.75% | pass |
| confirmation | `cold-first-use` | 24 | 0.500389 | [0.497689, 0.503969] | 98.75% | fail |

The confirmation freezes the pilot-derived resolution in its addendum and pins
the distinct pilot receipt. The chain recomputes two confirmation comparisons
and spends 0.025 family alpha on this first attempt, yielding per-comparison
confidence 0.9875. The losing timing arm contradicts an improvement premise and
is retained as **fail**; the issue’s premise is a working measurement pipeline,
not that every candidate improves performance. Unavailable/missing-content and
inconclusive paths are deterministic fixture results, not omitted smoke cells.

Each decoder-quality payload carries 1000 deterministic frame slots, 32000
information bits, 500 frame errors and 500 bit errors per arm. The synthetic
BER is 0.015625 with frame-Hoeffding 95% interval [0, 0.05857194083467375];
FER is 0.5 with Wilson 95% interval [0.4690696003681042, 0.5309303996318958].
These are pipeline fixture counts and interval calculations, not Monte Carlo
estimates. Repeating the timing child reuses the same slots and contributes no
additional independent quality samples. Both cold cells use exactly 10000
frozen calls per window with `calibrated: false`; the mechanism establishes
first-use workload, not a reset of any hardware cache.

Reproduction uses `dev/bench_results/f547c394/run-smoke-v2.sh`: prepare, bounded
session calls until complete, then finalize, first for pilot and then for
confirmation. Existing frozen campaigns are deliberately not overwritten;
resume their stages or independently evaluate a copied receipt directory with
`target/release/benchmark-acceptance`. Fresh measurement reproductions need an
isolated output area and an explicitly initialized family ledger; never reset
the published ledger to obtain another adoption attempt. Exact launch commands,
UTC start/end times and pause/complete exits are in each `launcher.log`.
The output-description correction in the current launcher does not change any
measured numeric setting; frozen producing snapshots preserve the executed script.

The cache/target clarification occurred while the confirmation was queued,
before its opening record, and the confirmation pins that text. The final
wording also makes explicit that unstable/inconclusive outcomes cannot override
the cap enforced by the same ledger in both receipts. No numerical setting or
acceptance calculation changes between these v2 document snapshots. This preparatory prose work occurred after the lock request, a sequencing
deviation from the dispatch’s instruction to finish document writing first.
No build, quality simulation or extra timed work ran inside a measurement lock.

## Criterion-by-criterion outcome

| Criterion | Worker status | Evidence |
|---|---|---|
| REQ-01 | MET | `protocol.md`, `addendum.schema.json`, `amendment-v3.md`; verified immutable pins in both accepted v3 r2 receipts and v1/v2 compatibility fixtures. |
| REQ-02 | MET | Protocol sampling/decision/settings sections; `abtest.rs`, `timing.rs`, ledger and numerical-resolution fixtures; cold receipt path. |
| REQ-03 | MET | Frozen effect/budget declarations, `trial_ledger.rs`, failed-attempt/omission/count/retry fixtures, and the retained negative confirmation. |
| REQ-04 | MET | Complete frozen-fact/host comparator, pass/fail/inconclusive/provenance fixtures, real Git changes between runner sessions, both bounded v3 r2 receipts and portable evaluation. |
| REQ-05 | MET | Canonical mutex regression, inherited-lock runner check, two-session release journals, checkpoints, observed host/core/worker fields and declared metric/build/conversion schema. |
| REQ-06 | MET | Matched/fastest decoder schemas, frozen information-bit counts, independent-frame BER and paired FER bound; actual runner-produced accepted decoder cells. Real comparator compatibility remains family work by criterion. |
| REQ-07 | MET | Protocol, schema, validation, preservation records, launcher, addenda, plans, logs, receipts and summaries are committed and linked. The canonical Hoeffding citation is registered, and two independent Terra xhigh reviews converge on semantic PASS. |

## Remaining scope boundaries

The immutable v1 artifacts retain v1 behavior and contradictions. The named
v1 evaluator boundary is required while those committed receipts need
reproducible acceptance. Family survey results and production optimizations
are excluded by the literal issue scope; no other issue's evidence or worktree
is changed. General binomial event-count intervals and algebraic binomials
found by the sweep do not assume decoded bits are independent. The OSD campaign
already uses block-level uncertainty (`crates/gf2-sim/src/osd_campaign.rs`).

The cited Hoeffding paper is verified at its DOI and original PDF. The lead
registered `Hoeffding1963` in the canonical citation registry;
[citation-additions.toml](citation-additions.toml) retains the exact
issue-local citation provenance. No invented qualified citation address is
used. The normative P-17 text remains in `protocol.md`.
