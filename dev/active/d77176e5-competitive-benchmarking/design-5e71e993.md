# Design: BCH row B1 batch encoding

Design for @/issue/5e71e993 (row B1, BCH(15, 5), $t = 3$, $m = 4$, against
AFF3CT @/citation/Cassagne2019). Row B1 runs the encoder path of row B2, and
the fix is the one in the design of @/issue/60c82c4c
(`dev/active/d77176e5-competitive-benchmarking/design-60c82c4c.md`); this document holds the B1 profile, the
comparator analysis and the B1 estimate.

## Measurement basis

The basis of the B2 design applies: indication, host fraktaali (AMD Ryzen 9
5900X), `taskset -c 2-5`, 2026-11-12, gf2 at commit `2b922c0dd`, `--release`
with `-C target-cpu=native`; the comparator side is one round of the external
arm of `experiments/encode-compare/cutover.sh` with `CUTOVER_CELLS=W1/B1`. The
night-window table of @/issue/fd9d5416 supersedes the rates.

AFF3CT's fastest configuration (`Encoder_BCH` int8 at batch 1,
`Encoder_BCH_inter` int32 above) takes 55, 13.9, 13.3 and 12.4 ns per message
at batch 1, 16, 256 and 4096. gf2's `encode_batch` takes 145 ns at batch 1 and
83 ns above. Over a workspace at batch 4096, `clmul-fold` takes 66 ns,
`bitslice-interleaved` 41 ns and `poly-remainder-scalar` 61 ns. The driver's
lean single-block Barrett loop takes 4.5 ns per message at batch 4096.

The batch-1 cells include one clock read per call in both harnesses
(`clock_gettime` was 13 % of the gf2 batch-1 profile).

## Profile

At $k = 5$, $r = 10$ the reduction is one 5-coefficient block, yet gf2 spends
the same 83 ns per message as on B2 ($k = 64$), and the bit-serial reference
family is faster than the carry-less fold. The B1 profile at batch 1 has the
B2 profile's top entries in the same order: `packed_fold_reduce` 14 %,
`BitVec::write_run` 12 %, `validate_batch` 8 %, `fold_blocks_pclmul_safe` 6 %,
`reset_family` 6 %, codeword allocation (`from_elem`, `free`) 9 %. The causes
are causes 1 to 4 of the B2 design: one `FieldId` allocation per message in
validation, the store-forwarding stall in `BitVec::words_mut`, a seven-
accumulator chunk fold padding a one-block message ($W = 1$, $L = 7$), and the
per-call `encode_stamp` recomputation.

## Why AFF3CT is faster

`Encoder_BCH::__encode` (`$GF2_EXT_ROOT/c077a88b/aff3ct/src/Module/Encoder/BCH/Encoder_BCH.cpp`)
is the bit-serial LFSR over one element per bit: $k r = 50$ element
operations for B1. `Encoder_BCH_inter`
(`$GF2_EXT_ROOT/c077a88b/aff3ct/src/Module/Encoder/BCH/Encoder_BCH_inter.cpp`)
runs the same LFSR on `mipp::Reg<B>` lanes, one frame per lane: on AVX2 that is
8 frames for int32 (the fastest B1 configuration above batch 1) and 32 for
int8, with a `Reorderer_static` transpose in and out. The work is small enough
at B1 that a frame costs about 12 ns including the transposes. AFF3CT has no
per-call validation, no allocation and writes caller buffers. The algorithmic
cost is above gf2's two carry-less multiplies; the gap is gf2's per-message
overhead, which the prototype's 4.5 ns per message shows is not intrinsic.

Inter-frame SIMD is not needed for B1 in gf2: the batched block fold of the B2
design (change 5) already overlaps independent frames' multiply chains, and
the bit-sliced family, which is gf2's inter-frame path, stays available.

## Proposed change

The six changes of the B2 design, unchanged. Each applies at B1 as at B2:
$\lceil k/64 \rceil = 1 < L$, so change 4 selects the block fold and change 5's
batched arm runs it; changes 1 to 3 and 6 are independent of the row.

## Conformance tests (REQ-01)

The B2 design's tests cover B1: the shared family-agreement suite in
`crates/gf2-coding/tests/bch_encode_dispatch.rs` already holds row B1 at
batches 1, 16, 256 and 4096 under both layouts; it gains the forced-scalar run,
batch lengths 0, 1, 63, 64, 65 and $G \pm 1$, and B1 messages whose only set
coefficient is degree 0, 1 and $k - 1 = 4$.

## Target and routes

Charter decision D-7 (`dev/research/2caf738d-charter.md`) makes the counted
cell `encode_batch_into` with a reused workspace, since AFF3CT writes into
caller-provided buffers; the allocating `encode_batch` is reported beside it
as its own row with no 1.0 requirement. The target route is therefore the B2
design's route (a), and the planned harness step is that design's: after the
night window, `experiments/encode-compare/src/cutover/current.rs` times
`encode_batch_into` beside `encode_batch`, each its own row, with labels
citing D-7.

Route (b) is secondary: under D-4 the allocating default gets the cheap fixes
too. Its remaining cost after changes 1 to 6 and @/issue/c6b20bec is one heap
buffer per codeword (8.6 to 9.0 ns at batch 16 and about 17 ns at batch 256
and 4096 in the B2 design's indication, within 1 ns at $n = 15$), removed by
inline `BitVec` storage (b1, the recommendation) or by the matrix-returning
`encode_batch_matrix` (b2).

## Expected gain (estimate)

| Path after the change | Estimate, ns per message | vs AFF3CT (13.9 ns at batch 16, 12.4 at 4096) |
| --- | --- | --- |
| (a) `encode_batch_into`, batch $\ge 16$, D-7 cell | 7 to 10 | 1.2 to 2.0 |
| (a) `encode_batch_into`, batch 1, D-7 cell (harness clock read included) | 45 to 55 against 55 | 1.0 to 1.2 |
| (b) `encode_batch` after changes 1 to 6 and @/issue/c6b20bec, batch 16 | 16 to 19 | 0.73 to 0.87 |
| (b) same, batch 256 and 4096 | 24 to 28 | 0.45 to 0.55 |
| (b1) `encode_batch` with inline `BitVec` storage, batch $\ge 16$ | 8 to 12 | 1.0 to 1.7 |
| (b2) `encode_batch_matrix`, batch $\ge 16$ | 7 to 12 | 1.0 to 2.0 |
| (b), (b1), (b2), batch 1 | 55 to 70 against 55 | 0.8 to 1.0 |

Reasoning: the prototype's 4.5 ns plus a `BitVec` read and a one-word codeword
write per message ($n = 15$) and amortized group dispatch gives route (a);
route (b) adds the storage cost per codeword, (b1) an 88-byte struct
initialization in its place and (b2) the contiguous allocation's 0.1 to
2.3 ns. B1 has less margin than B2 on every route, since its 5-bit messages
leave AFF3CT's per-frame cost low.

## Open question for the lead

1. The batch-1 margin is the thinnest cell of REQ-02 on B1; if the window table
   leaves it below 1.0 after the change, the remaining per-call costs are the
   ones the B2 design's open question names.
