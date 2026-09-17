# Quantized, layered and QC-aware LDPC candidates

> **Diátaxis Type:** Explanation

Outcome record for `f63a2464`, the bounded checkpoint that decides the three
candidate families `3be770d5`'s lever ranking leaves standing on a comparator or
labelled estimate. The
[measurement contract](../1a379447-zen3-cpu-performance/measurement-contract.md)
and the [frozen protocol](../f547c394/protocol.md) with its
[version-4 amendment](../f547c394/amendment-v4.md) govern every timed cell.

Three documents are the frozen instruments this record is judged against, each
committed before the evidence it governs: the
[decision record](decision-record.md) with the families and the search and stop
budget, the [numerical-contract review](numerical-contract-review.md) with each
family's declared contract and its MSRV intrinsic feasibility, and the
[predeclared quality tolerances](quality-tolerances.md).

Every figure lives in the
[generated quality tables](../../bench_results/f63a2464/quality/tables.md), in the
[feasibility record](../../bench_results/f63a2464/intrinsic-feasibility/feasibility.json)
beside its emitted assembly, or in the
[confirmatory budget](../../bench_results/f63a2464/confirmatory-budget.json).
This report names conclusions and points at the section that carries each one.
Code claims are in [source-evidence.json](survey/source-evidence.json).

## What the checkpoint decides and what it does not

Nothing in the production decoder changes. The candidates live in a standalone
[prototype crate](survey/prototypes/) outside the workspace, consuming
`gf2-coding`'s code, edge layout and LLR types through their safe public
interfaces; the one `unsafe` module is the AVX2 check kernel of the QC candidate,
which carries its safety contract and is asserted byte-identical to its scalar
reference. No production selection changes and no prototype becomes a decoder by
measuring well.

## Feasibility

Every AVX2 form the three families need compiles at the repository MSRV, lowers
to the instruction the design assumes and agrees with its scalar reference over
exhaustive or boundary-covering inputs. The feasibility record names the
toolchain it observed, the emitted assembly's digest, the behavioural test
command's exit status and the mnemonics the compiler produced for each probe.
Two results are contract findings rather than instruction availability: the
canonical comparison-based sign rule lowers to a compare against zero, so a
vectorized f32 kernel can follow the float contract exactly, and a byte rotation
needs the cross-lane half-swap before the within-lane align because AVX2 has no
cross-lane byte shuffle. The review records both with their probes.

No family is ruled out by feasibility. One workload is: DVB-T2 admits no QC cell,
because its check rows carry no equal-degree circulant block partition — its
information part groups columns into blocks whose check indices advance by the
code's step and its parity part is a staircase accumulator [Etsi2015], against
the lifted structure NR BG1 is built from [ThreeGpp2017]. The review carries that
ruling with its source evidence, and the QC family's addenda declare no DVB-T2
cell rather than fabricating one.

## Prototypes

Each family has a decoder in the prototype crate and a shared behavioural suite
in `survey/prototypes/tests/contracts.rs` asserting every claim its contract
makes, including the claims a family does *not* satisfy.

Family QC is bit-exact against the canonical decoder. The suite asserts that at
the posterior level — every codeword position's `f32` bit pattern after the same
iteration count on the same input, over three lifting sizes and the three min-sum
rules, on random and all-zero codewords, on punctured and filler inputs, and
under early termination where the iteration counts and hard decisions must agree
as well. The AVX2 check kernel is asserted byte-identical to its scalar reference
including on signed zeros, both NaN encodings and both infinities, and at lifting
sizes that do and do not fill the kernel's vector body.

Families Q and L are changed contracts and the suite preserves that as measured
falsification rather than as an assumption: one test shows a coarse quantized
decoder parting from the canonical decoder's decisions on a noisy frame, and
another shows one layered sweep and one flooding iteration reaching different
states. The quantized alphabet's symmetry is asserted in both directions,
including that the AVX2 magnitude of the excluded type minimum stays negative,
which is why the alphabet excludes it.

## Quality

The untimed campaign decodes the frozen `c077a88b` recorded DVB-T2 and NR
bundles with every arm of every family, under the declared cell kinds of the
tolerances: the recorded bundle, the NR bundle under the mandatory systematic
puncturing of the NR rate matching, the NR bundle's all-zero frames under filler
LLRs, and the difficult subset the canonical arm leaves in error. It is bounded,
seeded, resumable and untimed; its append-only execution log is
[execution.log](../../bench_results/f63a2464/quality/execution.log) and its
per-cell records are beside it.

**The canonical baseline reproduces its own frozen record.** On both codes the
baseline arm's per-frame information-bit error vector equals the `c077a88b`
prepared gf2 record's (tables, "The canonical baseline against its own frozen
record"), so every comparison below is anchored to the corpus the epic already
measured.

**External conformance holds under each declared contract where a measured
external arm exists.** The layered arm's per-frame vectors equal AFF3CT's
horizontal-layered normalized-min-sum f32 record's on both codes, and the
wide-alphabet quantized arms' equal AFF3CT's `i16` flooding record's on NR
[Cassagne2019] (tables, "External conformance under each declared contract").
The narrow alphabet has no measured external arm on this host, which the review
records with `c077a88b`'s preserved evidence: the pinned AFF3CT build rejects
`i8` for the layered INTER mode whose `i16` form it does measure, and srsRAN
[Srsran2026] and OpenAirInterface [OpenAirInterface2026], which decode with
`int8` messages upstream, both fail to build here. The gap is recorded, not
filled by substituting a different operation. xdsopl [Xdsopl2026] exposes
min-sum, offset min-sum and corrected min-sum rather than the normalized rule,
so it is not a matched arm for any family here.

**Family QC's bit-exactness survives the measured cells.** Its per-frame error
vectors and its per-frame iteration vectors equal the canonical arm's on every NR
cell (tables, "Bit-exactness of family QC").

**The corpus certifies almost nothing, as it did for `c077a88b`.** The paired
non-inferiority bound is at or above zero for every arm on every cell whose
canonical arm decodes nearly all frames; only the punctured stress cell, where
the canonical arm itself fails most frames, produces certifying bounds (tables,
"Paired non-inferiority against the canonical arm"). This is insufficient
evidence at this frame count, not evidence that a candidate is worse, and the
frozen tolerance is not revised to manufacture a certification.

**The punctured cell is a stress cell, not an NR operating point.** It applies
the mandatory systematic puncturing to LLRs recorded for the untransmitted mother
code at its own waterfall, so it represents no rate-matched link; its value is
that every arm faces the identical input. Its counts stand as recorded.

### What the frozen screen admits

The exploratory screen of the tolerances admits a configuration only when, on
*every* measured cell of its family, it introduces no frame error on a frame the
canonical arm decodes cleanly and its aggregate bit errors do not exceed the
canonical arm's. Applied literally to the evidence (tables, "The exploratory
screen"):

- **Family QC is admitted.** Every cell it declares passes both conditions.
- **Family L is not admitted.** Its single configuration passes on both recorded
  cells and the filler cell and fails condition 1 on the punctured cell. Its
  frame errors on that cell are *lower* than the canonical arm's and its
  aggregate bit errors are lower too; it loses frames the canonical arm decodes
  cleanly while gaining more elsewhere, and the frozen condition is a per-frame
  dominance condition, not a total.
- **Family Q is not admitted.** Every one of its eight frozen configurations
  fails on the DVB-T2 recorded cell. The two coarsest fail everywhere. The
  remaining six pass every NR cell — several decode the NR recorded cell with no
  frame error at all, where the canonical arm does not.

Two observations the screen does not act on are preserved because they bear on
any successor work. The `i8` and `i16` arms at the same channel scale produce
identical per-frame evidence on both workloads, so at that scale the narrow
alphabet never saturates and the quantization loss lies in the scale rather than
the width. And the layered arm reaches its decisions in materially fewer units of
its own schedule than the flooding arms use of theirs, which the tables report as
sweeps against iterations and do not compare.

## Timed evaluation

Family QC is the one family the frozen screen admits, so it is the one family
with timed cells under this issue. Three pilot families are frozen, each with its
own genesis append-only ledger and its own campaign seed:
[single-worker](addendum-ldpc-qc-intra-frame-single-worker-pilot.json), which
measures sustained throughput and single-frame latency — the axis inter-frame
batching cannot improve — against the canonical decoder;
[multicore](addendum-ldpc-qc-intra-frame-multicore-pilot.json), which measures the
same pair at six and twelve physical cores and at twenty-four logical CPUs; and
[comparator](addendum-ldpc-qc-comparator-single-worker-pilot.json), which measures
the candidate against the matched external arm, AFF3CT's scalar f32 flooding
normalized-min-sum decoder [Cassagne2019]. The fastest quality-compatible
external arm is the separate question `c077a88b` closed with an empty shortlist;
that outcome is preserved rather than replaced by the matched arm.

Each pilot's cells declare throughput or latency, the workers and core arm, the
batching with batch fill accounted, and the decoder contract; the arms report
memory, iteration distributions and the conversion, setup, pack and dispatch
costs separately, and the candidate fills no batch because it vectorizes one
frame. Every timed execution checks each worker's per-frame bit errors against
the reused `c077a88b` prepared quality, so a candidate that parted from the
declared contract fails the run rather than reporting a faster wrong answer.

The confirmatory cell budget is recomputed for each family from its own ledger
rather than assumed: the
[budget record](../../bench_results/f63a2464/confirmatory-budget.json), written by
[confirmatory-budget.py](survey/confirmatory-budget.py), reads the frozen alpha
and resample count from the protocol's source of truth and the attempt index from
the chain, and reports the largest cell count P-20's tail-support rule admits. A
confirmation is frozen from its committed pilot receipt by the canonical freezer,
so it follows the window rather than this session.

[run-smoke.sh](../../bench_results/f63a2464/run-smoke.sh) is the wire-contract
gate the campaigns pass before queueing. It validates each throwaway plan and its
addendum through `ldpc-plan-check`, the checks the runner applies before opening
a campaign, then drives every arm of every cell with the runner's own case
encoder, request sentinel and child environment in the validation role: one
untimed dispatch per arm, every placement and decision check applied, zero timing
windows recorded and refused if any arm reports one. Its request mirror is pinned
against the runner's request by a test in the arms crate, and its result mirror
reproduces the arm's field order, so a field added, removed or reordered on
either side fails there.
[run-campaign.sh](../../bench_results/f63a2464/run-campaign.sh) is the
`window`-mode launcher, following the `07ca8585` precedent: it checks every arm
executable against the recorded
[build identity](../../bench_results/f63a2464/preparation/build-identity.json),
creates the ledger genesis once, prints the authoritative execution log before
the first session, resumes an interrupted campaign under its own identity, and
refuses a confirmation whose addendum is uncommitted or modified. The three
pilots are queued in
[queue.tsv](../1a379447-zen3-cpu-performance/bench-window/queue.tsv).

## Criterion status

| Criterion | Status | Evidence or remaining work |
|---|---|---|
| REQ-01 | Partly met — pilots queued | Contract, protocol, addendum and ledger identities frozen and committed; arms built at Rust 1.95 with recorded digests; every arm smoked without a timing window. No cell is measured until the benchmark window runs the three queued pilots. |
| REQ-02 | MET | The decision record names the three families and freezes the exploratory and confirmatory search and stop budget before any prototype exists; every externally sourced claim cites a registry key. |
| REQ-03 | MET | The numerical-contract review states the canonical contract on all six axes, each family's declared contract against it, and the MSRV intrinsic feasibility record with its emitted assembly. DVB-T2's inapplicability to family QC is explained with source evidence rather than dropped. |
| REQ-04 | Partly met — quality axis measured, timing axis queued for one family | Every family's prototype is evaluated on the measured DVB-T2 and NR cells on the quality axis, with iteration distributions and equivalent stopping. The timing axis is queued for family QC only, because the frozen screen admits no configuration of families L and Q; that consequence is stated under Limits. |
| REQ-05 | MET | Tolerances predeclared and committed before any quality result; BER and FER with counts and intervals over random and all-zero codewords, punctured and filler inputs, mixed convergence and the difficult subset; external conformance under each declared contract, with the unavailable narrow-alphabet comparator preserved. No bit-exactness is claimed without evidence, and the one family that claims it is tested at the posterior level and projected at the cell level. |
| REQ-06 | Not started | Completes after the benchmark window from the pilots' own receipts. |

## Limits and problems

**The frozen screen excludes families L and Q from timed evaluation, and REQ-04's
timing axis is therefore unmet for them under this issue.** The screen was
committed before any quality result and is applied literally; revising it after
reading the evidence would be a falsification defect. The exclusions are narrow
and are recorded above with the exact cell that causes each: family L fails only
the punctured stress cell, where its own frame error count is lower than the
canonical arm's, and family Q fails only the DVB-T2 recorded cell while passing
every NR cell. Whether those two families deserve timed cells under a differently
frozen screen is outside an instrument this issue may change.

**No citation registry key resolves the standard works on layered decoding or on
min-sum quantization.** The registry supplies the implementations this survey
measures against — [Cassagne2019], [Srsran2026], [Xdsopl2026],
[OpenAirInterface2026] — and the standards the codes come from [Etsi2015]
[ThreeGpp2017], but nothing for the schedule and quantization literature the two
changed-contract families rest on. Those families' design claims are therefore
made from measured evidence alone and cite no prose title.

**The recorded corpus cannot certify quality non-inferiority.** At its frame
count the paired bound stays above zero wherever the canonical arm decodes nearly
everything, which is the same limit `c077a88b` recorded. A family that needs
certification needs a corpus sized for it, which is a separate experiment.

**The DVB-T2 workload has no QC arm.** The inapplicability is structural and
recorded with its evidence, so family QC's timed evidence covers one of the two
measured workloads.

## Reproduction

From the worktree root: extract the recorded-input archive the
[build identity](../../bench_results/f63a2464/preparation/build-identity.json)
names into `target/ldpc-inputs`, build the baseline and candidate arms with the
two commands that file records, and run
[record-preparation.py](survey/record-preparation.py). The untimed work runs
outside the mutex:
[record-intrinsic-feasibility.py](survey/record-intrinsic-feasibility.py) for the
MSRV feasibility record and its assembly,
[run-quality.py](survey/run-quality.py) for the quality campaign and
[summarize-quality.py](survey/summarize-quality.py) for its tables, which
reproduce byte for byte on a re-run. Then
[run-smoke.sh](../../bench_results/f63a2464/run-smoke.sh) proves the wire contract
of every arm, and [run-campaign.sh](../../bench_results/f63a2464/run-campaign.sh)
`FAMILY MODE RUN_ID prepare` followed by `window` runs a campaign. The prototype
and probe crates are standalone workspaces with their own target directories;
`cargo +1.95 test --release` in each runs their behavioural suites.
