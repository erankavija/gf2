# Candidate decision record: quantized, layered and QC-aware LDPC

> **Diátaxis Type:** Explanation

REQ-02's required decision record for `f63a2464`. It names the three candidate
families the checkpoint evaluates and freezes their exploratory and confirmatory
search and stop budget. It is committed before any prototype of any family
exists, so no budget below is chosen with a prototype result in view.

The [measurement contract](../1a379447-zen3-cpu-performance/measurement-contract.md)
and the [frozen protocol](../f547c394/protocol.md) with its
[version-4 amendment](../f547c394/amendment-v4.md) govern every timed cell.
Prior evidence this record builds on is the decoder comparison of
[`c077a88b`](../c077a88b/findings.md), the throughput-gap profile and lever
ranking of [`3be770d5`](../3be770d5/findings.md), and the allocation-free update
of [`07ca8585`](../07ca8585/findings.md). This record states no measured value:
every figure it rests on stays in those reports' generated tables and receipts.
Code claims are in [source-evidence.json](survey/source-evidence.json), written
by [make-source-evidence.py](survey/make-source-evidence.py), which refuses a
claim whose fragment does not locate exactly one line.

The canonical decoder is `LdpcDecoder` in `gf2-coding`, an f32 flooding min-sum
family decoder over the canonical `EdgeLayout` with syndrome early termination.
It is the baseline of every comparison here and remains the production decoder
unless a family qualifies under the rules below.

## The question

`3be770d5`'s lever ranking carries three levers with no gf2 mechanism and only a
comparator or labelled estimate behind them: quantized and layered decoding, and
QC-aware intra-frame work. This checkpoint decides each of them on measured
evidence rather than leaving the estimate standing. "No qualifying candidate" is
a valid outcome for every family; not evaluating a family is not.

## Family Q: quantized min-sum with saturation and scaling

**What changes.** The message alphabet becomes `i8` or `i16`. A channel LLR
enters as `round(scale * llr)` clipped to the type's signed range; variable-node
accumulation saturates instead of wrapping; the check-node reduction keeps the
min/second-min/sign structure over magnitudes of the integer type.

**What stays.** The flooding schedule, the edge layout, the syndrome stopping
rule and the decoder's public interface.

**Numerical status.** A changed numerical contract. The float contract's
signed-zero and NaN rules have no image in a two's-complement alphabet, and
saturation is not a rounding of the float result, so bit-exactness against the
canonical decoder is neither claimed nor testable. Quality is evidence, under
REQ-05's predeclared tolerance.

**External arms.** The measured quantized arm available on this host is AFF3CT's
horizontal-layered normalized-min-sum `i16` INTER mode [Cassagne2019], already
pinned and measured by `c077a88b`, whose capability screen also records that the
pinned C++11 build rejects `i8` for that mode. srsRAN [Srsran2026] decodes with
`int8` messages and OpenAirInterface [OpenAirInterface2026] with clipped `int8`;
`c077a88b` preserves the build failure of each on this host, so neither is a
measurable arm and neither is substituted by a different operation. xdsopl
[Xdsopl2026] exposes min-sum, offset min-sum and corrected min-sum rather than
the normalized rule, so it enters only as a quality-compatible arm, never a
matched one.

**Consumer benefit.** Two `i16` messages or four `i8` messages occupy one f32
lane, so the same AVX2 register width carries two or four times the edges. The
benefit is throughput at equal or acceptable frame error rate; it is not a
correctness improvement, and a quantized decoder that loses frames the float
decoder decodes is not adopted whatever it measures.

## Family L: layered min-sum schedules

**What changes.** The flooding schedule becomes a horizontal (row) layered
schedule: checks are processed in layers, and each layer's outgoing messages are
computed from posteriors that already carry the preceding layers' updates of the
same iteration.

**What stays.** The f32 alphabet, the min-sum rules and their scaling, the edge
layout and the syndrome stopping rule.

**Numerical status.** A changed numerical contract. A layered schedule reaches a
different message state at the end of an iteration, and its iteration counts are
not comparable with flooding counts under the protocol's rule against comparing
unadjusted iteration counts across stopping contracts. Bit-exactness against the
canonical decoder is not claimed.

**External arms.** AFF3CT's horizontal-layered normalized-min-sum f32 scalar mode
[Cassagne2019] is the matched-algorithm arm once gf2 declares the same layered
contract, and `c077a88b` already pins and measures it. Its f32 and `i16` INTER
modes are the fastest quality-compatible arms in the same schedule.

**Consumer benefit.** A layered schedule converges in fewer iterations for the
same frame error rate, so the benefit is throughput through iteration count at
equal quality. Per-iteration cost may rise, and the decision is on the product,
measured, not on the iteration count alone.

## Family QC: QC-aware intra-frame vectorization

**What changes.** The update is vectorized across the lifted positions of one
circulant block inside a single frame, instead of across frames. For the NR BG1
mother code the lifting size is the vector length available to one frame's
update, and a base-graph edge's QC rotation is a cyclic shift of that block's
message vector. This is the alternative REQ-02 requires the checkpoint to weigh
against the inter-frame batching of `ed3d490e`.

**What stays.** The f32 alphabet, the flooding schedule, the min-sum rules, the
syndrome stopping rule and, for a check-node reduction performed in the canonical
order, the numerical contract itself.

**Numerical status.** Unchanged, conditionally. Vectorizing across lifted
positions reassociates nothing inside a check, because each lane is a distinct
check of the lifted code; the variable-node accumulation order over a variable's
edges is the property that must be preserved, and it is preserved when lanes
carry lifted positions rather than edges of one node. Under that condition the
candidate is bit-exact against the canonical decoder, and the claim is tested
rather than asserted.

**Applicability.** The NR BG1 workload is quasi-cyclic by construction
[ThreeGpp2017]. The DVB-T2 workload is an IRA code whose information part has a
360-periodic address structure and whose parity part is a staircase
[Etsi2015]; the periodic part admits the same treatment and the staircase does
not, so a DVB-T2 arm is a partial application and is declared as one, not
dropped.

**Consumer benefit.** Single-frame latency, which inter-frame batching cannot
improve: a QC-aware intra-frame decoder vectorizes one frame, so it carries no
batch-fill cost and no batch-formation delay. Throughput is the second axis.

## The frozen search and stop budget

Frozen for all three families before any prototype exists.

### Exploratory stage, per family

| Item | Budget |
|---|---|
| Prototype configurations screened untimed | at most 8 |
| Timed pilot campaigns | at most 1 |
| Cells in that pilot | at most 6 |
| Pilot trials per cell | the protocol's `max_pilot_trials_per_cell` |

A configuration is one frozen choice of the family's free parameters: for Q the
pair (width, scale) with its saturation limit, for L the layer partition and its
message-reuse rule, for QC the block vectorization width and the rotation
implementation. The eight-configuration cap is spent on the untimed screens of
REQ-03 and REQ-05, which need no benchmark window; a configuration that neither
screen admits never reaches a timed cell.

### Confirmatory stage, per family

| Item | Budget |
|---|---|
| Confirmatory campaigns | at most 1, the protocol's per-candidate cap |
| Cells in that campaign | at most 6 |
| Attempts after a non-passing outcome | none |

Six is the largest cell count a first confirmatory attempt admits, recomputed
from the tooling rather than assumed: `trial_ledger::attempt_alpha` counts the
ledger entries that reserve a comparison, so a family whose genesis ledger
carries one exploratory reservation and then its first confirmation has one such
entry and spends `alpha / (1 * 2)`; `receipt.rs` records a cell as
`not-confirmatory` when `bootstrap_resamples * corrected_alpha / 2` falls below
twenty, and `corrected_alpha` is that attempt budget divided by the cell count.
With the frozen `alpha` and `bootstrap_resamples`, seven cells fall below the
threshold and six do not. Each family recomputes this from its own ledger before
its addendum is frozen; a family whose ledger history admits no confirmatory cell
records the arithmetic as a preserved outcome and runs no confirmation.

### Stop rules

A family stops, and its outcome is recorded with the evidence that stopped it,
at the first of:

- **S1 — contract or feasibility.** The numerical-contract review or the Rust
  1.95 intrinsic feasibility record rules the family out before any prototype.
  The ruling is explained with its evidence; the family is not dropped silently.
- **S2 — quality screen.** No admitted configuration meets the predeclared
  quality tolerance on the measured DVB-T2 and NR cells. The family publishes
  its failing counts and intervals and enters no timed confirmation.
- **S3 — exhausted exploration.** The eight configurations are spent with none
  passing both screens. The family records no qualifying candidate.
- **S4 — confirmatory outcome.** The confirmation returns its outcome. A
  `fail`, `inconclusive`, `not-material`, `unstable` or `quality-incompatible`
  cell ends the family under this issue; it authorizes no further attempt and
  no margin or tolerance is revised after the measurement.

No family's budget is transferable to another family, and no stop rule is
satisfied by an argument that a family would probably lose.

## What this record does not decide

Adoption. A prototype of any family is an experiment outside the production
decoder path and does not become the production decoder by measuring well; REQ-06
publishes the proceed/no-proceed decision for each family after the window, and a
selected family delivers through a reviewed design and worker-sized scope, not
through this issue's prototypes.

Inter-frame batching itself, which is `ed3d490e`'s subject. Family QC is measured
as the intra-frame alternative to it, and neither is forced to win here.

## Decisions after the confirmations

The sections above are the frozen instrument and are applied here as written.
Every figure behind a decision is in the
[quality tables](../../bench_results/f63a2464/quality/tables.md) or the
[timing tables](../../bench_results/f63a2464/timing-tables.md);
[findings.md](findings.md) carries the evidence walk and the limits.

**Reading of the confirmatory cap.** The cap of one confirmatory campaign
applies per ledger (repository owner's ruling, 2026-10-05). Family QC asks three
canonical questions on three ledgers, so each ledger carries one pilot
reservation and one confirmatory reservation, and rule S4 is applied to each
ledger's confirmation separately (timing tables, "Stop rule S4 per ledger").

| Family | Rule that closes it | Decision | Evidence |
|---|---|---|---|
| Q, quantized min-sum | S2 | No proceed | quality tables, "The exploratory screen" |
| L, layered min-sum | S2 | No proceed | quality tables, "The exploratory screen" |
| QC, single-worker ledger | S4, no ending outcome | Qualifies | timing tables, the single-worker confirmation |
| QC, multicore ledger | S4, no ending outcome | Qualifies | timing tables, the multicore confirmation |
| QC, comparator ledger | S4, every cell `fail` | Ended; no further attempt, no margin revised | timing tables, the comparator confirmation |
| QC, candidate family | — | Proceed to the production design proposed below | the three rows above |

The comparator ledger's `fail` is recorded as the evaluator records it. Its
direction follows from the plan's arm positions, which the timing tables print
beside each cell: the QC prototype holds the baseline position and AFF3CT's
scalar f32 flooding normalized-min-sum decoder [Cassagne2019] the candidate
position, so a speedup below the reciprocal of the equivalence margin states
that the external arm is the slower one. The confirmation therefore establishes
no material gap in the external arm's favour, which is the question that family
asks, and its acceptance summary reports that the receipt does not qualify.

The proceed decision for family QC rests on the two ledgers that compare the
prototype with the canonical decoder under an unchanged numerical contract. It
selects a design direction, and the canonical `LdpcDecoder` stays the production
decoder until the scope below delivers its own evidence.

### Proposed production design for family QC

A proposal for review; no issue exists for it and no production code changes
under `f63a2464`.

- **Ownership.** `gf2-coding` owns the circulant-block layout derived from
  `QuasiCyclicLdpc` and a QC-aware flooding update behind the existing
  `LdpcDecoder` interface, selected by the code's structure. `gf2-kernels-simd`
  owns the one AVX2 check-block kernel with its safety contract; the scalar
  reference stays in `gf2-coding` and is the fallback where the capability is
  absent.
- **Contract.** The canonical float contract, unchanged: the posterior of every
  codeword position carries the canonical decoder's `f32` bit pattern after the
  same iteration count, under every min-sum rule and under early termination.
  The shared behavioural suite asserts it for the scalar path and the kernel
  separately, including signed zeros, NaN encodings and infinities.
- **Dispatch domain.** Codes that expose an equal-degree circulant block
  partition. DVB-T2 keeps the canonical path, for the structural reason the
  [numerical-contract review](numerical-contract-review.md) records.
- **Budget.** The complexity budget the confirmation addenda freeze: one new
  unsafe kernel and their declared source-line ceiling.

### Proposed worker-sized scope

1. **Layout and scalar path.** The block layout and the scalar QC-aware flooding
   update in `gf2-coding`, with the posterior-level equivalence suite over
   several lifting sizes, the min-sum rules, punctured and filler inputs and
   early termination. No `unsafe`.
2. **Kernel and dispatch.** The AVX2 check-block kernel in `gf2-kernels-simd`
   with its safety contract, byte-identity tests against the scalar reference,
   Rust 1.95 verification, runtime dispatch and the tested fallback.
3. **Before/after evidence and selection rule.** A fresh pilot and confirmation
   of the production path against a pinned pre-change baseline under the
   measurement contract, covering what this checkpoint leaves unmeasured: the
   prototype's own memory footprint, lifting sizes below and at the vector
   width with their dispatch overhead, and a rate-matched NR operating point.
   The selection rule for the dispatch domain is frozen from that evidence.

Inter-frame batching (`ed3d490e`) remains a separate candidate; neither
excludes the other, and a comparison of the two on sustained throughput belongs
to the third item's addendum.
