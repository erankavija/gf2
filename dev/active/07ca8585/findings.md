# Allocation and edge traversal in the CPU min-sum LDPC update

> **Diátaxis Type:** Explanation

Design record and outcome for `07ca8585`. The
[measurement contract](../1a379447-zen3-cpu-performance/measurement-contract.md)
and the [shared protocol](../f547c394/protocol.md) govern every timed cell.
The predecessor `3be770d5` ranks the levers this issue spends; its
[findings](../3be770d5/findings.md) and
[tables](../../bench_results/3be770d5/tables.md) are the authoritative source
of every share, interval and labelled Amdahl ceiling quoted by pointer below.
Code claims are in [source-evidence.json](survey/source-evidence.json), written
by [make-source-evidence.py](survey/make-source-evidence.py), which refuses a
claim whose recorded line does not contain its fragment.

## The path this issue replaces

The decoder keeps its messages in two jagged vectors of vectors, one inner
vector per check and one per variable, and its neighbour lists in two more
(`source-evidence.json`, lever `layout`). Nothing links a check's view of an
edge to the variable's view, so both node updates recover the link by search:
the check update locates every gathered input with a linear scan of the
variable's check list, and the variable update reaches each incoming message
through a linear scan of the check's variable list, twice per edge
(lever `edge-indexing`). The exact comparison counts those scans cost per edge
are derived per code in the predecessor's
[structural table](../../bench_results/3be770d5/tables.md), and the sampled
time they take is the top-ranked lever in its
[lever ranking](../../bench_results/3be770d5/tables.md).

The check update then recomputes one reduction per outgoing edge over the
other `d_c - 1` inputs, so a check of degree `d_c` performs `d_c (d_c - 1)`
gathers and `d_c` reductions (lever `reduction`). Each of those reductions
calls `Llr::boxplus_minsum_n`, which collects its inputs into a fresh
`Vec<f32>` before an indirect call into the kernel table (levers `allocation`
and `dispatch`): one heap allocation per edge per iteration. The predecessor's
[allocation census](../../bench_results/3be770d5/preparation/alloc-census.jsonl)
records exactly that relation per frame, and its
[profile](../../bench_results/3be770d5/v4-r1-steady-profile/profile.md) names
the libc region the copy into that vector reaches. Allocation removal is the
second-ranked lever in the same ranking.

Two further heap requests per syndrome check come from termination rather than
from the update: the hard-decision vector the decoder builds each iteration and
the vector the syndrome matvec allocates for its output (lever `allocation`).
They are not the update, but they stand between the decoder and the zero
steady-state allocation count REQ-07 requires, so this issue removes them
through the same canonical layout.

AFF3CT [Cassagne2019] is the compatible decoder the predecessor measures
against. Its flooding decoder resolves every edge through a precomputed index,
writes every output of a check from one shared minimum, second-minimum and
sign reduction, and checks its syndrome on the soft posteriors without a
hard-decision vector (`3be770d5/survey/source-evidence.json`, project
`aff3ct`). The structure below is that structure, in this decoder's numerical
contract.

## The canonical edge layout

One `EdgeLayout` value, built once per decoder, is the only edge indexing in
`gf2-coding`. Edges are numbered in check-major scan order: check `c` owns the
canonical ids `check_offsets[c] .. check_offsets[c + 1]`, in the parity-check
matrix's CSR `row_iter` order, and `check_edge_var[e]` is edge `e`'s variable.
Variable `v` owns the variable-major slots `var_offsets[v] .. var_offsets[v+1]`
in CSC `col_iter` order, and `var_edge_to_check_edge[f]` is the canonical id of
slot `f`. Those two orders are exactly the orders the previous
`check_neighbors` and `var_neighbors` lists carried, so no message changes the
position it occupies within its node, and `check_edge_to_var_edge` inverts the
second map for consumers that keep a variable-major message array.

Both message arrays are flat and indexed by canonical edge id. The check update
walks a contiguous run of each; the variable update walks a variable's slots and
indexes both arrays through one `u32` load per edge. No search survives in
either update, and the layout is built in time linear in the edge count from a
per-variable position table.

The syndrome reads the same layout. Hard decisions go into a reused byte buffer
sized `n` at construction, each check's parity is the XOR of its edges'
decisions over its contiguous run, and the scan stops at the first unsatisfied
check. The result is the predicate `is_valid_codeword` computes, over the same
matrix rows, without materialising either vector.

## The shared reduction

`min_sum_check_row` performs the whole of one check. Pass one reads the `d_c`
inputs once and tracks the smallest magnitude, the second smallest, the position
of the smallest, and the product of the input signs. Pass two reads them once
more and writes each output from that reduction: an output takes the second
smallest magnitude when it sits at the position of the smallest and the smallest
otherwise, and its sign is the shared product times its own sign. Every check
therefore visits its incoming messages exactly twice, which is REQ-08's bound
linear in check degree, and the three min-sum variants differ only in the final
scaling applied to the excluded minimum.

The leave-one-out identity is exact rather than approximate in both factors. A
sign is `±1.0`, so dividing the shared product by an input's own sign is a
multiplication by that same `±1.0` and is exact whatever the order. The excluded
minimum is the smallest magnitude when the input is not at the smallest
position, and the second smallest when it is, which is the minimum over the
other inputs including when the two smallest magnitudes are equal. So the shared
reduction reproduces the per-output reduction bit for bit.

### What the reduction fixes and what it changes

The canonical reduction follows the supported scalar reference in both rules
recorded under lever `numerical-contract`: an input's sign is taken by
comparison, so negative zero counts as positive, and the magnitude fold skips a
NaN input. The three min-sum variants, the empty and degree-one cases, clipping
and finite extrema are unchanged, and the behavioural suite in
`crates/gf2-coding/tests/ldpc_check_update_contract.rs` asserts the shared
reduction against that reference directly.

The decoder's previous min-sum and normalized min-sum paths did not always
reach that reference. With the default `simd` feature on a host whose AVX2
kernel is selected, they reached `Llr::boxplus_minsum_n`, whose vector lanes
take the IEEE sign bit and whose scalar tail takes the comparison, so one
kernel disagrees with itself and with the scalar reference on a negative-zero
input. Only whole groups of eight inputs reach the lanes, so the disagreement
needs a check of degree nine or more: it cannot arise on the frozen DVB-T2
workload, whose checks are of degree six and seven, and can arise on the frozen
NR workload. **This is a disclosed behavioural change on that path**, in the
direction of the declared contract: after this issue the decoder follows the
scalar reference for every supported configuration and every check degree.
`Llr::boxplus_minsum_n` itself is unchanged, and the kernel's internal
disagreement stays open under its owning issue `39cbde20`, which owns the
kernel's contract and its assembly artefact.

## Allocation counting

REQ-07 is demonstrated by a counting global allocator in a test binary,
`crates/gf2-coding/tests/ldpc_decode_allocations.rs`, built on the same
mechanism as the predecessor's
[census](../../bench_results/3be770d5/preparation/alloc-census.jsonl): the test
crate installs a `GlobalAlloc` that forwards every request to `System` and
counts only while a flag is set, so counting covers exactly the section between
the two stores. The counters are exact, deterministic for a fixed decoder and
input, and are not a timing.

The three phases are counted separately, as REQ-07 requires. Construction of
the decoder allocates, and the test records that it does rather than asserting a
figure. The first prepared decode through a caller-provided codeword buffer
allocates the buffer's storage, which is the documented workspace growth. Every
later decode of the same decoder, at a fixed supported configuration and with
the buffer prepared, allocates zero times, reallocates zero times and frees zero
times, for each of the four algorithms and for both early-termination settings.
The steady-state entry point is `decode_codeword_into`, which writes the hard
decisions into the caller's buffer; the owning-buffer entry points
`decode_to_codeword` and `decode_iterative` keep their signatures and delegate
to it, so they allocate exactly the vectors they return.

## What the evidence shows

Every figure below lives in the [tables](../../bench_results/07ca8585/tables.md);
this section names the conclusion and the section that carries it.

### The update path

The derived per-iteration counts of the two loop structures are in the tables'
"Structural work per flooding iteration" section. The replacement performs no
position-search comparison and no per-edge allocation on either code, reads each
check's incoming messages exactly twice, and reaches the same per-edge indexed
read and write counts as the compatible decoder [Cassagne2019] the predecessor
measures against. The check degrees the two passes are linear in are in
"Representative degree distributions".

### Allocation

REQ-07 holds. The tables' "Steady-state allocation counter" section projects the
test's own run: every steady-state section, over four algorithms and both
early-termination settings on two codes, requests no allocation, no reallocation
and no deallocation, and construction and the first prepared decode are recorded
separately as the phases that do allocate. The separate census of the measured
harness, in "Allocation census of the measured harness", counts what the change
removes from the whole-frame decode over the frozen workloads, and carries the
behavioural check beside it: the two generations agree on the iteration count of
every censused frame and neither makes a bit error against the frozen `c077a88b`
per-frame evidence.

### Throughput

The before/after and comparator campaigns are in the tables' "Campaigns"
section, each with its acceptance verdict, its `qualifies` flag, its finding
count on its own Source line, its journaled placement and its per-arm iteration
distribution. The single-worker pilot of each family is accepted and measures
both codes; both pilots put the changed decoder ahead of the path it replaces
and narrow the comparator gap the predecessor measured, at intervals the tables
carry. A pilot decides nothing: it fixes the resolution its confirmation freezes.

## Adoption

Adoption is decided by the `ldpc-update-single-worker-v1` and
`ldpc-update-multicore-v1` families against the frozen worthwhile-effect and
non-regression margins, not by the pilots. The single-worker confirmation
addendum is frozen from its committed pilot receipt by the canonical freezer
([addendum](addendum-ldpc-update-single-worker.json), with its
[derivation record](addendum-ldpc-update-single-worker-derivation.txt)), and the
comparator single-worker confirmation likewise
([addendum](addendum-ldpc-update-comparator-single-worker.json),
[derivation](addendum-ldpc-update-comparator-single-worker-derivation.txt)).
Those confirmations and the two multicore pilots exceed a working session's
timed budget and are queued for a benchmark window; the tables gain their cells
when they run, and a family in which nothing qualifies keeps the established
path and stays recorded exactly as the evaluator records it.

The tree carries the replacement now, on the strength of the accepted pilots'
direction, the allocation counter and the behavioural suite. What the queued
confirmations decide is whether that replacement clears its own frozen margins;
until they report, no cell of this issue records an adoption.

## Reproduction

From the worktree root: extract the `c077a88b` recorded-input archive the
[build identity](../../bench_results/07ca8585/preparation/build-identity.json)
names into `target/ldpc-inputs`, build the `3be770d5` harness once from the
working tree and once from an export of the pinned pre-change revision with the
commands that file records, and run
[record-preparation.py](survey/record-preparation.py). Then
[run-smoke.sh](../../bench_results/07ca8585/run-smoke.sh) proves the wire
contract of every arm on a throwaway plan, and
[run-campaign.sh](../../bench_results/07ca8585/run-campaign.sh) `FAMILY MODE
RUN_ID prepare` followed by `window` runs a campaign. The allocation evidence is
untimed and runs outside the mutex:
[record-alloc-census.py](survey/record-alloc-census.py) and
[record-allocation-counter.sh](survey/record-allocation-counter.sh). A
confirmation refuses to run until its addendum is committed and unmodified.
[summarize.py](survey/summarize.py) regenerates the tables from the committed
evidence and reproduces them byte for byte.
