# M4RI comparison and dense-parity outcome (jit:50f0bd42)

> **Diátaxis Type:** Research

**Decision: no adoption.** The established fused AND-popcount route remains
the production route. This is the single dense-parity decision record. It
rests on the confirmation of the M4RI [AlbrechtBard2026] comparator family and
on the empty fusion portfolio; the exploratory pilot sizes the resolution and
decides nothing.

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

Every measured pilot cell carries the exploratory role, decision `regressed`, and
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

## Resolution rule and frozen confirmation addendum

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

## Confirmation evidence

The [confirmation receipt](../../../bench_results/2037941f/2037941f-dense-matvec-vs-m4ri/v4-r1-confirmation/receipt.json)
holds fresh pairs under its own campaign identity, with the arm executables
whose digests the pilot receipt and the smoke record share. Its reservation is
the family's one confirmatory line in the append-only
[ledger](../../../bench_results/2037941f/dense-matvec-vs-m4ri-ledger.jsonl).
The [execution log](../../../bench_results/2037941f/2037941f-dense-matvec-vs-m4ri/v4-r1-confirmation/execution.log)
passes the shared log verifier against the receipt and its
[plan](../../../bench_results/2037941f/2037941f-dense-matvec-vs-m4ri/v4-r1-confirmation/plan.json):
each confirmatory cell starts, checkpoints and completes once with status
`measured` at the protocol's confirmatory pair count, in one session closed by
`complete`. The canonical evaluator reproduces the
[acceptance summary](../../../bench_results/2037941f/2037941f-dense-matvec-vs-m4ri/v4-r1-confirmation/acceptance-summary.md)
byte for byte from a copy of the receipt directory. The summary is accepted,
does not qualify for production selection, and carries no P-20 note on either
cell. The receipt's
[resolution-evidence snapshot](../../../bench_results/2037941f/2037941f-dense-matvec-vs-m4ri/v4-r1-confirmation/inputs/resolution-evidence/receipt.json)
is the pinned pilot, and its
[unavailable-row companion](../../../bench_results/2037941f/2037941f-dense-matvec-vs-m4ri/v4-r1-confirmation/unavailable-rows.tsv)
repeats the predeclared unqualified shapes at zero samples and zero
comparisons. The confirmation section of the generated
[tables](../../../bench_results/2037941f/m4ri-gap-tables.md)
carries each cell's interval, pair count, selected paths, and setup costs.

## Decision

The decision uses the confirmation receipt only. The
[frozen addendum](../../2037941f-profile-and-optimize-mid-range-buffer-operations/dense-parity-addendum.md#estimator-confidence-and-multiple-comparisons)
fixes the rule: each cell's interval is taken at the corrected alpha the
ledger's attempt and the family's reserved comparisons give, and is compared
with the family's equivalence margin and material-gap threshold. The summary's
family line states that alpha and confidence.

| Confirmatory cell | Recorded decision | Recorded outcome | Reading under the frozen rule |
|---|---|---|---|
| `m4ri-gap-65x512-warm` | `regressed` | `fail` | The interval's upper bound lies below the reciprocal equivalence margin: the M4RI whole consumer is slower than gf2 beyond equivalence, and no material external gap exists. |
| `m4ri-gap-65x4096-warm` | `regressed` | `fail` | The same reading. |

Both comparisons fail the comparator-gap objective in gf2's favour. The
wider cell's confirmation half-width in the tables exceeds the pilot-derived
resolution; it stays below the family ceiling, the evaluator's endpoint check
passes, and the interval lies far from both margins, so the reading does not
depend on it. The comparison holds for the fresh whole-consumer boundary,
where bit-wise packing and unpacking are charged to the M4RI arm. It makes no
claim about isolated `mzd_mul` throughput.

1. **Eligible candidates.** None. The fusion portfolio is empty, and the
   comparator family has no adoption objective.
2. **Production change.** None selected. A comparator outcome authorizes no
   production change in either direction, and the complexity budget of the
   comparator family admits no production line.
3. **Preserved evidence.** Both failing confirmatory rows, every exploratory
   pilot row, and the unavailable companions of both receipts stay committed.
4. **Prior evidence.** The fused AND-popcount route is the retained baseline,
   and the generic carry-save comparator stays the preserved
   [non-qualifying result](../../5cbb6545/findings.md). Neither is presented
   as new optimization work.

## Fusion portfolio

The fusion portfolio has no candidate. The
[baseline and portfolio outcome](../c73ffa25/outcome.md) selects no distinct
in-bound fusion, so REQ-04 to REQ-07 have no eligible identity: there is no
candidate implementation, candidate receipt, annotated candidate assembly, or
new unsafe boundary.

## Criterion disposition

- **REQ-01:** Both receipts hold raw samples, commands, pinned inputs, runtime
  paths, and the cost boundaries described under *Pilot evidence*.
- **REQ-02:** *Semantic equivalence* covers the frozen dimensions and boundary
  shapes; fresh whole-consumer and retained-state boundaries stay separate.
- **REQ-03:** Losing and unavailable rows are committed and adopt nothing.
- **REQ-04 to REQ-07:** No eligible identity; see *Fusion portfolio*.
- **REQ-08 to REQ-10:** *Decision* applies the frozen rules to confirmatory
  evidence and selects no adoption.
