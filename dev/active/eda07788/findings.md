# Shift and coding-permutation baselines

> **Diátaxis Type:** Reference (research findings)

## Question

Which of gf2's arbitrary-offset bit shifts, quasi-cyclic circulant rotations,
DVB-T2 bit interleaving and 5G NR rate matching have a reproducible,
operation-equivalent open-source external baseline; where one exists and both
sides expose it, what do committed receipts say about the gap; and where one
does not, what exactly is missing on which side?

## Method

Four candidate operations were surveyed against the sources the issue names and
the epic's prior baseline survey (`dev/active/4e732b56/baseline-survey/`):
AFF3CT [Cassagne2019] (the issue's starting point), xdsopl/LDPC [Xdsopl2026]
and srsRAN_Project [Srsran2026]. Source was read directly rather than inferred
from documentation, then the mapping was made executable wherever a claim about
semantics could be checked by running code instead of asserting prose.

Four committed commands produce every artifact below:

- `dev/active/eda07788/survey/fetch-build.sh` clones and verifies the pinned
  xdsopl/LDPC checkout, builds the three timed arms in two flavours, and runs
  the adapter correctness gate. Its output is
  `dev/active/eda07788/survey/validation-output-v3.txt`.
- `dev/active/eda07788/survey/run-analysis.sh` runs the two mapping analyses
  that reach no timed cell. Its output is
  `dev/active/eda07788/survey/analysis-output-v3.txt`.
- `dev/active/eda07788/survey/inspect-sources.py` verifies the pinned external
  trees and emits `survey/source-evidence.json`, including exact source lines,
  the AFF3CT 5G matrix inventory and the negative DVB-T2 search.
- `dev/bench_results/eda07788/run-dvb-t2-baselines.sh pilot-v3|confirmation-v3`
  measures the frozen family addenda as bounded checkpointed sessions under the
  canonical CCX1 mutex.

The current timed work is governed by the epic's
[measurement contract](../1a379447-zen3-cpu-performance/measurement-contract.md)
and [protocol version 3](../f547c394/protocol.md). The preserved receipts below
ran under protocol version 1; each carries its exact immutable protocol at
`inputs/protocol.md`, so no historical link misidentifies those bytes as v3.
Correctness is established before timing. No production kernel changes: the
whole diff is under `dev/`.

### Pinned external sources

| Source | Pin | License | Staging |
|---|---|---|---|
| xdsopl/LDPC [Xdsopl2026] | commit `32357d8ad55a6a302c34e093759f0454e45cca56` | permissive, `LICENSE` copied into `survey/xdsopl-pin.txt` | fetched by this issue's `survey/fetch-build.sh` |
| AFF3CT [Cassagne2019] | tag `v4.7.0`, commit `e8a65c5047262d97a15563b9edc961f69b2792cc`; configuration submodule `ecae10cd0375f12febe2f5513f8ca39f2540c640` | MIT | verified by `survey/inspect-sources.py` |
| srsRAN_Project [Srsran2026] | tag `release_25_10`, commit `d2f4b70dda8e2c557d5b05a0ac5f92dbddda19bc` | AGPL-3.0-only | verified by `survey/inspect-sources.py` |

### Build identities of the timed arms

`fetch-build.sh` builds two flavours, each compiling the Rust side and the C++
shim at one architecture level so no arm mixes levels:

| Arm | Build identity | Flags |
|---|---|---|
| gf2 `DvbT2BitInterleaver::interleave` | `native` | `RUSTFLAGS=-C target-cpu=native` |
| gf2 `DvbT2BitInterleaver::interleave` | `conservative-portable` | `RUSTFLAGS=-C target-cpu=x86-64` |
| xdsopl `PCTITL` adapter | `external` | `RUSTFLAGS=-C target-cpu=native`, `g++ -O3 -march=native -std=c++17` |

The `conservative-portable` arm is the scalar/compiler control REQ-04 requires:
the same source at the baseline x86-64 architecture level, with no target
feature beyond SSE2. The external arm is always built at its best, so every
cell that compares it against the portable gf2 build is a conservative bound on
gf2.

## Operation mapping outcomes

| Operation | External comparator | Mapping | Measured |
|---|---|---|---|
| DVB-T2 §6.1.3 bit interleaving | xdsopl `PCTITL` | operation-equivalent, validated bit-exact | yes, six confirmatory cells; gf2 leads 2.2-2.3x, most of it conversion cost |
| 5G NR rate matching / puncturing | AFF3CT `Puncturer_5G` | selection rule equivalent, validated over six configurations | no: neither project exposes the step as an entry point |
| 5G NR circulant rotation | AFF3CT `_CSRAA`, srsRAN `circ_shift_backward` | external arms exist, no gf2 counterpart | no: gf2 exposes no per-frame data-buffer rotation |
| Arbitrary zero-fill bit shift | none found | unmatched | no |

Only the first row reaches the scorecard. The other three are results, not
gaps, and none of them was turned into a cell by comparing unrelated
arithmetic.

## 1. DVB-T2 bit interleaver — matched, measured

gf2's `DvbT2BitInterleaver`
(`crates/gf2-coding/src/ldpc/dvb_t2/bit_interleaver.rs`) implements ETSI EN
302 755 v1.4.1 [Etsi2015] §6.1.3 (parity interleaving followed by column-twist
interleaving) for 16-QAM and 64-QAM.

AFF3CT has no DVB-T2 support: its LDPC encoder directory covers DVB-S2 only
(`src/Module/Encoder/LDPC/DVBS2/`), a search of `src` and `include` for
`dvb.?t2` and `302 755` returns nothing, and its generic
`src/Tools/Interleaver/*` cores (ARP, CCSDS, Column_row, Golden, LTE, NO,
Random, Random_column, Row_column, User) are unrelated interleaver families,
not a standards-defined column twist.

xdsopl/LDPC targets DVB-S2 and DVB-T2: its `README.md` links the same ETSI
document (`en_302755v010401p.pdf`) among its DVB standards, and
`dvb_t2_tables.hh` carries the EN 302 755 Annex A and B parity-check tables
that `tables_handler.cc:150` selects. Its `interleaver.hh` defines
`PITL<TYPE,N,Q>` (stage 1) and `PCTITL<TYPE,N,Q,CT>` (stage 1 composed with
stage 2, with `CT8`/`CT12` implementing the column-twist formula parameterized
by the Table 10 twist offsets).

Its own `itls_handler.cc` composes `PCTITL` with an outer
`BITL<..., MUX8/MUX12>` wrapper that folds in the §6.1.4/§6.1.5 bit-to-cell
demux; gf2's module scopes that stage out, so the operation-equivalent baseline
is `PCTITL` used directly, not the outer `BITL` composition. The same file
instantiates `PCTITL<code_type, 64800, 90, CT>` (line 81) and
`PCTITL<code_type, 16200, 25, CT>` (line 102) for the Rate 1/2 frames, which is
an independent confirmation of the `(N, Q)` pairs
`survey/xdsopl-shim/xdsopl_shim.cpp` instantiates.

### Semantic equivalence, established before timing

With `in[i] = i`, `PITL::fwd` produces `out[K+360q+m] = K+Q·m+q`, matching gf2's
`parity_perm[k+360t+s] = k+q·s+t` field for field. `PCTITL`'s subsequent
`CT::fwd(out+Nc·row, in, Nr, row)` resolves to
`out[row·Nc+c] = in[c·Nr + (row+Nr−tc[c]) mod Nr]`, which is exactly gf2's
`inverse[j] = parity_perm[c·nr + (r+nr−twist[c]) mod nr]` with `c = j mod nc`,
`r = j / nc`, `j = row·nc + c`.

`survey/gf2-side/src/bin/validate-permutation-equivalence.rs` checks that
analytic result and more, and `fetch-build.sh` runs it against the build it
produces, so the evidence comes from the committed path rather than from an
ad-hoc clone. Its committed output
(`dev/active/eda07788/survey/validation-output-v3.txt`) reads:

```
PASS canonical bit order: 128/128 bits round-trip, lengths 0/1/63/64/65/127/128 preserved with zero tail padding
PASS qam16-r12-normal: 64800/64800 positions match
PASS qam64-r12-normal: 64800/64800 positions match
PASS qam16-r12-short: 16200/16200 positions match
PASS qam64-r12-short: 16200/16200 positions match
PASS qam16-r12-normal: 2 banks of 64800 bits, both timed arms agree bit-for-bit
PASS qam64-r12-normal: 2 banks of 64800 bits, both timed arms agree bit-for-bit
PASS qam16-r12-short: 2 banks of 16200 bits, both timed arms agree bit-for-bit
PASS qam64-r12-short: 2 banks of 16200 bits, both timed arms agree bit-for-bit
```

The three checks cover what REQ-03 names: canonical little-endian bit order and
zero tail padding through the conversion helpers at the word boundaries 0, 1,
63, 64, 65, 127 and 128; the standards-defined output permutation at every
position of all four MODCODs (16-QAM and 64-QAM, Normal and Short FECFRAME,
Rate 1/2, twist offsets cross-checked against gf2's own
`test_64qam_normal_col_twist_spec_example` and `test_twist_offsets_match_spec`);
and the two timed arms' whole-consumer outputs on identical seeded frames, which
is the exact code path each arm times.

### Cells, arms and conversion costs

Both frozen addenda declare whole-consumer cells whose timed call includes every
conversion cost, as REQ-03 requires:

- The gf2 arms operate on gf2's native packed `BitVec` end to end and report
  `pack_ns = unpack_ns = 0`. That zero is a measured property of the arm,
  reported rather than omitted, and the interleaver's one-time table
  construction is reported separately as `setup_ns`.
- The external arm operates in xdsopl's native per-bit `int32` representation,
  so each timed call unpacks the packed frame, applies `PCTITL::fwd` and packs
  the result back. Both conversions run inside the timed closure and are
  reported as `unpack_ns` and `pack_ns` averaged over the measured windows.

Every cell names the established gf2 implementation as its baseline arm and the
arm whose lead needs attribution as its candidate, so a speedup above 1 favours
the candidate: above 1 in a `-gap-` cell means xdsopl is ahead of gf2, above 1
in a `-control-` cell means the `native` gf2 build is ahead of the
conservative-portable control.

### Coverage, and the arms that are not meaningful here

The confirmatory family is six cells: two MODCODs (16-QAM and 64-QAM, Rate 1/2
Normal FECFRAME) crossed with three comparisons — `native` gf2 against xdsopl,
the `conservative-portable` control against xdsopl, and the control against
`native` gf2. Every one is a single-core-latency cell with a warm cache state.
The exploratory pilot adds four cells the confirmatory family does not carry: an
identity cell that measures the resolution, both Short FECFRAME MODCODs for
small-input coverage, and a `streaming` cell whose working set rotates through
the eight fixture banks so successive calls do not reuse cache-resident data.

The 6-, 12- and 24-CPU core arms the measurement contract asks for "where
meaningful" are not meaningful for this operation, and no samples were
fabricated for them. Both arms permute one FECFRAME per call with no
intra-frame parallelism, and neither project exposes a multi-frame batch entry
point, so a multicore cell would have to spread frames across a thread pool that
exists in the survey harness and in neither implementation. That measures the
harness. The throughput dimension those arms would probe — behaviour once the
working set stops fitting in cache — is covered instead by the exploratory
`streaming` cell, which changes the working set rather than the worker count.

### Historical protocol-v1 measured results

Host: `fraktaali`, AMD Ryzen 9 5900X (12 cores, SMT active, all 24 logical CPUs
in the runner's mask, `powersave` governor), Linux 7.2.2-arch1-1, rustc 1.97.0
(2d8144b78 2026-07-07), g++ (GCC) 16.2.1. Every timed session ran under
`dev/scripts/ccx1-bench-flock.sh --full-host` with the lock evidence journalled;
`settings_deviation` is false, so the frozen shared settings were in force.

Two protocol-v1 receipts are preserved byte-for-byte and were accepted with no
findings by their receipt-local evaluator. Protocol v3 supersedes them for the
current criterion decision:

| Receipt | Label | Cells | Pairs per cell | Sessions | Digest |
|---|---|---|---|---|---|
| `dev/bench_results/eda07788/2026-09-08-eda07788-dvb-t2-pilot/` | pilot | 10 exploratory | 6 | 2 | `ca889363f8180e86cf635f720014c9b373421b12e53b3484825ebc1cbfeaa33f` |
| `dev/bench_results/eda07788/2026-09-08-eda07788-dvb-t2-confirmation/` | confirmation | 6 confirmatory | 24 | 3 | see `receipt.json` |

The confirmation cites the pilot receipt by path and digest as its resolution
evidence, and the runner snapshots it at
`inputs/resolution-evidence/receipt.json`. Neither receipt qualifies for
production selection, which is correct: this issue proposes no production
change.

#### Confirmatory cells

Speedup of medians, `median(baseline) / median(candidate)`, with the percentile
bootstrap interval at the family confidence `1 - 0.05/6 = 0.99167` over 24
fresh pairs and 10 000 resamples. No window was flagged in 1440.

| Cell | Speedup | Interval | Decision | Outcome |
|---|---|---|---|---|
| `qam16-r12-normal-gap-native-vs-xdsopl` | 0.4583 | [0.4536, 0.4666] | regressed | fail |
| `qam16-r12-normal-gap-portable-vs-xdsopl` | 0.4563 | [0.4533, 0.4579] | regressed | fail |
| `qam16-r12-normal-control-portable-vs-native` | 0.9949 | [0.9744, 0.9994] | not-worse | not-material |
| `qam64-r12-normal-gap-native-vs-xdsopl` | 0.4302 | [0.4289, 0.4316] | regressed | fail |
| `qam64-r12-normal-gap-portable-vs-xdsopl` | 0.4266 | [0.4249, 0.4278] | regressed | fail |
| `qam64-r12-normal-control-portable-vs-native` | 0.9932 | [0.9896, 0.9984] | not-worse | not-material |

**Reading the `fail` outcomes.** In a comparator-gap cell the candidate is the
comparator, so `improved` would mean a material gap in the comparator's favour
and `regressed` is the protocol's token for the opposite: the established
implementation is materially ahead. gf2 runs the operation in 0.43-0.46 of the
external adapter's whole-consumer time on both MODCODs and under both gf2 build
identities, so the comparator gap this family was frozen to detect does not
exist in the comparator's direction. Nothing here is a defect in gf2, and
nothing was hidden: the four cells are recorded with the token the decision rule
assigns them.

#### Attribution: where the difference is

The whole-consumer cell includes representation conversion, and the receipt
reports it per arm, so the difference is attributable rather than merely
observed. Per-call medians over the confirmatory pairs:

| Arm | Call | Unpack | Pack | Setup |
|---|---|---|---|---|
| gf2 `native` | 186.4 µs (16-QAM), 186.8 µs (64-QAM) | 0 | 0 | 817 µs, once |
| gf2 `conservative-portable` | 185.5 µs (16-QAM), 185.7 µs (64-QAM) | 0 | 0 | 809 µs, once |
| xdsopl `external` | 406.8 µs (16-QAM), 434.2 µs (64-QAM) | 27.2 µs | 206-208 µs | 0 |

The external arm spends about 233 µs of its 407-435 µs call converting between
gf2's packed frame and its own one-`int32`-per-bit representation — most of the
difference. Its permutation alone, 174-202 µs, is comparable to gf2's 185 µs on
the native packed `BitVec`. The honest statement of the result is therefore
narrower than the raw ratio: **on this operation the two permutation kernels are
close, and gf2's whole-consumer lead comes from not paying a representation
conversion its native packed layout makes unnecessary.** That is exactly the
cost the measurement contract requires inside a whole-consumer cell, and it
would have been invisible had the conversion been excluded.

gf2's `setup_ns` of roughly 0.81 ms is one `DvbT2BitInterleaver::new`, which
builds two 64 800-entry index tables. It is reported separately and never
amortized into the per-call figure, because the table is built once per MODCOD
and reused.

#### The scalar/compiler control

Both control cells put `-C target-cpu=native` within 0.5-0.7% of the
conservative-portable build, and both intervals lie inside the 10% equivalence
margin and inside the family's declared measurement resolution of 0.06. The
architecture level buys this operation nothing measurable, which is what one
should expect of a scalar gather over a 518 KB index table: the work is
load-latency and cache traffic, not arithmetic width. The intervals sit slightly
below 1, but the family's declared resolution does not support a claim that
either build is faster, and the decision recorded is `not-worse`.

#### Exploratory coverage

The pilot's other four cells are exploratory and support no adoption decision.
They size the family and cover the dimensions the confirmatory cells do not:

| Cell | Speedup | Interval (95%) |
|---|---|---|
| `qam16-r12-normal-null-native-vs-native` (identity) | 1.0023 | [0.9900, 1.0176] |
| `qam16-r12-short-gap-native-vs-xdsopl` | 0.1673 | [0.1556, 0.1740] |
| `qam64-r12-short-gap-native-vs-xdsopl` | 0.1601 | [0.1593, 0.1670] |
| `qam16-r12-normal-streaming-gap-native-vs-xdsopl` | 0.5269 | [0.5238, 0.5281] |

The identity cell lands on 1 as it must. The Short FECFRAME cells show the
external arm's fixed conversion cost dominating even harder at a quarter of the
frame size, and the streaming cell shows gf2's lead narrowing from 0.456 to
0.527 once the working set rotates through eight banks and stops being
cache-resident — the direction the smaller packed footprint predicts. These are
exploratory figures and are labelled as such.

The v1 family's measurement resolution, 0.06, is the largest symmetric relative
confidence-interval half-width over all ten pilot cells, computed by
`dev/bench_results/eda07788/pilot-resolution.py` from the committed pilot
acceptance summary. Restricted to the six cells this family measures the largest
is 0.0245. Protocol v3 uses the wider of the two endpoint deviations and
corrected alpha, so the current resolution comes from a fresh v3 pilot.

## 2. 5G NR rate matching / puncturing — selection rule matched, no timed cell

**This corrects a falsified claim.** The earlier survey recorded that AFF3CT
v4.7.0 "has no 3GPP TS 38.212 / 5G NR support at all". That is wrong. The
searches it ran (`NR`, `38.212`, `38.211`, `nr_ldpc`, `rate.match`) do return
nothing, but the support is spelled `5G`:

- `src/Tools/Code/LDPC/Standard/5G/5G_base_graph.cpp` selects BG1 or BG2 from
  `K` and the rate, chooses the lifting size `Zc` from the eight 3GPP lifting
  sets, and sets `K_LDPC = Kb·Zc`, `N_LDPC = Nb·Zc` (lines 115-116).
- `conf/enc/LDPC/5G/NR_*.txt` holds 97 pinned NR parity-check matrices.
- `src/Module/Puncturer/LDPC/Puncturer_5G.cpp:33` implements the TS 38.212
  §5.4.2.1 bit selection: a circular buffer over `[2·Zc, N_cw)` read forward
  with wraparound, skipping the filler range `[K, K_LDPC)`, until `N` bits are
  emitted.
- The factory path wires it in:
  `src/Factory/Module/Puncturer/LDPC/Puncturer_LDPC.cpp:109-116`,
  `src/Factory/Tools/Codec/LDPC/Codec_LDPC.cpp:75-84`,
  `src/Factory/Module/Encoder/LDPC/Encoder_LDPC.cpp:101-114,144-155`, and
  `src/Factory/Module/Decoder/LDPC/Decoder_LDPC.cpp:156-161`.

So AFF3CT is an applicable, pinned external path for this operation, and the
earlier conclusion that gf2's layout is structurally incomparable to it does not
survive contact with the code either.
`survey/analysis/src/bin/compare-nr-rate-matching.rs` recovers gf2's
transmitted-column list through the public `prepare_llrs` mapping and evaluates
AFF3CT's rule — transcribed from `Puncturer_5G.cpp:33-47`, not linked — on gf2's
own code parameters. The two select **identical bits in all six surveyed
configurations**:

```
AGREE  BG2 target_n=256 target_k=121 (Z=22, full_k=220, full_n=1144, 2Z=44, parity_kept=179): 256 selected bits identical
AGREE  BG2 target_n=512 target_k=200 (Z=26, full_k=260, full_n=1352, 2Z=52, parity_kept=364): 512 selected bits identical
AGREE  BG2 target_n=1024 target_k=400 (Z=52, full_k=520, full_n=2704, 2Z=104, parity_kept=728): 1024 selected bits identical
AGREE  BG1 target_n=1024 target_k=512 (Z=24, full_k=528, full_n=1632, 2Z=48, parity_kept=560): 1024 selected bits identical
AGREE  BG1 target_n=2048 target_k=1024 (Z=48, full_k=1056, full_n=3264, 2Z=96, parity_kept=1120): 2048 selected bits identical
AGREE  BG1 target_n=4096 target_k=2048 (Z=96, full_k=2112, full_n=6528, 2Z=192, parity_kept=2240): 4096 selected bits identical
```

Two qualifications belong with that result.

First, the comparison isolates the **selection rule** by evaluating it on gf2's
parameters. The two projects derive the mother-code dimension differently for
BG2: gf2 uses the base-graph maximum `K_b = 10` for `full_k = K_b·Z` and applies
the §5.2.2 block-size-dependent `K_b` only to lifting-size selection
(`kb_for_z_selection`, `crates/gf2-coding/src/ldpc/nr_5g/mod.rs:218`), while
AFF3CT uses its block-size-dependent `Kb` for both, so its `K_LDPC` is `6·Zc` or
`8·Zc` for small `K`. On the same `(K, N)` the two therefore rate-match
different mother codewords even though the rule applied to each is the same.
The analysis output records both lifting choices per configuration. AFF3CT's
own `Kb` table additionally maps `560 < K ≤ 640` to `8` through a branch
identical to the one below it (`5G_base_graph.cpp:75-82`), where gf2's
`kb_for_z_selection` documents `9` for that band.

Second, **no timed cell follows**, and none was manufactured. gf2's equivalent
step is the `transmitted_cols` gather fused inside `encode_rate_matched`
(`crates/gf2-coding/src/ldpc/nr_5g/mod.rs:1120-1123`) with no public entry
point of its own; AFF3CT's is a protected member of a streampu module. Timing
either project's surrounding encoder would compare encoders, not bit selection,
and re-implementing the gather in the survey harness would time the harness.
The tracked, falsifiable follow-up is a whole-consumer encoder cell (gf2's
`encode_rate_matched` against AFF3CT's `Encoder_LDPC_QC` plus `Puncturer_5G`),
which is a different operation family from this issue's permutations and needs
its own addendum.

srsRAN's rate matcher
(`lib/phy/upper/channel_coding/ldpc/ldpc_rate_matcher_impl.cpp:104`,
`select_bits`, with `shift_k0` computed at line 60 per Table 5.4.2.1-2) is the
same §5.4.2.1 selection with a nonzero redundancy-version starting offset. It
stays pinned as a second external candidate for that follow-up.

## 3. 5G NR quasi-cyclic circulant rotations — external arms exist, gf2 has no counterpart

Both surveyed sources apply a real circulant permutation to a data buffer:

- srsRAN `lib/phy/upper/channel_coding/ldpc/ldpc_encoder_generic.cpp:135` calls
  `srsvec::circ_shift_backward(out_node, codeblock_node, node_shift)` with the
  shift read from the lifted base graph, and
  `include/srsran/srsvec/circ_shift.h` documents the forward and backward
  circular shifts it uses.
- AFF3CT `src/Module/Encoder/LDPC/QC/Encoder_LDPC_QC.cpp:87` rotates each
  `Zc`-sized generator block with `std::rotate` inside `_CSRAA` (line 77),
  called per systematic block from `_encode` (line 105).

The gap is entirely on gf2's side. Its only circulant primitive is
`CirculantMatrix::to_edges` (`crates/gf2-coding/src/ldpc/core.rs:533`), which
generates `(row, col)` sparse-matrix coordinates for one `Z×Z` block; every
caller is matrix construction (`core.rs:718`, `nr_5g/mod.rs:2127`, the sparse
benchmark emitter and the QC tests), and it runs once per code rather than per
frame. gf2's `nr_5g` encoder applies parity through one dense
`matvec_transpose` and never isolates a cyclic shift of a data buffer.

Timing gf2's one-time coordinate generator against a per-frame production kernel
would not be a comparable operation, and writing a gf2-side circulant-shift
primitive is production kernel work this survey excludes. Recorded as an
inapplicable mapping and kept out of the scorecard. Both external arms stay
pinned for the day gf2 exposes such a primitive.

## 4. Arbitrary zero-fill bit shifts — unmatched

`BitVec::shift_left`/`shift_right` (`crates/gf2-core/src/bitvec.rs:475,536`)
and their AVX2 word-shift kernels
(`avx2_shift_left_words`/`avx2_shift_right_words`,
`crates/gf2-kernels-simd/src/x86/avx2.rs:460,512`) shift an arbitrary-length
bit vector by an arbitrary offset, zero-filling the vacated positions. This is a
raw bit-vector primitive, not a coding-domain operation with a standards
mapping, and none of AFF3CT, xdsopl/LDPC or srsRAN_Project exposes a
general-purpose arbitrary-offset zero-fill bit-vector shift as a public
operation: AFF3CT's and xdsopl's permutation-shaped operations are all
fixed-purpose interleavers and puncturers over coded-bit orderings, and srsRAN's
`srsvec` shift primitives are circular.

That a circular shift is not a substitute is measured rather than asserted.
`survey/analysis/src/bin/validate-shift-semantics.rs` checks gf2's shifts
against an independent zero-fill reference and against a wrap-around rotation of
the same input:

```
PASS zero-fill semantics: 148 shifts over lengths [0, 1, 63, 64, 65, 127, 128, 4096, 64800] match an independent zero-fill reference with canonical indexing and zero tail padding
PASS wrap is a different operation: 88/88 nonzero offsets give a different result under wrap-around rotation than under gf2's zero-fill shift
```

The offsets cover 0, 1, 63, 64, 65, 127, 128, `len−1`, `len` and `len+1` for
each length, so the saturating cases where the whole vector is cleared are
included. This is an explicit unmatched result, not forced into the scorecard.

## Preserved negative, unavailable and falsifying results

- AFF3CT exposes no DVB-T2 interleaver. Its generic interleaver cores are not
  operation-equivalent substitutes and were not measured as if they were.
- Three of the four surveyed operations reach no timed cell, for three
  different and separately evidenced reasons. None was replaced by a different
  operation to produce a number.
- The earlier survey's claim that AFF3CT has no 5G NR support is contradicted by
  the code and is corrected in section 2; the contradiction is recorded here
  rather than silently rewritten.
- The earlier survey's claim that gf2's rate-matching column order is
  structurally incomparable to AFF3CT's block pattern is contradicted by the
  measured agreement over six configurations and is corrected in section 2.

## Protocol-v3 continuation

The continuing scientific family is `dvb-t2-bit-interleave-baselines` for both
pilot and confirmation. The v3 pilot addendum is frozen at
`addendum-dvb-t2-bit-interleave-v3-pilot.json`. The confirmation addendum is
created and frozen only after the fresh pilot supplies its receipt digest and
conservative resolution.

`dvb-t2-bit-interleave-baselines-trial-ledger.jsonl` begins with the completed
v1 confirmation rather than an empty history. The imported reservation records
six comparisons and the two candidate identities derived by the canonical
`trial_ledger::candidate_ids` function from the receipt-local v1 addendum, saved
plan and `receipt.arms`. `trial-ledger-v1-import.json` pins all three input
digests and identifies the import as retrospective accounting rather than a
premeasurement reservation. No other failed or interrupted confirmation exists
under this issue's receipt tree.

`producing-inputs.json` narrows the v3 producing closure to this launcher,
harness, production dependencies and the shared protocol implementation. The
runner plan names that issue-owned manifest, so unrelated benchmark families do
not enter this family's behavioral identity.

## Criterion outcomes

**REQ-01 — measurement contract, receipts pinning contract, protocol and
addendum.** Partially met. The v1 receipts preserve the full historical evidence
and negative outcomes. Current completion requires accepted v3 pilot and
confirmation receipts with the issue-owned producing manifest, current protocol
and ledger prefix. No production change is proposed, so before/after adoption
evidence does not apply.

**REQ-02 — survey and pin applicable coding permutation paths, starting with
AFF3CT; preserve unmatched results for arbitrary shifts.** Met. AFF3CT v4.7.0,
xdsopl/LDPC and srsRAN_Project are pinned by commit with their licenses and
exact source lines recorded in `survey/source-evidence.json`. AFF3CT's 5G NR
LDPC support is recorded together with the executable agreement between its bit
selection and gf2's. Arbitrary zero-fill shifts are recorded as unmatched, with
the difference from wrap-around measured.

**REQ-03 — validate zero-fill versus wrap, offsets, canonical bit order, tails
and standards permutations; include packing and cross-lane conversion in
comparable cells.** Met. Zero-fill versus wrap and offset ranges are covered by
`survey/analysis/src/bin/validate-shift-semantics.rs` (148 shifts over nine
lengths including 0/1/63/64/65, offsets through `len+1`); canonical bit order,
tails and the standards permutation are covered by
`survey/gf2-side/src/bin/validate-permutation-equivalence.rs`. Packing and
cross-lane conversion remain inside every whole-consumer cell and are reported
per arm.

**REQ-04 — frozen family addenda and internal/external baseline receipts for
measured-relevant cells including scalar/compiler controls; record inapplicable
mappings without forcing unrelated arithmetic.** Partially met. The v1 pilot and
confirmation remain historical evidence. The v3 pilot is frozen and the
continuing family ledger is seeded. The six measured-relevant cells and
scalar/compiler controls are carried forward from the v1 confirmation after the
fresh pilot fixes the v3 resolution. A fresh accepted v3 pilot and confirmation
remain before this criterion is current.

## Recommendations

- Keep xdsopl/LDPC's `PCTITL` as the epic's reference comparator for DVB-T2
  §6.1.3 bit interleaving. No production change is proposed by this issue.
- Track a whole-consumer 5G NR encoder comparison (gf2 `encode_rate_matched`
  against AFF3CT `Encoder_LDPC_QC` plus `Puncturer_5G`, with srsRAN as a second
  arm) as a separate issue with its own addendum. It is a different operation
  family from this issue's permutations.
- Re-survey circulant rotations against srsRAN `circ_shift_backward` and AFF3CT
  `_CSRAA` if gf2 ever exposes an isolated per-frame circulant shift of a data
  buffer.

## Open questions

- Whether a per-frame circulant-shift primitive is worth adding to gf2 at all is
  a production-kernel design question outside this survey's scope.
- Whether gf2 should expose its rate-matching bit selection as its own API is
  the same kind of question; without it, the operation cannot be compared
  against AFF3CT or srsRAN in isolation, only inside an encoder.

## Citations

- [Cassagne2019] — AFF3CT.
- [Xdsopl2026] — xdsopl/LDPC.
- [Srsran2026] — srsRAN_Project.
- [Etsi2015] — ETSI EN 302 755 v1.4.1 (DVB-T2).
