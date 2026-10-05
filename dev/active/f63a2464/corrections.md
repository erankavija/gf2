# Corrections to the frozen instruments of f63a2464

> **Diátaxis Type:** Reference

The [decision record](decision-record.md) above its section "Decisions after the
confirmations", the [numerical-contract review](numerical-contract-review.md)
and the [predeclared quality tolerances](quality-tolerances.md) are frozen
instruments, committed before the evidence they govern, and keep their original
bytes. This record preserves each statement in them that the committed evidence
contradicts or does not support, quotes it, states what the evidence
establishes, and gives the reading the other documents of this issue use.

No correction changes a budget, a stop rule, the exploratory screen, the
bit-exactness rule or the value `fer_ratio_max`. Entries C-01 to C-08 state why
the decisions of [findings.md](findings.md) do not depend on the corrected
statement; C-09 records an execution that contradicts the frozen budget and the
decision that follows from it.

## C-01: the fastest quality-compatible external arms

Decision record, "Family L", External arms:

> Its f32 and `i16` INTER modes are the fastest quality-compatible arms in the
> same schedule.

**Contradicted.** `c077a88b` times six AFF3CT [Cassagne2019] horizontal-layered
normalized-min-sum candidates, the f32 scalar, f32 INTER and `i16` INTER modes
on each code, in one exploratory pilot against the gf2 decoder of that issue.
Every one of the six carries the outcome "quality admission unestablished
(P-19)" ([`c077a88b` tables](../../bench_results/c077a88b/tables.md),
"Quality-compatible timing"), and its
[findings](../c077a88b/findings.md) state that the eligible shortlist is empty
and that no quality confirmation or fastest-candidate selection is made. No
external arm is established as quality-compatible, and none as the fastest such
arm.

**Reading used.** The INTER modes are surveyed candidates with exploratory
timings and no quality admission. REQ-04's fastest quality-compatible comparison
has no admitted arm on this corpus; this issue makes no such comparison and
reports its absence rather than a result.

**Decisions.** Unaffected: family L stops at S2 on the exploratory screen before
any timed cell, and family QC's matched cells use the flooding f32 arm.

## C-02: what `c077a88b` measures of the layered scalar arm

Decision record, "Family L", External arms, and the review's ruling "L
proceeds":

> AFF3CT's horizontal-layered normalized-min-sum f32 scalar mode
> [Cassagne2019] is the matched-algorithm arm once gf2 declares the same layered
> contract, and `c077a88b` already pins and measures it.

> `c077a88b` already pins and measures the matched external layered arm it
> needs.

**Narrowed.** `c077a88b` pins that mode's build, times it as one of the six
unadmitted candidates of C-01 and records its per-frame quality on the frozen
bundles. Its matched-algorithm family, pilot and confirmation, is the flooding
f32 arm alone. That the layered scalar mode is a matched arm for this issue's
layered prototype is this issue's own result: the prototype's per-frame error
vectors equal that mode's prepared record on both codes
([quality tables](../../bench_results/f63a2464/quality/tables.md), "External
conformance under each declared contract").

**Decisions.** Unaffected: no timed layered cell exists under this issue.

## C-03: the measured quantized arm

Decision record, "Family Q", External arms:

> The measured quantized arm available on this host is AFF3CT's
> horizontal-layered normalized-min-sum `i16` INTER mode [Cassagne2019], already
> pinned and measured by `c077a88b`

**Narrowed.** That mode is one of the unadmitted candidates of C-01. The
`c077a88b` prepared quality also carries untimed AFF3CT flooding `i16` records
for both codes, and those are the records this issue's conformance table
compares the wide-alphabet flooding arms with (quality tables, "External
conformance under each declared contract").

**Decisions.** Unaffected: family Q stops at S2.

## C-04: the layered schedule's iteration benefit

Decision record, "Family L", Consumer benefit:

> A layered schedule converges in fewer iterations for the same frame error
> rate, so the benefit is throughput through iteration count at equal quality.

**Unsupported as a general claim.** No registry key resolves a source for it,
which findings.md records under Limits, and this issue commits no artifact that
establishes it beyond the measured cells.

**Reading used.** It is the hypothesis family L is evaluated under. The scoped
observation is the iteration column of the quality tables ("Quality on the
measured cells"): on each recorded cell the layered arm's count of layer sweeps
and the canonical arm's count of flooding iterations, over the stated frames,
with the frame error counts and intervals of both arms beside them. The tables
do not compare the two units, and the corpus certifies no equality of frame
error rate (quality tables, "Paired non-inferiority against the canonical arm").

**Decisions.** Unaffected: the screen that stops family L reads frame and bit
errors only.

## C-05: the tolerance rationale's decibel margin

Quality tolerances, "The certification rule", Rationale:

> A tenth more frame errors is within the spread a tenth of a decibel of
> implementation margin covers on the waterfall of both measured codes

**Withdrawn.** No committed measurement, derivation or registry key supports a
relation between a decibel margin and a frame-error ratio on these codes, and
the recorded bundles hold one operating point per code, which cannot supply one.

**Reading used.** `fer_ratio_max = 1.10` stands as predeclared, as a declared
engineering choice whose remaining rationale is the rest of the frozen
paragraph: the benefit on offer is throughput or latency at an unchanged link
budget, and the ratio is small enough that a candidate decoding materially fewer
frames cannot pass. The value, the paired bound and the confidence are
unchanged.

**Decisions.** Unaffected: the certification rule certifies nothing on any cell
whose canonical arm decodes nearly all frames, whatever its rationale, and no
family's decision rests on a certification.

## C-06: the levers of `3be770d5`

Decision record, "The question":

> `3be770d5`'s lever ranking carries three levers with no gf2 mechanism and only
> a comparator or labelled estimate behind them: quantized and layered decoding,
> and QC-aware intra-frame work.

**Corrected count.** The ranking of
[`3be770d5`](../3be770d5/findings.md) carries two such rows relevant here,
"Quantized/layered decoding" on a comparator estimate and "QC-aware intra-frame
work" on a labelled estimate. This issue splits the first into families Q and L.

Review, ruling on QC:

> the check-node update — the dominant named decoder work in `3be770d5`'s lever
> ranking after `07ca8585`

**Withdrawn.** The cited ranking ranks levers by removed profile share and
states no such dominance. The DVB-T2 ruling does not need it: it rests on the
absence of an equal-degree check-block partition, which the review's source
evidence carries.

## C-07: the DVB-T2 arm of family QC

Decision record, "Family QC", Applicability:

> the periodic part admits the same treatment and the staircase does not, so a
> DVB-T2 arm is a partial application and is declared as one, not dropped.

**Contradicted by the review.** The numerical-contract review rules that DVB-T2
has no equal-degree check-block partition for the check update the candidate
vectorizes, and declares the DVB-T2 QC cell unavailable with that reason. No
partial DVB-T2 arm exists; family QC's addenda declare no DVB-T2 cell.

**Decisions.** Recorded in findings.md under Feasibility and Limits; family QC's
evidence covers the NR workload only.

## C-08: single-frame latency under inter-frame batching

Decision record, "Family QC", Consumer benefit:

> Single-frame latency, which inter-frame batching cannot improve

**Unmeasured premise.** No artifact of this issue measures an inter-frame
batched decoder; that candidate is `ed3d490e`'s subject. The frozen
single-worker addenda repeat the phrase in their family descriptions.

**Reading used.** Single-frame latency is the axis family QC targets and
measures. Nothing here states what inter-frame batching achieves on it.

**Decisions.** Unaffected: the latency cells compare the candidate with the
canonical decoder.

## C-09: three confirmatory campaigns against a cap of one

Decision record, "Confirmatory stage, per family":

> | Timed pilot campaigns | at most 1 |

> | Confirmatory campaigns | at most 1, the protocol's per-candidate cap |

> | Attempts after a non-passing outcome | none |

The frozen confirmation addenda of family QC, in each family description:

> Confirmatory stage: every cell is confirmatory and decides this family on
> fresh samples

**Contradicted by the execution.** The decision record names three candidate
families, Q, L and QC, and caps each at one confirmatory campaign. Family QC ran
three confirmatory campaigns, one on each of its ledgers (single-worker,
multicore and comparator), each frozen and described as deciding "this family".
The reading that takes the cap per ledger is written after the budget is frozen.
It is no explicit amended version of the budget and is followed by no fresh
confirmation, which the measurement contract requires of a change to
experimental rules. The three campaigns therefore exceed the frozen cap. The
family's three pilot campaigns, one per ledger, exceed the exploratory cap of
one timed pilot campaign in the same way.

**Reading used.** Family QC has no valid confirmatory verdict under the frozen
budget (repository owner's ruling, 2026-10-05). The three receipts, their ledger
reservations and their frozen addenda keep their bytes and stay committed as
run. Their recorded outcomes, including each receipt's `qualifies` field and
the comparator cells' `fail`, are reported exactly as the evaluator records
them and decide nothing.

**Decisions.** Family QC does not proceed, and with families Q and L stopped at
S2 no family proceeds. The frozen record allows no further confirmatory attempt
for family QC under this issue.

## C-10: "proceeds" in the numerical-contract review

Review, "Rulings":

> **Q proceeds, in both widths, without a matched external `i8` arm.**

> **L proceeds.**

> **QC proceeds on the NR workload and is unavailable on DVB-T2, with the
> reason.**

**Scoped.** These rulings are REQ-03's: each family passes the contract and
feasibility review and goes on to a prototype. They are written before any
prototype and are no proceed decision under REQ-06. The measured decisions are
the decision record's table "Decisions after the confirmations": no family
proceeds.
