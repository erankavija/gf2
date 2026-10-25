# Measurement preparation: rework attempt 1

> **Diátaxis Type:** Reference

Preparation for `@/issue/d06a99ed`, under the governing
[measurement contract](../../measurement-contract.md). The committed pilot and
confirmation receipts close F1; [Measurement outcome](#measurement-outcome)
points to them.

## Prior-findings and deferred-item audit

The complete [round-one review](../../reviews/d06a99ed-r1.md), code-review run
`b96051c6-3c03-44ec-ae68-6a16126f2793`, and research-review run
`f7415d76-56df-4159-8863-b43ac7d1c5d0` supply the findings below. Both gate-run
records are inspected before edits. There is no earlier closed gate finding.
The issue links [implementation.md](../implementation.md); its deferred-item
marker search returns no matches. Its explicit absence of measured execution
is the F1 evidence gap the committed receipts close. The review's deferred-audit heading and its statement
that no marker exists are audit narration, not a deferred implementation item.

| Round and source | Finding | Current disposition |
|---|---|---|
| R1 code-review F1 | Commit current pinned before/after measurements, including dispatch overhead, as required by the governing measurement contract. | CLOSED at `2fd908bd3`: the pilot and confirmation receipts, with execution logs, checkpoints, input snapshots and acceptance summaries, are committed; dispatch-inclusive small whole-call cells are part of the confirmation. |
| R1 research-review F1 | REQ-01 is marked met without required current pinned before/after measurements of the production routing change, including dispatch overhead. | CLOSED at `2fd908bd3`: the [outcome table](outcome.md) and the [findings](../../6230e0b6/findings.md) criterion row cite the committed confirmation. |
| R1 lead review L1 | Parameterize the existing batch checkpoint/resume tests over canonical scalar, portable and native route cases, with explicit worker bounds and route assertions. Preserve their interruption and partial-wave assertions. | CLOSED at `04652b0ee`; the independent reviewer verifies canonical scalar, portable and native checkpoint/resume coverage. No checkpoint source edits belong to this preparation. |
| R1 prior-findings audit | Obsolete three-argument constructor example in docs/how-to/select-acceleration.md. | CLOSED at `24db08ef9`, the pinned after revision. |
| Linked implementation deferred-marker search | No matches. | No additional deferred item. |

## Bounded comparison

The baseline source is `40d0d7aff`; the after source is
`24db08ef936e9b8eb42d31205d37c30207ba81a0`. Both use the existing native
`tuned-profile` arm, the same committed coding envelope and the same recorded
DVB-T2 and NR input/quality records. The baseline's production routing selects
lanes for full waves at every worker count. The after routing selects lanes
at one worker and the single-frame route at the multicore cells.

The [declaration](declaration.json) contains exactly six cells, all with the
non-regression objective:

| Code | Per-worker batch | Configured workers | Purpose |
|---|---:|---:|---|
| DVB-T2 | 1 | 1 | Small dispatch-inclusive whole call; both select single-frame |
| DVB-T2 | 8 | 1 | Full-wave lane path on both revisions |
| NR | 1 | 1 | Small dispatch-inclusive whole call; both select single-frame |
| NR | 8 | 1 | Full-wave lane path on both revisions |
| DVB-T2 | 8 | 12 physical | Measured losing multicore lane configuration |
| NR | 8 | 24 logical | Measured losing multicore lane configuration |

Remaining loss-worker configurations retain the conservative selector tests.
The frozen protocol's 10,000 bootstrap resamples admit at most six cells for
a first confirmatory attempt. This worker-policy candidate has no earlier
confirmatory reservation. A pilot uses six pairs per cell. The declaration
fixes both margins before the pilot; the canonical freezer derives resolution
and rejects a resolution that reaches either margin. Confirmation uses the
protocol's fixed pair count. Outcomes are recorded as evaluated, including
inconclusive or adverse results.

The existing evaluator's [empty-chain bound](confirmatory-bound.json), produced
with `benchmark-acceptance --next-attempt /dev/null` for this family, reports
the first-attempt capacity. It creates no reservation or measurement ledger.

The unchanged `3be770d5` driver includes worker dispatch in every timed call.
Its `conversion.dispatch_ns` diagnostic is the median of empty-body executor
dispatches after the measured windows. It is not isolated selector-predicate
timing. The small whole-call cells include the selector's production cost,
conversion and extraction; no isolated microbenchmark is introduced.

## Reuse and preparation

[prepare.py](prepare.py) composes the addendum and plan using the existing
`make-family.py` helpers: bundles, cell merge, addendum, build projection,
plan and canonical write. Its only adaptation is to give the two unchanged
`tuned` build rows distinct before/after names and absolute executable paths.
The existing launcher accepts executable digests from several original build
records. Neither the original records nor the shared catalogue is rewritten.

Detached source checkouts live under `target/d06a99ed-builds/{before,after}`.
The existing `record-gf2-arms-build.py` builds Rust 1.95 native release arms,
with the existing staged AFF3CT library, and writes the two preparation records
beside this document. Both source checkouts remain isolated from main's tests
and production files. No historical receipt or input snapshot changes.

See [commands.md](commands.md) for exact preparation, smoke, freezer and
measurement commands. The queue-line files are proposals for the lead to
insert; this preparation changes no shared queue or tracker data.

The launcher requires each timed plan's addendum to be committed. The
sequential [window.sh](window.sh) runs the pilot, freezes and composes
confirmation, then performs the lead-reviewed narrow commit of the generated
addendum, plan and derivation before invoking confirmation. It skips the commit
when these three paths already match HEAD. Frozen addendum and plan files are
reused on resume. No receipt, ledger or unrelated staged file enters that
commit, and the existing commitment check remains intact.

## Preparation corrections

The first native recorder invocation builds successfully but cannot emit its
full catalogue record: `no target file carries the pinned executable of the
catalogued arm aff3ct-flooding-nms-f32`. The isolated source checkout contains
no historical target executable. A symlink under each checkout's
`target/pinned/release` supplies the existing byte-identical pinned AFF3CT arm;
the recorder and all historical artifacts stay unchanged. One intermediate
setup invocation uses the checkout as its working directory for a main-relative
path and reports `FileNotFoundError`; absolute repository paths correct it.
The cached baseline recorder then completes successfully.

## Preparation verdict

Both unchanged Rust 1.95 native release recorders complete. Their
[before](before-build.json) and
[after](after-build.json) records name distinct tuned
executables. Both isolated source checkouts are clean.

The existing runner's `check` accepts the generated
[pilot plan](pilot/plan.json) and [addendum](pilot/addendum.json). Its
[untimed smoke](untimed-smoke.json) completes all six cells and twelve arm
validation dispatches, with zero timing windows. The
[execution log snapshot](untimed-execution.log) preserves that validation.
Observed paths are single-frame for both small controls, AVX2 lanes for both
one-worker full waves, and lanes before versus single-frame after at both
multicore cells. The existing driver verifies placement, worker counts and
prepared per-frame quality throughout.

The lead-reviewed launcher repairs a partial freeze by rerunning the canonical
freezer if either its addendum or derivation is absent, preserving an existing
addendum's frozen UTC timestamp. It reuses an existing confirmation plan.
The narrow automatic commit covers only addendum, plan and derivation; no
manual user step or commitment-check bypass is required.

`bash -n window.sh`, Python compilation, and the diff whitespace check pass.
Extracted bundle directories are reproducible from the committed archive by
`prepare.py` and are locally ignored; the generated plans and validation records
remain durable. The preparation itself records no timing sample.

## Catalogue integration correction

The two comparison snapshots are [before-build.json](before-build.json) and
[after-build.json](after-build.json), with the recorder's bytes preserved.
The survey's `record-peak-memory.py::build_records` discovers active catalogue
owners by the exact basename `build-identity.json`; their arm sets must be
disjoint, and each owner has its catalogue-wide smoke and validation records.
These before/after snapshots describe the same catalogue at different source
revisions, so they are explicit comparison inputs rather than active owners.
Registering them as owners causes ambiguous arm selection and incorrectly
requires catalogue-wide evidence beside each comparison snapshot.

The family composition helper and launcher consume both snapshots through
explicit path arguments and retain executable-digest verification. No active
owner is superseded, and no shared discovery rule, schema, gate or test changes.
Recorder reproduction runs in ignored target output directories and copies
its unchanged output to the explicit comparison filenames.

The affected family-generator, smoke, validation and peak-memory suites pass
all 56 tests after staging the comparison snapshot renames. Explicit-path
untimed validation again passes twelve dispatches across six cells, with zero
timing windows. The canonical live calibration manifest regeneration passes
all seven family closure checks; historical receipt inputs remain unchanged.

## Measurement outcome

The benchmark window runs [window.sh](window.sh). Both campaigns are verified
from their own execution logs: six `cell-start` and six `cell-complete`
records, every cell `measured` at its declared pairs, and a terminal
`complete` record.

- [Pilot acceptance summary](../../../../bench_results/d06a99ed/worker-routing/d06a99ed-worker-routing-pilot/acceptance-summary.md):
  accepted, exploratory.
- [Confirmation acceptance summary](../../../../bench_results/d06a99ed/worker-routing/d06a99ed-worker-routing-confirmation/acceptance-summary.md):
  accepted, does not qualify; one cell records `inconclusive`. The generated
  [outcome table](outcome.md) gives every cell's decision and outcome.

The [implementation record](../implementation.md), section *Before/after
measurement*, interprets the cells, including the dispatch-inclusive small
whole-call cells and the empty-body executor-dispatch diagnostic.
