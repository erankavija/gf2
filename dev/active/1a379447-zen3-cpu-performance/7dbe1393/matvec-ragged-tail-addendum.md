# Matvec ragged-tail measurement addendum

> **Diátaxis Type:** Reference
>
> **Addendum identity:** `7dbe1393-matvec-ragged-tail-v1`
>
> **Owning issue:** `7dbe1393`
>
> **Frozen UTC:** `2026-10-08T11:39:41Z`

This addendum freezes one question, its cells, margins and decision rule:
whether a fused AND-parity kernel gives the allocated `BitMatrix::matvec` a
whole-consumer benefit over the fused AND-population-count kernel its SIMD
lane calls, at row strides that leave a partial 256-bit vector. It specializes
the [Zen 3 measurement contract](../measurement-contract.md) and
`zen3-benchmark-protocol` version 4 in [`protocol.md`](../f547c394/protocol.md)
with [`amendment-v4.md`](../f547c394/amendment-v4.md). A campaign JSON
addendum transcribes the family below into the version-4
[schema](../f547c394/addendum.schema.json); it may supply content pins and a
pilot-derived resolution, and it may not change a cell, margin, limit or rule.

The cost boundary, fixtures, cache policy, correctness oracle, sampling,
estimator and receipt contents are those of the
[dense-parity addendum](../2037941f-profile-and-optimize-mid-range-buffer-operations/dense-parity-addendum.md),
identity `2037941f-dense-parity-v2`, for its `allocated-matvec` question, as
implemented by the
[dense-parity harness](../2037941f-profile-and-optimize-mid-range-buffer-operations/dense-parity-harness.md).
This document states only what differs. Code claims are claims of this
issue's [source ledger](survey/source-evidence.json), cited by claim
identifier in backticks.

## Question and family

The lane-threshold family of `4337c02e` contrasts the SIMD lane with the
scalar lane; its rows are descriptive, and they place the scalar lane ahead at
the strides 9, 10, 62, 63 and 66 and the SIMD lane ahead at 32
([tables](../4337c02e/tables.md), sections *Pilot* and *Confirmation*). The
[instruction attribution](asm-attribution-tables.md) of this issue counts what
each lane executes per row at those strides, and the
[attribution](attribution.md) reads it: the SIMD lane's kernel counts every
vector and every tail word where the product needs one parity per row. The
candidate is a kernel that folds a row's intersection and counts once
(`parity-masked-tail`, `parity-fold`).

| Field | Value |
|---|---|
| Family identity | `7dbe1393-matvec-ragged-tail` |
| Purpose | `consumer-family` |
| Ledger | `dev/bench_results/7dbe1393/matvec-ragged-tail-ledger.jsonl`, an empty genesis file before the pilot |
| Holdout | none declared |

No earlier family asks this question on a ledger, so this family's first
confirmation is attempt 1.

## Arms and what they contrast

Both arms are one executable: the harness `dense-arm` built
`conservative-portable` with the `simd` feature. Every stride of the family is
at or above the conservative threshold, so the selector sends each to the SIMD
lane (`matvec-threshold-conservative`, `matvec-selector`).

| Arm | Timed call | Row kernel |
|---|---|---|
| baseline `matvec-a` | public `BitMatrix::matvec` | bundle `and_popcnt_fn`, low bit kept (`matvec-simd-kernel-call`) |
| candidate `matvec-lane-parity` | public `BitMatrix::matvec_parity_kernel` (`matvec-parity-entry`, `ragged-route-call`) | bundle `and_parity_fn` (`matvec-parity-kernel-call`, `parity-bundle-field`) |

A speedup above one favours the parity kernel (`ragged-cell-arms`). The
baseline is the selection production makes. The two lane bodies hold the same
row loop, bundle call and output appends; the listings
[`simd-matvec`](survey/asm/simd/simd-matvec.asm.txt) and
[`parity-matvec`](survey/asm/simd/parity-matvec.asm.txt) of the assembly record
show them, and the *Row body* column of the instruction attribution is equal
on the two lanes at every stride.

What a cell does not hold equal: the baseline enters through `matvec`, a
symbol of its own that compares the stride with the threshold, and the
candidate's entry is inlined into the arm. The *Entries* section of the
instruction attribution states the baseline entry's instructions per call; a
call covers 1024 rows.

## Cells

Every cell has 1024 rows, $64W$ columns, the `full` column shape, the `warm`
cache state, the `whole-consumer` metric with conversion and setup costs
included, `single-core-latency` scaling, the `single-core` arm and one worker.
One reported operation is one call including the allocation and the appends of
its returned `BitVec`. The multi-core arms are inapplicable for the reason the
dense-parity addendum states.

| Cell | $W$ | Words past the last whole vector | Objective | Role in the confirmation |
|---|---:|---:|---|---|
| `ragged-r1024-9w-warm` | 9 | 1 | `improvement` | confirmatory |
| `ragged-r1024-10w-warm` | 10 | 2 | `improvement` | confirmatory |
| `ragged-r1024-62w-warm` | 62 | 2 | `improvement` | confirmatory |
| `ragged-r1024-63w-warm` | 63 | 3 | `improvement` | confirmatory |
| `ragged-r1024-66w-warm` | 66 | 2 | `improvement` | confirmatory |
| `ragged-r1024-32w-control-warm` | 32 | 0 | `non-regression` | confirmatory |

The five ragged strides are those the issue names. The control is a whole
number of vectors, where the baseline's kernel runs no tail and the
lane-threshold rows place the SIMD lane ahead of the scalar lane: a candidate
that regresses an aligned stride fails there.

The family holds one cache state. Acceptance verifies a declared resolution
against the widest half-width of the whole pinned pilot, so a cold or
streaming cell would set the resolution of every confirmatory cell
([outcome of `4337c02e`](../4337c02e/outcome.md)).

The cells follow the lane-threshold cells in the harness's single cell order,
in the order listed here, and take their workload seeds from the same
SplitMix64 stream at their ordinals. Earlier cells keep their ordinals and
seeds.

## Sampling and comparisons

Every pilot cell runs the protocol's pilot maximum of paired executions and
every confirmatory cell the protocol's confirmatory count, on fresh pairs. No
pilot pair enters the confirmation.

The confirmation reserves $m=6$ comparisons at attempt $t=1$. With the
protocol's shared settings, $\alpha_1 = 0.05/(1\cdot 2) = 0.025$
(`attempt-alpha`) and $\alpha_c = 0.025/6$, so each bootstrap tail holds
$10000\,\alpha_c/2 = 20.8\overline{3}$ expected draws, above the twenty P-20
requires (`p20-tail-rule`); a seventh cell would fall below it. The harness
contract test
`the_ragged_tail_family_contrasts_the_parity_kernel_with_the_simd_lane`
recomputes this from `SHARED_SETTINGS`. A second confirmation is excluded by
the one-attempt rule and by the same floor.

## Margins and resolution

| Family | Worthwhile speedup | Equivalence margin | Material gap | Resolution ceiling | One plus ceiling | Complexity budget | Pilot trials per cell |
|---|---:|---:|---:|---:|---:|---|---:|
| ragged tail | 1.08 | 1.05 | none | 0.035 | 1.035 | 1 unsafe kernel, 96 added source lines | 2 |

- The worthwhile speedup is the dense-parity addendum's bar for the same
  allocated consumer.
- The equivalence margin is the finest non-inferiority statement that addendum
  finds available at this consumer's resolution.
- The resolution ceiling is the dense-parity allocated family's, for the same
  boundary (`ragged-resolution-ceiling`).
- The family holds no comparator-gap cell, so it declares no material gap.
- The complexity budget is the dense-parity addendum's for one candidate
  kernel of this consumer.
- The pilot budget covers the pilot and one replacement for a pilot voided
  under the protocol's voided-attempt rule.

The pilot-derived resolution $r$ is the largest relative bootstrap half-width
over every cell of the pinned pilot at $\alpha_c$, as the canonical freezer
computes and floors it, pinned with the pilot receipt and digest. Every pilot
cell is a confirmatory cell, so the freezer's scope and the scope acceptance
verifies are the same six cells. If $r$ exceeds the ceiling the family records
`resolution-insufficient`, runs no confirmation, and production keeps its
kernel. Both margins strictly exceed one plus the ceiling: $1.08 > 1.035$ and
$1.05 > 1.035$.

## Decision rule

The rule reads the confirmation's acceptance summary and nothing else.

1. **Validity.** The summary's verdict is `accepted` and it qualifies the
   receipt. Otherwise the outcome is *no change*.
2. **Non-regression.** The control cell's outcome is `pass`. Otherwise the
   outcome is *no change*.
3. **Improvement.** Every one of the five ragged cells has the outcome
   `pass`. Otherwise the outcome is *no change*.
4. **Adoption.** With all three satisfied the outcome is *adopt*: the SIMD
   lane of `matvec` calls the parity kernel at every stride it serves.

The rule adopts on all five ragged cells because the lane is one body at
every stride at or above the threshold: a kernel that passes at some ragged
strides and not at others leaves the lane's choice to a stride rule this
family does not measure. Every cell stays in the receipt with its recorded
outcome whatever the rule yields.

*Adopt* changes the kernel the SIMD lane calls and no selector. The value of
`bit_matrix.matvec_simd_min_words` belongs to `63bad95d`; this family measures
no stride below the conservative threshold and no stride other than its six,
and states that limit.

## Search and stopping

One pilot of all cells, then one confirmation of the same six cells on the
same sources. No adaptive extension, no early stop, no reuse of pilot pairs.
An `inconclusive`, `unstable` or `not-confirmatory` cell is terminal for this
family and counts as not passing in the rule.
