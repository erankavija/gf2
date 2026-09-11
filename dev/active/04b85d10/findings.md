# Bit-storage costs in production consumers

> **Diátaxis Type:** Explanation

Survey for `04b85d10`. No production kernel, selector, encoder or decoder is
changed and no independent BCH campaign is started. The
[measurement contract](../1a379447-zen3-cpu-performance/measurement-contract.md)
and [protocol version 3](../f547c394/protocol.md) govern the six receipts.

This report states no measured value. Each conclusion points to its source:

- **tables § `<receipt>`, row `<cell>`**: a receipt's section of the
  [generated receipt tables](../../bench_results/04b85d10/tables.md), the
  receipt directory shortened to its part after `2026-09-10-04b85d10-`, and a
  cell ID. The row holds the speedup with the acceptance tool's interval, the
  decision and outcome, each arm's median with its order-statistic interval,
  the observed routes, the fixture seed with its generator and the
  conversion probes.
- **sweep § `<section>`, row `<workload>` `<size>` `<route>`**: the repeated
  profile's
  [sweep summary](../../bench_results/04b85d10/2026-09-10-04b85d10-profile-v3-repeated/profile-summary.md),
  which gives each route's wall time per call as the median over the profile
  sessions with its order-statistic interval, its exact allocation counts and
  bytes, and the setup and conversion probes with what each times.
- **attribution § `<section>`, case `<case>`**: the repeated profile's
  [attribution summary](../../bench_results/04b85d10/2026-09-10-04b85d10-profile-v3-repeated/attribution-summary.md):
  counter ratios as medians with order-statistic intervals, each symbol's
  self samples with their count, share and Wilson interval, and the useful
  bandwidths, per-call symbol costs, instruction rates and bootstrapped ratios
  of two routes.
- **`asm/<file>`** and **`rep-01/alloc-trace/<case>.txt`**: exact counts from
  the measured executable's disassembly and allocation-site backtraces, in the
  same profile directory.

Each generated file states its sample counts and interval methods and names
every fixture seed beside its generator and version. `tables.md` names its
generator; [`run-profile.sh`](survey/run-profile.sh) writes the two profile
summaries with `survey/summarize-profile.py` and
`survey/summarize-attribution.py`.

## Question and outcome

Which current production consumers spend enough time in packed logical
operations, population and fused reductions, or transpose and bitslice
conversions to justify a Zen 3 kernel experiment, and which apparent kernel
opportunities disappear inside the whole consumer?

Three protocol-v3 families answer it, one per consumer group and per
downstream issue. All six receipts are accepted with zero findings (tables §
each receipt's heading); none qualifies for production selection, because
each family contains a confirmed not-material cell and because this issue
adopts nothing.

1. **Layout.** The packed binary BCH batch entry point selects
   `PolyRemainderScalar` for the mother code of the DVB-T2 short frame
   ($m = 14$, $B = 256$; the standard's short-frame BCH code is defined over
   $\mathrm{GF}(2^{14})$ and its normal-frame code over $\mathrm{GF}(2^{16})$
   [Etsi2015]) although its registered `ClmulFold` and
   `BitsliceInterleaved` families are confirmed faster as whole consumers
   (tables § `layout-v3-confirmation`, rows
   `layout-bch-encode-fold-m14-b256-1core` and
   `layout-bch-encode-bitslice-m14-b256-1core`). Once the fold removes the
   remainder cost, the bit-serial `packed_write_codeword` holds nearly all of
   the consumer's samples (attribution § Sampled shares, case
   `bch-m14-b256-clmul`), and it costs about the same per 256-message batch in
   every family (attribution § Per-call cost of one symbol). The detected AVX2
   64x64 transpose is confirmed faster than the portable primitive in
   isolation and on the six-core streaming arm (tables §
   `layout-v3-confirmation`, rows `layout-transpose-block-256-1core` and
   `layout-transpose-block-4096-6core`), but in the dense 4096-square
   transpose the outer tiling loop holds more samples than the kernel
   (attribution § Sampled shares, case `dense-transpose-4096`).
2. **Count.** The fused AVX2 AND-popcount holds most of the sampled dense
   1024x4096 matvec (attribution § Sampled shares, case
   `dense-matvec-1024x4096`). The measured executable contains no `POPCNT`
   instruction: the conservative-portable scalar population count is the
   bit-twiddle sequence, and `avx2_popcnt` and `avx2_and_popcnt` count bytes
   with `VPSHUFB` nibble lookups summed by `VPSADBW`
   (`asm/popcnt-attribution.txt`). Against that scalar fallback the AVX2 route
   is confirmed faster on a 512 KiB buffer and at the eight-word cutover, and
   re-dispatching per call is confirmed slower than calling the scalar backend
   directly on four words (tables § `count-v3-confirmation`, rows
   `count-popcount-bandwidth-65536w-1core`,
   `count-popcount-threshold-8w-1core` and
   `count-popcount-dispatch-4w-1core`). On an all-zero 507-word buffer the
   early-exit search is confirmed faster than the full count (row
   `count-zero-test-507w-1core`).
3. **Logical.** The four-word row XOR is confirmed faster through the detected
   SIMD backend than through the current scalar cutover, and hoisting dispatch
   out of an eight-word row loop is confirmed faster (tables §
   `logical-v3-confirmation`, rows `logical-row-xor-threshold-4w-1core` and
   `logical-row-xor-dispatch-8w-1core`). The same hoist is not material on
   64-word rows under the family's margin, and on L3-resident 8192-word rows
   its interval spans one (rows `logical-row-xor-dispatch-64w-1core` and
   `logical-row-xor-dispatch-8192w-1core`).

Confirmed not-material findings, preserved for the downstream issues: the
allocating BCH entry point against the caller-buffer one (tables §
`layout-v3-confirmation`, row `layout-bch-encode-caller-buffer-m14-b256-1core`);
the full population count against the early-exit spelling in the whole DVB-T2
LDPC validity check (tables § `count-v3-confirmation`, row
`count-ldpc-check-64800-1core`); dispatch hoisting on 64- and 8192-word rows.
Descriptive, not campaigned: the DVB-T2 compatibility BCH encoder is a
separate field-polynomial route (its pinned latency in tables §
`layout-v3-pilot`, row `layout-dvb-bch-encode-7200-control-1core`), and the
BCH batch path allocates one field identity per message in its validation
prologue (`rep-01/alloc-trace/bch-m14-b256-current.txt`; its cost in sweep §
Transpose, bitslice and BCH encoding consumers, rows `field-id-hint`).

## Evidence and method

### Receipts

| Family | Ledger | Pilot receipt | Confirmation receipt | Resolution derivation | Confirmation addendum |
|---|---|---|---|---|---|
| `bit-storage-logical-consumers` | [ledger](../../bench_results/04b85d10/v3-bit-storage-logical-consumers-family-ledger.jsonl) | [`logical-v3-pilot`](../../bench_results/04b85d10/2026-09-10-04b85d10-logical-v3-pilot/acceptance-summary.md) | [`logical-v3-confirmation`](../../bench_results/04b85d10/2026-09-10-04b85d10-logical-v3-confirmation/acceptance-summary.md) | [`pilot-resolution-v3-logical.txt`](pilot-resolution-v3-logical.txt) | [`addendum-bit-storage-logical-v3-confirmation.json`](addendum-bit-storage-logical-v3-confirmation.json) |
| `bit-storage-count-consumers` | [ledger](../../bench_results/04b85d10/v3-bit-storage-count-consumers-family-ledger.jsonl) | [`count-v3-pilot`](../../bench_results/04b85d10/2026-09-10-04b85d10-count-v3-pilot/acceptance-summary.md) | [`count-v3-confirmation`](../../bench_results/04b85d10/2026-09-10-04b85d10-count-v3-confirmation/acceptance-summary.md) | [`pilot-resolution-v3-count.txt`](pilot-resolution-v3-count.txt) | [`addendum-bit-storage-count-v3-confirmation.json`](addendum-bit-storage-count-v3-confirmation.json) |
| `bit-storage-layout-consumers` | [ledger](../../bench_results/04b85d10/v3-bit-storage-layout-consumers-family-ledger.jsonl) | [`layout-v3-pilot`](../../bench_results/04b85d10/2026-09-10-04b85d10-layout-v3-pilot/acceptance-summary.md) | [`layout-v3-confirmation`](../../bench_results/04b85d10/2026-09-10-04b85d10-layout-v3-confirmation/acceptance-summary.md) | [`pilot-resolution-v3-layout.txt`](pilot-resolution-v3-layout.txt) | [`addendum-bit-storage-layout-v3-confirmation.json`](addendum-bit-storage-layout-v3-confirmation.json) |

Each receipt's tables heading gives its digest, the family's comparison count
$m$ and the per-comparison confidence; each resolution derivation gives the
pilot's widest relative bootstrap half-width, recomputed at the corrected
alpha, and the frozen `effect.measurement_resolution`; each confirmation
addendum's `effect` gives the worthwhile and equivalence margins, which
strictly exceed one plus that resolution, with their rationale. Every cell's
pair count and flagged windows are the tables' *Pairs* and *Flagged* columns.
Pilots run at the confirmatory sample size so the resolution they observe is
the resolution the confirmation has. The frozen addenda are
`addendum-bit-storage-{logical,count,layout}-v3-{pilot,confirmation}.json`
beside this file; each confirmation names its pilot receipt by path and
SHA-256 as resolution evidence, and P-03 recomputed the widest relative
half-width from the raw pairs. The logical pilot's L3-resident 8192-word row
set a resolution its pilot margins did not clear, so the logical confirmation
addendum raises both margins and records why (`effect.rationale`); a cell
whose interval lies between the pilot margin and the raised one records
not-material under that rule, and its interval is reported.

Correction to both frozen layout addenda: their family question calls the
$m = 14$ BCH row "the DVB-T2 normal-frame mother code". That row is the
mother code of the DVB-T2 short frame; the normal frame's mother field is
$\mathrm{GF}(2^{16})$ [Etsi2015]. The cells declare degree 14, so every layout
BCH cell and its interval measure the short-frame mother code. The addenda,
their receipts and the generator `survey/make-addenda.py` keep the frozen text.

Each family's $m$ is the confirmatory cell count on its first and only
attempt, which spends $\alpha_t = 0.05/2$ split by Bonferroni over $m$. At
these $m$ every cell keeps P-20's twenty expected bootstrap draws per tail,
and no acceptance summary reports a `not-confirmatory` cell. Identity-control
cells (RREF, LDPC syndrome, dense matvec, dense transpose, DVB BCH) are
pilot-only: they size the resolution and record each pinned consumer's
baseline latency, and their trivially passing non-regression would otherwise
spend alpha.

The launcher is
[`run-consumer-campaigns.sh`](../../bench_results/04b85d10/run-consumer-campaigns.sh):
builds through `scripts/cargo-budget.sh`, correctness through
`consumer-verify` (every check passing, in every `launcher.log`), bounded
timed sessions under `dev/scripts/ccx1-bench-flock.sh --full-host`,
checkpointed resume, and `benchmark-acceptance` at finalization. Re-running
the current acceptance tool over all six receipts reproduces every summary
byte for byte.

### Diagnostic profile

The repeated profile
([`2026-09-10-04b85d10-profile-v3-repeated`](../../bench_results/04b85d10/2026-09-10-04b85d10-profile-v3-repeated/))
runs one profile session repeatedly, each under its own
`dev/scripts/ccx1-bench-flock.sh --full-host` invocation, with one
`consumer-profile` executable throughout. Its
[`repetitions.log`](../../bench_results/04b85d10/2026-09-10-04b85d10-profile-v3-repeated/repetitions.log)
records every session's start with the executable digest, every completion
and every discarded unfinished start; its
[`host.txt`](../../bench_results/04b85d10/2026-09-10-04b85d10-profile-v3-repeated/host.txt)
records the host, toolchain, executables and source digests. A session first
proves the compared routes agree (`rep-*/verify.jsonl`), then sweeps the
current production routes of `survey/profile-cases.py` with per-call wall
time and allocation counts and bytes from a counting global allocator,
records user-space hardware counters for the routes of
`survey/counter-cases.txt` and DWARF call graphs at a fixed cycle period for
the whole consumers of `survey/report-cases.txt`; the first session also
records allocation-site backtraces for the allocating consumers of
`survey/alloc-trace-cases.txt`. Release disassembly of the routines the
profile attributes cost to gives
[frame-traffic classification](../../bench_results/04b85d10/2026-09-10-04b85d10-profile-v3-repeated/asm/frame-traffic.txt)
and
[population-count attribution](../../bench_results/04b85d10/2026-09-10-04b85d10-profile-v3-repeated/asm/popcnt-attribution.txt),
which tests each instruction's mnemonic and operands exactly.

Timings, counter ratios and single-route derived figures are medians over the
sessions with the distribution-free order-statistic interval; sampled shares
are self-sample counts pooled over the sessions with Wilson intervals
[Wilson1927]; a ratio of two sweep routes is a percentile bootstrap of the
ratio of medians over both routes' session values [EfronTibshirani1993].
Allocation and disassembly counts are exact. The sweep locates and explains
costs; the receipts decide materiality. Profiler availability was observed,
and the host record keeps it: `perf_event_paranoid` limits `perf stat` and
`perf record` to this process's own user-space events, the generic
`stalled-cycles-backend` event is not supported, and valgrind is installed
but dhat was not needed, because the counting allocator reports counts, bytes
and sites. The launcher is [`run-profile.sh`](survey/run-profile.sh).

### Pinned baseline

Every arm of every receipt is one `consumer-arm` executable (tables § each
receipt, line *Arm executables*), built with Rust 1.95 (the repository MSRV;
*Toolchain* in each heading) without `RUSTFLAGS`: a conservative-portable
build whose SIMD kernels are selected at run time. The producing closure is
the issue-owned manifest
[`survey/producing-inputs.json`](survey/producing-inputs.json) (the three
production crates the harness links, the harness and its lockfile, the plan
derivation, the launcher, the lock wrapper and the protocol tooling),
snapshotted into each receipt's `inputs/producing/`. Those snapshots are the
receipts' source: the harness has since gained the `fixture_rng` record field
and probes that time what they name, so a build from the current tree is a
different executable. The host is an AMD Ryzen 9 5900X with AVX2, PCLMULQDQ
and POPCNT; each receipt records its kernel, SMT state, governors and CPU mask
per session and pins protocol, contract and schema by receipt-local digest.
Every code claim below is a row of
[`survey/source-evidence.json`](survey/source-evidence.json) (path, line and
verbatim text verified at generation), named here by claim id.

### History

Protocol-v1 evidence is immutable: the
[v1 pilot](../../bench_results/04b85d10/2026-09-08-04b85d10-consumers-pilot/acceptance-summary.md),
exploratory cells whose acceptance summary gives the receipt digest, pair
counts and toolchain, and the
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

The first v3 profile,
[`2026-09-10-04b85d10-profile-v3`](../../bench_results/04b85d10/2026-09-10-04b85d10-profile-v3/profile-summary.md),
is history too and supports no claim here. It ran every route once, so its
figures carry no interval; its records give seeds without their generator;
three of its probes timed something other than what they name: the LDPC
`setup` constructed the short-frame code at both lengths, the LDPC `unpack`
proxy appended 32448 bits where the syndrome has 32400, and the RREF
`dispatch` timed `select_backend_for_size` where `rref` calls
`resolve_xor_inplace`. Its `asm/popcnt-attribution.txt` counted every
disassembly line containing `popcnt`, symbol headers, branch-target labels
and operands naming a population-count routine included, so its per-symbol
counts are not `POPCNT` instructions; its own disassembly of `avx2_popcnt`
and `avx2_and_popcnt` (`asm/avx2-popcnt.txt`, `asm/avx2-and-popcnt.txt`)
holds no `POPCNT` mnemonic. The repeated profile replaces it; its bytes are
unchanged.

## Actual production routes

| Consumer | Public entry point to executed kernel (claim ids) | Observed in the receipts |
|---|---|---|
| Logical row | `BitMatrix::row_xor` calls `kernels::ops::xor_inplace`, which calls `resolve_xor_inplace(len)` on every call; below eight words that is `scalar_xor_inplace`, at or above eight words the detected `LogicalFns::xor_fn`, `avx2_xor_into` on this host (`row-xor-public-entry`, `xor-inplace-resolves-per-call`, `resolve-xor-simd-fn`, `simd-min-words-default`, `avx2-xor-into`). The baked four-word cutover exists only under `--cfg gf2_tuning_baked` (`baked-simd-min-words`). | 4 words `ops-dispatched/scalar`; 8, 64 and 8192 words `ops-dispatched/simd`. |
| Dense RREF | `alg::rref::rref` selects an eight-wide Gray table above 512 columns, clones the input, resolves one XOR kernel, allocates one table per pivot block and applies rows through the inline `row_xor_slice_from` (`rref-block-width`, `rref-clone`, `rref-resolves-xor-once`, `rref-table-per-block`, `rref-row-apply`). | 1024-square `current/blocked-m4ri`. |
| Dense matvec | `BitMatrix::matvec` dispatches on the stride; eight or more words reach `matvec_simd`, whose per-row parity is `LogicalFns::and_popcnt_fn` (`avx2_and_popcnt`) appended to a fresh `BitVec`; the scalar route is private (`matvec-route`, `matvec-simd-min-words`, `matvec-fused-kernel`, `matvec-output-alloc`, `matvec-scalar-private`). | 1024x4096 `current/simd-and-popcnt`. |
| Sparse LDPC parity | `LdpcCode::syndrome` is `SpBitMatrixDual::matvec`, which delegates to the CSR `SpBitMatrix::matvec`: one `x.get(c)` per stored index into a fresh `BitVec` (`ldpc-syndrome`, `ldpc-h-dual`, `ldpc-dual-matvec`, `ldpc-csr-bit-at-a-time`, `ldpc-syndrome-output-alloc`). | $n = 64800$ `current/csr-bit-at-a-time-matvec`. |
| Population count | `BitVec::count_ones` is `ops::popcount`, re-dispatched per call at the same eight-word cutover to `avx2_popcnt` or the scalar fallback (`count-ones-entry`, `popcount-dispatch`, `simd-backend-popcount`, `avx2-popcnt`). | 4 words scalar, 8 and 65536 words `simd-backend/avx2`. |
| Any-nonzero | `LdpcCode::is_valid_codeword` computes the syndrome and tests `count_ones() == 0`; `find_first_one` reaches `avx2_find_first_one`, which can stop at the first nonzero vector (`ldpc-is-valid-count`, `find-first-one-simd`, `avx2-find-first-one`). | Both spellings bit-identical on zero, weight-one and random words. |
| Block transpose | `transpose::detect` publishes `avx2-bit-twiddle` under AVX2; the PSHUFB lane exists behind `detect_pshufb` and is not selected (`transpose-detect-avx2`, `transpose-pshufb-alternative`). | `transpose-detected/avx2-bit-twiddle` against `transpose-scalar/portable`. |
| Dense transpose | `BitMatrix::transpose` resolves the block kernel once, allocates the output, selects the outer loop through `transpose_route` and tiles through `transpose_inner_loop` with two 64-word local blocks (`dense-transpose-resolves-once`, `dense-transpose-output-alloc`, `dense-transpose-route`, `dense-transpose-tile-scratch`). | 4096-square `current/macro-tiled-8`. |
| Packed BCH batch | `encode_batch_into` selects the family once through `select_family` and calls `encode_batch_family_into`, which runs one partition over the caller's workspace. The reference family is always admitted; bitslice and fold need the active profile's minimum batch, which the conservative profile does not grant at $B = 256$. Every per-frame family ends in `packed_write_codeword`, which copies the message one bit at a time (`bch-batch-into-selects`, `bch-batch-into-delegates`, `bch-family-into-partition`, `bch-family-admission`, `bch-bitslice-admission`, `bch-fold-admission`, `bch-reference-serial-reduce`, `bch-reference-write`, `bch-write-bit-at-a-time`, `bch-fold-reduce`, `bch-bitslice-batch`). | `current/PolyRemainderScalar` on the short-frame mother code ($m = 14$, $B = 256$); forced arms `family-pinned/BitsliceInterleaved`, `family-pinned/ClmulFold`. |
| Allocating and parallel packed BCH | `encode_batch` allocates its output and reduces over thread-local scratch; `encode_batch_parallel_into` selects the family once and recurses through `rayon::join`, with no forced-family variant (`bch-allocating-entry`, `bch-allocating-scratch`, `bch-parallel-selects-once`, `bch-parallel-join`). | `current-allocating/PolyRemainderScalar` against `caller-buffer/PolyRemainderScalar`; parallel path in the sweep only. |
| DVB-T2 compatibility BCH | `BchEncoder::encode_batch` maps `encode`, which expands the message into `Gf2mPoly` coefficients and divides by the generator (`dvb-bch-batch-maps-encode`, `dvb-bch-field-poly`, `dvb-bch-div-rem`). | Short frame $n = 7200$ `current/field-polynomial-div-rem`. |

## Why these sizes represent consumers

Row widths 4, 8 and 64 words bracket the conservative eight-word cutover
and one L1-resident row; 64 rows make each bank a whole number of 64x64
blocks; 8192 words make a 4 MiB L3-resident bank where dispatch is expected
to vanish into cache traffic. 507 words is the packed width of the DVB-T2
rate-1/2 normal-frame LDPC syndrome [Etsi2015]; 65536 words (512 KiB) is one
core's L2. The
1024-square RREF is the smallest square that selects the eight-wide table;
1024x4096 gives 64-word rows so the fused matvec kernel runs whole 32-byte
vectors on a 512 KiB matrix. The 4096-square transpose selects the
macro-tiled loop on a 2 MiB matrix. Block counts 256 and 4096 are an
L2-resident run and an L3-resident streaming run. $m = 14$, $B = 256$ is the
mother code of the DVB-T2 short frame (row T2S of
`dev/active/4e732b56/workload-selection.md`; DVB-T2 defines its short-frame
BCH code over $\mathrm{GF}(2^{14})$ and its normal-frame code over
$\mathrm{GF}(2^{16})$ [Etsi2015]) at that contract's steady-state batch; the
sweep adds $m = 8$
(row B3), $m = 16$ (the normal-frame mother field, row T2N) and batches 1 to
1024. The LDPC rows are the standard's rate-1/2 short- and normal-frame
parity-check matrices [Etsi2015].

## Confirmed cells

Speedups are baseline median over candidate median; values above one favour
the candidate. tables § `logical-v3-confirmation`, § `count-v3-confirmation`
and § `layout-v3-confirmation` hold every confirmatory cell's speedup, its
interval at the family's per-comparison confidence, its decision and its
outcome, and the three pilot sections hold the pilot cells. The question and
outcome above names each confirmatory cell by its row.

Pilot-only identity controls record each pinned whole consumer's baseline
latency in their *Baseline median* column, and every control ratio's interval
contains one (tables § `logical-v3-pilot`, rows
`logical-dense-rref-1024-control-1core` and
`logical-ldpc-syndrome-64800-control-1core`; § `count-v3-pilot`, row
`count-dense-matvec-1024x4096-control-1core`; § `layout-v3-pilot`, rows
`layout-dense-transpose-4096-control-1core` and
`layout-dvb-bch-encode-7200-control-1core`). The 507-word zero test declares
`set_bit` equal to the buffer's bit length (its addendum cell's `workload`):
the buffer is all zero, the valid-codeword case and the worst case for an
early exit. Twelve- and twenty-four-CPU arms are meaningful here only for the
parallel BCH entry point, whose only protocol-expressible cell is an identity
control. v1 measured it at eight pairs, both intervals containing one (v1
pilot acceptance summary, rows `layout-bch-encode-parallel-12core` and
`layout-bch-encode-parallel-24logical`), and v3 records its scaling in the
sweep instead (sweep § Transpose, bitslice and BCH encoding consumers, rows
`bch-encode-batch-parallel`).

## Attribution: time, allocation, traffic and limits

Every figure behind this section is a sweep or attribution row with its
interval, or an exact count; "case" names an attribution call-graph or
counter case. A pass on a cell decides materiality; this section explains
where the time goes.

| Consumer | Where the time goes | Allocation and conversion traffic | What limits it |
|---|---|---|---|
| 64-row x 64-word XOR | The dispatched, resolved and direct SIMD-backend routes differ by the dispatch term the receipt cell measures (sweep § Logical row, parity and dense-matrix consumers, rows `row-xor` `rows=64 words=64`). | None (same rows). Useful bandwidth of two loads and one store per word in attribution § Useful bandwidth, first row. | Negligible front-end stalls; dispatch adds a small share of the instructions per call (attribution § Counter ratios, rows `row-xor-64w-dispatched` and `row-xor-64w-resolved`): the loop is bound by its L1 loads and stores. |
| 64-row x 8192-word XOR | Same routes on an L3-resident 4 MiB bank (sweep, rows `row-xor` `rows=64 words=8192`). | None; useful bandwidth in § Useful bandwidth, second row. | L1 load misses per load well above the 64-word row's (§ Counter ratios, row `row-xor-8192w-dispatched`): cache traffic, and the confirmed interval spans one. |
| Dense RREF 1024 | The `rref` body holds most samples, `avx2_xor_into` a minor share, unresolved libc addresses a smaller one (case `dense-rref-1024`; time in sweep row `dense-rref` `cols=1024 rows=1024`). | Hundreds of allocations and megabytes per call, the working clone and one Gray table per pivot block (same sweep row; `rep-01/alloc-trace/dense-rref-1024.txt`; `rref-clone`, `rref-table-per-block`); a one-shot setup (sweep § Reported setup and conversion probes). | High IPC with negligible front-end stalls (§ Counter ratios, row `dense-rref-1024`): a table-building and copying consumer whose XOR ceiling is the kernel's sampled share. |
| Dense matvec 1024x4096 | `avx2_and_popcnt` holds most samples, `matvec_simd` and the `and_popcnt_fn` entry the rest (case `dense-matvec-1024x4096`; time in sweep row `dense-matvec` `cols=4096 rows=1024`). | One output allocation per call and its bit-append (same row; unpack probe in § Reported setup and conversion probes); the dispatch probe reads zero, below 1 ns per call. | Matrix-read bandwidth (§ Useful bandwidth) at the IPC and miss rates of § Counter ratios, row `dense-matvec-1024x4096`: the fused reduction is the actionable path. |
| LDPC syndrome $n = 64800$ | `SpBitMatrix::matvec` holds nearly every sample of the syndrome and of the validity check in both spellings (cases `ldpc-syndrome-64800`, `ldpc-codeword-check-64800` and `ldpc-codeword-check-64800-find`). | One output allocation per call and its bit-append (sweep row `ldpc-syndrome` `n=64800`; unpack probe). | One bit gather per stored index (`ldpc-csr-bit-at-a-time`; § Counter ratios, row `ldpc-syndrome-64800`); the count and the search stay below the listing threshold (the bounded rows of cases `ldpc-codeword-check-64800` and `ldpc-codeword-check-64800-find`). |
| 507-word zero test | Count and search on the all-zero buffer, and the search's exit when the first bit is set (sweep § Count and fused-reduction consumers, rows `zero-test` `set_bit=32448 words=507` and `set_bit=0 words=507`). | None. | The early exit is confirmed faster when both spellings scan the buffer (tables § `count-v3-confirmation`, row `count-zero-test-507w-1core`) and far faster when the first bit is set (attribution § Ratios of two routes, first row); the search executes fewer instructions per call for more branch misses and front-end stalls (§ Instructions per unit of work; § Counter ratios, rows `zero-test-507w-allzero-count` and `zero-test-507w-allzero-find`). |
| 65536-word popcount | SIMD backend and scalar fallback at 512 KiB, and the SIMD backend at 32 KiB (sweep, rows `popcount` `words=65536` and `words=4096`). | None. | Dispatch adds nothing visible at 512 KiB (rows `popcount` `words=65536`, `ops-dispatched` against `simd-backend`). The SIMD route's useful bandwidth at 32 and 512 KiB and the scalar fallback's fraction of it (§ Useful bandwidth) bound what a faster kernel can gain. No route here issues `POPCNT`: the scalar fallback is the bit-twiddle sequence and the AVX2 route counts with `VPSHUFB` nibble lookups and `VPSADBW` byte sums (`asm/popcnt-attribution.txt`). The whole bit-twiddle sequence, masks and byte-sum multiplier, sits in the harness's scalar arm (`Prepared::run`), `LdpcCode::is_valid_codeword`, `BitMatrix::matvec_scalar` and the tail loop of `avx2_and_popcnt`; the 64x64 transpose and the BCH reductions use some of the masks without the multiplier, to permute bits (same file, last section). |
| 256 block transposes | Scalar and detected AVX2 lanes (sweep § Transpose, bitslice and BCH encoding consumers, rows `transpose-64x64` `blocks=256`). | None. | The AVX2 lane executes a fraction of the scalar lane's instructions per block (§ Instructions per unit of work) at a lower IPC with negligible front-end stalls (§ Counter ratios, rows `transpose64-256-detected` and `transpose64-256-scalar`): it is bounded by vector-unit throughput and its own dependency chain, not by dispatch or the front end. |
| Dense transpose 4096 | The outer tiling loop of `BitMatrix::transpose` holds more samples than `transpose_64x64_avx2` (case `dense-transpose-4096`; time in sweep row `dense-transpose` `cols=4096 rows=4096`). | One output allocation per call (same row; `rep-01/alloc-trace/dense-transpose-4096.txt`); input and output traffic in § Useful bandwidth. | L1 and last-level miss rates (§ Counter ratios, row `dense-transpose-4096`): tile assembly, the two intentional 64-word scratch blocks and cache traffic bound the whole consumer. |
| Packed short-frame mother BCH ($m = 14$), $B = 256$ | Current family: `encode_systematic_with` holds most samples and `packed_write_codeword` the rest; fold: `packed_write_codeword` nearly all, `fold_block_pclmul` little; bitslice: the batch body most, `bitslice_reduce_avx2` little (cases `bch-m14-b256-current`, `bch-m14-b256-clmul` and `bch-m14-b256-bitslice`). Every family, the table family included, in sweep rows `bch-encode-batch` `batch=256 degree=14`. | One field-identity allocation per message in `validate_batch` (`rep-01/alloc-trace/bch-m14-b256-current.txt`; its cost in rows `field-id-hint`); batch fill, workspace and dispatch probes (§ Reported setup and conversion probes, rows `bch-encode-batch`). | Low IPC, front-end stalls and branch misses with almost no L1 misses (§ Counter ratios, rows `bch-m14-b256-reference`, `bch-m14-b256-clmul` and `bch-m14-b256-bitslice`): control and data dependencies, and above all the bit-serial codeword write, whose per-batch cost is the same in every family (§ Per-call cost of one symbol). |
| Allocating packed BCH | Allocating and caller-buffer entry points (sweep rows `bch-encode-batch-alloc` `batch=256 degree=14`). | About twice the allocations and many times the bytes per call of the caller-buffer entry point (same rows). | Confirmed not material (tables § `layout-v3-confirmation`, row `layout-bch-encode-caller-buffer-m14-b256-1core`) while the scalar recurrence and the bit-serial write dominate. |
| Parallel packed BCH | The current family at 1, 6, 12 and 24 workers, $B = 256$ and $1024$ (sweep rows `bch-encode-batch-parallel`). | The same per-message allocation (same rows). | Scaling of the current family only; the entry point exposes no forced-family arm. |
| DVB-T2 compatibility BCH | `BchEncoder::encode` holds most samples and `Gf2mElement` multiplication the rest (case `dvb-bch-7200-b1`); short and normal frames in sweep rows `dvb-bch-encode`. | Hundreds of allocations and megabytes per 16-frame call, the growing coefficient vectors of `encode` (sweep row `dvb-bch-encode` `batch=16 n=7200`; `rep-01/alloc-trace/dvb-bch-7200-b1.txt`; `dvb-bch-field-poly`, `dvb-bch-div-rem`). | A field-polynomial route (§ Counter ratios, row `dvb-bch-7200-b1`). Recorded for its owner; not campaigned here. |

### Scratch buffers versus compiler spills

[`asm/frame-traffic.txt`](../../bench_results/04b85d10/2026-09-10-04b85d10-profile-v3-repeated/asm/frame-traffic.txt)
counts frame stores and loads in one bounded Rust 1.95 disassembly of each
routine of the measured executable and joins them with the declaration that
settles intent; each routine's counts are its row there.

- No frame traffic: `avx2_xor_into`, `avx2_and_into`, `avx2_popcnt`,
  `avx2_and_popcnt`, `avx2_find_first_one`, `packed_write_codeword`.
- Intentional scratch: the portable 64x64 transpose and the AVX2 lane copy
  the block into a local they mutate in place (`transpose-scalar-scratch`,
  `transpose-avx2-scratch`); the tiled dense transpose holds `tile_in` and
  `tile_out` (`dense-transpose-tile-scratch`); `unpack_parity` transposes the
  bit-sliced register back through a block (`bch-unpack-parity`).
- Compiler spill or register pressure: `bitslice_reduce_{scalar,avx2}`,
  `fold_block_{scalar,pclmul}`, `packed_table_reduce`, `packed_fold_reduce`,
  `matvec_scalar`, `matvec_simd` and `SpBitMatrix::matvec` store to their
  frames but declare no local buffer.

## Any-nonzero opportunities

Production sites that ask only whether a packed syndrome is zero and spell it
as a full count, with the measured consequence where one exists:

- `LdpcCode::is_valid_codeword` (`ldpc-is-valid-count`): confirmed not
  material at $n = 64800$ (tables § `count-v3-confirmation`, row
  `count-ldpc-check-64800-1core`); the syndrome matvec holds nearly every
  sample of the check (attribution § Sampled shares, case
  `ldpc-codeword-check-64800`).
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

Each entry names the consumer, the materiality hypothesis, the evidence it
ranks on and the downstream issue that tests it. An entry with a confirmatory
cell ranks on that cell's interval; the others rank on the sampled share with
its Wilson interval that the change could move. Not-material findings stay
listed so the downstream issues do not rediscover them.

1. **Word-wise systematic codeword assembly for packed BCH (`1d4fd63d`).**
   Consumer: `encode_batch_into` on the mother code of the DVB-T2 short
   frame ($m = 14$, $B = 256$), the confirmed layout cells. Hypothesis:
   replacing the bit-serial `packed_write_codeword` with word copies and
   shifted parity placement removes most of the per-batch write cost that
   every family pays (attribution § Per-call cost of one symbol). Share it
   could move: nearly all of the fold consumer and the smaller part of the
   current one (attribution § Sampled shares, cases `bch-m14-b256-clmul` and
   `bch-m14-b256-current`, row `packed_write_codeword`). This is a
   bit-layout packing change and belongs with the conversion costs `1d4fd63d`
   REQ-09 measures.
2. **Packed BCH family selection (`1d4fd63d`).** Consumer: the same entry
   point on the short-frame mother code. Hypothesis: admitting `ClmulFold`
   or `BitsliceInterleaved` at $m = 14$, $B = 256$ is a confirmed
   whole-consumer gain under the current admission rule (tables §
   `layout-v3-confirmation`, rows `layout-bch-encode-fold-m14-b256-1core` and
   `layout-bch-encode-bitslice-m14-b256-1core`); the tuning profile, not a
   kernel, is the change. The sweep points the same way at $B = 16$ and on
   the normal-frame mother field ($m = 16$, $B = 256$) (attribution § Ratios
   of two routes, the four BCH rows); no receipt measures either, so neither
   is a confirmed gain. The bitslice family is the conversion consumer named
   in the issue.
3. **Fused AND-popcount in dense matvec (`5cbb6545`).** Consumer:
   `BitMatrix::matvec` on 64-word rows. Hypothesis: a Zen 3 fused reduction
   that beats the nibble-lookup `avx2_and_popcnt` moves up to the kernel's
   sampled share of the consumer (attribution § Sampled shares, case
   `dense-matvec-1024x4096`), at the bandwidth and IPC of § Useful bandwidth
   and § Counter ratios; no current kernel uses the host's `POPCNT`
   (`asm/popcnt-attribution.txt`), so a `POPCNT`-based reduction is one
   untested arm. The whole-matvec cell must include the output allocation and
   bit-append (sweep § Reported setup and conversion probes, row
   `dense-matvec` `cols=4096 rows=1024`).
4. **A `POPCNT`-enabled scalar arm and the four/eight-word popcount cutover
   (`5cbb6545`).** Consumer: `BitVec::count_ones`. Hypothesis: no kernel of
   the conservative build issues `POPCNT`: the scalar fallback is the
   bit-twiddle sequence and the AVX2 kernels are nibble lookups
   (`asm/popcnt-attribution.txt`), so `5cbb6545`'s REQ-07 "scalar POPCNT"
   comparison needs a feature-detected `popcnt` arm. Against the current
   fallback the AVX2 route is confirmed faster at 512 KiB and at eight words,
   and per-call dispatch is confirmed costly at four words (tables §
   `count-v3-confirmation`, rows `count-popcount-bandwidth-65536w-1core`,
   `count-popcount-threshold-8w-1core` and
   `count-popcount-dispatch-4w-1core`).
5. **Row-XOR cutover and dispatch hoisting at 4 to 8 words (`2037941f`).**
   Consumer: `BitMatrix::row_xor` and elimination loops. Hypothesis: the
   four-word row is confirmed faster through the SIMD backend and the
   eight-word loop with dispatch hoisted (tables § `logical-v3-confirmation`,
   rows `logical-row-xor-threshold-4w-1core` and
   `logical-row-xor-dispatch-8w-1core`); 64 words is not material under the
   family's margin and reportable under a rule the mid-range story declares
   itself, and 8192 words is not material (rows
   `logical-row-xor-dispatch-64w-1core` and
   `logical-row-xor-dispatch-8192w-1core`).
6. **Complete transpose consumer (`1d4fd63d`).** Consumer:
   `BitMatrix::transpose` and the bitslice parity unpack. Hypothesis: a
   PSHUFB or movemask block transform can improve the isolated primitive, over
   which detected AVX2 is already confirmed faster than scalar (tables §
   `layout-v3-confirmation`, row `layout-transpose-block-256-1core`), but the
   whole 4096-square transpose is bounded by its outer tiling loop, its output
   allocation and its cache traffic (attribution § Sampled shares, case
   `dense-transpose-4096`; sweep row `dense-transpose` `cols=4096 rows=4096`;
   § Useful bandwidth); tile assembly and packing must be in the cell.
7. **Early-exit any-nonzero in repeated candidate checks (`5cbb6545`).**
   Consumer: ORBGRAND. Hypothesis: the isolated early-exit gain, confirmed
   when both spellings scan the buffer (tables § `count-v3-confirmation`, row
   `count-zero-test-507w-1core`) and far larger when the first bit is set
   (attribution § Ratios of two routes, first row), becomes material where
   the check is a large share of per-candidate work; the LDPC validity check
   is confirmed not material and must not justify a library change alone.
8. **RREF table and copy traffic, not another XOR kernel (`2037941f` only if
   its dense-matrix scope is amended).** Consumer: `alg::rref::rref`.
   Hypothesis: the allocations and bytes per call (sweep row `dense-rref`
   `cols=1024 rows=1024`) and the XOR kernel's minor sampled share
   (attribution § Sampled shares, case `dense-rref-1024`) cap any XOR-only
   gain; reuse of the Gray table buffer is the lever.

Preserved negative results: the caller-buffer BCH entry point, the LDPC full
count, and dispatch hoisting on 8192-word rows and, under the family's margin,
on 64-word rows (the not-material rows above); the per-message field-identity
allocation in the BCH batch prologue (sweep rows `field-id-hint`); and the
DVB-T2 compatibility encoder, which reaches no packed kernel and is not this
issue's campaign.

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
- **REQ-03 — MET.** The repeated profile quantifies each consumer (sweep and
  attribution summaries): sampled shares with their sample counts and Wilson
  intervals; per-call times, conversion and dispatch probes, counter ratios
  and useful bandwidth as medians with order-statistic intervals; exact
  allocation counts and bytes with sites; and Rust 1.95 disassembly with
  exact instruction attribution. Spills and intentional scratch are
  classified with verified citations; any-nonzero sites are enumerated with
  the one measured consumer. Every seeded input names its generator and
  version beside its seed, in the profile records, the summaries and the
  receipt tables.
- **REQ-04 — MET.** Baseline receipts measure the pinned pre-change
  executable; the ranked experiments carry consumer, hypothesis, the
  confirmed receipt interval or the sampled-share interval they rank on, and
  the downstream issue; not-material findings are preserved.
- **REQ-05 — MET.** Changes are confined to `dev/active/04b85d10` and
  `dev/bench_results/04b85d10`; the existing runner, acceptance tool and
  wrapper are reused unchanged.
