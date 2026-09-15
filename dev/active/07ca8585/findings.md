# Allocation and edge traversal in the CPU min-sum LDPC update

> **Diátaxis Type:** Explanation

Design record and outcome for `07ca8585`. The
[measurement contract](../1a379447-zen3-cpu-performance/measurement-contract.md)
and the [shared protocol](../f547c394/protocol.md) govern every timed cell.
The predecessor `3be770d5` ranks the levers this issue spends; its
[findings](../3be770d5/findings.md) and
[tables](../../bench_results/3be770d5/tables.md) are the authoritative source
of every share, interval and labelled Amdahl ceiling referenced below.
Code claims are in [source-evidence.json](survey/source-evidence.json), written
by [make-source-evidence.py](survey/make-source-evidence.py), which refuses a
claim whose recorded line does not contain its fragment.

## Baseline update path

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
slot `f`. Those two orders are exactly the orders the baseline
`check_neighbors` and `var_neighbors` lists carry, so no message changes the
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

### The GPU device layout

The GPU LDPC stage derives its device layout from the same `EdgeLayout`, so its
hard decisions stay the CPU decoder's. On this host's gfx1030 they are:
[gpu-byte-identity.md](../../bench_results/07ca8585/preparation/gpu-byte-identity.md)
records the kernel build, the six byte-identity legs that exercise the LDPC stage
and their verdicts. All six pass, over both frozen workloads and all three
supported algorithms, so the device layout carries the canonical layout without
changing what the stage decides.

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

### Numerical contract

The canonical reduction follows the supported scalar reference in both rules
recorded under lever `numerical-contract`: an input's sign is taken by
comparison, so negative zero counts as positive and a NaN counts as negative,
and the magnitude fold skips a NaN input. The three min-sum variants, the empty
and degree-one cases, clipping and finite extrema are unchanged, and the
behavioural suite in
`crates/gf2-coding/tests/ldpc_check_update_contract.rs` asserts the shared
reduction against that reference directly.

### The disclosed numerical change

The pinned baseline decoder's min-sum and normalized min-sum paths do not always
reach that reference. With the default `simd` feature on a host whose
AVX2 kernel is selected, they reach `Llr::boxplus_minsum_n`, whose vector lanes
take the IEEE sign bit and propagate a NaN through `_mm256_min_ps`, while its
scalar tail does neither, so one kernel disagrees with itself and with the
scalar reference.

**The exact condition.** Only whole groups of eight inputs reach the lanes, so
the disagreement needs a check whose excluded input set fills a lane group: a
check of degree nine or more, carrying a negative-zero or NaN incoming message.

**Which frozen workload reaches it.** The DVB-T2 workload cannot: its checks are
of degree six and seven, so every excluded input set runs in the kernel's scalar
tail, which already follows the comparison rule. The NR BG1 workload can: it
carries checks of degree ten and nineteen, as the tables'
"Representative degree distributions" section records. Reaching it still needs a
negative-zero or NaN message, which a recorded channel LLR does not produce.

**The test that measures it.**
`the_public_reduction_api_and_the_scalar_contract_part_on_non_finite_messages`,
in `crates/gf2-coding/tests/ldpc_check_update_contract.rs`, decodes one
input set through both reductions on a code with degree-ten checks and asserts
that they differ and that the decoder follows the scalar one. The divergence is
therefore measured rather than claimed, and the test fails if a backend change
removes it. The sibling test
`decoder_matches_the_scalar_contract_on_saturating_and_extreme_llrs` asserts the
decoder against the scalar contract over the same extreme inputs.

**What the frozen workloads show.** The allocation census decodes the recorded
DVB-T2 and NR frames through both arms; the tables' "Allocation census of the
measured harness" section reports how many censused frames the two generations
agree on for the iteration count and how many each decodes with no bit error
against the frozen `c077a88b` per-frame evidence. The measured gap therefore
does not rest on the discrepancy.

`Llr::boxplus_minsum_n` itself is unchanged by this issue, and the kernel's
internal disagreement stays open under `@/issue/39cbde20`, which owns the
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

Every campaign is in the tables' "Campaigns" section with its acceptance
verdict, its `qualifies` flag, its finding count on its own Source line, its
journaled placement and, where its cells declare a decoder, its per-arm iteration
distribution. The before/after, full-decoding comparator, fixed-iteration and
check-node families each publish an accepted single-worker pilot and the accepted
confirmation frozen from it. The before/after confirmation qualifies the
canonical update on both workloads. The three comparator confirmations preserve
their material gaps in AFF3CT's favour and select no production route. The two
multicore pilots publish every physical-core and SMT cell as exploratory evidence;
their accepted-but-nonqualifying status remains explicit. A pilot decides
nothing.

### The residual gap

All three comparator confirmations leave the canonical decoder behind AFF3CT
[Cassagne2019] at one worker on both codes, by the factors their cells carry, and
the isolated check-node cells locate part of that gap in the update itself rather
than only in the whole decode. The predecessor's
[lever ranking](../../bench_results/3be770d5/tables.md) accounts for what remains
and assigns each part an owner. The levers this issue spends are canonical edge
indexing, allocation removal, the shared reduction, the flat layout and the
syndrome and termination work. What it does not spend are the three the same
ranking carries as comparator estimates with no gf2 mechanism: inter-frame SIMD,
tracked by `ed3d490e`, and quantized and layered decoding with QC-aware
intra-frame work, tracked by `f63a2464`. The predecessor's refutation rule for
this issue's levers is a re-sampled profile rather than a clock. The completed
[profile series](../../bench_results/07ca8585/v4-r1-resampled-profile/profile.md)
uses its session script, case set and arm catalogue unchanged over this issue's
content-pinned `after` executable. Its generated category tables contain no
`edge-position-search`, `min-sum-input-vec`, `min-sum-dispatch` or
`min-sum-reduction` category for any gf2 record case; the shared
`check-node-loop` is the dominant named decoder work that remains. The
[attribution ledger](../../bench_results/07ca8585/v4-r1-resampled-profile/attribution.tsv)
and cached inline chains resolve the sampled executable, and the append-only
execution log records every bounded session. Thus the sampled categories for
the levers this issue spends disappear without converting the remaining gap
into a claim of optimality.

### The three REQ-10 granularities

REQ-10 asks for the matched comparison at three granularities over the same
frozen workloads. Each is a family of its own, each has an accepted single-worker
pilot and a confirmation frozen from that pilot's committed receipt, and each
publishes a comparison without selecting anything.

**Full decoding** is the `3be770d5` steady-state operation unchanged, measured by
`ldpc-update-comparator-single-worker-v1` on both codes. Its frozen confirmation
publishes the single-core comparison, and the companion multicore pilot publishes
the physical-core and SMT comparison.

**Full iterations** is `ldpc-update-fixed-iteration-v1`: both arms decode at the
iteration cap with syndrome stopping off, so each performs the same declared
number of flooding iterations on every frame instead of stopping at a passing
syndrome. The measured harness refuses an arm whose settings differ from the
settings its prepared quality evidence was produced under, and the reused
`c077a88b` corpus was produced under syndrome stopping, so this issue produced
the corpus these settings require: `preparation/quality-fixed/`, untimed and
committed, every recorded frame decoded once per arm by
[ldpc-fixed-quality](survey/arms/src/bin/ldpc-fixed-quality.rs) from the same
frozen frames. The tables' "Prepared quality at the iteration cap" section
carries its counts, and the campaign launcher checks its digests before the arms
read it.

**Check-node updates** is `ldpc-update-checknode-v1`: one flooding check-node
pass over a prepared variable-to-check message array, with no variable update, no
termination rule, no conversion and no allocation inside the timed call. Neither
the measured harness nor the pinned AFF3CT shim exposes the update rule outside a
whole decode, so this issue adds one arm on each side, in
[its own workspace](survey/arms/) with its own target directory, which is why
every executable the frozen whole-decoding confirmations were built from stays
byte-identical. The gf2 arm runs `min_sum_check_row` over the canonical
`EdgeLayout` check runs; the AFF3CT arm runs AFF3CT's own
`tools::Update_rule_NMS` over the check-node scan order of its flooding decoder,
through a translation unit whose C entry points are disjoint from the pinned
shims' so a binary may link both.

Both arms read the same messages on the same edges. One function derives the
prepared array and both arms call it, so neither can prepare its own; each cell
freezes the checksum of that array and of the pass's output in the canonical
check-major edge order, and every worker of either arm reproduces both or the arm
fails. The matched-ness is measured rather than argued: the tables'
"Matched-ness of the isolated check-node arms" section records that the two
passes write bit-identical outputs on both codes and that AFF3CT's transpose is
the canonical check-edge-to-variable-edge map, which is what places the same
message on the same edge.

Scaling is REQ-10's second axis. The single-core arm of every family is measured.
The `ldpc-update-multicore-v1` and
`ldpc-update-comparator-multicore-v1` pilots measure both workloads at six and
twelve physical cores and at twenty-four logical CPUs. Their tables preserve the
full set of directions, including the DVB-T2 SMT cell in which the comparator
ordering reverses, without promoting exploratory cells into confirmation.

## Adoption

Adoption is decided by the `ldpc-update-single-worker-v1` family against the
frozen worthwhile-effect and non-regression margins, not by a pilot. Its
confirmation addendum is frozen from its committed pilot receipt by the canonical freezer
([addendum](addendum-ldpc-update-single-worker.json), with its
[derivation record](addendum-ldpc-update-single-worker-derivation.txt)), and the
comparator single-worker confirmation likewise
([addendum](addendum-ldpc-update-comparator-single-worker.json),
[derivation](addendum-ldpc-update-comparator-single-worker-derivation.txt)).
The two other REQ-10 granularity families freeze their own confirmations the
same way
([check-node](addendum-ldpc-update-checknode.json), with its
[derivation](addendum-ldpc-update-checknode-derivation.txt), and
[fixed-iteration](addendum-ldpc-update-fixed-iteration.json), with its
[derivation](addendum-ldpc-update-fixed-iteration-derivation.txt)); they publish
comparisons and select nothing, so no adoption follows them. The accepted
before/after confirmation qualifies on both frozen workloads, so the canonical
layout and shared reduction are the selected production route for the declared
single-worker domain. The multicore family measures independent decoder instances
running that same route rather than a distinct candidate; its exploratory results
characterize scaling and make no separate adoption claim. Every nonqualifying
pilot and every comparator direction remains in the tables exactly as its
evaluator records it.

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

The REQ-10 granularity arms are a second build, from
[survey/arms](survey/arms/) into its own target directory, with the command and
the digests
[kernel-build-identity.json](../../bench_results/07ca8585/preparation/kernel-build-identity.json)
records; [record-kernel-preparation.py](survey/record-kernel-preparation.py)
writes that file and the producing manifest those families' plans select. Their
untimed preparation runs outside the mutex:
[ldpc-checknode-verify](survey/arms/src/bin/ldpc-checknode-verify.rs) for the
matched-ness receipt and
[ldpc-fixed-quality](survey/arms/src/bin/ldpc-fixed-quality.rs) for the
fixed-stopping quality corpus. `run-campaign.sh checknode` and
`run-campaign.sh fixed-iteration` then run those families, and
[run-profile-resample.sh](../../bench_results/07ca8585/run-profile-resample.sh)
runs the re-sampled profile series. The GPU byte-identity legs need only a
gfx1030 and the two commands
[gpu-byte-identity.md](../../bench_results/07ca8585/preparation/gpu-byte-identity.md)
records.

[summarize.py](survey/summarize.py) regenerates the tables from the committed
evidence and reproduces them byte for byte.
