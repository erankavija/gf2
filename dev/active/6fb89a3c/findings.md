# Transpose, logical-buffer and BCH conversion arms against M4RI, Bitshuffle and ISA-L

> **Diátaxis Type:** Explanation

Survey for `6fb89a3c` (epic `1a379447`) on `fraktaali` (AMD Ryzen 9 5900X, Linux 7.2.2, Rust 1.95.0, GCC 16.2.1). It changes no production code; the diff is under `dev/`. The [measurement contract](../1a379447-zen3-cpu-performance/measurement-contract.md) and [protocol v3](../f547c394/protocol.md) govern every receipt, and each receipt pins both, the addendum schema and its addendum by digest. The [generated tables](../../bench_results/6fb89a3c/tables.md) project the committed receipts and acceptance summaries; every number below comes from them unless another artifact is named, and each rests on 24 executions per arm and cell.

## Answer

Of the 20 confirmation cells, one external arm holds a material gap: M4RI's `mzd_transpose` is ahead of gf2's `BitMatrix::transpose` at 65x65, speedup 1.873 [1.852, 1.900]. gf2's tile grid explains it (§4.1). M4RI's 63x63 lead, 1.062 [1.056, 1.074], is below the 1.2 material-gap threshold. gf2 is materially ahead in the other 18 cells. The transpose (8 cells) and logical-buffer (9 cells) confirmations are `not-confirmatory`: their full cell sets exceed the $m \le 6$ that P-20's tail-support rule permits on a first attempt, so their intervals are fresh-sample evidence without a confirmatory decision. The BCH confirmation (3 cells) is confirmatory and all three cells are `fail`: gf2's production materialization is materially faster than the established M4RI construction.

## 1. Design

| Family (ledger) | Question | $m$ | Confirmation receipt | Outcome |
|---|---|---:|---|---|
| `transpose-vs-external` | gf2's 64x64 kernel and its 63/64/65 `BitMatrix::transpose` against M4RI, and against Bitshuffle through a measured geometry adapter | 8 | [`v3-r1-6fb89a3c-transpose-confirmation`](../../bench_results/6fb89a3c/v3-r1-6fb89a3c-transpose-confirmation/receipt.json) | 8 `not-confirmatory` |
| `logical-buffer-vs-isal` | fresh-output `xor_inplace` against ISA-L `xor_gen_base` at 7 to 65 words and with three sources | 9 | [`v3-r1-6fb89a3c-logical-confirmation`](../../bench_results/6fb89a3c/v3-r1-6fb89a3c-logical-confirmation/receipt.json) | 9 `not-confirmatory` |
| `bch-genmatrix-consumer` | `BchCode::generator_matrix` against M4RI's shifted-polynomial fill plus `mzd_echelonize_m4ri` at B1, B2, B3 | 3 | [`v3-r1-6fb89a3c-bch-confirmation`](../../bench_results/6fb89a3c/v3-r1-6fb89a3c-bch-confirmation/receipt.json) | 3 `fail` (gf2 ahead) |

The three questions differ in operation, comparator and consumer contract, so each has its own append-only ledger (`dev/bench_results/6fb89a3c/v3-*-family-ledger.jsonl`), created empty before its first v3 campaign. The protocol-v1 pilots of the same questions were exploratory-only and spent no comparisons (`dev/bench_results/6fb89a3c/v3-ledger-origin.json`). All three confirmation receipts are accepted with no errors; none qualifies for production selection, because none proposes a production change.

Every cell compares warm single-core latency on CPU 0 with 24 counterbalanced pairs of fresh child processes, five 100 ms windows per execution, and a 10000-resample percentile bootstrap [Efron1979] of the speedup of medians $\hat s = \operatorname{median}_i b_i / \operatorname{median}_i c_i$ at $\alpha_c = 0.025/m$, where $m$ counts the family's reservations on its first confirmatory attempt. The baseline $b$ is always gf2 and the candidate $c$ the external arm, so $\hat s > 1$ favours the external arm. Every family uses a material-gap threshold of 1.2 and an equivalence margin of 1.1. The gf2 arms are conservative-portable builds (`-C target-cpu=x86-64`) whose kernels come from the production runtime dispatch; the external arms are built with `-O3 -march=native`. 6-, 12- and 24-core arms are inapplicable: each surveyed routine completes a call on one thread, so those arms would replicate independent calls instead of exercising a parallel path. The issue's cells are small and cache-resident; streaming sizes are outside them.

P-20 requires $10000\,\alpha_c/2 \ge 20$ expected draws per bootstrap tail, which a first attempt meets only for $m \le 6$. The transpose and logical families confirm every pilot cell and expect 15.6 and 13.9 draws. A freeze script pinned in the pilot receipts had proposed six-cell subsets sized to that bound; the confirmation keeps the whole questions instead, which removes confirmatory claims rather than adding any. The BCH pilot addendum had already excluded its three reference-oracle cells from confirmation, since they time a test oracle rather than the production route.

| Family | Resolution pilot | Widest relative half-width at the pilot's $\alpha = 0.025$ | Frozen resolution |
|---|---|---:|---:|
| transpose | `v3-r2-6fb89a3c-transpose-pilot` | 0.0125 | 0.02 |
| logical | `v3-r1-6fb89a3c-logical-pilot` | 0.0213 | 0.03 |
| BCH | `v3-r1-6fb89a3c-bch-pilot` | 0.0188 | 0.02 |

Both margins strictly exceed one plus each resolution. The derivations are `pilot-resolution-v3-{transpose,logical,bch}.txt` and the frozen addenda `addendum-{transpose,logical-buffer,bch-genmatrix}-v3-confirmation.json`, both written by `survey/freeze-confirmation.py`. Workload seeds expand through SplitMix64 [Steele2014], implemented as `tuning_campaign_support::abtest::SplitMix64` (`dev/tools/tuning-campaign-support/src/abtest.rs`) in the gf2 arms and as `splitmix64_next` (`survey/harness_common.h`) in the C arms; the BCH cells carry a seed that no generator consumes. The protocol bootstrap draws from xoshiro256** seeded through SplitMix64 [BlackmanVigna2021]; the descriptive intervals in the tables use the SplitMix64 implementation in `survey/summarize.py`.

## 2. Operation equivalence, adaptation and unavailable comparisons (REQ-03)

`survey/verify-bit-mapping.py` compares each arm's output with naive bit arithmetic from the same seeds and writes `survey/correctness-report.json`; `survey/verify-arm-framing.py` exercises the child protocol of every arm (`survey/framing-report.json`). Code claims are in `survey/source-evidence.json`.

- **M4RI transpose.** gf2 bit $(r, c)$ is M4RI bit $(r, c)$ and the output bit $(c, r)$ equals it; three 64x64 seeds and the 63x63, 64x64, 65x65, 1x64, 64x1 and 100x130 geometries match. M4RI keeps padding bits zero and pads each row to an even word count. Consumer calls allocate a fresh output on both sides.
- **Bitshuffle transpose.** 64 elements of 8 bytes in one block of 64 are gf2's 64x64 transpose with no adapter; among the eight byte and bit orders searched, only the direct mapping passes. The library rejects element counts that are not multiples of eight, so 63 and 65 rows are unavailable unpadded. The padded adapter (rows rounded up to a multiple of eight, each row packed into $\lceil c/8 \rceil$ bytes, the first $c$ planes unpacked) matches at all six geometries and copies nothing at 64x64. Its copies cost 179 [178, 179.5] ns (pack) and 163 [162, 163] ns (unpack) at 63x63, and 185.5 [185, 188] ns and 168 [168, 169] ns at 65x65: 0.580 [0.577, 0.580] and 0.478 [0.476, 0.481] of the Bitshuffle consumer call.
- **ISA-L XOR.** Destination bit $b$ of word $w$ is the XOR of the sources' bit $b$ of word $w$, LSB-first, at nine word counts and with two and three sources. `xor_gen` takes $\text{vects} = \text{sources} + 1$ pointers, destination last and 32-byte aligned, and writes a fresh destination; gf2 matches that with a destination copy followed by in-place XORs. Forming ISA-L's pointer array costs 3 [3, 3] ns, 0.083 [0.083, 0.084] of its 7-word call and 0.0098 [0.0097, 0.0098] of its 64-word call. gf2's destination copy costs 2 to 5 ns in whole-nanosecond probes, 0.27 to 0.48 of its call (per-cell medians; intervals in the tables).
- **BCH generator matrix.** The two routes produce different systematic bases; after both layouts are reindexed to polynomial-degree order, their full-rank row spaces are equal for B1 (15,5), B2 (127,64) and B3 (255,223). Raw dump identity is falsified by design.
- **Unavailable, not substituted:** unpadded Bitshuffle at 63 and 65 rows; ISA-L's public `xor_gen`, whose SSE/AVX/AVX-512 dispatch needs NASM, which the host lacks; ISA-L source/destination aliasing (gf2's accumulate form) and alignments other than 32 bytes, both outside `raid.h`'s contract.

## 3. Pins and observed backends (REQ-02)

`survey/build-evidence.json` records sources, licences, build commands, library and executable digests, and per-route instruction mixes from `objdump`; `survey/fetch-build.sh` reproduces the builds.

| Library | Source | Licence | Build | Observed route |
|---|---|---|---|---|
| M4RI [AlbrechtBard2026] | release 20260122, tarball SHA-256 `7e033ca1fd36be8861e2f67d9d124c398fc0d830209bb0226462485876346404` | GPL-2.0-or-later: COPYING is GPLv2, `mzd.h` says "version 2 or higher", the README says GPLv2+ | `CFLAGS='-O3 -march=native -fPIC' ./configure --disable-static` | whole 64x64 blocks in `_mzd_transpose_base` use general-purpose instructions only; the small-matrix routines use ymm; no `cpuid` |
| Bitshuffle [Bitshuffle2026] | 0.5.2, commit `52aec3b80d05606c090956aecfe868489d96b95c` | MIT | `bitshuffle_core.c` and `iochain.c`, `-O3 -march=native -fPIC` | compile-time AVX2 route `bshuf_trans_bit_elem_AVX`; the library reports AVX2; no `cpuid` |
| ISA-L [IsaL2026] | v2.32.1, commit `7c3479e0a9dac17f448603ec1ad64c7c625f530c` | BSD-3-Clause | `raid/raid_base.c`, `-O3 -march=native -fPIC` | `xor_gen_base`, a byte loop of 33 general-purpose instructions that GCC did not vectorize; public `xor_gen` unavailable |
| gf2 | this tree | — | `-C target-cpu=x86-64`, LTO | kernel chosen at run time and reported by every execution: `avx2-bit-twiddle`; `gf2-xor_inplace (scalar)` below eight words and `(simd)` from eight |

## 4. Results

Each table gives the gf2 and external median ns per call (descriptive, 24 executions; bootstrap intervals in the tables) and the evaluator's speedup interval at the family's per-comparison confidence. No window is flagged in any cell.

### 4.1 Transpose (confidence 0.99688, `not-confirmatory`)

| Cell | gf2 ns | External ns | $\hat s$ [interval] | Decision |
|---|---:|---:|---|---|
| 64x64 kernel vs M4RI | 46.17 | 143.3 | 0.3222 [0.3202, 0.3237] | regressed |
| 64x64 kernel vs Bitshuffle | 46.34 | 243.5 | 0.1903 [0.1895, 0.2005] | regressed |
| consumer 63x63 vs M4RI | 136.5 | 128.5 | 1.062 [1.056, 1.074] | not-worse |
| consumer 64x64 vs M4RI | 123.0 | 154.9 | 0.7944 [0.7885, 0.8038] | regressed |
| consumer 65x65 vs M4RI | 366.8 | 195.9 | 1.873 [1.852, 1.900] | improved |
| consumer 63x63 vs Bitshuffle adapter | 136.5 | 590.1 | 0.2313 [0.2290, 0.2340] | regressed |
| consumer 64x64 vs Bitshuffle adapter | 123.5 | 250.5 | 0.4928 [0.4866, 0.4996] | regressed |
| consumer 65x65 vs Bitshuffle adapter | 367.2 | 739.7 | 0.4964 [0.4919, 0.5007] | regressed |

The 65x65 gap has a mechanism in the code and a matching measured cost. At 65x65 `BitMatrix::transpose` walks a 2x2 grid of 64x64 tiles: four tile loads that zero-pad rows beyond the matrix, four full kernel calls and four tile stores, although three of the tiles hold a single row or column of data (claims `gf2-bitmatrix-tile-zero-pad`, `gf2-bitmatrix-tile-kernel-call`). M4RI transposes the one whole block and then a 64x1 strip, a 1x64 strip and a 1x1 corner with routines sized to them (`m4ri-transpose-odd-whole-block`, `m4ri-transpose-64xlt64-strip`, `m4ri-transpose-lt64x64-strip`, `m4ri-transpose-small-corner`). From 64x64 to 65x65, gf2's consumer time grows by 2.982 [2.956, 3.005] and M4RI's by 1.265 [1.257, 1.272]; gf2's 65x65 call costs 7.945 [7.891, 8.000] times its 64x64 kernel call. At 63x63 M4RI takes its small-matrix route (`m4ri-transpose-small-path`) while gf2 pads one tile; M4RI's lead there is not material.

The kernel losses have visible causes: M4RI's whole-block path is general-purpose word code, and Bitshuffle's AVX2 routine mallocs and frees a scratch buffer in every call, plus a second one in the byte transpose it calls first (`bitshuffle-avx-malloc-per-call`, `bitshuffle-sse-byte-malloc-per-call`), against gf2's AVX2 kernel. Through the adapter Bitshuffle stays behind gf2 at 64x64, where it copies nothing, and its copies make up about half of its call at 63x63 and 65x65 (§2).

### 4.2 Logical buffers (confidence 0.99722, `not-confirmatory`)

| Words (sources) | gf2 ns | ISA-L ns | $\hat s$ [interval] |
|---|---:|---:|---|
| 7 (2) | 7.013 | 35.95 | 0.1951 [0.1938, 0.1965] |
| 8 (2) | 5.600 | 40.92 | 0.1368 [0.1354, 0.1375] |
| 9 (2) | 7.135 | 45.14 | 0.1581 [0.1568, 0.1589] |
| 16 (2) | 6.221 | 82.01 | 0.07585 [0.07552, 0.07640] |
| 32 (2) | 8.023 | 156.4 | 0.05130 [0.05103, 0.05386] |
| 63 (2) | 14.07 | 302.3 | 0.04656 [0.04629, 0.04759] |
| 64 (2) | 12.53 | 307.7 | 0.04071 [0.04055, 0.04128] |
| 65 (2) | 13.47 | 311.3 | 0.04327 [0.04294, 0.04391] |
| 64 (3) | 18.73 | 441.8 | 0.04240 [0.04228, 0.04276] |

Every decision is `regressed`: gf2 is materially ahead. ISA-L's portable `xor_gen_base` handles one byte per loop iteration (`isal-xor-gen-base-byte-loop`); gf2's SIMD route XORs four words per AVX2 instruction (`gf2-avx2-xor-instruction`). gf2's dispatch threshold shows in its own costs: 8 words take 0.798 [0.792, 0.801] of the 7-word time because the SIMD route starts at eight words (`gf2-xor-dispatch-threshold`), and 63 and 65 words, which leave a scalar tail, cost 1.123 [1.116, 1.130] and 1.075 [1.068, 1.083] of the 64-word time. A third source costs gf2 1.495 [1.485, 1.503] and ISA-L 1.436 [1.433, 1.438] of the two-source time at 64 words. These results cover ISA-L's portable reference only; its SIMD `xor_gen` remains unmeasured.

### 4.3 BCH generator matrix (confidence 0.99167, confirmatory)

| Code $(n, k)$ | gf2 ns | M4RI ns | $\hat s$ [interval] | Outcome |
|---|---:|---:|---|---|
| B1 (15, 5) | 44.35 | 216.9 | 0.2045 [0.2018, 0.2060] | `fail` |
| B2 (127, 64) | 516.1 | 4054 | 0.1273 [0.1260, 0.1294] | `fail` |
| B3 (255, 223) | 1706 | 30790 | 0.0554 [0.0547, 0.0568] | `fail` |

`fail` in a comparator-gap cell means the external arm is materially slower. gf2 fills the systematic generator column by column from a recurrence on the generator polynomial (`gf2-bch-generator-matrix-impl`, `gf2-bch-write-generator`); M4RI copies the matrix of $k$ shifted polynomial rows and reduces it with `mzd_echelonize_m4ri` in every call. Code construction (gf2) and the shifted fill (M4RI) are setup outside the windows; the tables report them per execution, with M4RI's single-pass copy and reduction probes.

This contradicts the protocol-v1 BCH pilot, which reported M4RI ahead at 1.700 [1.679, 1.782], 7.568 [7.211, 7.696] and 7.080 [7.063, 7.099] (6 pairs each, 0.95 confidence). That pilot timed `bch_generator_matrix_by_encoding`, the test-support oracle that encodes each basis vector at $O(k^2 r)$ cost (`gf2-bch-reference-complexity`), under the gf2 name. The v3 pilot's exploratory oracle cells reproduce its direction at 0.975 confidence, 1.680 [1.662, 1.702], 7.711 [7.613, 7.741] and 7.031 [6.947, 7.058], while the production route's speedups are the 0.20 to 0.055 above. The v1 conclusion is withdrawn.

## 5. Changes between pilots and confirmations, and deviations

`dev/bench_results/6fb89a3c/v3-pilot-to-confirmation-executables.txt`, written with `survey/compare-arm-code.py`, records every executable difference:

- gf2 logical arm: rustfmt reformatted its source, inserting five lines. `.text` and `.rodata` are identical; of the 26 differing bytes, 20 are the build ID and six are `.data.rel.ro` source-location line numbers, each larger by 5. The pilot build was reproduced byte for byte from the pilot source.
- ISA-L arm: GCC had deleted the pilot's pointer-array probe loop, which therefore reported 0 ns; a compiler barrier now forces every repetition's stores. `xor_body` and `xor_gen_base` are identical at unchanged addresses.
- Bitshuffle arm: the adapter probes average 100000 warm repetitions instead of one cold pass. The timed functions are identical; harness functions moved by 304 bytes and library functions by 320.
- The M4RI transpose, M4RI generator-matrix, gf2 transpose and gf2 BCH executables are byte-identical.

Pilot and confirmation probe values for Bitshuffle and ISA-L therefore measure different things; the tables label each. Further deviations:

- The failed transpose pilot `v3-r1` measured five cells before the gf2 arm rejected the adapter field of its case; `v3-r2` re-measured all eight, so five cells had two pilot trials where the pilot addendum declared one. The confirmation addendum records two.
- The gf2 arms warm their working set in the calibration calls that precede the first timed window, not in a separate pass before calibration as the protocol words it; the C arms make the separate pass.
- The logical confirmation's driver was terminated externally while session 3 waited for the lock; the campaign resumed under its identity, with the commands appended to its launcher log, and completed in five sessions.
- M4RI's generator-matrix arm reports a single-pass copy and reduction in the protocol's `pack_ns` and `dispatch_ns` fields. A single pass exceeds the warm per-call median, so the tables name these probes and treat them as no share of the timed call.

## 6. Immutable history

- Protocol-v1 exploratory pilots, superseded: `2026-09-08-6fb89a3c-transpose-pilot` (SHA-256 `ac8d8d57a6c9319aaf71a7b7afcff7d5770df55c340d4c04b297ac0374c6b7b6`), `2026-09-08-6fb89a3c-logical-pilot` (`42c3574c67b734c33e50e976b67410275e7a28a1f3c63b7c3ff62009f1b63969`) and `2026-09-08-6fb89a3c-bch-pilot` (`b3bf3282b447573c72954fe73dca2877d4f9f5aa3edef527ca17fc4dbb1d9277`). The two failed v1 transpose launches (`…-failed-framing`, `…-failed-canonical-conversion`) are not receipts.
- `v3-r1-6fb89a3c-transpose-pilot-failed-adapter-case` holds the transpose ledger's reservation 0 and five completed cells without a receipt; none of its data is used.
- The accepted v3 pilots `v3-r2-6fb89a3c-transpose-pilot`, `v3-r1-6fb89a3c-logical-pilot` and `v3-r1-6fb89a3c-bch-pilot` serve only as resolution evidence.

## 7. Criteria

| Criterion | Status | Evidence |
|---|---|---|
| REQ-01 contract | MET | three accepted v3 confirmations and three accepted v3 pilots, each pinning contract, protocol, schema, addendum, producing-input snapshot and ledger prefix; correctness and framing gates; launcher `dev/bench_results/6fb89a3c/run-campaign.sh`; no production change, so no before/after pair is due; losing, not-confirmatory and failed results kept (§4 to §6) |
| REQ-02 pins | MET | §3; `survey/build-evidence.json`, `survey/fetch-build.sh`, `survey/source-evidence.json` |
| REQ-03 mappings | MET | §2; `survey/correctness-report.json`, `survey/bitshuffle-bit-mapping.md`, the probe and share tables |
| REQ-04 addenda and receipts | MET | frozen v3 pilot and confirmation addenda and receipts for the 64x64 kernel, the 63/64/65 consumers against both comparators, 7 to 65 words and three-source parity at 32-byte alignment, with setup and arrangement probes; transpose and logical are `not-confirmatory` for the reason in §1 |
| REQ-05 BCH | MET | `v3-r1-6fb89a3c-bch-confirmation`: current pinned measurements of both implementations through the established `bch_genmatrix` rows and the M4RI construction of issue `4e732b56`; no historical receipt substitutes |

## 8. Follow-ups

1. An edge-strip route for partial tiles in `BitMatrix::transpose` should bring gf2's 65x65 to 64x64 cost ratio, 2.982 [2.956, 3.005], toward M4RI's 1.265 [1.257, 1.272] and close the 65x65 gap; re-running `transpose-consumer-65-vs-m4ri` falsifies it. Tracked by `1d4fd63d` (bit transpose and bitslice conversion).
2. ISA-L's SIMD `xor_gen` needs a new pinned arm, and NASM is a host prerequisite for building it; only then can the logical family speak about ISA-L's fastest route. Tracked by `2037941f` (mid-range buffers), whose REQ-03 compares against ISA-L.
3. A confirmatory decision on the whole transpose or logical question needs a protocol amendment that raises `bootstrap_resamples` or relaxes the tail-support rule; until then this evidence stays non-confirmatory.
