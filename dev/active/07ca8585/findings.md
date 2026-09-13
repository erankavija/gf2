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

## Adoption

The frozen non-regression and worthwhile-effect rules of the family addenda
decide adoption, and a family in which nothing qualifies keeps the established
path and stays recorded. The campaign design, the cells, the arms and the
outcome are in [plan.md](plan.md), and every figure is projected by the
committed generator into [tables](../../bench_results/07ca8585/tables.md).
