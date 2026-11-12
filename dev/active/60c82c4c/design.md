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

| Row B2, batch 4096 | ns per message | Mb/s |
| --- | --- | --- |
| bchlib, `CONFIG_BCH_CONST_PARAMS` ($m = 7$, $t = 10$) | 19.1 | 3356 |
| gf2 `encode_batch` (selects `clmul-fold`) | 82.5 | 776 |
| gf2 `encode_batch_family_into`, `clmul-fold`, workspace | 64.5 | 993 |
| gf2 same, `table-remainder` | 52.6 | 1217 |
| gf2 same, `bitslice-interleaved` | 44.3 | 1446 |
| lean single-block Barrett loop (driver prototype, see below) | 4.7 | 13542 |

gf2's cost per message is flat in $k$: row B1 ($k = 5$) costs 83 ns per
message on the same path. The time is overhead around a reduction that is two
carry-less multiplies.

## Profile

`encode_batch_family_into`, `clmul-fold`, B2, batch 4096: 295 cycles and
1120 instructions per message (IPC 3.8, so instruction- and stall-bound rather
than memory-bound). Shares of `cycles:u`:

| Symbol | Share | Cause |
| --- | --- | --- |
| `packed_fold_reduce` | 27 % | chunk-fold scaffolding for a one-block message, see 3 |
| `BitVec::write_run` | 25 % | store-forwarding stall in `BitVec::words_mut`, see 2 |
| `validate_batch` + `malloc`/`free`/`Arc<FieldIdRepr>::drop_slow` | 21 % | one `FieldId` allocation per message, see 1 |
| `fold_chunks_pclmul` + `fold_blocks_pclmul_safe` | 12 % | the reduction itself |
| `packed_place_codeword`, family dispatch | 8 % | placement and per-message calls |

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

1. **Hoist the field-identity check** (gf2-coding, `encode.rs`).
   `validate_batch` decides `F::field_id_hint().is_some()` once per batch and
   skips the per-message `validate_symbol_field` when the type carries the
   identity. Separately, `FieldId::prime` allocating per call is a gf2-core
   defect; it is filed as its own issue (see open questions) rather than
   widened into this change.
2. **Guarded index invalidation** (gf2-core, `bitvec.rs`). `words_mut` clears
   the rank/select slot only when it holds an index:
   `let slot = ..get_mut()..; if slot.is_some() { *slot = None; }`. This takes
   the store-forwarding stall off every `BitVec` writer in the workspace, safe
   code, no API change.
3. **Cached encode stamp** (gf2-coding, `spec.rs`/`encode.rs`). `BchCode`
   computes its stamp once at construction (the code is immutable) and
   `validate_workspaces` compares one word.
4. **Block fold when a chunk cannot fill** (gf2-coding, `encode.rs`).
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
- `bch_encode_dispatch_allocation.rs`: the fold family over a workspace still
  allocates nothing (guards change 1's removal of the per-message `Arc`).
- gf2-core: a `BitVec` test that a rank query after `write_bits` still sees
  the written bits (the index invalidation of change 2 stays correct).

## Expected gain (estimate)

The driver prototype computes B2's codewords (descending layout, checked
equal to `encode_batch` on all 4096 messages) with one bit reversal, two
PCLMULQDQ and one reversal of the parity per message, in 4.7 ns; it writes a
plain word array, not `BitVec`s. That bounds the arithmetic.

| Path after the change | Estimate, ns per message | vs bchlib 19.1 ns |
| --- | --- | --- |
| `encode_batch_into` with workspace, batch $\ge 16$ | 8 to 12 | 1.6 to 2.4 |
| `encode_batch_into`, batch 1 (one clock read in the harness included) | 45 to 55 against bchlib's 48 | 0.9 to 1.1 |
| `encode_batch` (allocating), batch $\ge 16$ | 25 to 30 | 0.65 to 0.75 |

Reasoning: the prototype's 4.7 ns plus a `BitVec` read and one word-array
write per message at L1 latency, plus amortized group dispatch. The
allocating entry point keeps its measured 17 ns of `malloc`/`free` per
codeword, so it cannot reach 1.0 on B2 under this design; the batch-1 cell
rests on change 3 and on the harness's per-call clock read, which every arm
pays.

## Open questions for the lead

1. **Which gf2 entry point the cutover adapter times.** bchlib and AFF3CT
   write into preallocated buffers; gf2's adapter times the allocating
   `encode_batch`. Under @/inv/performance-deliverable the matched
   configuration is `encode_batch_into` with a workspace and preallocated
   codewords. Recommendation: add it to `current.rs` as a second configuration
   (the summary already takes an arm's fastest), keep `encode_batch` beside it.
   Without that, REQ-02 is out of reach on B2 (estimate above).
2. **`FieldId::prime` allocation.** File a gf2-core issue to make prime-field
   identities allocation-free (a static per prime or an inline repr), since
   every type-level identity query pays it; change 1 only removes it from the
   batch loop.
3. **Batch 1.** If the window table shows bchlib ahead at batch 1 after this
   change, the remaining per-call costs are `select_family` (profile
   resolution) and the `reset_family` stamp comparison; resolving both once
   per workspace is the next step and needs a decision on whether a workspace
   may cache its selected family.
