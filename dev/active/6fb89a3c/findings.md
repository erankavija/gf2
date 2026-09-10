# Transpose and logical-buffer comparison arms against M4RI, Bitshuffle and ISA-L

| Field | Value |
|---|---|
| JIT issue | `6fb89a3c` (Establish transpose and logical-buffer comparison arms) |
| Epic | `1a379447` (Maximize Zen 3 CPU throughput against open-source baselines) |
| Protocol used by the exploratory receipts | `zen3-benchmark-protocol` version 1 (`dev/active/f547c394/protocol.md`) |
| Host | `fraktaali`, AMD Ryzen 9 5900X, Linux 7.2.2, Rust 1.95.0, GCC 16.2.1 |
| Harness | `dev/active/6fb89a3c/survey/` |
| Receipts | `dev/bench_results/6fb89a3c/` |
| Status | Exploratory survey complete; protocol-version-2 confirmation blocked on dependency `f547c394` |

## 1. Question

This survey asks whether operation-equivalent primitives from M4RI,
Bitshuffle, or ISA-L expose useful Zen 3 performance gaps against gf2's
64x64 bit transpose, logical-buffer XOR, and the BCH generator-matrix
consumer. The libraries do not share gf2's layout or API by construction, so
the comparison first establishes their exact bit mappings and unavailable
geometries. The survey changes no production kernel.

## 2. Methodology

The three pilot addenda were frozen at `2026-09-08T09:12:10Z`, before their
accepted runs, and pass both the committed JSON Schema and the typed semantic
validator in `survey/gf2-side/src/bin/validate_addenda.rs`. The runner snapshots
the exact addendum, protocol, schema, measurement contract, plan, producing
sources, and executables into each receipt. Every timed chunk runs under
`dev/scripts/ccx1-bench-flock.sh --full-host` with
`CARGO_CI_NO_LOCK=1`, checkpoints at no more than two cells per session, and
resumes without repeating completed cells.

The accepted pilot receipts use Rust 1.95.0 on CPU 0. Each cell contains six
paired executions, five calibrated windows per arm per pair (60 windows total),
10,000 bootstrap resamples, and a 95% interval. Zero windows are flagged in all
14 cells. Ratios below are `baseline ns/call / external-candidate ns/call`, so a
ratio above 1 favors the external arm. Because every cell is exploratory, the
acceptance verdict is `accepted`, `qualifies=false`; no pilot supports an
adoption decision.

### 2.1 Operation-equivalent mappings (REQ-03)

`survey/verify-bit-mapping.py` compares every dump against independent naive
bit arithmetic before timing and writes `survey/correctness-report.json`.
`survey/verify-arm-framing.py` also exercises nine child-v2 request shapes,
including fixed and whole-consumer results, and writes
`survey/framing-report.json`.

- **M4RI transpose:** gf2 row `r`, column `c` maps directly to M4RI bit
  `(r,c)`, and output `(c,r)` equals input `(r,c)`. Row word bit `c` is
  `1u64 << c`. Three fixed seeds and dimensions 63x63, 64x64, 65x65, 1x64,
  and 64x1 match the naive transpose. M4RI zeroes partial-word tail bits.
  Input and output are distinct; the fixed 64x64 candidate reuses a
  preallocated output while whole-consumer calls allocate their result.
- **Bitshuffle transpose:** the smallest passing mapping is direct for exactly
  64 elements of eight bytes with block size 64; no bit reversal or byte
  permutation adapter executes, so geometry-adaptation cost is zero by
  construction in that cell. The output matches the naive transpose for three
  seeds. Element counts 63, 65, and 1 are unavailable because Bitshuffle
  requires a multiple of eight; padding would define a different comparison
  and is not timed. Input and output are distinct as required by its API.
- **ISA-L logical XOR:** for word `w` and bit `b`, output is
  `src0[w].bit[b] XOR src1[w].bit[b]`, preserving gf2's LSB-first bit order.
  Counts 1, 7, 8, 9, 16, 32, 63, 64, and 65 words pass. The exact parity arity
  is `vects=3`: two sources and one output. All buffers are 32-byte aligned.
  ISA-L's three-pointer array is formed inside every timed call; gf2 pays the
  corresponding fresh-destination copy before `xor_inplace` inside every
  timed call. Source/destination aliasing is unavailable because ISA-L does
  not contractually permit it, and no aliasing result is substituted.
- **BCH generator matrix:** B1 `(15,5)`, B2 `(127,64)`, and B3 `(255,223)`
  use the established gf2 generator-matrix-by-encoding and M4RI shifted-
  polynomial/RREF constructions. After each documented layout is reindexed
  into polynomial-degree order, their full-rank row spaces are equal. Literal
  dump identity is falsified because the algorithms choose different bases and
  systematic column layouts; row-space equality is the operation-equivalent
  observable.

### 2.2 Frozen cells and conversion accounting

| Addendum | Cells | Scope and accounting |
|---|---|---|
| `addendum-transpose-pilot.json` | fixed 64x64 vs M4RI; fixed 64x64 vs Bitshuffle; 63x63, 64x64, 65x65 whole-consumer vs M4RI | Fixed cells time only the mapped kernel. Whole-consumer calls create a fresh output; all six setup observations per arm are separately present in each receipt's `conversion.setup_ns`. |
| `addendum-logical-buffer-pilot.json` | 7, 8, 9, 63, 64, 65 words at 32-byte alignment | Every call creates a fresh output. The gf2 copy and ISA-L pointer arrangement are in the timed operation; all six setup observations per arm are also recorded. |
| `addendum-bch-genmatrix-pilot.json` | B1, B2, B3 whole-consumer | Reuses the established BCH construction. Code/polynomial setup has six observations per arm; each timed call allocates and constructs a fresh matrix. |

The setup fields are raw per-execution observations rather than aggregated
estimates; consequently this report does not attach a confidence interval to a
derived setup statistic. The primary ns/call ratios include all work declared
inside each operation above and carry the receipt-generated intervals.

## 3. Findings

### 3.1 Transpose pilot

Receipt:
`dev/bench_results/6fb89a3c/2026-09-08-6fb89a3c-transpose-pilot/receipt.json`
(SHA-256 `ac8d8d57a6c9319aaf71a7b7afcff7d5770df55c340d4c04b297ac0374c6b7b6`).
It spans three checkpointed sessions and is accepted with no findings.

| Cell | Selected paths (gf2 / external) | Ratio, 95% interval | n | Exploratory decision |
|---|---|---:|---:|---|
| fixed 64x64 vs M4RI | `avx2-bit-twiddle` / `m4ri-mzd_transpose` | 0.339 [0.322, 0.385] | 6 pairs, 60 windows | external regressed |
| fixed 64x64 vs Bitshuffle | `avx2-bit-twiddle` / `bitshuffle-bshuf_bitshuffle-avx2` | 0.190 [0.189, 0.192] | 6 pairs, 60 windows | external regressed |
| whole 63x63 vs M4RI | `BitMatrix::transpose` / `m4ri-mzd_transpose-tiled` | 1.058 [1.051, 1.076] | 6 pairs, 60 windows | external not worse |
| whole 64x64 vs M4RI | `BitMatrix::transpose` / `m4ri-mzd_transpose-tiled` | 0.797 [0.780, 0.807] | 6 pairs, 60 windows | external regressed |
| whole 65x65 vs M4RI | `BitMatrix::transpose` / `m4ri-mzd_transpose-tiled` | 1.874 [1.847, 1.912] | 6 pairs, 60 windows | external improved |

The fixed external arms lose to gf2's AVX2 bit-twiddle kernel. The 65x65
whole-consumer cell exposes a material M4RI lead, while the neighboring 63x63
and 64x64 cells do not establish the same direction. This boundary-sensitive
result is the principal transpose hypothesis for version-2 confirmation.

### 3.2 Logical-buffer pilot

Receipt:
`dev/bench_results/6fb89a3c/2026-09-08-6fb89a3c-logical-pilot/receipt.json`
(SHA-256 `42c3574c67b734c33e50e976b67410275e7a28a1f3c63b7c3ff62009f1b63969`).
It spans three checkpointed sessions and is accepted with no findings.

| Words | Selected paths (gf2 / ISA-L) | Ratio, 95% interval | n | Exploratory decision |
|---:|---|---:|---:|---|
| 7 | `gf2-xor_inplace (scalar)` / `isa-l-xor_gen_base-arity3-aligned32` | 0.160 [0.159, 0.161] | 6 pairs, 60 windows | ISA-L regressed |
| 8 | `gf2-xor_inplace (simd)` / same ISA-L base path | 0.117 [0.117, 0.118] | 6 pairs, 60 windows | ISA-L regressed |
| 9 | `gf2-xor_inplace (simd)` / same ISA-L base path | 0.153 [0.152, 0.154] | 6 pairs, 60 windows | ISA-L regressed |
| 63 | `gf2-xor_inplace (simd)` / same ISA-L base path | 0.056 [0.055, 0.073] | 6 pairs, 60 windows | ISA-L regressed |
| 64 | `gf2-xor_inplace (simd)` / same ISA-L base path | 0.036 [0.036, 0.037] | 6 pairs, 60 windows | ISA-L regressed |
| 65 | `gf2-xor_inplace (simd)` / same ISA-L base path | 0.041 [0.038, 0.044] | 6 pairs, 60 windows | ISA-L regressed |

The exact available ISA-L scalar parity arm loses at every measured size even
after both sides pay their operation-equivalent fresh-output arrangement. This
does not characterize ISA-L's public multi-binary SIMD dispatcher, which is
unavailable on this host build and remains explicitly unmeasured.

### 3.3 BCH contextual pilot

Receipt:
`dev/bench_results/6fb89a3c/2026-09-08-6fb89a3c-bch-pilot/receipt.json`
(SHA-256 `b3bf3282b447573c72954fe73dca2877d4f9f5aa3edef527ca17fc4dbb1d9277`).
It spans two checkpointed sessions and is accepted with no findings.

| Cell | Selected paths (gf2 / M4RI) | Ratio, 95% interval | n | Exploratory decision |
|---|---|---:|---:|---|
| B1 `(15,5)` | `bch_generator_matrix_by_encoding/B1` / `m4ri-genmatrix-rref` | 1.700 [1.679, 1.782] | 6 pairs, 60 windows | M4RI improved |
| B2 `(127,64)` | `bch_generator_matrix_by_encoding/B2` / `m4ri-genmatrix-rref` | 7.568 [7.211, 7.696] | 6 pairs, 60 windows | M4RI improved |
| B3 `(255,223)` | `bch_generator_matrix_by_encoding/B3` / `m4ri-genmatrix-rref` | 7.080 [7.063, 7.099] | 6 pairs, 60 windows | M4RI improved |

These are current pinned measurements produced for this issue, not historical
pre-cutover receipts. They identify a contextual generator-matrix construction
gap but do not measure encoder throughput and do not authorize replacing the
production construction.

## 4. Recommendations

1. Retain gf2's existing fixed 64x64 AVX2 transpose and logical-buffer XOR.
   Both available external kernel arms lose throughout their mapped cells.
2. Carry the 65x65 M4RI whole-consumer boundary behavior and the B1/B2/B3
   generator-matrix gaps into fresh protocol-version-2 confirmations. Attribute
   allocation, tiling, and partial-word effects before proposing production
   work.
3. Do not infer an ISA-L SIMD result from `xor_gen_base`. A future host with
   NASM may add the public `xor_gen` dispatcher as a distinct pinned arm and
   must revalidate its selected binary path.
4. Do not pad Bitshuffle's unavailable 63/65-element geometries. A padded
   batch is a separately declared whole-consumer operation with its padding
   and unpacking inside the comparison.

## 5. Losing, unavailable, and falsifying results

- M4RI's fixed 64x64 transpose and Bitshuffle's fixed 64x64 transform lose to
  gf2; M4RI's whole-consumer 64x64 arm also loses.
- ISA-L's exact `xor_gen_base` arm loses at all six measured buffer sizes.
- ISA-L's public SIMD `xor_gen` dispatcher is unavailable because NASM is
  absent, so `raid_multibinary.asm` and its SIMD objects cannot be built. The
  scalar base arm is named as such and is not presented as the dispatcher.
- Bitshuffle comparisons at 63 and 65 elements are unavailable without
  padding, and no different padded operation substitutes for them.
- Source/destination aliasing is unavailable for the ISA-L comparison.
- BCH raw dump identity is falsified; canonical row-space equivalence passes.
- Two pre-receipt transpose campaigns are preserved under
  `dev/bench_results/6fb89a3c/` as rejected framing evidence. One exposed a
  missing C JSON terminator; the other exposed generic-map key ordering in a
  typed conversion object. Neither finalized a receipt, and none of their
  samples or ratios support these findings.

## 6. External pins and observed backends (REQ-02)

The machine-readable source, license, build, archive/library digest, linked
symbol, harness-binary digest, and `--backend` observations are in
`survey/build-evidence.json`. Sources are staged under
`.agents/ext/6fb89a3c`.

| Library | Exact source | License | Build | Binary-observed path |
|---|---|---|---|---|
| M4RI | release `20260122`, tarball SHA-256 `7e033ca1fd36be8861e2f67d9d124c398fc0d830209bb0226462485876346404` | GPL-2.0-or-later | `CFLAGS='-O3 -march=native -fPIC' ./configure --disable-static` | `mzd_transpose` uses the word-SWAR 64x64 path with no runtime dispatcher; the consumer calls `mzd_echelonize_m4ri` (`m4ri-rref`). The library also records SSE2 as compiled. |
| Bitshuffle | tag `0.5.2`, commit `52aec3b80d05606c090956aecfe868489d96b95c` | MIT | core `bitshuffle_core.c` and `iochain.c`, `-O3 -march=native -fPIC`, static minimal archive | `bshuf_bitshuffle` selects the compiled AVX2 path on this host; AVX2 and SSE2 are compiled, AVX-512 and NEON are not. |
| ISA-L | tag `v2.32.1`, commit `7c3479e0a9dac17f448603ec1ad64c7c625f530c` | BSD-3-Clause | `raid/raid_base.c`, `-O3 -march=native -fPIC`, static base archive | `xor_gen_base`, scalar, no runtime dispatch; public `xor_gen` unavailable because NASM is absent. |

Source references resolve through the issue's registered citation keys
`[AlbrechtBard2026]`, `[Bitshuffle2026]`, and `[IsaL2026]`.

## 7. Criterion outcome and protocol boundary

- **REQ-01 — PARTIAL.** Correctness, release-build provenance, exclusive timed
  execution, immutable input snapshots, checkpoint/resume, negative evidence,
  and independent acceptance are present in the three accepted version-1 pilot
  receipts. No production code changes, so before/after adoption evidence is
  not applicable. The dispatch forbids treating a version-1 confirmation as
  final, and dependency `f547c394` still exposes protocol version 1; therefore
  final comparison/adoption confirmation is not claimed.
- **REQ-02 — MET.** `survey/build-evidence.json` pins every source, license,
  build command/flag, linked symbol, binary digest, and actually observed
  backend, including the unavailable ISA-L public dispatcher.
- **REQ-03 — MET.** `survey/correctness-report.json`,
  `survey/bitshuffle-bit-mapping.md`, and the preflight scripts validate bit
  mappings, five transpose geometries, partial-word padding, distinct-buffer
  aliasing contracts, nine XOR sizes, parity arity, alignment, arrangement,
  and BCH row-space equivalence. Unavailable operations are explicit.
- **REQ-04 — PARTIAL.** Three schema-valid frozen pilot addenda and accepted
  baseline receipts cover fixed 64x64 transforms, 63/64/65 matrix consumers,
  7/8/9 and 63/64/65-word logical buffers, 32-byte alignment, and setup/
  conversion accounting. Confirmatory addenda and receipts await protocol
  version 2 and must cite these committed pilots as resolution evidence.
- **REQ-05 — MET.** The B1/B2/B3 receipt reuses the established BCH harness
  construction, measures both current pinned implementations in this campaign,
  and does not substitute a historical receipt or start an encoder campaign.

The accepted pilots pin protocol SHA-256
`4cc897ed267e40a8bfa1fbc18950e6539c4456d0e22a6b84d42d9dab43132117`.
Protocol version 2 is not present in this checkout or current `main` at the
time of this report. A final confirmation therefore stops at the explicit
dependency boundary required by the dispatch.
