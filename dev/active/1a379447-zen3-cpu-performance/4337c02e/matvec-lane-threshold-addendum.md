# Matvec lane-threshold measurement addendum

> **Diátaxis Type:** Reference
>
> **Addendum identity:** `4337c02e-matvec-lane-threshold-v1`
>
> **Owning issue:** `4337c02e`
>
> **Frozen UTC:** `2026-10-05T19:34:54Z`

This addendum freezes one question, its cells, margins and resolution rule:
at which row strides the scalar lane of the public `BitMatrix::matvec` gives a
whole-consumer benefit over the SIMD lane the selector takes. It specializes
the [Zen 3 measurement contract](../measurement-contract.md) and
`zen3-benchmark-protocol` version 4 in
[`protocol.md`](../../f547c394/protocol.md) with
[`amendment-v4.md`](../../f547c394/amendment-v4.md). A campaign JSON addendum
transcribes the family below into the version-4
[schema](../../f547c394/addendum.schema.json); it may supply content pins and
a pilot-derived resolution, and it may not change a cell, margin, limit or
rule.

The cost boundary, fixtures, cache policies, arms, correctness oracle,
sampling, estimator and receipt contents are those of the
[dense-parity addendum](../../2037941f-profile-and-optimize-mid-range-buffer-operations/dense-parity-addendum.md),
identity `2037941f-dense-parity-v2`, for its `allocated-matvec` question, as
implemented by the
[dense-parity harness](../../2037941f-profile-and-optimize-mid-range-buffer-operations/dense-parity-harness.md).
This document states only what differs. Code claims are claims of the
[source ledger of `63bad95d`](../63bad95d/survey/source-evidence.json), cited
by claim identifier in backticks.

## Question and family

The selector sends a stride at or above the conservative threshold to the SIMD
lane and every smaller stride to the scalar lane (`matvec-threshold-read`,
`matvec-threshold-conservative`). The dense-parity pilot holds exploratory
rows that contrast the two lanes at five strides: rows
`matvec-r1024-{8,9,63,64,65}w-scalar-reference-warm` of
[the mid-range tables](../../2037941f-profile-and-optimize-mid-range-buffer-operations/mid-range-tables.md),
section `v4-r1-2037941f-dense-allocated-matvec`. Those rows take the scalar
lane from a second executable built without the `simd` feature. Exploratory
rows select nothing, and that addendum keeps its scalar-reference cells
exploratory and forbids a threshold edit, so the question needs its own
family.

| Field | Value |
|---|---|
| Family identity | `4337c02e-matvec-simd-threshold` |
| Purpose | `consumer-family` |
| Ledger | `dev/bench_results/4337c02e/matvec-simd-threshold-ledger.jsonl`, created as an empty genesis file before the pilot |
| Holdout | none declared |

The family measures the lane contrast and yields a threshold value. It
changes no selector. Installing a value, measuring the profile build that
carries it and the holdout strides belong to the selector-calibration
families of `63bad95d`
([calibration plan](../63bad95d/calibration-plan.md), overlap and adoption
sections). This family declares no stride that the
[holdout declaration](../63bad95d/survey/matvec-holdout-cells.json) of
`63bad95d` reserves; the generated [cell table](cell-table.md) checks it.

No earlier family asks this question on a ledger: the scalar-reference rows
above are exploratory cells of `2037941f-dense-allocated-matvec`, whose
ledger carries no spending reservation
([budget record of `63bad95d`](../63bad95d/calibration-budget.md), ledger
table). This family's first confirmation is therefore attempt 1.

## Arms and what they contrast

Both arms are one executable: the harness `dense-arm` built
`conservative-portable` with the `simd` feature.

| Arm | Timed call | Lane at a stride of eight words or more | Lane below eight words |
|---|---|---|---|
| baseline `matvec-a` | public `BitMatrix::matvec` | SIMD (fused AND-population-count, `matvec-fused-kernel`) | scalar |
| candidate `matvec-lane-scalar` | public `BitMatrix::matvec_with_route` with `MatvecRoute::Scalar` | scalar | scalar |

A speedup above one favours the scalar lane. The baseline is the selection
production makes today. The candidate runs the scalar lane a raised threshold
makes production run at that stride, so each cell is a before and after cell
of the changed selection on its stride.

[`survey/lane-call-sites.txt`](survey/lane-call-sites.txt), written by
[`survey/record-lane-call-sites.sh`](survey/record-lane-call-sites.sh) from
the arm executable, lists the lane symbols and their callers: `matvec` and
`matvec_with_route` call one `matvec_scalar` symbol, so the candidate executes
the instructions production executes on the scalar lane, by identity of the
symbol. The assembly record of `63bad95d`
([`matvec-asm-comparison.json`](../63bad95d/survey/matvec-asm-comparison.json))
compares the entry's listings before and after `matvec_with_route`.

What a cell does not include: the candidate enters through
`matvec_with_route`, which matches on the route argument, where production
compares the stride with the threshold inside `matvec`. The lane argument
passes an optimisation barrier, so the match stays in the measured call. The
exploratory control cell, at a stride the selector sends to the scalar lane,
measures that entry difference with the lane held equal.

## Cells

Every cell has 1024 rows, $64W$ columns, the `full` column shape, the
`whole-consumer` metric with conversion and setup costs included,
`single-core-latency` scaling, the `single-core` arm and one worker. One
reported operation is one call including the allocation and the appends of
its returned `BitVec`. The multi-core arms are inapplicable for the reason the
dense-parity addendum states.

Confirmatory cells, all `warm`, all with the `improvement` objective:

| Cell | $W$ | Why it is declared | Role in the confirmation |
|---|---:|---|---|
| `lane-r1024-8w-warm` | 8 | exploratory row favours the scalar lane; the present threshold | confirmatory |
| `lane-r1024-9w-warm` | 9 | exploratory row favours the scalar lane | confirmatory |
| `lane-r1024-32w-warm` | 32 | interior stride: a value of 64 or more rests on it | confirmatory |
| `lane-r1024-63w-warm` | 63 | exploratory row favours the scalar lane | confirmatory |
| `lane-r1024-64w-warm` | 64 | neighbour; exploratory row does not favour the scalar lane | confirmatory |
| `lane-r1024-65w-warm` | 65 | neighbour; exploratory row is inconclusive | confirmatory |

The sixth cell is the 32-word stride because the exploratory rows place the
end of the scalar lane's lead between 63 and 64 words, and a threshold there
moves every stride from 8 to 63. With cells at 8, 9 and 63 only, that value
would rest on the two ends of the interval; the 32-word cell puts one
confirmatory cell inside it.

Exploratory cells, measured in the pilot only:

- `lane-r1024-7w-control-warm`, a `non-regression` cell: both arms run the
  scalar lane, so it shows the entry difference stated above;
- `lane-r1024-{10,62,66}w-warm`: neighbours of the lane strides;
- `lane-r1024-{8,9,63,64,65}w-streaming`;
- `lane-r1024-8w-cold` and `lane-r1024-64w-cold`, with `cold_calls` one.

The cells follow the dense-parity cells in the harness's single cell order,
in the order listed here, and take their workload seeds from the same
SplitMix64 stream at their ordinals. The dense-parity cells keep their
ordinals and seeds.

## Sampling and comparisons

Every exploratory cell runs the protocol's pilot maximum of paired executions
and every confirmatory cell the protocol's confirmatory count, on fresh
pairs. No pilot pair enters the confirmation.

The confirmation reserves $m=6$ comparisons at attempt $t=1$. With the
protocol's shared settings, $\alpha_1 = 0.05/(1\cdot 2) = 0.025$ and
$\alpha_c = 0.025/6$, so each bootstrap tail holds
$10000\,\alpha_c/2 = 20.8\overline{3}$ expected draws, above the twenty P-20
requires; a seventh cell would fall below it. The harness contract test
`the_lane_threshold_family_contrasts_the_two_lanes_at_its_frozen_strides`
recomputes this from `SHARED_SETTINGS`. A second confirmation is excluded by
the one-attempt rule and by the same floor.

## Margins and resolution

| Family | Worthwhile speedup | Equivalence margin | Material gap | Resolution ceiling | One plus ceiling | Complexity budget | Pilot trials per cell |
|---|---:|---:|---:|---:|---:|---|---:|
| lane threshold | 1.08 | 1.05 | none | 0.035 | 1.035 | 0 unsafe kernels, 0 added source lines | 2 |

- The worthwhile speedup is the dense-parity addendum's bar for the same
  allocated consumer.
- The equivalence margin is the finest non-inferiority statement that addendum
  finds available at this consumer's resolution.
- The resolution ceiling is the dense-parity allocated family's, for the same
  boundary.
- The family holds no comparator-gap cell, so it declares no material gap.
- The outcome is one value of an existing selector, so the complexity budget
  is zero.
- The pilot budget covers the pilot and one replacement for a pilot voided
  under the protocol's voided-attempt rule.

The pilot-derived resolution $r$ is the largest relative bootstrap half-width
over the six confirmatory cells at $\alpha_c$, rounded upward to the next
0.001 and pinned with the pilot receipt and digest by the canonical freezer.
If $r$ exceeds the ceiling the family records `resolution-insufficient`, runs
no confirmation, and the threshold stays unchanged. Both margins strictly
exceed one plus the ceiling: $1.08 > 1.035$ and $1.05 > 1.035$.

## Resolution rule

The rule reads the confirmation's acceptance summary and nothing else. Let
the lane strides in ascending order be
$s_1,\dots,s_6 = 8, 9, 32, 63, 64, 65$.

1. **Validity.** The summary's verdict is `accepted`. Otherwise the outcome
   is *no change*.
2. **Leading run.** Let $k$ be the number of leading lane strides whose cell
   outcome is `pass`: $s_1,\dots,s_k$ all pass and $s_{k+1}$ does not, or
   $k=6$.
3. **Value.** If $k=0$ the outcome is *no change* and the threshold stays at
   its conservative value. Otherwise the confirmed threshold value is
   $s_k + 1$: one of 9, 10, 33, 64, 65 or 66.

A lane cell that passes after the leading run ends does not move the value:
the selector is a single boundary, and a stride set that is not one interval
from the present threshold keeps the smaller value. Every cell stays in the
receipt with its recorded outcome whatever the rule yields.

A value above 10 places strides on the scalar lane that no confirmatory cell
of this family measures: 10 to 31 for the value 33, and also 33 to 62 for a
value of 64 or more. This family states that limit and does not close it: the
holdout strides of `63bad95d` lie in those intervals and decide adoption
there.

[`survey/make-outcome.py`](survey/make-outcome.py) applies the rule and writes
the outcome record of this issue: the value or *no change*, each cell's
outcome as the summary records it, and the step that decided. That record
hands the value to `63bad95d`.

## Search and stopping

One pilot of all cells, then one confirmation of the six confirmatory cells
on the same sources. No adaptive extension, no early stop, no
reuse of pilot pairs. An `inconclusive`, `unstable` or `not-confirmatory`
cell is terminal for this family and counts as not passing in the rule.
