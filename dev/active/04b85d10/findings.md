# Bit-storage costs in production consumers

> **Diátaxis Type:** Explanation

Survey for `04b85d10`. No production kernel, selector, encoder or decoder is
changed and no independent BCH campaign is started. The
[measurement contract](../1a379447-zen3-cpu-performance/measurement-contract.md)
and [protocol version 3](../f547c394/protocol.md) govern the six receipts; the
[generated receipt tables](../../bench_results/04b85d10/tables.md) are the
numerical projection of every cell, and the
[profile attribution summary](../../bench_results/04b85d10/2026-09-10-04b85d10-profile-v3/attribution-summary.md)
is the projection of every counter and call graph cited below.

## Question and outcome

Which current production consumers spend enough time in packed logical
operations, population and fused reductions, or transpose and bitslice
conversions to justify a Zen 3 kernel experiment, and which apparent kernel
opportunities disappear inside the whole consumer?

Three protocol-v3 families answer it, one per consumer group and per
downstream issue. All six receipts are accepted with zero findings; none
qualifies for production selection, because each family contains a
confirmed not-material cell and because this issue adopts nothing.

1. **Layout.** The packed binary BCH batch entry point selects
   `PolyRemainderScalar` for the DVB-T2 normal-frame mother code
   ($m = 14$, $B = 256$) although its registered `ClmulFold` and
   `BitsliceInterleaved` families are confirmed 2.70x [2.688, 2.706] and
   2.49x [2.483, 2.493] faster as whole consumers. After the fold, 96% of
   the remaining time is the bit-serial `packed_write_codeword`, which costs
   about 14.8 ms per 256-message batch in every family. The detected AVX2
   64x64 transpose is confirmed 1.93x [1.914, 1.943] faster than the portable
   primitive in isolation and 1.89x [1.876, 1.934] on the six-core streaming
   arm, but it is 31% of the sampled dense 4096-square transpose.
2. **Count.** The fused AVX2 AND-popcount is 80% of the sampled dense
   1024x4096 matvec. The conservative-portable scalar population count is a
   bit-twiddle sequence with no `POPCNT` instruction; the AVX2 route is
   confirmed 3.04x [2.916, 3.225] faster on a 512 KiB buffer and 1.20x at the
   eight-word cutover, while re-dispatching per call costs 1.54x on four words.
3. **Logical.** The four-word row XOR is confirmed 1.35x [1.348, 1.363] faster
   through the detected SIMD backend than through the current scalar cutover,
   and hoisting dispatch out of an eight-word row loop is confirmed 1.12x
   [1.108, 1.122]. The same hoist is not material on 64-word rows under the
   family's 1.10 rule (1.057 [1.051, 1.098]) and absent on L3-resident
   8192-word rows (0.996 [0.943, 1.035]).

Confirmed not-material findings, preserved for the downstream issues: the
allocating BCH entry point against the caller-buffer one (1.002
[0.999, 1.008]); the full population count against the early-exit spelling in
the whole DVB-T2 LDPC validity check (0.998 [0.996, 1.003]); dispatch hoisting
on 64- and 8192-word rows. Descriptive, not campaigned: the DVB-T2
compatibility BCH encoder is a separate field-polynomial route at 11.25 ms per
Short-frame codeword, and the BCH batch path allocates 56 bytes per message in
its validation prologue at 0.011 ms per 1024 messages.

## Evidence and method

### Receipts

| Family (ledger) | Pilot receipt | Confirmation receipt | Widest pilot half-width; frozen resolution | Worthwhile / equivalence | $m$; per-comparison confidence |
|---|---|---|---|---|---|
| `bit-storage-logical-consumers` ([ledger](../../bench_results/04b85d10/v3-bit-storage-logical-consumers-family-ledger.jsonl)) | [`logical-v3-pilot`](../../bench_results/04b85d10/2026-09-10-04b85d10-logical-v3-pilot/acceptance-summary.md) `fe1c361b…` | [`logical-v3-confirmation`](../../bench_results/04b85d10/2026-09-10-04b85d10-logical-v3-confirmation/acceptance-summary.md) `bade034c…` | 0.065141; 0.07 ([derivation](pilot-resolution-v3-logical.txt)) | 1.10 / 1.10 | 4; 0.99375 |
| `bit-storage-count-consumers` ([ledger](../../bench_results/04b85d10/v3-bit-storage-count-consumers-family-ledger.jsonl)) | [`count-v3-pilot`](../../bench_results/04b85d10/2026-09-10-04b85d10-count-v3-pilot/acceptance-summary.md) `e0d76094…` | [`count-v3-confirmation`](../../bench_results/04b85d10/2026-09-10-04b85d10-count-v3-confirmation/acceptance-summary.md) `1b05b2d3…` | 0.020966; 0.03 ([derivation](pilot-resolution-v3-count.txt)) | 1.05 / 1.05 | 5; 0.995 |
| `bit-storage-layout-consumers` ([ledger](../../bench_results/04b85d10/v3-bit-storage-layout-consumers-family-ledger.jsonl)) | [`layout-v3-pilot`](../../bench_results/04b85d10/2026-09-10-04b85d10-layout-v3-pilot/acceptance-summary.md) (digest in its summary) | [`layout-v3-confirmation`](../../bench_results/04b85d10/2026-09-10-04b85d10-layout-v3-confirmation/acceptance-summary.md) `5e64f498…` | 0.020127; 0.03 ([derivation](pilot-resolution-v3-layout.txt)) | 1.10 / 1.05 | 5; 0.995 |

Every pilot cell and every confirmatory cell measured 24 pairs, five fixed
100 ms windows per arm execution, zero flagged windows. Pilots run at the
confirmatory sample size so the resolution they observe is the resolution the
confirmation has. The frozen addenda are
`addendum-bit-storage-{logical,count,layout}-v3-{pilot,confirmation}.json`
beside this file; each confirmation names its pilot receipt by path and
SHA-256 as resolution evidence, and P-03 recomputed the widest relative
half-width from the raw pairs. The logical family's pilot carried 1.05
margins; its L3-resident 8192-word row set the resolution at 0.07, so the
confirmation addendum raises both margins to 1.10 and records why. A cell
between 1.05 and 1.10 records not-material under that rule and its interval
is reported.

Each family's $m$ is the confirmatory cell count on its first and only
attempt ($\alpha_t = 0.05/2 = 0.025$, Bonferroni over $m$). With $m \le 5$
every cell keeps at least twenty expected bootstrap draws per tail, so P-20
reports no `not-confirmatory` cell. Identity-control cells (RREF, LDPC
syndrome, dense matvec, dense transpose, DVB BCH) are pilot-only: they size
the resolution and record each pinned consumer's baseline latency, and their
trivially passing non-regression would otherwise spend alpha.

The launcher is
[`run-consumer-campaigns.sh`](../../bench_results/04b85d10/run-consumer-campaigns.sh):
builds through `scripts/cargo-budget.sh`, correctness through
`consumer-verify` (eight checks, zero failures, recorded in every
`launcher.log`), timed sessions of at most three cells under
`dev/scripts/ccx1-bench-flock.sh --full-host`, checkpointed resume, and
`benchmark-acceptance` at finalization. Re-running the current acceptance
tool over all six receipts reproduces every summary byte for byte.

### Diagnostic profile

The [profile receipt](../../bench_results/04b85d10/2026-09-10-04b85d10-profile-v3/profile-summary.md)
(`2026-09-10-04b85d10-profile-v3`) sweeps 180 current production routes with
per-call wall time and allocation counts and bytes from a counting global
allocator, records user-space hardware counters for twenty routes, 999 Hz
DWARF call graphs for twelve whole consumers, allocation-site backtraces for
seven allocating consumers, and release disassembly of nineteen routines with
[frame-traffic classification](../../bench_results/04b85d10/2026-09-10-04b85d10-profile-v3/asm/frame-traffic.txt)
and [`POPCNT` attribution](../../bench_results/04b85d10/2026-09-10-04b85d10-profile-v3/asm/popcnt-attribution.txt).
Each sweep row is one run without an interval; the sweep locates and explains
costs, the receipts decide. Profiler availability was observed, not assumed:
`perf_event_paranoid` is 2, so `perf stat` and `perf record` work on this
process's own user-space events and the generic `stalled-cycles-backend`
event is unsupported; `valgrind --tool=dhat` 3.25.1 is installed and was not
needed because the counting allocator reports counts, bytes and sites. Its
launcher [`run-profile.sh`](survey/run-profile.sh) runs the whole timed session
inside one `--full-host` wrapper invocation.

### Pinned baseline

Every arm of every receipt is one executable, `consumer-arm` SHA-256
`960bb280fdc1f5d02cd2bad3569a87a791d35ccee08f031411a2be3ec75f4dfa`, built
with Rust 1.95.0 (the repository MSRV) without `RUSTFLAGS`: a
conservative-portable build whose SIMD kernels are selected at run time. The
producing closure is the issue-owned manifest
[`survey/producing-inputs.json`](survey/producing-inputs.json) (290 build
inputs: the three production crates the harness links, the harness and its
lockfile, the plan derivation, the launcher, the lock wrapper and the protocol
tooling), snapshotted into each receipt's `inputs/producing/`. Host: AMD Ryzen
9 5900X, Linux 7.2.2, SMT on, `powersave` governor, AVX2, PCLMULQDQ and POPCNT
present; the receipts pin protocol `1d42ac3c…`, contract `9f3c7563…` and
schema `513f3762…`. Every code claim below is a row of
[`survey/source-evidence.json`](survey/source-evidence.json) (71 claims, path,
line and verbatim text verified at generation), named here by claim id.

### History

Protocol-v1 evidence is immutable: the
[v1 pilot](../../bench_results/04b85d10/2026-09-08-04b85d10-consumers-pilot/acceptance-summary.md)
(`df45ad57…`, fifteen exploratory cells at eight pairs, Rust 1.97) and the
[v1 profile](../../bench_results/04b85d10/2026-09-07-04b85d10-profile/profile-summary.md).
Its addendum and launcher are kept byte-identical under `superseded/`. Three
things make it history rather than baseline: production sources changed
after it (`crates/gf2-kernels-simd/src/x86/clmul.rs`, `gf2m.rs`,
`crates/gf2-core/src/field/vec.rs`), its cell
`layout-bch-encode-alloc-1core` paired the allocating entry point against
itself while its report described a caller-buffer comparison, and several of
its scratch-versus-spill citations named `detect()` or a call site instead of
the routine they classified. The v3 families open their ledgers with the v1
pilot as a zero-comparison genesis line
([`ledger-genesis.json`](ledger-genesis.json)); no v1 confirmatory
reservation exists to import.

## Actual production routes

| Consumer | Public entry point to executed kernel (claim ids) | Observed in the receipts |
|---|---|---|
| Logical row | `BitMatrix::row_xor` calls `kernels::ops::xor_inplace`, which calls `resolve_xor_inplace(len)` on every call; below eight words that is `scalar_xor_inplace`, at or above eight words the detected `LogicalFns::xor_fn`, `avx2_xor_into` on this host (`row-xor-public-entry`, `xor-inplace-resolves-per-call`, `resolve-xor-simd-fn`, `simd-min-words-default`, `avx2-xor-into`). The baked four-word cutover exists only under `--cfg gf2_tuning_baked` (`baked-simd-min-words`). | 4 words `ops-dispatched/scalar`; 8, 64 and 8192 words `ops-dispatched/simd`. |
| Dense RREF | `alg::rref::rref` selects an eight-wide Gray table above 512 columns, clones the input, resolves one XOR kernel, allocates one table per pivot block and applies rows through the inline `row_xor_slice_from` (`rref-block-width`, `rref-clone`, `rref-resolves-xor-once`, `rref-table-per-block`, `rref-row-apply`). | 1024-square `current/blocked-m4ri`, 819 us. |
| Dense matvec | `BitMatrix::matvec` dispatches on the stride; eight or more words reach `matvec_simd`, whose per-row parity is `LogicalFns::and_popcnt_fn` (`avx2_and_popcnt`) appended to a fresh `BitVec`; the scalar route is private (`matvec-route`, `matvec-simd-min-words`, `matvec-fused-kernel`, `matvec-output-alloc`, `matvec-scalar-private`). | 1024x4096 `current/simd-and-popcnt`, 16.2 us. |
| Sparse LDPC parity | `LdpcCode::syndrome` is `SpBitMatrixDual::matvec`, which delegates to the CSR `SpBitMatrix::matvec`: one `x.get(c)` per stored index into a fresh `BitVec` (`ldpc-syndrome`, `ldpc-h-dual`, `ldpc-dual-matvec`, `ldpc-csr-bit-at-a-time`, `ldpc-syndrome-output-alloc`). | $n = 64800$ `current/csr-bit-at-a-time-matvec`, 274 us. |
| Population count | `BitVec::count_ones` is `ops::popcount`, re-dispatched per call at the same eight-word cutover to `avx2_popcnt` or the scalar fallback (`count-ones-entry`, `popcount-dispatch`, `simd-backend-popcount`, `avx2-popcnt`). | 4 words scalar, 8 and 65536 words `simd-backend/avx2`. |
| Any-nonzero | `LdpcCode::is_valid_codeword` computes the syndrome and tests `count_ones() == 0`; `find_first_one` reaches `avx2_find_first_one`, which can stop at the first nonzero vector (`ldpc-is-valid-count`, `find-first-one-simd`, `avx2-find-first-one`). | Both spellings bit-identical on zero, weight-one and random words. |
| Block transpose | `transpose::detect` publishes `avx2-bit-twiddle` under AVX2; the PSHUFB lane exists behind `detect_pshufb` and is not selected (`transpose-detect-avx2`, `transpose-pshufb-alternative`). | `transpose-detected/avx2-bit-twiddle` against `transpose-scalar/portable`. |
| Dense transpose | `BitMatrix::transpose` resolves the block kernel once, allocates the output, selects the outer loop through `transpose_route` and tiles through `transpose_inner_loop` with two 64-word local blocks (`dense-transpose-resolves-once`, `dense-transpose-output-alloc`, `dense-transpose-route`, `dense-transpose-tile-scratch`). | 4096-square `current/macro-tiled-8`, 654 us. |
| Packed BCH batch | `encode_batch_into` selects the family once through `select_family` and calls `encode_batch_family_into`, which runs one partition over the caller's workspace. The reference family is always admitted; bitslice and fold need the active profile's minimum batch, which the conservative profile does not grant at $B = 256$. Every per-frame family ends in `packed_write_codeword`, which copies the message one bit at a time (`bch-batch-into-selects`, `bch-batch-into-delegates`, `bch-family-into-partition`, `bch-family-admission`, `bch-bitslice-admission`, `bch-fold-admission`, `bch-reference-serial-reduce`, `bch-reference-write`, `bch-write-bit-at-a-time`, `bch-fold-reduce`, `bch-bitslice-batch`). | `current/PolyRemainderScalar` at $m = 14$, $B = 256$; forced arms `family-pinned/BitsliceInterleaved`, `family-pinned/ClmulFold`. |
| Allocating and parallel packed BCH | `encode_batch` allocates its output and reduces over thread-local scratch; `encode_batch_parallel_into` selects the family once and recurses through `rayon::join`, with no forced-family variant (`bch-allocating-entry`, `bch-allocating-scratch`, `bch-parallel-selects-once`, `bch-parallel-join`). | `current-allocating/PolyRemainderScalar` against `caller-buffer/PolyRemainderScalar`; parallel path in the sweep only. |
| DVB-T2 compatibility BCH | `BchEncoder::encode_batch` maps `encode`, which expands the message into `Gf2mPoly` coefficients and divides by the generator (`dvb-bch-batch-maps-encode`, `dvb-bch-field-poly`, `dvb-bch-div-rem`). | Short frame $n = 7200$ `current/field-polynomial-div-rem`, 11.25 ms per codeword. |

## Why these sizes represent consumers

Row widths 4, 8 and 64 words bracket the conservative eight-word cutover
and one L1-resident row; 64 rows make each bank a whole number of 64x64
blocks; 8192 words make a 4 MiB L3-resident bank where dispatch is expected
to vanish into cache traffic. 507 words is the packed width of the DVB-T2
rate-1/2 normal-frame syndrome; 65536 words (512 KiB) is one core's L2. The
1024-square RREF is the smallest square that selects the eight-wide table;
1024x4096 gives 64-word rows so the fused matvec kernel runs whole 32-byte
vectors on a 512 KiB matrix. The 4096-square transpose selects the
macro-tiled loop on a 2 MiB matrix. Block counts 256 and 4096 are an
L2-resident run and an L3-resident streaming run. $m = 14$, $B = 256$ is the
mother code of the DVB-T2 normal frame at the L2-resident steady-state batch
of `dev/active/4e732b56/workload-selection.md`; the sweep adds $m = 8$ and
$m = 16$ and batches 1 to 1024. The LDPC rows are the DVB-T2 rate-1/2 frames,
not synthetic matrices.

## Measured ratios and intervals

Speedups are baseline median over candidate median; values above one favour
the candidate. Per-arm medians, observed routes and conversion phases for
every cell, pilots included, are in
[tables.md](../../bench_results/04b85d10/tables.md).

| Family | Cell (baseline -> candidate) | Baseline -> candidate median | Speedup [interval] | Outcome |
|---|---|---|---|---|
| logical | 64-word row XOR, dispatched -> resolved | 338.9 -> 320.6 ns | 1.0570 [1.0510, 1.0975] | not-material at 1.10 |
| logical | 8-word row XOR, dispatched -> resolved | 110.4 -> 98.9 ns | 1.1161 [1.1082, 1.1218] | pass |
| logical | 4-word row XOR, dispatched (scalar) -> SIMD backend | 109.3 -> 80.8 ns | 1.3532 [1.3477, 1.3626] | pass |
| logical | 8192-word row XOR, dispatched -> resolved | 34.50 -> 34.65 us | 0.9956 [0.9426, 1.0353] | not-material |
| count | 4-word popcount, dispatched -> scalar backend | 7.3 -> 4.7 ns | 1.5439 [1.5392, 1.5481] | pass |
| count | 8-word popcount, scalar -> SIMD backend | 6.2 -> 5.1 ns | 1.2004 [1.1953, 1.2063] | pass |
| count | 65536-word popcount, scalar -> SIMD backend | 25.29 -> 8.31 us | 3.0439 [2.9160, 3.2251] | pass |
| count | 507-word all-zero test, count -> find-first-one | 69.0 -> 61.7 ns | 1.1174 [1.1111, 1.1207] | pass |
| count | LDPC validity $n = 64800$, count -> find-first-one | 272.2 -> 272.7 us | 0.9982 [0.9956, 1.0029] | not-material |
| layout | 256 block transposes, scalar -> detected AVX2 | 22.02 -> 11.42 us | 1.9276 [1.9136, 1.9430] | pass |
| layout | 4096 block transposes, six cores streaming | 365.1 -> 192.8 us | 1.8932 [1.8762, 1.9343] | pass |
| layout | BCH $m = 14$, $B = 256$, current -> bitslice | 40.388 -> 16.229 ms | 2.4886 [2.4830, 2.4933] | pass |
| layout | BCH $m = 14$, $B = 256$, current -> fold | 40.433 -> 14.984 ms | 2.6985 [2.6880, 2.7060] | pass |
| layout | BCH $m = 14$, $B = 256$, allocating -> caller buffer | 40.520 -> 40.428 ms | 1.0023 [0.9985, 1.0080] | not-material |

Pilot-only identity controls record the pinned baseline latencies: RREF
1024-square 819.0 us (1.0004 [0.9971, 1.0060]), LDPC syndrome 274.1 us
(1.0025 [0.9993, 1.0072]), dense matvec 1024x4096 16.19 us (1.0014
[0.9832, 1.0223]), dense transpose 4096-square 653.9 us (0.9993
[0.9961, 1.0061]) and DVB-T2 Short-frame BCH 11.252 ms (1.0015
[0.9978, 1.0033]); all at 24 pairs and 97.5% confidence. The 507-word zero
test declares `set_bit` 32448, which equals the buffer length: the buffer is
all zero, the valid-codeword case and the worst case for an early exit.
Twelve- and twenty-four-CPU arms are meaningful here only for the parallel
BCH entry point, whose only protocol-expressible cell is an identity control;
v1 measured it (0.979 [0.895, 1.103] and 1.014 [0.922, 1.064] at eight pairs)
and v3 records its scaling in the sweep instead.

## Attribution: time, allocation, traffic and limits

Figures are single-run diagnostics from the profile receipt; derived ratios
are in its attribution summary. A pass on a cell decides materiality; this
section explains where the time goes.

| Consumer | Sampled shares and per-call cost | Allocation and conversion traffic | Measured limit |
|---|---|---|---|
| 64-row x 64-word XOR | 344 ns dispatched, 322 ns resolved, 318 ns SIMD backend; 32 row pairs per call. | No allocation. About 49 KiB of load/load/store traffic per call, about 140 GB/s from L1. | IPC 2.65, front-end stalls 0.60%, L1 load misses 12.4%: an L1 store-bandwidth loop in which dispatch is a 6% term. |
| 64-row x 8192-word XOR | 35.3 us dispatched, 36.5 us resolved, 34.6 us SIMD backend. | About 6 MiB per call, about 178 GB/s over the resident 4 MiB bank. | IPC 2.03, L1 misses 28.8%, LLC misses 1.6% of references: cache traffic, and the confirmed interval spans one. |
| Dense RREF 1024 | 798 us; `rref` body 72.9%, `avx2_xor_into` 14.2%, `xor_fn` 1.7%, libc memory routines about 5%. | 267 allocations and 2.39 MiB per call (the working clone and one Gray table per pivot block), peak live 173 KiB; setup 106 us once. | IPC 4.62, front-end stalls 0.61%: a table-building and copying consumer whose XOR ceiling is the 14% kernel share. |
| Dense matvec 1024x4096 | 15.9 us; `avx2_and_popcnt` 80.2%, `matvec_simd` 17.1%, `and_popcnt_fn` 2.5%. | One 128-byte output allocation and bit-append (unpack proxy 1.05 us); dispatch below clock resolution. | IPC 3.46, front-end stalls 1.75%, L1 misses 10.7%, LLC misses 2.1%; matrix reads about 33 GB/s: the fused reduction is the actionable path. |
| LDPC syndrome $n = 64800$ | 269 us; `SpBitMatrix::matvec` 99.7% in the syndrome, the count spelling and the early-exit spelling alike. | One 4056-byte output allocation; output bit-append proxy 23 us. | IPC 3.73, front-end stalls 4.2%, branch misses 1.7%: bit gathers dominate; the count is below 0.5%. |
| 507-word zero test | Count 69 ns, find-first 62 ns on the all-zero buffer; 4.9 ns when bit 0 is set. | None. | Early exit is 1.12x when both scan the buffer and 14x when the first bit is set; the search path trades 29% fewer instructions for 0.36% branch misses and 1.0% front-end stalls. |
| 65536-word popcount | SIMD 8.0 us (65 GB/s) at 512 KiB and 505 ns (65 GB/s) at 32 KiB; scalar 25.9 us (20 GB/s). | None. | Constant useful bandwidth across L1 and L2 with L1 misses 6.4% and 33.3%: a load-and-execute plateau, not dispatch. The scalar fallback has no `POPCNT`: the binary's only `POPCNT` instructions are inside `avx2_and_popcnt` (20) and `avx2_popcnt` (10), and the 0x5555/0x3333 bit-twiddle constants sit in the scalar paths of `Prepared::run`, `is_valid_codeword` and `matvec_scalar`. |
| 256 block transposes | Scalar 24.9 us, AVX2 11.5 us; 1570 versus 543 instructions per block. | None. | AVX2 IPC 2.59 with 0.04% front-end stalls against scalar IPC 4.82: the lane is bounded by vector-unit throughput and its own dependency chain, not by dispatch or the front end. |
| Dense transpose 4096 | 600 us; `BitMatrix::transpose` outer loop 64.7%, `transpose_64x64_avx2` 31.2%. | One 2 MiB output allocation per call; input and output traffic 4 MiB, about 7 GB/s. | IPC 2.89, L1 misses 18.9%, LLC misses 26.1% of references: tile assembly, the two intentional 64-word scratch blocks and cache traffic bound the whole consumer. |
| Packed BCH $m = 14$, $B = 256$ | Current 40.0 ms: `encode_systematic_with` 62.8%, `packed_write_codeword` 37.0%. Fold 15.2 ms: `packed_write_codeword` 96.3%, `fold_block_pclmul` 3.2%. Bitslice 15.7 ms: batch body 91.9%, `bitslice_reduce_avx2` 7.6%. Table 16.3 ms. | 256 allocations of 56 bytes per call in `validate_batch` (0.011 ms per 1024 messages); batch fill 0.31 ms; workspace 6 us; dispatch 3 ns. | IPC 1.65, 1.38 and 1.44 with front-end stalls 8.1%, 12.0% and 8.8%, branch misses 5.4% to 8.0%, L1 misses under 0.12%: control and data dependencies, and above all the bit-serial codeword write of about 14.8 ms per batch, which is common to every family. |
| Allocating packed BCH | 40.2 ms allocating, 40.1 ms caller buffer. | 513 allocations and 561 KiB per call against 256 and 14 KiB. | Confirmed not material while the scalar recurrence and the bit-serial write dominate. |
| Parallel packed BCH | 41.6, 7.21, 4.33 and 3.61 ms at 1, 6, 12 and 24 workers ($B = 256$); 161, 27.6, 25.0 and 12.5 ms ($B = 1024$). | Same per-message allocation. | Single-run scaling of the current family only; the entry point exposes no forced-family arm. |
| DVB-T2 compatibility BCH | 11.25 ms per Short-frame codeword; `BchEncoder::encode` 86.9%, `Gf2mElement` multiplication 12.4%; 58.6 ms per Normal frame. | 272 allocations and 12.8 MiB per 16-frame call (message and shifted polynomials, division temporaries, fresh codeword). | IPC 1.20: a field-polynomial route. Recorded for its owner; not campaigned here. |

### Scratch buffers versus compiler spills

[`frame-traffic.txt`](../../bench_results/04b85d10/2026-09-10-04b85d10-profile-v3/asm/frame-traffic.txt)
counts frame stores and loads in one bounded Rust 1.95 disassembly of each
routine and joins them with the declaration that settles intent:

- No frame traffic: `avx2_xor_into`, `avx2_and_into`, `avx2_popcnt`,
  `avx2_and_popcnt`, `avx2_find_first_one`, `packed_write_codeword`.
- Intentional scratch: the portable 64x64 transpose (235 stores, 235 loads)
  and the AVX2 lane (40, 19) copy the block into a local they mutate in place
  (`transpose-scalar-scratch`, `transpose-avx2-scratch`); the tiled dense
  transpose (75, 115) holds `tile_in` and `tile_out`
  (`dense-transpose-tile-scratch`); `unpack_parity` (18, 4) transposes the
  bit-sliced register back through a block (`bch-unpack-parity`).
- Compiler spill or register pressure: `bitslice_reduce_{scalar,avx2}`
  (4 stores each), `fold_block_{scalar,pclmul}` (9, 10),
  `packed_table_reduce` (16, 33), `packed_fold_reduce` (4, 10),
  `matvec_scalar` (17, 27), `matvec_simd` (10, 15) and `SpBitMatrix::matvec`
  (22, 29) declare no local buffer.

## Any-nonzero opportunities

Production sites that ask only whether a packed syndrome is zero and spell it
as a full count, with the measured consequence where one exists:

- `LdpcCode::is_valid_codeword` (`ldpc-is-valid-count`): confirmed not
  material at $n = 64800$; the syndrome matvec is 99.7% of the check.
- The LDPC BP decoder's per-iteration early-termination check and terminal
  check (`ldpc-bp-early-termination`, `ldpc-bp-final-check`): the same
  consumer repeated per iteration, bounded by the same matvec.
- ORBGRAND's candidate loop (`orbgrand-count`): each candidate clones the
  base syndrome, XORs a few columns and counts, so the count is a comparable
  share of per-candidate work and the first nonzero bit's position varies;
  this is the one site where the spelling may be material.
- BP-OSD's post-correction check (`bp-osd-count`), product-code row and column
  validity (`product-row-count`, `product-col-count`) and GLDPC component
  checks (`gldpc-component-check`): one matvec per check, bounded by it.

Counts in conformance tests, assertions and APIs returning a Hamming weight
are not candidates.

## Ranked candidate experiments

Each entry names the consumer, the materiality hypothesis, the measured share
it could move and the downstream issue that tests it. Not-material findings
stay listed so the downstream issues do not rediscover them.

1. **Word-wise systematic codeword assembly for packed BCH (`1d4fd63d`).**
   Consumer: `encode_batch_into` on the DVB-T2 mother codes. Hypothesis:
   replacing the bit-serial `packed_write_codeword` with word copies and
   shifted parity placement removes most of the 14.8 ms per 256-message batch
   that every family pays. Share it could move: 96% of the fold consumer
   (15.0 ms) and 37% of the current one. This is a bit-layout packing change
   and belongs with the conversion costs `1d4fd63d` REQ-09 measures.
2. **Packed BCH family selection (`1d4fd63d`).** Consumer: the same entry
   point. Hypothesis: admitting `ClmulFold` or `BitsliceInterleaved` for
   $m = 14$ at $B \ge 16$ (sweep: fold 0.95 ms against 2.50 ms at $B = 16$)
   is a confirmed 2.70x and 2.49x whole-consumer gain at $B = 256$ under the
   current admission rule; the tuning profile, not a kernel, is the change.
   The bitslice family is the conversion consumer named in the issue.
3. **Fused AND-popcount in dense matvec (`5cbb6545`).** Consumer:
   `BitMatrix::matvec` on 64-word rows. Hypothesis: a Zen 3 fused reduction
   that beats the nibble-LUT `avx2_and_popcnt` moves up to 80% of a 15.9 us
   consumer at 33 GB/s and IPC 3.46; the whole-matvec cell must include the
   128-byte output allocation and bit-append.
4. **A `POPCNT`-enabled scalar arm and the four/eight-word popcount cutover
   (`5cbb6545`).** Consumer: `BitVec::count_ones`. Hypothesis: the
   conservative build's scalar fallback is a bit-twiddle sequence, so its
   REQ-07 "scalar POPCNT" comparison needs a feature-detected `popcnt` arm;
   against the current fallback the AVX2 route is 3.04x at 512 KiB and 1.20x
   at eight words, and per-call dispatch costs 1.54x at four words.
5. **Row-XOR cutover and dispatch hoisting at 4 to 8 words (`2037941f`).**
   Consumer: `BitMatrix::row_xor` and elimination loops. Hypothesis: the
   four-word row is confirmed 1.35x faster through the SIMD backend and the
   eight-word loop 1.12x with dispatch hoisted; 64 words is 1.057
   [1.051, 1.098], not material under a 1.10 rule and reportable under a rule
   the mid-range story declares itself; 8192 words is not material.
6. **Complete transpose consumer (`1d4fd63d`).** Consumer: `BitMatrix::transpose`
   and the bitslice parity unpack. Hypothesis: a PSHUFB or movemask block
   transform can improve the isolated primitive (detected AVX2 is already
   1.93x over scalar) but the whole 4096-square transpose is bounded by the
   64.7% outer loop, the 2 MiB output allocation and 7 GB/s traffic; tile
   assembly and packing must be in the cell.
7. **Early-exit any-nonzero in repeated candidate checks (`5cbb6545`).**
   Consumer: ORBGRAND. Hypothesis: the isolated 1.12x (all-zero) to 14x
   (first bit set) spread becomes material where the check is a large share
   of per-candidate work; the LDPC validity check is confirmed not material
   and must not justify a library change alone.
8. **RREF table and copy traffic, not another XOR kernel (`2037941f` only if
   its dense-matrix scope is amended).** Consumer: `alg::rref::rref`.
   Hypothesis: 267 allocations and 2.39 MiB per call with a 14% XOR share cap
   any XOR-only gain; reuse of the Gray table buffer is the lever.

Preserved negative results: the caller-buffer BCH entry point (1.002), the
LDPC full count (0.998), dispatch hoisting on 8192-word rows (0.996) and on
64-word rows under 1.10 (1.057), the 56-byte per-message allocation in the BCH
batch prologue (0.011 ms per 1024 messages), and the DVB-T2 compatibility
encoder, which reaches no packed kernel and is not this issue's campaign.

## Criterion-by-criterion outcome

- **REQ-01 — MET.** Six accepted v3 receipts pin the contract, protocol,
  schema and addenda by receipt-local digest, the issue-owned producing
  closure, the executable and toolchain, host and lock observations, journals
  and checkpoints; confirmations use fresh samples with resolution evidence
  from distinct pilots; negative and not-material outcomes are retained. No
  production change is adopted, so no before/after evidence is owed here.
- **REQ-02 — MET.** Profiles cover logical row/RREF/matvec/LDPC, count and
  fused-reduction, and transpose/bitslice consumers including the current
  packed BCH path and the DVB-T2 compatibility encoder; routes are verified
  citations and sizes are justified against the cache hierarchy and the
  DVB-T2 frames.
- **REQ-03 — MET.** Sampled shares, allocation counts and bytes with sites,
  conversion and dispatch phases, hardware-counter ratios, useful bandwidth
  and Rust 1.95 disassembly quantify each consumer; spills and intentional
  scratch are classified with verified citations; any-nonzero sites are
  enumerated with the one measured consumer.
- **REQ-04 — MET.** Baseline receipts measure the pinned pre-change
  executable; eight ranked experiments carry consumer, hypothesis, movable
  share and downstream issue; not-material findings are preserved.
- **REQ-05 — MET.** Changes are confined to `dev/active/04b85d10` and
  `dev/bench_results/04b85d10`; the existing runner, acceptance tool and
  wrapper are reused unchanged.
