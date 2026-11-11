# Family contracts

The contracts the families in [families/](families) and the arms in
[arms.json](arms.json) follow. Family descriptions cite them by name.

## `arm-support`

Arms are `gf2-batch-single-frame` (forced `SingleFrame`), the tuned
production arm, the QC prototype arm and the AFF3CT flooding arm. A pre-change
or variant executable is a distinct catalogue name whose build record has its
own `build_root`, which `make-family.py` resolves per arm name.

Timed frames repeat a fixed prefix, and the iteration record covers each worker
and frame of that prefix. gf2 and QC counts are read from their outcomes after
timing and labelled direct; the QC arm's store is part of its measured
operation. AFF3CT's timed clones have no counter, so its counts come from
replaying the prefix on the counting prototype after timing, are labelled
replay-derived, and require equal decisions. The record uses the runner's
journal and adds no schema.

## `nr-pilot-frame`

The family is `55c3e433-nr-route-pilot`, on its own ledger, exploratory,
twelve pairs per cell, with at most one timed campaign. An attempt voided for a
procedural defect under the protocol does not count.

Configurations are `w1-b1`, `w1-b8`, `p6-b8`, `p12-b8` and `l24-b8`, resolved
by the `physical-cores-6`, `physical-cores-12` and `logical-cpus-24` sets,
which need `--full-host`. Workload, NMS factor 0.75, cap 50, stopping, timed
prefix and timing boundary are those of `1362381c-ldpc-aff3ct-integration`.

Cells pair the single-frame arm with QC and with AFF3CT at each configuration,
ten in all. QC against AFF3CT is the ratio of the two pairings and is labelled
indirect.

## `check-variants`

`cached-signed` computes $\alpha(+1 \cdot m)$ and $\alpha(-1 \cdot m)$ once per
check for both minima and selects among them, which keeps the numerical
contract's written operation order. `select-reduced` adds comparison/select
reduction to `cached-signed`, keeping strict ties, skipped NaN magnitudes,
infinity initialization and comparison signs.

Each variant is a patch of `min_sum_check_row`, built as its own arm from an
otherwise unchanged tree. No runtime selector exists, so posterior equality is
checked across executables against the unchanged build.

The family is `55c3e433-check-variants`, on its own ledger, exploratory,
twelve pairs, with at most one timed campaign. Cells pair unchanged production
with each variant at `w1-b1` and `p12-b8` on both codes, eight in all.

A nomination is exploratory. A variant is eligible when its bits match, its
speedup lower bound clears $1/1.1$ on all four cells, and its lower bound
reaches the 1.1 improvement margin on at least one cell. Among eligible
variants, the larger smallest lower bound is nominated.

## `qc-go-rule`

The QC route's selection predicate takes the QC route when the algorithm is
`DecoderAlgorithm::NormalizedMinSum`, the code carries attached QC structure of
NR BG1 Z=384, and the case is one worker with one frame or twelve workers with
eight frames per worker. The route falls back to scalar where the kernel is
unavailable. Every other case keeps its existing route, including one worker
with eight frames. The predicate has no measured threshold, so it is no
calibrated selector and needs no holdout.

## `adoption-family`

The family is `55c3e433-integrated-adoption`, purpose `decoder-family`, on its
own ledger. Its stages are one twelve-pair pilot, a freeze by
[freeze-confirmation.py](freeze-confirmation.py) with resolution from that
pilot, and one 24-pair confirmation of six cells.

The baseline is a pinned pre-change arm, and the candidate carries the
nominated variant, the delivered route, or both. Cells are both codes at
`w1-b1`, `w1-b8` and `p12-b8`, with the tuned envelope on both sides.

The family declares one worthwhile speedup and equivalence 1.1. The worthwhile
speedup is 1.25 for a candidate carrying QC and 1.1 for a variant alone. The
freezer refuses margins not above one plus resolution. Losing cells the
candidate targets carry objective `improvement` and all other cells
`non-regression`. A candidate qualifies only when the canonical evaluator's
receipt reports `qualifies` true and the conformance suites pass. The frozen
complexity budget is `max_new_unsafe_kernels` 1 and `max_added_source_lines`
1,500 for QC, and 0 and 150 for a variant. A combined candidate sums them. An
inconclusive or failed result leaves production unchanged, with one attempt per
candidate. The family declares no holdout.

## `comparator-family`

`55c3e433-adopted-vs-aff3ct` measures the gap of the named adopted route
against AFF3CT, using the six cells, tuned arm, ratio direction and margins of
`1362381c-ldpc-aff3ct-integration`, through a same-family pilot and freeze, and
one confirmation only in an owner-armed window. The family has its own ledger:
the earlier family's AFF3CT candidate holds that ledger's confirmation
reservation. Tables reuse `benchmark-acceptance` of the
`tuning-campaign-support` package.
