# M4RI comparison and dense-parity outcome: pilot stage (jit:50f0bd42)

> **Diátaxis Type:** Research

This record covers the exploratory pilot and the frozen confirmation of the
M4RI [AlbrechtBard2026] comparator family, and the standing of the fusion
portfolio. It states no confirmatory verdict: the confirmation is a
[queue line](../bench-window/queue.tsv) for the
benchmark window.

## Pilot evidence

The [pilot receipt](../../../bench_results/2037941f/2037941f-dense-matvec-vs-m4ri/v4-r1-pilot/receipt.json)
holds every raw window of every paired execution, the arm executables and
their digests, host and toolchain observations, and the input snapshots under
its `inputs/` directory. Its
[acceptance summary](../../../bench_results/2037941f/2037941f-dense-matvec-vs-m4ri/v4-r1-pilot/acceptance-summary.md)
records an accepted pilot that does not qualify for production selection, with
its finding count under its Findings heading. The canonical evaluator
reproduces both summary files byte for byte from a copy of the receipt
directory.

The [execution log](../../../bench_results/2037941f/2037941f-dense-matvec-vs-m4ri/v4-r1-pilot/execution.log)
passes the shared [log verifier](../../../scripts/verify-campaign-log.py) against
the receipt and its [plan](../../../bench_results/2037941f/2037941f-dense-matvec-vs-m4ri/v4-r1-pilot/plan.json):
each declared cell starts, checkpoints and completes once with status
`measured` at its declared pair count, each resumed session closes with
`paused`, and the last closes with the single `complete` record. The
[launcher log](../../../bench_results/2037941f/2037941f-dense-matvec-vs-m4ri/v4-r1-pilot/launcher.log)
retains the exact invocation, the addendum, plan and executable digests, and
each session's exit status.

The generated [tables](../../../bench_results/2037941f/m4ri-gap-tables.md) carry
each cell's pair count, per-arm medians, speedup interval, outcome, and flagged
windows. Their *Selected paths* section is the runtime path evidence: the gf2
lane and stride each cell resolves, the M4RI fresh or retained route, and the
loaded shared object with its digest. Their *Conversion and setup costs*
section reports the construction each arm performs outside its windows. The
[frozen addendum](../../2037941f-profile-and-optimize-mid-range-buffer-operations/dense-parity-addendum.md#m4ri-charged-components)
fixes which of initialization, packing, product, unpacking, and disposal each
cell charges inside its timed call. The external build and its commands are
the [qualification record](../../2037941f-profile-and-optimize-mid-range-buffer-operations/m4ri-probe-record.txt)
and [build pins](../../2037941f-profile-and-optimize-mid-range-buffer-operations/survey/m4ri-build-pins.sh)
in the receipt's producing-input snapshot. The matrices are pinned by the
workload seeds of the [campaign addendum](../../2037941f-profile-and-optimize-mid-range-buffer-operations/campaigns/dense-matvec-vs-m4ri.json)
and the fixture source in the same snapshot.

## Semantic equivalence

The [release validation record](../../2037941f-profile-and-optimize-mid-range-buffer-operations/survey/dense-harness-validation.txt)
lists the exact-output oracle result for each qualified M4RI shape, including
the one-bit and word-boundary shapes, against the independent row-parity
oracle. The [confirmation smoke](confirmation-smoke.json) drives both arms of
both confirmatory cells through the shared runner's validation position with
no timing window, and records the route and loaded object each arm reports.

## Pilot observations

Every measured cell carries the exploratory role, decision `regressed`, and
outcome `pilot` in the acceptance summary. The compared arm is M4RI, so the
gf2 arm leads in each cell; the tables give each interval at its pair count.
The two operation boundaries stay separate:

- **Fresh whole-consumer cells.** Both arms enter with canonical gf2 words.
  The M4RI arm initializes, packs bit by bit, multiplies, unpacks, and
  disposes inside the timed call. These cells answer the comparator question.
- **Retained-state cells.** Both arms reuse their converted inputs; the M4RI
  arm still creates, unpacks, and disposes its output. This is the closest
  boundary to operation throughput the family measures. It is not a pure
  `mzd_mul` measurement and replaces no fresh cell.

The [unavailable-row companion](../../../bench_results/2037941f/2037941f-dense-matvec-vs-m4ri/v4-r1-pilot/unavailable-rows.tsv)
records each predeclared unqualified anchor shape as `unavailable` with zero
samples and zero comparisons. These shapes have no runner cell and no
measurement.

## Resolution rule and frozen confirmation

The generated [resolution record](../../../bench_results/2037941f/m4ri-gap-resolution.md)
applies the frozen rule: the largest relative half-width over the family's
confirmatory cells at the first confirmation's corrected alpha, rounded upward,
against the fixed family ceiling. It records `eligible`, and it recomputes the
attempt alpha and P-20's tail support from the ledger and the evaluator
sources.

The [confirmation addendum](../../2037941f-profile-and-optimize-mid-range-buffer-operations/campaigns/dense-matvec-vs-m4ri-confirmation.json)
is the canonical freezer's output for the
[freeze script](../../2037941f-profile-and-optimize-mid-range-buffer-operations/survey/freeze-m4ri-confirmation.sh)'s
arguments, and its
[derivation record](../../2037941f-profile-and-optimize-mid-range-buffer-operations/campaigns/dense-matvec-vs-m4ri-confirmation-derivation.txt)
sits beside it. It pins the pilot receipt by path and SHA-256, keeps the
frozen margins, and gives the two fresh mid-range cells the confirmatory role.
`dense-campaign verify-confirmation` holds it to the pilot transcription and
the family ceiling. The other qualified shapes and both retained-state cells
remain exploratory pilot evidence.

The confirmation runs fresh pairs under its own campaign identity on the
family's append-only [ledger](../../../bench_results/2037941f/dense-matvec-vs-m4ri-ledger.jsonl),
with the arm executables whose digests the pilot receipt and the smoke record
share. This record claims no confirmation receipt, table, or verdict.

## Fusion portfolio

The fusion portfolio has no candidate. The
[baseline and portfolio outcome](../c73ffa25/outcome.md) selects no distinct
in-bound fusion, so REQ-04 to REQ-07 have no eligible identity: there is no
candidate implementation, candidate receipt, annotated candidate assembly, or
new unsafe boundary. The fused AND-popcount route is the baseline, and the
generic carry-save comparator stays the preserved
[non-qualifying result](../../5cbb6545/findings.md). Neither is presented as new
optimization work.

## Decision status

A comparator gap authorizes no production change. The frozen addendum makes
the M4RI family an attribution question with a zero-line complexity budget,
and no M4RI outcome can select a production route. With no fusion candidate
and no adoption objective in the comparator family, the established fused
AND-popcount route remains selected and every losing and unavailable row stays
in its receipt directory. The decision record that REQ-08 to REQ-10 require
rests on confirmatory evidence only; this pilot-stage record supplies none.
