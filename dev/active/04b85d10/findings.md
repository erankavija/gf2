# Bit-storage costs in production consumers

## Question and outcome

Which current production consumers spend enough time in packed logical
operations, population/fused reductions, or transpose/bitslice conversions to
justify a Zen 3 experiment, and which apparent kernel opportunities disappear
inside the whole consumer?

The survey selects three actionable experiment families and preserves four
important negative results:

1. The current packed binary BCH batch entry point selects
   `PolyRemainderScalar` for the measured $m=14$, $B=256$ row even though its
   already-registered `ClmulFold` and `BitsliceInterleaved` routes have a large
   whole-consumer gap. This is the strongest consumer opportunity, but this
   issue changes no selector or kernel.
2. Dense matrix-vector multiplication spends 80.21% of sampled cycles in the
   fused AVX2 AND-popcount kernel, so population-count experiments have a
   direct production consumer. Mid-range row XOR has a smaller, separately
   measurable dispatch/threshold opportunity.
3. The detected AVX2 64x64 transpose is about twice as fast as the scalar
   primitive, but it is only 32.26% of the sampled 4096x4096 dense-transpose
   consumer. A replacement must include the outer tile assembly, output
   allocation, and scratch traffic.
4. Replacing a full syndrome population count with an early-exit nonzero test
   is **not material** in the measured DVB-T2 LDPC whole-codeword check: sparse
   syndrome construction consumes 99.63% of sampled cycles. The isolated
   spelling matters only when a set bit occurs early. Likewise, row dispatch
   is not material for large cache-resident rows, BCH output allocation is not
   material beside the current scalar recurrence, and RREF is not primarily a
   logical-kernel problem.

These are profiling and experiment-selection findings only. No production
kernel, selector, encoder, or decoder is changed, and no independent BCH
campaign is started.

## Evidence and method

Two complementary artifacts are used:

- The [diagnostic profile export](../../bench_results/04b85d10/2026-09-07-04b85d10-profile/profile-summary.md)
  sweeps 168 current production routes. It records per-call wall time,
  allocation counts/bytes, setup and conversion phases, hardware counters,
  999 Hz sampled call graphs, correctness checks, and release disassembly.
  Each sweep row is one exploratory run and deliberately has no inferential
  interval. It locates and explains costs; it is not used for an adoption
  decision.
- The [protocol pilot](../../bench_results/04b85d10/2026-09-08-04b85d10-consumers-pilot/acceptance-summary.md)
  contains 15 predeclared cells with eight paired executions per cell and five
  fixed timing windows per arm execution. Its ratio-of-medians and percentile
  bootstrap interval are recomputed from the raw pairs by the independent
  acceptance tool. Every cell is exploratory, so every outcome remains
  `pilot` and the receipt does not qualify a production selection.

The diagnostic profiler export is not itself a Zen 3 protocol receipt. Running
the current acceptance tool over it exits 2 with `No such file or directory`
because the directory has no protocol `receipt.json`; the exact command and
verdict are preserved in [acceptance-verdict.txt](../../bench_results/04b85d10/2026-09-07-04b85d10-profile/acceptance-verdict.txt).

Before timing, `consumer-verify` checks row XOR and population-count backends
over nine boundary-spanning lengths, both zero-test spellings over four
patterns, scalar/detected transpose equivalence and involution, dense
transpose round trips, dense matvec against a naive parity reference, LDPC
syndrome spellings on zero/weight-one/random words, and all available BCH
families over three code rows and five batch sizes. The committed
[`verify.jsonl`](../../bench_results/04b85d10/2026-09-07-04b85d10-profile/verify.jsonl)
records eight passed groups and zero failures. The protocol launcher repeats
that verifier before its first timed cell.

### Pinned baseline

The baseline is the current pre-change gf2 implementation, built as an external
path consumer with `simd` and `parallel`, release ThinLTO, and one codegen unit.
The diagnostic run used Rust 1.97.0/LLVM 22.1.6 on Linux 7.2.2 and a Ryzen 9
5900X with AVX2, POPCNT and PCLMULQDQ, SMT on, and the `powersave` governor. Its
three executable SHA-256 identities and every harness-source digest are in
[`host.txt`](../../bench_results/04b85d10/2026-09-07-04b85d10-profile/host.txt).
The production crate/build inputs are unchanged from its recorded revision
`aa8d6724` through this branch's measured pilot. Source-control revision is
navigation only; the accepted pilot pins the exact executable bytes, toolchain,
producing-input closure, and receipt-local contract, protocol, schema and
frozen addendum bytes.

The frozen pilot addendum is
[`addendum-bit-storage-consumers-pilot.json`](addendum-bit-storage-consumers-pilot.json),
SHA-256 `8f1d6e0f9f58d8038d9cd61e6d616b95c6ca801fce4e48fbaba85e5d15fdfdae`.
The release build completes before timing; timed children run only under
`dev/scripts/ccx1-bench-flock.sh --full-host`. The launcher uses bounded
four-cell sessions and the canonical append-only journal/checkpoint resume
mechanism.

Protocol version 1 governs this exploratory receipt. The announced version-2
amendment is not present in this checkout, and no version-1 confirmation is
claimed as final. This issue needs no confirmation because it adopts nothing;
downstream candidate issues must freeze and run their own version-current
confirmatory before/after campaigns.

## Actual production routes

The route names below are observed routes, not intended designs.

| Consumer class | Public entry point to executed kernel | Observed result |
|---|---|---|
| Logical row | `gf2_core::kernels::ops::xor_inplace` calls `resolve_xor_inplace(len)` on every call, then `select_backend_for_size`; below eight words it returns `scalar_xor_inplace`, and at or above eight words on this host it reaches `SimdBackend::xor` then `avx2_xor_into`. The resolved comparison hoists the returned function pointer. `BitMatrix::row_xor` uses the same public operation. | Four words selects scalar; 8–8192 words select AVX2. |
| Dense RREF | `gf2_core::alg::rref::rref(matrix, false)` selects block width 8 above 512 columns, clones the matrix, resolves one XOR function for Gray-table construction, builds one table per pivot block, and applies table rows through the inline `BitMatrix::row_xor_slice_from`. | 1024x1024 is `blocked-m4ri`; AVX2 XOR appears only in table construction, not every elimination row. |
| Dense parity/matvec | `BitMatrix::matvec` calls `matvec_route(stride_words)`; the 4096-column row has 64 words, selects SIMD, resolves `maybe_simd`, and calls `matvec_simd`, whose per-row parity is `LogicalFns::and_popcnt_fn` -> `avx2_and_popcnt`. The output is appended to a fresh `BitVec`. | 1024x4096 is `simd-and-popcnt`; 80.21% of sampled cycles land in the fused kernel. |
| Sparse LDPC parity | `LdpcCode::syndrome` calls its `SpBitMatrixDual` row-oriented `SpBitMatrix::matvec`, which walks CSR indices one bit at a time and appends the syndrome to a fresh `BitVec`. | Both DVB-T2 lengths take `csr-bit-at-a-time-matvec`; there is no logical word kernel below it. |
| Population count | `BitVec::count_ones` -> `kernels::ops::popcount` -> `select_backend_for_size`; four words selects scalar, while eight words and above select `SimdBackend::popcount` -> `avx2_popcnt`. | The dispatch boundary is eight words in the conservative portable build. |
| Any-nonzero | `LdpcCode::is_valid_codeword` constructs `syndrome`, then calls `syndrome.count_ones() == 0`. The comparison route constructs the same syndrome and calls `find_first_one().is_none()`, which reaches `avx2_find_first_one` and can stop after the first nonzero vector. | Full-count and first-set spellings are bit-identical; whole-consumer gain is negligible at $n=64800$. |
| Block transpose | `gf2_kernels_simd::transpose::detect` observes AVX2 and returns `transpose_64x64_avx2`; the portable comparison calls `transpose_64x64_scalar`. | Production detects `avx2-bit-twiddle`, not the available PSHUFB alternative. |
| Dense transpose | `BitMatrix::transpose` resolves `maybe_transpose` once, allocates the output matrix, calls `transpose_route`, and invokes the detected 64x64 primitive from `transpose_inner_loop`. | 1024x1024 takes the simple loop; 4096x4096 takes the macro-tiled loop with eight blocks per edge. |
| Current packed BCH batch | `BinaryBchCode::encode_batch_into` builds the systematic plan, calls the one canonical `select_family`, then `encode_batch_family_into` -> `encode_partition`. For the current conservative profile and $m=14$, $B=256$, selection is `PolyRemainderScalar`; it reaches `BitVec::encode_systematic_with` once per message and `packed_write_codeword`. Forced family arms use the public `encode_batch_family_into`; bitslice reaches `packed_bitslice_batch`, detected transpose/absorb/unpack kernels, and `packed_write_codeword`, while fold reaches `packed_fold_reduce` and PCLMULQDQ. | The actual current route is scalar, not bitslice. Every forced family is correctness-equivalent before timing. |
| Allocating/parallel packed BCH | `BinaryBchCode::encode_batch` allocates the output collection and uses thread-local scratch. `encode_batch_parallel_into` selects the same family once and recursively partitions across caller workspaces through `rayon::join`. | Current $m=14$ selection remains `PolyRemainderScalar` at one, six, twelve and twenty-four declared workers. |
| DVB-T2 compatibility BCH | `BchEncoder::encode_batch` maps `BchEncoder::encode` over messages. `encode` expands message bits into `Gf2mPoly`, shifts, calls field-polynomial `div_rem`, adds the remainder, and bit-appends a new codeword. | This separate public route is `field-polynomial-div-rem`; it does not reach the packed batch selector or transpose/bitslice kernels. |

## Why these sizes represent consumers

The word ladder brackets the conservative eight-word SIMD cutover and spans
cache regimes: 4, 8 and 16 words cover the cutover, 64 words are one L1-small
row, 512 words make a 256 KiB 64-row bank, and 8192 words make a 4 MiB bank.
Population count additionally uses 127 and 507 words (the packed widths of
the two DVB-T2 rate-1/2 syndrome outputs), 4096 words (32 KiB), and 65536
words (512 KiB). Every row bank has 64 rows so it is a whole number of 64x64
blocks and makes per-row comparisons stable.

Dense 256, 512, 1024 and 2048 squares move from L1-sized through L2 and L3;
the 1024x4096 matvec gives 64-word rows and a 512 KiB matrix, exposing the
fused parity kernel without exceeding one core's L2 by more than metadata.
The 1024- and 4096-square transpose cells are 128 KiB and 2 MiB packed
matrices and select the simple and macro-tiled outer loops respectively.
Block counts 1, 16, 256 and 4096 bracket per-call overhead, L1, L2 and L3
working sets.

The LDPC rows are the DVB-T2 rate-1/2 short and normal frame lengths 16200 and
64800, not synthetic sparse matrices. BCH degrees 8, 14 and 16 are benchmark
row B3 and the mother fields of the DVB-T2 short/normal rows. Batch sizes 1,
16, 64, 256 and 1024 bracket single-frame overhead, one bitslice wave,
additional lane groups, cache-resident steady state, and parallel work. The
ranked pilot uses $m=14$, $B=256$: its packed message plus codeword working set
is about 445 KiB, the declared L2-resident steady-state row in
`dev/active/4e732b56/workload-selection.md`. The parallel cells use $B=1024$
so every declared worker receives useful work.

## Measured ratios and intervals

<!-- PILOT_RESULTS_BEGIN -->
The finalized receipt
[`2026-09-08-04b85d10-consumers-pilot`](../../bench_results/04b85d10/2026-09-08-04b85d10-consumers-pilot/acceptance-summary.md)
is accepted with zero findings, does not qualify for production selection, and
has receipt digest
`df45ad5784a356701fda313221d072b1c2cc48433c090fc63c684cbc644b2060`.
It completed in four bounded sessions with checkpoint/resume. The addendum
snapshot and source addendum both hash to
`8f1d6e0f9f58d8038d9cd61e6d616b95c6ca801fce4e48fbaba85e5d15fdfdae`.

| Cell (baseline -> candidate) | Candidate speedup | 95% paired-bootstrap interval | Samples | Result for experiment selection |
|---|---:|---:|---:|---|
| Row XOR, 64 words (`ops-dispatched` -> `ops-resolved`) | 1.0861x | [1.0652, 1.1088] | 8 pairs; 80 windows; 0 flagged | Mid-sized dispatch resolution is measurable. |
| Row XOR, 4 words (`ops-dispatched` -> `simd-backend`) | 1.3360x | [1.3294, 1.3463] | 8 pairs; 80 windows; 0 flagged | The current cutover leaves a sizeable short-row SIMD opportunity. |
| Dense RREF, 1024 square (`current-a` -> identical `current-b`) | 1.0051x | [1.0032, 1.0092] | 8 pairs; 80 windows; 0 flagged | Control pair resolves a sub-percent drift; it is not an optimization arm. |
| LDPC syndrome, n=64800 (`current-a` -> identical `current-b`) | 0.9953x | [0.9864, 1.0099] | 8 pairs; 80 windows; 0 flagged | No material route-level drift; sparse matvec remains the baseline. |
| Popcount, 4 words (`ops-dispatched` -> `scalar-backend`) | 1.5042x | [1.5028, 1.5092] | 8 pairs; 80 windows; 0 flagged | Dispatch plus the selected short-input route is material. |
| Popcount, 8 words (`scalar-backend` -> `simd-backend`) | 1.1661x | [1.1601, 1.1689] | 8 pairs; 80 windows; 0 flagged | SIMD wins at the current boundary. |
| Any-nonzero, 507 words with middle bit set (`count-ones` -> `find-first-one`) | 1.1241x | [1.1215, 1.1285] | 8 pairs; 80 windows; 0 flagged | Early exit helps an isolated representative syndrome. |
| LDPC validity, n=64800 (`count-ones` -> `find-first-one`) | 1.0043x | [0.9935, 1.0097] | 8 pairs; 80 windows; 0 flagged | **Not material** in the whole consumer; syndrome construction dominates. |
| 256 block transposes (`transpose-scalar` -> `transpose-detected`) | 1.9281x | [1.9071, 1.9441] | 8 pairs; 80 windows; 0 flagged | Detected AVX2 is materially faster in the isolated primitive. |
| BCH m=14, B=256 (`family-reference` -> `family-bitslice`) | 2.4841x | [2.4814, 2.4930] | 8 pairs; 80 windows; 0 flagged | Bitslice is materially faster than the path current selection chooses. |
| BCH m=14, B=256 (`family-reference` -> `family-fold`) | 2.7344x | [2.7297, 2.7377] | 8 pairs; 80 windows; 0 flagged | Fold has the largest measured whole-consumer single-core benefit. |
| Allocating BCH m=14, B=256 (`current-a` -> caller-buffer `current-b`) | 1.0013x | [0.9987, 1.0019] | 8 pairs; 80 windows; 0 flagged | **Not material** while scalar recurrence dominates. |
| 4096 block transposes on 6 cores (`transpose-scalar` -> `transpose-detected`) | 1.9583x | [1.9261, 2.0087] | 8 pairs; 80 windows; 0 flagged | Primitive benefit survives the six-core work distribution. |
| Current BCH parallel path, 12 cores (`current-12-a` -> identical `current-12-b`) | 0.9790x | [0.8945, 1.1028] | 8 pairs; 80 windows; 0 flagged | **Not material / unresolved**; the control interval spans substantial noise. |
| Current BCH parallel path, 24 logical CPUs (`current-24-a` -> identical `current-24-b`) | 1.0142x | [0.9221, 1.0639] | 8 pairs; 80 windows; 0 flagged | **Not material / unresolved**; SMT adds no supported current-path claim. |
<!-- PILOT_RESULTS_END -->

All ratios above are candidate speedups (baseline median divided by candidate
median); values above one favor the named candidate. Each interval is the
protocol's whole-pair percentile bootstrap over $n=8$ paired executions, with
five raw timing windows per arm execution. They are exploratory resolution
evidence, not family-wise confirmatory intervals and not adoption decisions.

## Attribution: time, allocation, traffic and limits

The figures in this section are descriptive single-run diagnostics drawn from
the raw JSONL/counter/profile files beside `profile-summary.md`. Call-graph
shares come from one approximately three-second 999 Hz sampling session per
route (`perf report` displays about 2K–3K retained samples); those correlated
samples are not assigned an inferential confidence interval. All candidate
ranking instead uses the paired bootstrap intervals above.

| Consumer | Diagnostic attribution | Allocation/conversion traffic | Limit evidenced |
|---|---|---|---|
| 64-row x 64-word XOR | Dispatched 390.6 ns/call versus resolved 373.2 ns/call; 32 row-pair operations occur per call. | Zero timed allocations. Approximately 48 KiB of useful two-load/one-store row traffic per call gives about 126 GB/s from the measured elapsed time. | L1 traffic dominates; hoisting has only a small ceiling. Front-end stalled cycles are 0.47% of cycles. |
| 64-row x 8192-word XOR | Dispatched 39.28 us/call and resolved 39.57 us/call in the sweep. | Zero allocations; about 6 MiB useful traffic per call, approximately 160 GB/s over the repeatedly resident 4 MiB bank. | Dispatch is below sweep noise; cache/bandwidth is the useful limit. L1-load miss rate is 25.0%, cache-miss/reference rate 1.68%, front-end stalls 1.58% of cycles. |
| Dense RREF 1024x1024 | 0.915 ms/call. Sampled share: RREF body 73.17%, AVX2 XOR 13.37%, XOR wrapper 1.83%, allocator symbol 0.50% plus several libc memory symbols. | 267 allocations and 2,391,008 allocated bytes per call; peak live 172,608 bytes. One-shot input construction is 118 us and outside the consumer call. | 4.49 instructions/cycle and 0.96% front-end stalled cycles show no front-end/dependency-starved logical kernel. Rebuilt M4RI tables and matrix-copy traffic dominate the bit-storage cost; an XOR-only speedup has a 13.37% sampled-share ceiling. |
| Dense matvec 1024x4096 | 17.66 us/call; `avx2_and_popcnt` is 80.21% and `matvec_simd` 17.52% of sampled cycles. | One 128-byte output allocation/call; output append proxy 0.900 us, selector proxy below the integer-nanosecond clock resolution, setup 368 us once. | 3.47 instructions/cycle, 2.05% front-end stalls, 12.0% L1-load misses and 1.84% last-level cache misses/reference. The fused reduction is the actionable dependency/instruction path; useful matrix reads are about 29 GB/s. |
| DVB-T2 LDPC $n=64800$ | Syndrome 310.4 us; full validity check 311.0 us and early-exit spelling 310.1 us. Sparse matvec is 99.63% of sampled validity-check cycles. | One 4056-byte syndrome allocation/call; output-fill proxy about 24 us; no dispatch phase. | 3.69 instructions/cycle, 4.00% front-end stalls, 2.01% L1-load misses and 1.54% cache misses/reference. The count pass is not material; sparse gathers/branches and syndrome construction are the limit. |
| 507-word any-nonzero | All-zero worst case: count 78.3 ns, find-first 69.6 ns. First-bit-set best case: count 78.7 ns, find-first 5.1 ns. | Zero allocations and conversions. | Early exit can be about 15.5x on the favorable pattern but only 1.13x when both spellings scan the buffer. The first-search path trades fewer instructions for data-dependent branches; the all-zero run records 1.05% front-end stalls versus 0.04% for count. |
| 4096/65536-word SIMD popcount | 567 ns over 32 KiB and 9.22 us over 512 KiB. | Zero allocations. Useful read rates are about 58 and 57 GB/s. | Nearly identical useful bandwidth across the two sizes and less than 0.20% front-end stalls identify a load/execute-throughput plateau rather than dispatch. The 512 KiB row has 33.3% L1-load misses but only 1.68% cache misses/reference. |
| 256 block transposes | Scalar 24.64 us; detected AVX2 12.81 us. | Zero heap allocations. | AVX2 retires 2.61 instructions/cycle with 0.04% front-end stalls; the scalar path retires 5.21 instructions/cycle but roughly 2.9x as many instructions per elapsed second. The primitive is instruction-work limited, not dispatch limited. |
| Dense transpose 4096x4096 | 666 us/call; outer `BitMatrix::transpose` is 62.57% and AVX2 block transpose 32.26% of sampled cycles. | One 2 MiB output allocation/call, 2 MiB peak live; selector proxy 4 ns; one-shot input setup 1.33 ms. Minimum input+output useful traffic is 4 MiB/call, about 6.3 GB/s. | 2.91 instructions/cycle, 1.20% front-end stalls, 18.1% L1-load misses and 17.2% cache misses/reference. Tile assembly, intentional block scratch and output/cache traffic bound whole-consumer benefit. |
| Packed BCH $m=14$, $B=256$ | Current/reference 45.7/45.4 ms; bitslice 18.8 ms; CLMUL fold 17.4 ms in the sweep. Reference sampled shares are 62.67% recurrence and 37.05% `packed_write_codeword`; bitslice is 92.13% batch family plus 6.99% isolated AVX2 reduce. | Workspace current still records 256 allocations and 14,336 bytes/call; batch fill about 0.31–0.43 ms, workspace construction about 6.4 us, dispatch 3 ns. | Reference, bitslice and fold retire 1.64, 1.37 and 1.38 instructions/cycle with 6.99%, 8.46% and 9.08% front-end stalls. L1 miss rates remain below 0.12% for all: arithmetic/control dependencies and bitwise codeword writing, not bandwidth, limit this L2-resident row. |
| Allocating packed BCH $m=14$, $B=256$ | 45.68 ms/call, indistinguishable from workspace current in the sweep. | 513 allocations and 561,152 bytes/call versus 256 and 14,336 for the caller-buffer path; peak live 546,872 bytes. | The extra output allocation is **not material** while the scalar recurrence dominates. Preserve it as a negative result; retest only after a faster family makes allocation an appreciable share. |
| Compatibility DVB BCH | $n=7200$, $B=16$: 204.9 ms/call, 272 allocations and 12.78 MiB allocated/call. $n=32400$, $B=1$: 66.8 ms, 21 allocations and 3.65 MiB/call. The $n=7200$, $B=1$ call graph assigns 86.49% to `BchEncoder::encode` and 12.53% to field multiplication. | Conversion to field-polynomial coefficients, division temporaries and fresh codeword storage occur inside every encode. | This is a separate field-polynomial route, not evidence for a transpose kernel. It is recorded for the existing migration owner; REQ-05 forbids turning it into an independent encoder campaign here. |

The generic `stalled-cycles-backend` perf event is unavailable on this Zen 3
host (`No supported events found`), so no backend-stall percentage is
fabricated. The committed core groups quantify front-end stalled cycles, IPC,
branch misses, and the cache groups above. Release disassembly then identifies
the actual dependency shapes: serial scalar parity/remainder accumulators,
four scalar matvec accumulators, one vector accumulator in AVX2 AND-popcount,
and the staged mask/shift/XOR transpose. Downstream experiments should use
supported Zen 3 IBS or named raw PMU events if they need a causal backend-stall
breakdown.

### Scratch buffers versus compiler spills

[`asm/frame-traffic.txt`](../../bench_results/04b85d10/2026-09-07-04b85d10-profile/asm/frame-traffic.txt)
classifies one bounded disassembly of every relevant routine against the source
declaration that settles intent:

- `avx2_{and,popcnt,and_popcnt,xor,find_first_one}` have no frame traffic.
- BCH parity unpack (18 frame stores/4 loads), dense transpose (77/110), and
  both scalar and AVX2 64x64 transpose (235/235 and 40/19) use intentional
  algorithmic scratch buffers.
- Bitslice reduce scalar/AVX2 (4 stores each), scalar/PCLMUL fold (9/10),
  packed table/fold reduce (16/33 and 4/10), dense matvec scalar/SIMD (15/27
  and 9/15), and sparse matvec (13/24) declare no local buffer; their frame
  traffic is compiler spill/register pressure.
- `packed_write_codeword` has no frame traffic, so its 37.05% reference share
  is bit-at-a-time input/output work rather than stack scratch.

## Any-nonzero opportunities

The following production sites ask only whether a packed syndrome is zero and
therefore can be measured with `find_first_one().is_none()` (or a canonical
`any_nonzero` spelling) instead of completing a population count:

- `LdpcCode::is_valid_codeword` in `crates/gf2-coding/src/ldpc/core.rs` —
  directly measured here and **not material** at DVB-T2 normal-frame size.
- ORBGRAND's candidate loop in `crates/gf2-coding/src/grand/orbgrand.rs` —
  potentially material because the loop tests many incrementally updated
  syndromes and favorable candidates may exit early; measure pattern
  position/distribution before changing it.
- BP-OSD's corrected-word check in
  `crates/gf2-coding/src/osd/bp_osd.rs` — one post-correction syndrome check;
  likely bounded by the parity-check matvec, so it needs a whole-decoder cell.
- Product-code row and column validity loops in
  `crates/gf2-coding/src/product/mod.rs` — each syndrome can exit early and the
  outer loops already stop at the first invalid component; measure invalid
  word distributions and matrix extraction together.

Count uses in conformance tests, assertions, weight/parity calculations, and
APIs returning the exact Hamming weight are not candidates: they require the
full count or do not affect production performance.

## Ranked candidate experiments

1. **Packed BCH family selection and complete bitslice/fold consumer
   (`1d4fd63d` as the conversion consumer).** Hypothesis: selecting an existing
   non-reference family for the current $m=14$, $B=256$ packed entry point
   materially reduces whole-consumer time because the exploratory gap is much
   larger than dispatch/setup noise and the cell is arithmetic-bound. Measure
   both `ClmulFold` and `BitsliceInterleaved`, including batch fill, workspace,
   parity unpack/transposes, `packed_write_codeword`, allocations, and the
   current selector. Do not infer the winner from the sweep; freeze a
   version-current before/after family and preserve losing rows. This issue
   does not change the selector.
2. **Fused AND-popcount in dense matvec (`5cbb6545`).** Hypothesis: a Zen 3
   population-count candidate that improves the existing nibble-LUT fused
   kernel can materially improve 1024x4096 dense matvec because 80.21% of its
   sampled cycles execute that kernel. Compare scalar POPCNT, current AVX2 and
   CSA/Harley-Seal across the row-width/cache ladder, then require a
   whole-matvec interval with output allocation included. Preserve small and
   memory-throughput no-win regions.
3. **Eight-to-64-word logical dispatch/threshold (`2037941f`).** Hypothesis:
   hoisting dispatch or adjusting/unrolling the selected AVX2 row operation can
   help repeated mid-range row consumers, while it cannot help 8192-word rows
   whose dispatch disappears into cache traffic. Freeze neighboring 4/8/16/64
   word cells and actual `BitMatrix::row_xor`/RREF consumers; require the
   whole-consumer confidence bound, not an isolated AVX2 label.
4. **Complete transpose consumer (`1d4fd63d`).** Hypothesis: PSHUFB or an
   adapter-complete movemask geometry can improve the isolated detected block
   transform, but whole dense transpose is capped by the current block's
   32.26% sampled share unless it also reduces tile/scratch/cache traffic.
   Compare the same little-endian 64x64 relation, tails and non-square shapes;
   include tile assembly, packing/unpacking and the packed BCH consumer. A
   faster isolated transform that loses after conversion is a retained
   negative result.
5. **Early-exit any-nonzero in repeated candidate checks (`5cbb6545`).** The
   directly measured LDPC validity substitution is **not material** and should
   not justify a library change alone. Hypothesis: ORBGRAND may benefit because
   it performs the check repeatedly and the position of the first nonzero bit
   varies. Profile that whole consumer first; include all-zero worst case and
   early/middle/late nonzero distributions. BP-OSD and product-code checks are
   secondary only if their whole-consumer attribution clears the frozen
   materiality threshold.
6. **RREF table/scratch reuse, not another XOR kernel (`2037941f` only if its
   scope is amended by the lead).** Hypothesis: reducing the 267 allocations
   and 2.39 MiB of per-call allocation traffic could help RREF; accelerating
   XOR alone cannot exceed its 13.37% sampled share. This is lower priority and
   outside the named mid-range kernel story unless tracked explicitly.

The allocating packed BCH path, large-row dispatch, LDPC full-count spelling,
and XOR-only RREF idea remain documented not-material/no-win findings. They
must not disappear when downstream issues select only the top experiments.

## Criterion-by-criterion outcome

- **REQ-01 — MET for profiling/selection.** The accepted exploratory release
  receipt pins the frozen version-1 protocol, measurement contract, addendum,
  exact executable and producing inputs; raw samples, host/lock evidence,
  journal, checkpoints, correctness run and independent acceptance summary are
  committed. No production change is adopted, so before/after confirmation is
  neither claimed nor required here. Version 2 is absent and explicitly left
  to downstream confirmatory work.
- **REQ-02 — MET.** The 168-row diagnostic sweep and 15-cell accepted pilot
  cover logical row/RREF/dense parity/sparse parity, count/fused reduction,
  block/dense transpose, packed BCH family/bitslice/parallel/allocation routes,
  and the separate current DVB BCH entry point. The tables above state the
  actual public-entry-to-kernel routes and justify every ladder.
- **REQ-03 — MET.** Paired intervals and sample counts quantify ranked gaps;
  raw profiler shares, allocation bytes/counts, setup/pack/unpack/batch-fill/
  dispatch phases, counter ratios, cache working sets/useful bandwidth and
  generated code explain the limits. The unsupported generic backend-stall
  event is reported rather than fabricated; front-end stalls and assembly
  dependency chains are preserved. Scratch and spill traffic and all
  production any-nonzero candidates are classified.
- **REQ-04 — MET.** Both the diagnostic baseline export and accepted pilot are
  committed, the ranked experiments carry family-specific consumer/materiality
  hypotheses, and negative/not-material findings remain explicit. The baseline
  is the pinned current pre-change executable rather than historical encoder
  numbers.
- **REQ-05 — MET.** Changes are limited to profile harness portability,
  receipts and findings. There is no kernel optimization, tuning selection,
  encoder/decoder implementation change, or independent BCH campaign.
