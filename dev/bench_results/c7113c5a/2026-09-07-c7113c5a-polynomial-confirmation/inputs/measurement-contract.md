# Zen 3 performance measurement contract

This is the shared normative contract for epic `1a379447`. Issue criteria cite
this document rather than copying its requirements. It specifies how evidence
is produced; it is **not** a frozen experimental protocol or a performance receipt.
The protocol task supplies the executable rules and numeric settings before
confirmatory measurements begin. Repository engineering invariants also apply.

## Protocol identity and freezing

Maintain one versioned protocol and independently versioned family/cell addenda.
Every receipt pins immutable copies of the contract, protocol and applicable
addendum by content digest. Git commits are optional navigation metadata, never
critical provenance: Git-object availability, commit ancestry, HEAD changes and
repository-wide clean/dirty status cannot decide acceptance, evidence validity or
checkpoint/resume. Content identities of the producing code, executables,
configuration and inputs establish reproducibility. Unrelated documentation, JIT
and parallel-session changes leave those identities and existing evidence valid.
Addenda specify only cell-specific choices; they cannot silently override the shared
rules. Link the finalized artifacts from their owning issues so workers can discover
the exact inputs.

Exploratory pilots may inform workload selection, budgets and thresholds. Label
them exploratory and exclude their samples from confirmation. Persist an immutable content-pinned snapshot of all numeric
settings in the append-only execution record before that cell's first confirmatory trial: workload identities, warmup,
sample minima/maxima, repetitions, confidence level, outlier policy, cache state,
improvement and equivalence margins, material-gap threshold, search/stop budget,
and multiple-comparison family. The record binds the frozen content before the
first measurement event; an unrelated Git commit is not a protocol amendment.
Resolution evidence identifies a distinct pilot receipt by content digest. Commit
the snapshots and execution evidence for durable publication. Protocol amendments
that change experimental rules create a new explicit version,
retain all earlier data and contradictions, and require fresh confirmation; they
do not reclassify failed trials as pilots. Reserve independent holdout confirmation
for calibrated selectors and final integration.

## Statistics and adoption

Use paired, interleaved or randomized A/B runs to control drift. Define the
resampling unit, ratio-of-medians estimator and bootstrap confidence procedure,
including handling of paired observations. Control multiple comparisons within
predeclared families and account for selection across candidate trials. Declare
non-regression as an equivalence margin, not absence of statistical significance.

Choose worthwhile-effect thresholds and maintenance/complexity budgets by family
before confirmation, with a rationale tied to real consumer benefit and measurement
resolution. There is no blanket 15% or 1.5x hurdle: a repeatable smaller consumer
gain can qualify when the declared rule permits it. Select production candidates
only when confidence-bound improvement, correctness and non-regression rules pass
on their declared dispatch domain. Record negative, inconclusive, unavailable and
not-material cells; retain the established implementation where none qualifies.
Mandatory API or correctness outcomes remain mandatory; a no-win receipt cannot
silently waive a separate hard delivery criterion.

## Host and execution

Measure optimized release/bench builds on the prepared Ryzen 9 5900X under the
existing exclusive benchmark lock in `dev/scripts/`. Finish builds before timed
work. Observe topology, affinity, actual worker counts and nested pools, SMT,
clocks/governor and selected capabilities at runtime; do not infer them from build
configuration. Compare isolated one-core latency separately from sustained and
multicore throughput. Include 1, 6 and 12 physical-core arms and 24 logical CPUs
where meaningful, resolving actual CPU IDs at runtime. Inapplicable arms need a
reason, not fabricated samples.

Use bounded resumable runs. Open and print the authoritative append-only execution
log before the first bounded run, flush cell progress and terminal state, and
checkpoint completed cells without repeating them on resume. Follow the existing
CPU-budget and lock conventions, including the full-host wrapper's cargo-lock
exception. Never add a parallel private campaign framework.

## Comparable operations and costs

Family surveys pin exact source revisions, build flags, licenses, selected backend
evidence and reproducible commands. The citation registry identifies projects;
receipts identify builds. A surveyed candidate is not a proven faster arm. Choose
the fastest compatible **measured** arm per cell, and preserve every losing cell
instead of hiding it in an aggregate.

Match mathematical operation, polynomial/basis, bit mapping, precision, dimensions,
alignment, aliasing and output semantics. Include setup, table preparation,
packing/unpacking, representation conversion, batch fill, dispatch and output costs
in whole-consumer cells; report isolated kernels separately. Cover small inputs,
crossovers, cache-resident and streaming sizes. Report missing equivalent external
operations honestly: long-polynomial multiplication is not an independent raw
CLMUL batch, and a different transpose geometry needs a measured adapter.

Decoder comparisons have separate matched-algorithm and fastest quality-compatible
arms. Matched runs fix the parity-check matrix, recorded LLRs, precision, schedule,
normalization, iteration cap and stopping semantics. For other arms document
quantization/scaling, puncturing/fillers, rate matching, CRC versus syndrome stopping,
latency, batch size, memory and iteration distributions. Predeclare quality
tolerances and report BER/FER sample counts and confidence intervals. Do not compare
unadjusted iteration counts under different stopping contracts. Include random
codewords as well as all-zero words when validating saturating implementations.

## Correctness, explanation and receipts

Use the shared mathematical/backend and external conformance suites. Preserve
canonical little-endian bit indexing, zero tail padding, and relevant empty,
unaligned, partial-batch and 0/1/63/64/65 boundary cases. Keep deterministic seeded
behavior across worker counts, scheduling, fallback and checkpoint/resume under
the declared numerical contract. Explicitly test signed zero, ties, clipping and
supported non-finite cases in affected decoder paths. Numerical changes require
a reviewed contract before implementation/adoption; faster arithmetic does not
implicitly authorize different semantics.

Compile intrinsic-bearing designs with Rust 1.95 before implementation breakdown
and verify shipped code at that MSRV. Preserve tested scalar fallback, canonical
library/tuning abstractions and isolated unsafe kernels with safety contracts.
Newly selected intrinsic designs require their own feasibility evidence; an
instruction list is not such evidence.

Commit raw machine-readable samples, exact commands, input identities/seeds, source
and toolchain/build identities, runtime provenance, selected paths, interpretation
and acceptance summary under the owning issue's `dev/bench_results` directory.
Link durable artifacts at their final paths. Explain measured limits with assembly
and profiler evidence for allocation, dependencies, instruction mix, spills,
conversion, cache traffic or bandwidth. Give each material residual gap a measured
limit, preserved experiment or tracked falsifiable follow-up within the search
budget. Do not claim an absolute optimum from a finite search.

A production change requires a current pinned pre-change baseline and after
measurement, including small dispatch overhead. Historical receipts provide context,
not a substitute for that baseline. Reuse established harnesses and preserve their
behavioral identities. Documentation-only changes do not invalidate measurement
evidence. Core kernel calibration and decoder-specific calibration use the same
canonical mechanism but may proceed independently; final integration reconfirms
both production routes.
