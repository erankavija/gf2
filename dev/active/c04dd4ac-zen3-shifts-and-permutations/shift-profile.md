# Residual `BitVec` shift workload profile

> **Diátaxis Type:** Explanation

**Issue:** 85fc5ff4
**Protocol:** `zen3-benchmark-protocol` version 4

This artifact owns the workload profile and the materiality disposition for the
public in-place, zero-filling `BitVec::shift_left` and `BitVec::shift_right`
residual paths. It changes no production code and authorizes no ISA-specific
implementation.

This report states no measured value. Each quantitative conclusion points to
its source:

- **tables § `<section>`, row `<cell>`**: a section of the
  [generated evidence tables](../../bench_results/c04dd4ac/residual-shift-tables.md),
  written by
  [`survey/summarize-shift-receipt.py`](survey/summarize-shift-receipt.py) from
  the committed receipt, its acceptance summary, its execution log, the frozen
  addendum, the family ledger, the semantic validation record and the
  production-consumer audit. Every table names its own source file and digest,
  its sample counts and its interval method.
- **[`survey/shift-source-evidence.json`](survey/shift-source-evidence.json)**:
  the code claims below, each pinned by project, commit, path, line, verbatim
  line and reason by
  [`survey/make-shift-source-evidence.py`](survey/make-shift-source-evidence.py).

## Question and scope

Does the residual, non-word-aligned offset path of the public shift primitive
cost enough more than the existing word-aligned path to justify planning-time
feasibility work on a concrete vector or bit-manipulation form?

The complete production-source audit answers the consumer half. It scans every
Rust source below `crates/*/src`, excludes the owning module, and finds no
downstream production call of either method (tables § *Production-consumer
audit*). Whole-consumer measurement is therefore inapplicable rather than
skipped: this family measures the public primitive in isolation because no
production consumer exists to measure it inside. Benchmarks, examples and tests
call the primitive; none of them is a production consumer.

## The control is a workload reference, not a substitute

The two arms deliberately run different offsets at the same length, direction,
seeded representation and cache state. The baseline arm calls the public method
at the cell's residual offset, where a non-zero `k % 64` selects a safe scalar
double-word funnel that reads a word and its neighbour, combines them with the
shift and its complement, and stores one word per step. The candidate arm calls
the same public method at offset 64, where `k % 64` is zero and the call takes
the word-aligned dispatch: it hands whole words to the SIMD word-shift backend
when the non-default `simd` cargo feature is built and runtime detection
supplies one, and otherwise moves whole words scalar-wise. Either way it moves
words and touches no bit lanes. Both arms restore the zero tail padding. `survey/shift-source-evidence.json` pins each of those statements.

A ratio between these arms describes what the residual path costs relative to a
cheaper, non-equivalent workload at the same size. It cannot establish semantic
substitution, because offset 1 is not offset 64, and it cannot bound what any
residual implementation could reach, because the control performs strictly less
work per word. No row of the tables supports production adoption, and this
profile makes no adoption claim.

## Frozen method

[`shift-profile-addendum.json`](shift-profile-addendum.json) freezes ten
exploratory cells before any timing: both directions over five sizes, from a
single incomplete word through a 256-bit lane crossing, a byte-residual
mid-size, an L2-resident size and a streaming size, each with its own residual
offset against the offset-64 control (tables § *Frozen workload matrix*). The
first four sizes are warm single-core latency cells and the largest is a
streaming throughput cell. The search spends one exploratory trial per cell and
declares no confirmatory comparison, so the family ledger reserves zero
comparisons and names no candidate identity (tables § *Family accounting*).

The addendum predeclares the disposition rule it is judged by: a residual path
must consume at least one fifth more time than the word-aligned control at a
consequential size before this no-consumer family can justify planning-time
feasibility work for at most two concrete forms. That threshold is the
evaluator's improvement margin for every cell, so the acceptance tool's own
decision column is the disposition test rather than a separate reading of the
numbers (tables § *Materiality disposition under the frozen rule*). The frozen
complexity budget in the same file permits zero new unsafe kernels and zero
added production source lines, so a material result authorizes analysis, never
code.

## Correctness and representation evidence

The independent zero-fill verifier runs inside the measured executable, and the
launcher refuses to stage timing unless it reproduces the committed validation
bytes. It derives expected bits from canonical little-endian indices without
calling either shift method, and covers both directions, every length and
offset the issue's criteria name, offsets at and beyond each vector length,
256-bit lane crossings, incomplete final words and the public API's in-place
aliasing. Its case count, the offsets and lengths it enumerates and its failure
count are rows of tables § *Semantic oracle corpus*. Every checked result
preserves length and zero tail padding.

The receipt pins the behaviour digest of every producing source file. The
digests of the measured `bitvec.rs` and of the measured benchmark arm equal the
digests `survey/shift-source-evidence.json` records at the commits it pins, so
the paths described above are the paths that ran.

## Wire contract

The canonical child-v2 framing accepts an arm's request only when the child
re-encodes the runner's bytes exactly, so an arm whose request type does not
round-trip the runner's request fails its first child and measures nothing.
Code reading does not establish that contract.
[`survey/smoke-shift-arms.sh`](survey/smoke-shift-arms.sh) drives both arms of
every frozen cell with the runner's own request framing, result parser and child
environment, in the validation role: each arm performs one untimed dispatch,
applies its zero-fill oracle and returns no timing window, and the driver
refuses an arm that reports one. The smoke opens no campaign, so it takes no
host mutex, reserves nothing in the family ledger, writes no stage, finalizes no
receipt and emits no timing sample, and it runs outside the benchmark window.
[`shift-profile-smoke.json`](shift-profile-smoke.json) records the plan and
executable identities, the route each arm selected, the result lines parsed and
the window count.

## The voided launch attempt

Campaign `residual-shift-profile-85fc5ff4-v4` reaches the benchmark window with
an arm that cannot complete the handshake, and the executor voids it under the
[protocol's voided-attempt rule](../f547c394/protocol.md) rather than spending
its reservation: the defect is procedural and in the attempt's own launch, and
no result of it is read. Its reservation stays outside the chain the replacement
attempt reserves on, so the ledger's single line is the replacement campaign's
own genesis reservation (tables § *Family accounting*).

The aborted stage is preserved whole at
[`2026-09-16-85fc5ff4-residual-shift-profile-abandoned`](../../bench_results/85fc5ff4/2026-09-16-85fc5ff4-residual-shift-profile-abandoned/execution.log),
and
[`v4-voided-launch-attempt.json`](../../bench_results/85fc5ff4/v4-voided-launch-attempt.json)
names the campaign, the addendum digest, the defect, the cells measured and
unmeasured, and the abort. No figure in this profile comes from it: the stage
carries no checkpointed unit and no timing window.

## Campaign and its integrity

`survey/run-shift-profile.sh window` executes the campaign in the scheduled
benchmark window. The campaign is verified from its own execution log rather
than from the job's exit status: the log's `cell-start` and `cell-complete`
counts each equal the addendum's cell count, every receipt cell carries the
status `measured` at the declared pair count, and the log's terminal record is
`complete` (tables § *Execution-log integrity*). The campaign is bounded and
resumable, so it spans several sessions under one campaign identity, each
session checkpointing the cells it measures and the next recording the earlier
ones as completed in a prior session. The acceptance tool accepts the receipt,
reports the finding count on the tables' *Campaign acceptance* source line, and
records that the receipt does not qualify for production selection.

## Result

The residual path is not uniformly the expensive one. At the smallest size, one
incomplete word in both directions, the residual offset costs less than the
word-aligned control, and the evaluator records those two cells as regressed
against a ratio below the equivalence floor (tables § *Materiality disposition
under the frozen rule*). The two arms differ there by a fixed per-call cost
rather than by per-word work.

From the lane-crossing size upward the gap opens and grows with the amount of
data touched up to the resident size, where it is larger to the right than to
the left; at the streaming size it is smaller again in both directions. Most cells are material under the predeclared threshold; the exceptions
are the smallest-size cells in both directions, where the residual path is the
cheaper one, and the right-hand lane-crossing cell, whose interval straddles
the threshold rather than clearing it. Tables § *Materiality disposition under
the frozen rule* carries every ratio, its interval and the material count.

Every interval is a whole-pair percentile bootstrap over the addendum's six
counterbalanced pairs per cell, each execution contributing the protocol's five
calibrated windows; the tables' preamble states the resample count and the
corrected confidence, and each row carries its relative half-width. Six pairs
is a small sample: an interval here describes the spread of those pairs on one
host inside one benchmark window, not run-to-run variation across hosts or
builds.

The mechanism the gap follows is visible in the two measured paths rather than
in the numbers: the control moves whole words, while the residual path
performs one scalar funnel per word with a loop-carried read of the
neighbouring word. The ratio therefore grows where per-word work
dominates per-call overhead, which is what the size ordering shows.

## Disposition

The disposition is **material**, and it nominates two concrete forms for
planning-time feasibility work, the maximum the frozen addendum allows:

1. an AVX2 lane-crossing funnel that builds the neighbouring-word vector with a
   cross-lane permute or byte alignment and combines the shifted halves per
   256-bit group, behind runtime detection with the current scalar loop as the
   fallback;
2. a BMI2-gated scalar funnel that replaces the shift-and-complement pair with
   double-precision shift instructions over an unrolled word pair, likewise
   behind runtime detection with the current loop as the fallback.

This is a nomination, not an adoption: the profile authorizes no production
source, and its complexity budget stays at zero new unsafe kernels and zero
added lines. Nothing measured here shows either form would close the gap, since
the control does strictly less work per word than any correct residual shift
can; whether either form is implementable at the repository's Rust 1.95 floor
is settled by the record named below, and settling it is not a win.

Under REQ-05 the nomination keeps this profile open, and what it owes is
planning-time evidence rather than more measurement. The Rust 1.95 compile and
assembly record for both nominated forms, with the runtime-gated scalar
fallback each capability-gated form needs, is the
[feasibility record](shift-feasibility-record.md): both forms are feasible at
the MSRV, and that record carries the toolchain and assembly artefacts, the
correctness evidence and a scope proposal for the leaves. Feasibility is not a
win, and the record makes no speed claim.

The amended [plan](plan.md) carries the bracket step. It creates the candidate
implementation leaf for the BMI2-gated scalar funnel (`f8dd4dde`) and its A/B
confirmation leaf (`00dd43c3`), makes them depend on this profile, and re-homes
publication (`a0812b83`) behind the confirmation in the instantiated dependency
graph. The AVX2 lane-crossing funnel is nominated, feasible and unselected in
this epic; the plan's decision table records the ruling and its reason. A
family whose downstream consumer set is empty leaves open whether the primitive
is worth its maintenance: the confirmation's frozen retention rule decides
that, and this profile supplies the cost side of the argument only.

## Evidence map

| Evidence | Current result |
|---|---|
| [`shift-profile-addendum.json`](shift-profile-addendum.json) | Frozen before timing; protocol v4, ten exploratory cells, zero confirmatory comparisons |
| [`shift-profile-trial-ledger.jsonl`](shift-profile-trial-ledger.jsonl) | The replacement campaign's genesis reservation; tables § *Family accounting* projects it |
| [`shift-profile-validation.json`](shift-profile-validation.json) | Passing independent-oracle corpus; tables § *Semantic oracle corpus* |
| [`shift-profile-consumer-audit.json`](shift-profile-consumer-audit.json) | Passing; no downstream production caller; tables § *Production-consumer audit* |
| [`shift-profile-smoke.json`](shift-profile-smoke.json) | Passing: both arms answer the runner's request framing on every frozen cell, with zero timing windows |
| [`shift-feasibility-record.md`](shift-feasibility-record.md) | The REQ-05 planning-time record: both nominated forms feasible at Rust 1.95, with the toolchain, assembly and correctness artefacts |
| [`survey/shift-source-evidence.json`](survey/shift-source-evidence.json) | The pinned code claims behind every mechanism statement above |
| [`dev/bench_results/85fc5ff4/v4-voided-launch-attempt.json`](../../bench_results/85fc5ff4/v4-voided-launch-attempt.json) | The voided launch attempt, its preserved stage and its unmeasured cells |
| [`dev/bench_results/c04dd4ac/residual-shift-profile/`](../../bench_results/c04dd4ac/residual-shift-profile/) | The accepted exploratory receipt, its acceptance summary, execution log, checkpoints and input snapshots |
| [`dev/bench_results/c04dd4ac/residual-shift-tables.md`](../../bench_results/c04dd4ac/residual-shift-tables.md) | Every figure this report points to |
