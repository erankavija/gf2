# Transpose, logical-buffer and BCH conversion arms against M4RI, Bitshuffle and ISA-L

> **Diátaxis Type:** Explanation

Survey for `6fb89a3c` (epic `1a379447`). It changes no production code; the diff is under `dev/`. The [measurement contract](../1a379447-zen3-cpu-performance/measurement-contract.md) and [protocol v3](../f547c394/protocol.md) govern every receipt. Each receipt pins them, the addendum schema and its family addendum by digest, and records the host and toolchain it observed (`host`, `toolchain`). This report carries the argument and cites the evidence; every figure lives in the [generated tables](../../bench_results/6fb89a3c/tables.md) (`tables.md`, projected by `survey/summarize.py` from the receipts and acceptance summaries), in the receipts and summaries themselves, or in the survey's evidence files. Pointers name the `tables.md` section, block and row.

## Answer

One external arm holds a material gap over gf2: M4RI's `mzd_transpose` is ahead of `BitMatrix::transpose` in `transpose-consumer-65-vs-m4ri` (decision `improved`; `tables.md` → Overview → Confirmation outcomes, "External arm ahead"). gf2's tile grid explains it (§4.1). M4RI is also ahead in `transpose-consumer-63-vs-m4ri`, by less than the family's material-gap threshold (decision `not-worse`). gf2 is materially ahead in every other confirmation cell (decision `regressed`; each family's confirmation → Cells).

The transpose and logical-buffer confirmations are `not-confirmatory` throughout. Each confirms its full question, and the resulting comparison count leaves fewer expected bootstrap tail draws than P-20 requires, so their intervals are fresh-sample evidence without a confirmatory decision (Overview → Confirmation outcomes, "Tail draws"; protocol, Bootstrap numerical resolution). The BCH confirmation is confirmatory and every cell is `fail`: gf2's production materialization is materially faster than the established M4RI construction.

## 1. Design

| Family (ledger) | Question | Confirmation receipt |
|---|---|---|
| `transpose-vs-external` | gf2's canonical 64x64 kernel and its `BitMatrix::transpose` consumer at 63x63, 64x64 and 65x65 against M4RI, and against Bitshuffle through a measured geometry adapter | [`v3-r1-6fb89a3c-transpose-confirmation`](../../bench_results/6fb89a3c/v3-r1-6fb89a3c-transpose-confirmation/receipt.json) |
| `logical-buffer-vs-isal` | fresh-output `xor_inplace` against ISA-L `xor_gen_base` around gf2's scalar/SIMD dispatch threshold, around the 64-word boundary, at fill sizes between them and with a third source | [`v3-r1-6fb89a3c-logical-confirmation`](../../bench_results/6fb89a3c/v3-r1-6fb89a3c-logical-confirmation/receipt.json) |
| `bch-genmatrix-consumer` | `BchCode::generator_matrix` against M4RI's shifted-polynomial fill plus `mzd_echelonize_m4ri` for the codes B1, B2 and B3 of the established `bch_genmatrix` bench | [`v3-r1-6fb89a3c-bch-confirmation`](../../bench_results/6fb89a3c/v3-r1-6fb89a3c-bch-confirmation/receipt.json) |

Each family's cells and sizes are in its confirmation addendum (`addendum-{transpose,logical-buffer,bch-genmatrix}-v3-confirmation.json`, `cells`). The questions differ in operation, comparator and consumer contract, so each family has its own append-only ledger (`dev/bench_results/6fb89a3c/v3-*-family-ledger.jsonl`), created empty before its first v3 campaign. The protocol-v1 pilots of the same questions were exploratory-only and spent no comparisons (`dev/bench_results/6fb89a3c/v3-ledger-origin.json`). Every confirmation receipt is accepted without errors, and none qualifies for production selection because none proposes a production change (acceptance summaries, `verdict` and `qualifies`).

Every cell compares warm single-core latency under the protocol's frozen shared settings: counterbalanced pairs of fresh child processes, windowed per-execution medians and a percentile bootstrap [Efron1979] of the speedup of medians $\hat s = \operatorname{median}_i b_i / \operatorname{median}_i c_i$ at the ledger-corrected per-comparison alpha (protocol, Sampling design, Estimator and interval, Families and selection; each receipt's `settings` and each cell's `resolved_cpus`). The baseline $b$ is always gf2 and the candidate $c$ the external arm, so $\hat s > 1$ favours the external arm. The margins are each confirmation addendum's `effect.material_gap_threshold` and `effect.equivalence_margin` (Overview → Resolution). The gf2 arms are conservative-portable builds whose kernels come from the production runtime dispatch, and the external arms are native builds (receipt `arms`; `survey/build-evidence.json`). Multicore arms are inapplicable: each surveyed routine completes a call on one thread, so they would replicate independent calls instead of exercising a parallel path. The issue's cells are small and cache-resident; streaming sizes are outside them.

P-20's tail-support rule bounds how many confirmatory reservations a first attempt can carry. The transpose and logical families confirm every pilot cell and fall short of it (Overview → Confirmation outcomes, "Tail draws"; `pilot-resolution-v3-{transpose,logical}.txt`, line "expected draws per bootstrap tail"). A freeze script pinned in the pilot receipts had proposed subsets sized to fit the bound; the confirmation keeps the whole questions instead, which removes confirmatory claims rather than adding any. The BCH pilot addendum had already excluded its reference-oracle cells from confirmation, because they time a test oracle rather than the production route.

Each confirmation's resolution is its pilot's widest relative bootstrap half-width, rounded up, and both margins strictly exceed one plus it (Overview → Resolution; derivations `pilot-resolution-v3-{transpose,logical,bch}.txt`, written with the addenda by `survey/freeze-confirmation.py`). Workload seeds expand through SplitMix64 [Steele2014], implemented as `tuning_campaign_support::abtest::SplitMix64` (`dev/tools/tuning-campaign-support/src/abtest.rs`) in the gf2 arms and as `splitmix64_next` (`survey/harness_common.h`) in the C arms; the BCH cells carry a seed that no generator consumes. The protocol bootstrap draws from xoshiro256** seeded through SplitMix64 [BlackmanVigna2021]; the descriptive intervals in `tables.md` use the SplitMix64 implementation in `survey/summarize.py`.

## 2. Operation equivalence, adaptation and unavailable comparisons (REQ-03)

`survey/verify-bit-mapping.py` compares each arm's output with naive bit arithmetic from the same seeds and writes `survey/correctness-report.json`; `survey/verify-arm-framing.py` exercises the child protocol of every arm (`survey/framing-report.json`). Code claims are in `survey/source-evidence.json`, cited here by claim id.

- **M4RI transpose.** gf2 bit $(r, c)$ is M4RI bit $(r, c)$, and output bit $(c, r)$ equals it for the fixed seeds and tiled geometries the report lists (`transpose.m4ri`). M4RI keeps padding bits zero and pads each row to an even word count (`m4ri-excess-bits-policy`, `m4ri-rowstride-even`). Consumer calls allocate a fresh output on both sides (`gf2-bitmatrix-transpose-fresh-output`, `m4ri-transpose-fresh-output`).
- **Bitshuffle transpose.** One block of 64 eight-byte elements is gf2's 64x64 transpose with no adapter; of the byte and bit orders searched, only the direct mapping passes (`transpose.bitshuffle.mapping_search`). The library rejects element counts that are not multiples of eight, so 63 and 65 rows are unavailable unpadded (`bitshuffle-multiple-of-eight`). The padded adapter (rows rounded up to a multiple of eight, each row packed into $\lceil c/8 \rceil$ bytes, the first $c$ planes unpacked) matches at every validated geometry and copies nothing at 64x64. Its pack and unpack copies are measured in the adapter cells and take a large share of the Bitshuffle consumer call there (transpose confirmation → Probes and Shares, cells `transpose-consumer-{63,65}-vs-bitshuffle-adapter`).
- **ISA-L XOR.** Destination bit $b$ of word $w$ is the XOR of the sources' bit $b$ of word $w$, LSB-first, at every validated word count and arity (`logical_xor`). `xor_gen` takes the sources and then the destination as one 32-byte-aligned pointer array and writes a fresh destination (`isal-xor-gen-arity`, `isal-xor-gen-dest-last`, `isal-xor-gen-alignment`); gf2 matches that with a destination copy followed by in-place XORs. Forming ISA-L's pointer array is a small fraction of its call, largest at the smallest buffers, while gf2's destination copy is a substantial fraction of its much shorter call (logical confirmation → Probes and Shares).
- **BCH generator matrix.** The routes produce different systematic bases; after both layouts are reindexed to polynomial-degree order, their full-rank row spaces are equal for B1, B2 and B3 (`bch_genmatrix`). Raw dump identity is falsified by design.
- **Unavailable, not substituted:** unpadded Bitshuffle at 63 and 65 rows; ISA-L's public `xor_gen`, whose SSE/AVX/AVX-512 dispatch needs NASM, which the host lacks (`isal-multibinary-dispatch`; `build-evidence.json` → `host.nasm`); ISA-L source/destination aliasing (gf2's accumulate form) and alignments outside `raid.h`'s contract.

## 3. Pins and observed backends (REQ-02)

`survey/build-evidence.json` records sources, licences, build commands, library and executable digests, and per-route instruction mixes from `objdump`; `survey/fetch-build.sh` reproduces the builds.

| Library | Pin (`build-evidence.json`) | Licence | Build | Observed route |
|---|---|---|---|---|
| M4RI [AlbrechtBard2026] | release tarball by version and digest (`m4ri.version`, `m4ri.source`) | GPL-2.0-or-later: COPYING is the GPLv2 text, `mzd.h` says "version 2 or higher", the README says GPLv2+ (`m4ri-license-header`, `m4ri-license-header-version`, `m4ri-license-readme`) | `m4ri.build.configure` | whole 64x64 blocks in `_mzd_transpose_base` use general-purpose instructions only, the small-matrix routines use vector registers, and no route contains `cpuid` (`m4ri.disassembly.routes.mzd_transpose`) |
| Bitshuffle [Bitshuffle2026] | tag and commit (`bitshuffle.version`, `bitshuffle.source.commit`) | MIT | `bitshuffle.build.commands` | the compile-time AVX2 route `bshuf_trans_bit_elem_AVX`, which the library's own predicates report, with no `cpuid` (`bitshuffle.binary_observation`, `bitshuffle.disassembly`) |
| ISA-L [IsaL2026] | tag and commit (`isa_l.version`, `isa_l.source.commit`) | BSD-3-Clause | `isa_l.build.command` | `xor_gen_base`, a byte loop that GCC did not vectorize (`isa_l.disassembly.routes.xor_gen_base`: general-purpose instructions only); the public `xor_gen` is unavailable (`isa_l.binary_observation`) |
| gf2 | this tree, pinned by each receipt's producing snapshot | — | `host.rust_rustflags`, LTO | the kernel the runtime dispatch selects, reported by every execution (`selected_path`; Cells, "gf2 path") |

## 4. Results

`tables.md` holds each family's pilot and confirmation: per-cell medians with bootstrap intervals, the evaluator's speedup interval, decision and outcome, and the probes, shares and cross-cell ratios. No confirmation cell has a flagged window (Cells, "Flagged").

### 4.1 Transpose (`not-confirmatory`)

gf2's 64x64 kernel is materially ahead of both external kernels, and gf2's consumer is materially ahead of M4RI's at 64x64 and of the Bitshuffle adapter at every geometry. M4RI's consumer leads at 63x63 by less than the threshold and at 65x65 by a material gap (transpose confirmation → Cells).

The 65x65 gap has a mechanism in the code and a matching measured cost. At 65x65 `BitMatrix::transpose` walks a 2x2 grid of 64x64 tiles: four tile loads that zero-pad rows beyond the matrix, four full kernel calls and four tile stores, although three of the tiles hold a single row or column of data (`gf2-bitmatrix-tile-zero-pad`, `gf2-bitmatrix-tile-kernel-call`). M4RI transposes the one whole block and then a 64x1 strip, a 1x64 strip and a 1x1 corner with routines sized to them (`m4ri-transpose-odd-whole-block`, `m4ri-transpose-64xlt64-strip`, `m4ri-transpose-lt64x64-strip`, `m4ri-transpose-small-corner`). The costs agree: from 64x64 to 65x65 gf2's consumer time grows several-fold while M4RI's grows by a fraction, and gf2's 65x65 call costs several times its 64x64 kernel call (transpose confirmation → Cross-cell ratios, rows "gf2 BitMatrix::transpose 65x65 / 64x64", "M4RI mzd_transpose consumer 65x65 / 64x64" and "gf2 BitMatrix::transpose 65x65 / gf2 64x64 kernel"). At 63x63 M4RI takes its small-matrix route (`m4ri-transpose-small-path`) while gf2 pads one tile.

The kernel losses have visible causes. M4RI's whole-block path is general-purpose word code (§3). Bitshuffle's AVX2 routine mallocs and frees a scratch buffer in every call, plus a second one in the byte transpose it calls first (`bitshuffle-avx-malloc-per-call`, `bitshuffle-avx-calls-byte-elem`, `bitshuffle-sse-byte-malloc-per-call`). gf2 runs its AVX2 kernel (`gf2-transpose-avx2-runtime-check`). Through the adapter Bitshuffle stays behind gf2 even at 64x64, where it copies nothing.

### 4.2 Logical buffers (`not-confirmatory`)

gf2 is materially ahead of ISA-L's portable `xor_gen_base` in every cell, by a margin that broadly widens with buffer size (logical confirmation → Cells). `xor_gen_base` handles one byte per loop iteration (`isal-xor-gen-base-byte-loop`), while gf2's SIMD route XORs four words per AVX2 instruction (`gf2-avx2-xor-instruction`). gf2's dispatch threshold shows in its own costs: the 8-word call is cheaper than the 7-word call because the SIMD route starts at eight words (`gf2-xor-dispatch-threshold`), and the 63- and 65-word calls, which leave a scalar tail, cost more than the 64-word call. A third source raises both arms' cost by a similar factor (logical confirmation → Cross-cell ratios). These results cover ISA-L's portable reference only; its SIMD `xor_gen` remains unmeasured.

### 4.3 BCH generator matrix (confirmatory)

Every cell is `fail`: M4RI is materially slower than gf2 for B1, B2 and B3, increasingly so for the larger codes (BCH confirmation → Cells). gf2 fills the systematic generator column by column from a recurrence on the generator polynomial (`gf2-bch-generator-matrix-impl`, `gf2-bch-write-generator`); M4RI copies the matrix of $k$ shifted polynomial rows and reduces it with `mzd_echelonize_m4ri` in every call. Code construction (gf2) and the shifted fill (M4RI) are setup outside the windows, reported per execution with M4RI's single-pass copy and reduction probes (BCH confirmation → Probes).

This contradicts the protocol-v1 BCH pilot, which found M4RI ahead in every cell (`2026-09-08-6fb89a3c-bch-pilot/acceptance-summary.md`). That pilot timed `bch_generator_matrix_by_encoding`, the test-support oracle that encodes each basis vector at $O(k^2 r)$ cost (`gf2-bch-reference-complexity`), under the gf2 name. The v3 pilot's exploratory oracle cells reproduce its direction while its production cells do not (BCH pilot → Cells, rows `bch-genmatrix-reference-b*-vs-m4ri` against `bch-genmatrix-b*-vs-m4ri`). The v1 conclusion is withdrawn.

## 5. Changes between pilots and confirmations, and deviations

`tables.md` → Overview → Arm executables compares every arm's pilot and confirmation digest. `dev/bench_results/6fb89a3c/v3-pilot-to-confirmation-executables.txt`, written with `survey/compare-arm-code.py`, analyses the arms that differ:

- gf2 logical arm: rustfmt reformatted its source. `.text` and `.rodata` are identical; the other differing bytes are the build ID and the line fields of source locations below the inserted lines. The pilot build was reproduced byte for byte from the pilot source.
- ISA-L arm: GCC had deleted the pilot's pointer-array probe loop, so that probe recorded no measurement; a compiler barrier now forces every repetition's stores (`survey-isal-arrangement-probe-barrier`). `xor_body` and `xor_gen_base` are identical at unchanged addresses.
- Bitshuffle arm: the adapter probes average many warm repetitions instead of one cold pass (`survey-bitshuffle-pack-probe`). The timed functions are identical; harness and library functions moved.

The pilot and confirmation probes of these two C arms therefore measure different things, and `tables.md` labels each. Further deviations:

- The failed transpose pilot `v3-r1` measured part of the family before the gf2 arm rejected the adapter field of its case (its `README.md`). `v3-r2` re-measured every cell, so those cells had two pilot trials where the pilot addendum declared one; the confirmation addendum records the budget actually used (`search_budget.max_pilot_trials_per_cell`).
- The gf2 arms warm their working set in the calibration calls that precede the first timed window, not in a separate pass before calibration as the protocol words it; the C arms make the separate pass.
- The logical confirmation's driver was stopped between sessions and later resumed under the campaign's identity, with the commands appended to its launcher log; the campaign completed (acceptance summary, `sessions` and `resumed`).
- M4RI's generator-matrix arm reports a single-pass copy and reduction in the protocol's `pack_ns` and `dispatch_ns` fields. A single pass exceeds the warm per-call median, so `tables.md` names these probes and treats them as no share of the timed call.

## 6. Immutable history

- The protocol-v1 exploratory pilots `2026-09-08-6fb89a3c-transpose-pilot`, `2026-09-08-6fb89a3c-logical-pilot` and `2026-09-08-6fb89a3c-bch-pilot` are superseded; `v3-ledger-origin.json` records their digests. The failed v1 transpose launches (`…-failed-framing`, `…-failed-canonical-conversion`) are not receipts.
- `v3-r1-6fb89a3c-transpose-pilot-failed-adapter-case` holds the transpose ledger's first reservation and the cells it completed, without a receipt; none of its data is used.
- The accepted v3 pilots `v3-r2-6fb89a3c-transpose-pilot`, `v3-r1-6fb89a3c-logical-pilot` and `v3-r1-6fb89a3c-bch-pilot` serve only as resolution evidence.

## 7. Criteria

| Criterion | Status | Evidence |
|---|---|---|
| REQ-01 contract | MET | every v3 confirmation and pilot receipt accepted, each pinning contract, protocol, schema, addendum, producing-input snapshot and ledger prefix; correctness and framing gates; launcher `dev/bench_results/6fb89a3c/run-campaign.sh`; no production change, so no before/after pair is due; losing, not-confirmatory and failed results kept (§4 to §6) |
| REQ-02 pins | MET | §3; `survey/build-evidence.json`, `survey/fetch-build.sh`, `survey/source-evidence.json` |
| REQ-03 mappings | MET | §2; `survey/correctness-report.json`, `survey/bitshuffle-bit-mapping.md`, the `tables.md` probe and share blocks |
| REQ-04 addenda and receipts | MET | frozen v3 pilot and confirmation addenda and receipts for the 64x64 kernel, the 63/64/65 consumers against both comparators, and the logical buffer sizes and parity arity at ISA-L's alignment, with setup and arrangement probes; transpose and logical are `not-confirmatory` for the reason in §1 |
| REQ-05 BCH | MET | `v3-r1-6fb89a3c-bch-confirmation`: current pinned measurements of both implementations through the established `bch_genmatrix` rows and the M4RI construction of issue `4e732b56`; no historical receipt substitutes |

## 8. Follow-ups

1. An edge-strip route for partial tiles in `BitMatrix::transpose` should bring gf2's cost growth from 64x64 to 65x65 toward M4RI's and close the 65x65 gap (transpose confirmation → Cross-cell ratios); re-running `transpose-consumer-65-vs-m4ri` falsifies it. Tracked by `1d4fd63d` (bit transpose and bitslice conversion).
2. ISA-L's SIMD `xor_gen` needs a new pinned arm, and NASM is a host prerequisite for building it; only then can the logical family speak about ISA-L's fastest route. Tracked by `2037941f` (mid-range buffers), whose REQ-03 compares against ISA-L.
3. A confirmatory decision on the whole transpose or logical question needs a protocol amendment that raises `bootstrap_resamples` or relaxes the tail-support rule; until then this evidence stays non-confirmatory.
