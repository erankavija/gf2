# Design: short-message BCH batch encoding (rows B1 and B2)

Design for @/issue/60c82c4c (row B2, BCH(127, 64), $t = 10$, against bchlib
@/citation/Bchlib2026). Row B1 of @/issue/5e71e993 runs the same encoder path
and reuses this design; its document holds only the B1 specifics.

## Measurement basis

Every number here is an indication: host fraktaali (AMD Ryzen 9 5900X),
`taskset -c 2-5`, 2026-11-12, gf2 at commit `540483f2b`, `--release` with
`-C target-cpu=native`, rustc 1.97.0. The gf2 side ran a driver built outside
the tracked tree over the public API; it times the call the cutover adapter
`experiments/encode-compare/src/cutover/current.rs` times
(`BchCode::encode_batch`, descending layout) and also
`encode_batch_family_into` over one workspace with preallocated codewords.
The comparator side ran one round of the external arm of
`experiments/encode-compare/cutover.sh` (`encode-compare cutover round 1`,
`CUTOVER_CELLS=W1/B1,W1/B2`). Counters are `perf stat -e cycles:u,instructions:u`,
profiles `perf record -e cycles:u -F 999`. The night-window table of
@/issue/fd9d5416 supersedes the rates.

At batch 4096 bchlib with `CONFIG_BCH_CONST_PARAMS` ($m = 7$, $t = 10$)
takes 19.1 ns per message (3356 Mb/s) and gf2's `encode_batch`, which selects
`clmul-fold`, 82.5 ns (776 Mb/s). Over one workspace, `clmul-fold` takes
64.5 ns, `table-remainder` 52.6 ns and `bitslice-interleaved` 44.3 ns. The
driver's lean single-block Barrett loop (see below) takes 4.7 ns.

gf2's cost per message is flat in $k$: row B1 ($k = 5$) costs 83 ns per
message on the same path. The time is overhead around a reduction that is two
carry-less multiplies.

## Profile

`encode_batch_family_into`, `clmul-fold`, B2, batch 4096: 295 cycles and
1120 instructions per message (IPC 3.8, so instruction- and stall-bound rather
than memory-bound). The causes, with their share of `cycles:u`, are
numbered below:
`packed_fold_reduce` 27 % (cause 3), `BitVec::write_run` 25 % (cause 2),
`validate_batch` with the `malloc`, `free` and `Arc<FieldIdRepr>::drop_slow`
it triggers 21 % (cause 1), the reduction kernels `fold_chunks_pclmul` and
`fold_blocks_pclmul_safe` 12 %, and `packed_place_codeword` with the family
dispatch 8 %.

The allocating `encode_batch` adds 17 ns per message: one heap `BitVec` per
codeword from `vec![S::zeroed(..); len]`, freed when the caller drops it.

1. `validate_batch` (`crates/gf2-coding/src/bch/encode.rs`) calls
   `validate_symbol_field` per message, which calls `F::field_id_hint()` only
   to test `is_some()`. For `Fp<2>` that is `FieldId::prime(2)`, which builds an
   `Arc<FieldIdRepr>` (a 56-byte `malloc`) and drops it. The annotated hot
   instructions are the `malloc` call and the store into the fresh block.
2. `BitVec::words_mut` (`crates/gf2-core/src/bitvec.rs`) writes
   `*self.rank_select_index.get_mut().unwrap() = None` on every call. The
   compiler builds the `None` on the stack with an 8-byte store and reloads it
   as a 32-byte `vmovdqu`; that reload misses store forwarding and takes 65 %
   of `write_run`'s samples. `packed_place_codeword` calls `write_bits` at
   least twice per codeword.
3. For $r = 63$ the fold prepares $L = \max(W + 1, \lceil 7 / W \rceil) = 7$
   accumulators ($W = \lceil r/64 \rceil = 1$, PCLMULQDQ in-flight 7). A
   64-coefficient message is one block, so each message pads six zero blocks,
   multiplies seven chunk constants, and then `fold_accumulator_finish`
   reverses the seven accumulator words into a stack array (a second
   store-forwarding miss on the `vpermpd` reload), searches for the first
   nonzero word and Barrett-reduces up to seven blocks. Two `div` instructions
   (`% accumulators`) are also on the per-message path.
4. Per call, `validate_workspaces` recomputes `encode_stamp`: an FNV chain over
   $n$, $k$, both field presentations and the whole defining set, one dependent
   multiply per element. At batch 1 the fixed per-call cost is about 110 ns
   against 65 ns per message at batch 4096 (driver clock read included).

## Why the comparators are faster

- bchlib `bch_encode` (`$GF2_EXT_ROOT/040bc2b8/bchlib/src/bch.c`) is the Linux
  kernel table remainder: four 256-entry tables of
  $\lceil mt/32 \rceil = 2$ 32-bit words, one step per 32 message bits, so B2
  is two steps of four lookups. `CONFIG_BCH_CONST_PARAMS` makes the ecc word
  count a compile-time constant and the inner loop disappears. The call has
  no validation, no allocation and writes a preallocated byte buffer. Its
  per-message work exceeds the two carry-less multiplies gf2's fold already
  needs; it wins on overhead.
- gf2's `table-remainder` family is the same algorithm (32-bit blocks, four
  8-bit tables) and measures 52.6 ns, so the algorithm is not the gap.

## Proposed change

Six changes, none altering any codeword.

1. **Hoist the field-identity check** (gf2-coding, `crates/gf2-coding/src/bch/encode.rs`).
   `validate_batch` decides `F::field_id_hint().is_some()` once per batch and
   skips the per-message `validate_symbol_field` when the type carries the
   identity. `FieldId::prime` allocating an `Arc` per call is the gf2-core
   defect behind this cost; @/issue/c6b20bec removes it at its source, after
   which the per-message check costs no allocation on any path and the hoist
   saves only a branch per message.
2. **Guarded index invalidation** (gf2-core, `crates/gf2-core/src/bitvec.rs`). `words_mut` clears
   the rank/select slot only when it holds an index:
   `let slot = ..get_mut()..; if slot.is_some() { *slot = None; }`. This takes
   the store-forwarding stall off every `BitVec` writer in the workspace, safe
   code, no API change.
3. **Cached encode stamp** (gf2-coding, `crates/gf2-coding/src/bch/spec.rs`/`crates/gf2-coding/src/bch/encode.rs`). `BchCode`
   computes its stamp once at construction (the code is immutable) and
   `validate_workspaces` compares one word.
4. **Block fold when a chunk cannot fill** (gf2-coding, `crates/gf2-coding/src/bch/encode.rs`).
   `fold_accumulators_for` takes the dimension and returns zero, selecting the
   existing `fold_blocks` path, when the message has fewer than $L$ blocks:
   $\lceil k/64 \rceil < L$. The rule derives from the structure already used
   to size $L$ (block count against the multiplier's in-flight depth), so it
   satisfies @/inv/structural-performance-defaults; the existing
   `clmul_fold_accumulators` selector stays the override and admits zero as
   "block fold". With zero accumulators the staging run, the finish and both
   divisions leave the per-message path.
5. **Batched block fold** (kernel in gf2-kernels-simd, caller in gf2-coding).
   `SystematicKernel::encode_batch_family` gains a `ClmulFold` arm, as
   `BitsliceInterleaved` already has, used when 4 selects the block fold. It
   stages the message windows of a group of $G$ frames, frame-major, and makes
   one dispatched call to a new `BchFoldFramesFn`
   (`fn(registers: &mut [u64], low: &[u64], redundancy: usize, barrett: u64, blocks: &[u64], frames: usize)`)
   that advances the $G$ independent remainders in lockstep, so the
   multiply chains of different frames overlap and the dispatch, bounds checks
   and accumulator load/store are paid once per group. $G$ derives from the
   L1 footprint of $G \cdot (W + \lceil k/64 \rceil)$ words, with
   `BITSLICE_LANES` (64) as the structural default; frames within a group
   advance in lockstep up to the in-flight depth divided by $1 + W$. The
   PCLMULQDQ arm lives in `crates/gf2-kernels-simd/src/x86/bch_encode.rs`
   behind the existing `detect()` features; the portable arm is the same
   generic body over `clmul_u64_scalar`, as `fold_blocks_over` already is.
   No new `EncodeFamily`: the bytes, the registry and the profile schema are
   unchanged.
6. **Codeword placement without per-window calls** (gf2-coding). For the
   batched arm, `packed_place_codeword` assembles the $\lceil n/64 \rceil$
   output words of a codeword in a local array (message words, then the
   reversed parity window for the descending layout) and writes them with one
   `write_bits`. Changes 2 and 6 together remove `write_run` from the top of
   the profile.

The default route stays the walk over `EncodeFamily::preference`; on a host
with PCLMULQDQ it selects `clmul-fold` for B1 and B2 at every batch size, as
today, and change 4 picks the block fold structurally. A caller writes no new
parameter (@/inv/ergonomic-defaults).

**Scalar fallback.** The portable bundle (`bch_encode::scalar()`) gets the
same `BchFoldFramesFn` over the portable multiply, selected where `detect()`
returns `None` and under `force_scalar_encode_kernels`. Changes 1 to 4 and 6
are safe Rust outside the kernel crate.

## Target and harness step (planned)

Charter decision D-7 (`dev/research/2caf738d-charter.md`) fixes the counted
cell: it times the gf2 entry point whose output contract equals the
reference's. bchlib and AFF3CT write into caller-provided buffers, so REQ-02
reads `BchCode::encode_batch_into` with a reused workspace and preallocated
codewords. The allocating `encode_batch` is reported beside it as its own row
and carries no 1.0 requirement; D-4 of the same charter still asks that the
allocating default be fast, which route (b) covers.

After the night window of @/issue/fd9d5416, the current arm of the cutover
harness gains `encode_batch_into` as a second configuration in
`experiments/encode-compare/src/cutover/current.rs`, beside `encode_batch`.
The two configuration labels cite D-7: the `_into` row as the D-7 cell, the
allocating row as reported beside it. `experiments/encode-compare/cutover/summarize.py`
today reduces an arm to its fastest configuration and takes the first current
configuration as the ratio reference; it then reports each current
configuration as its own row with its own ratio and verdict, and the
pre-cutover comparison stays against `encode_batch`.

## Route (a), the target: `encode_batch_into`

Changes 1 to 6 take the D-7 cell to its ceiling; the estimate is in the table
below.

## Route (b), secondary: the allocating `encode_batch`

Under D-4 the allocating default gets the cheap fixes as well, measured as
the row beside the D-7 cell.

Changes 1 to 6 and @/issue/c6b20bec apply to this route unchanged. What they
leave is the codeword storage: `encode_batch` builds
`vec![S::zeroed(n, ..); len]`, one heap buffer per codeword cloned into an
88-byte `BitVec`, and the caller frees each. An indication on the same host,
cores and commit as above (driver building and dropping the batch's storage
alone, $n = 127$) costs that storage 8.6 ns per codeword at batch 16 and
16.7 to 17.4 ns at batch 256 and 4096; one contiguous allocation of the same
words costs 2.3 ns per codeword at batch 16 and 0.12 ns at batch 4096. The
$n = 15$ costs are within 1 ns of these.

Two ways to remove the per-codeword allocation, one per return type:

- **(b1) Existing return type `Vec<BitVec>`, inline storage.** Each `BitVec`
  owns its words, so no allocation can be shared across a `Vec<BitVec>`, and a
  pool or arena would need `BitVec`'s `Drop` to return buffers to it, a
  process-wide behavior every `BitVec` would pay. What removes the allocation
  inside the type is inline storage for short vectors: gf2-core's `BitVec`
  keeps up to two words in the struct and allocates above that. B1
  ($n = 15$) and B2 ($n = 127$) then allocate nothing per codeword; the batch
  pays one `Vec<BitVec>` allocation. Every `words`/`words_mut` call gains a
  representation branch, and `BitVec` is the workspace's most used type, so
  this is a gf2-core design change with its own conformance and performance
  evidence across all `BitVec` users.
- **(b2) Documented new return type, one allocation.**
  `BchCode::encode_batch_matrix(&self, messages: &[S], layout) -> Result<M, CodeError>`
  returns the batch as the code's matrix type `M` (`BitMatrix` for the packed
  binary code), codeword $i$ in row $i$, in one zeroed allocation. `M` is
  already a type parameter of `BchCode` and already the generator's type, so
  no new type is introduced; for `BitMatrix` the placement of change 6 writes
  `row_words_mut(i)` directly. The rustdoc states that row $i$ is the
  codeword `encode_batch` returns at position $i$, and the REQ-01 suite
  checks exactly that.

Recommendation: (b1). D-4 asks for the fix on `encode_batch` itself, and (b1)
gives it without changing that method's contract or the library's batch
convention (`&[BitVec]` in, `Vec<BitVec>` out, which the LDPC `encode_batch`
shares), so every producer of short `BitVec`s gains it. It is a gf2-core
change and needs its own issue with evidence across `BitVec` users. (b2) is
the fallback if that evidence rejects inline storage: as the companion method
above it leaves `encode_batch` itself unchanged, and moved into
`encode_batch`'s return type it would have to land at the batch convention's
source, every `encode_batch`, under @/inv/convention-convergence.

## Conformance tests (REQ-01)

All extend the shared suite in `crates/gf2-coding/tests/bch_encode_dispatch.rs`
rather than adding a parallel one (@/inv/shared-test-contracts).

- `registered_families_agree_on_the_corpus_rows` and
  `registered_families_agree_at_packed_word_boundaries` already compare every
  registered family against `EncodeFamily::REFERENCE` under both layouts; they
  run additionally under `force_scalar_encode_kernels(true)`, so the batched
  arm is checked on both bundles.
- Batch lengths 0, 1, 63, 64, 65 and $G \pm 1$ on rows B1 and B2, so a group
  boundary and a partial group are exercised.
- Dimension boundaries for the block-fold rule: narrow-sense codes with
  $k = 1$ ($m = 7$, $t = 32$), $k = 63$ ($m = 8$, $t = 30$), $k = 64$ (B2) and
  $k = 193 = 3 \cdot 64 + 1$ ($m = 9$, $t = 43$), each with messages whose only
  set coefficient is degree 0, 1, 63, 64 and $k - 1$ where those exist.
- A test that a selector of zero accumulators and the structural rule select
  the same bytes as a forced chunk fold.
- `crates/gf2-coding/tests/bch_encode_dispatch_allocation.rs`: the fold family over a workspace still
  allocates nothing (guards change 1's removal of the per-message `Arc`).
- gf2-core: a `BitVec` test that a rank query after `write_bits` still sees
  the written bits (the index invalidation of change 2 stays correct).
- Route (b2): for every corpus row, layout and the batch lengths above, row
  $i$ of `encode_batch_matrix` equals codeword $i$ of `encode_batch`, on both
  kernel bundles.

## Expected gain (estimate)

The driver prototype computes B2's codewords (descending layout, checked
equal to `encode_batch` on all 4096 messages) with one bit reversal, two
PCLMULQDQ and one reversal of the parity per message, in 4.7 ns; it writes a
plain word array, not `BitVec`s. That bounds the arithmetic.

| Path after the change | Estimate, ns per message | vs bchlib (21.0 ns at batch 16, 19.1 at 4096) |
| --- | --- | --- |
| (a) `encode_batch_into`, batch $\ge 16$, D-7 cell | 8 to 12 | 1.6 to 2.6 |
| (a) `encode_batch_into`, batch 1, D-7 cell (one clock read in the harness included) | 45 to 55 against bchlib's 48 | 0.9 to 1.1 |
| (b) `encode_batch` after changes 1 to 6 and @/issue/c6b20bec, batch 16 | 17 to 21 | 1.0 to 1.2 |
| (b) same, batch 256 and 4096 | 25 to 29 | 0.65 to 0.75 |
| (b1) `encode_batch` with inline `BitVec` storage, batch $\ge 16$ | 9 to 14 | 1.4 to 2.3 |
| (b2) `encode_batch_matrix`, batch $\ge 16$ | 8 to 14 | 1.4 to 2.6 |
| (b), (b1), (b2), batch 1 | 55 to 70 against bchlib's 48 | 0.7 to 0.9 |

Reasoning: the prototype's 4.7 ns plus a `BitVec` read and one word-array
write per message at L1 latency, plus amortized group dispatch, gives route
(a). Route (b) adds the measured storage cost per codeword (8.6 ns at batch
16, 17 ns above), which @/issue/c6b20bec does not touch since change 1
already takes the `FieldId` allocation out of the batch loop. (b1) replaces it
with the initialization of an 88-byte struct, (b2) with the contiguous
allocation's 0.1 to 2.3 ns. At batch 1 every allocating route pays at least
one allocation and free beside the harness's clock read, so it stays below
bchlib, which D-7 does not require of that row; the batch-1 D-7 cell rests
on change 3.

## Open question for the lead

1. **Batch 1.** If the window table shows bchlib ahead at batch 1 after this
   change, the remaining per-call costs are `select_family` (profile
   resolution) and the `reset_family` stamp comparison; resolving both once
   per workspace is the next step and needs a decision on whether a workspace
   may cache its selected family.
