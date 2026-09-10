# Shift and coding-permutation baselines

> **Diátaxis Type:** Reference (research findings)

## Result

Two operations have an operation-equivalent external arm, and both were
measured under protocol version 3.

- **DVB-T2 bit interleaving** against xdsopl/LDPC. The confirmation
  `confirmation-v3-eda07788-20260908t194030z` is accepted, and all six cells
  are `not-confirmatory` under P-20: 3.47 expected bootstrap draws per tail
  against the required 20. At the point estimates, gf2's whole-consumer call is
  2.040 to 2.183 times faster than the adapter (speedups 0.4580 to 0.4901, 24
  pairs each). The gap is the adapter's representation conversion.
- **5G NR LLR de-rate-matching** against AFF3CT. The confirmation
  `nr-derate-confirmation-eda07788-20260910t170957z` is accepted with zero
  findings, and all six cells are confirmatory. In the five gap cells gf2 is
  2.865 to 3.128 times faster (speedups 0.3197 to 0.3491, 24 pairs each). The
  decision records `fail`, the protocol's token for an external arm more than
  the 10% equivalence margin slower. The gap lies in AFF3CT's de-puncturing
  itself, not in the adapter.

In both families the native and conservative-portable gf2 builds cannot be
told apart. NR bit selection has no gf2 entry point. Circulant rotation has no
gf2 counterpart, and arbitrary zero-fill shifts have no external counterpart.
No production code changes.

## Question

Which of gf2's arbitrary-offset bit shifts, quasi-cyclic circulant rotations,
DVB-T2 bit interleaving and 5G NR rate matching have a reproducible,
operation-equivalent open-source external baseline? Where both sides expose
one, what do committed receipts say about the gap? Where one does not, what is
missing, and on which side?

## Method and evidence

The [measurement contract](../1a379447-zen3-cpu-performance/measurement-contract.md)
and [protocol version 3](../f547c394/protocol.md) govern all timed work.
Committed commands produce every artifact:

| Command | Output |
|---|---|
| `survey/fetch-build.sh` | xdsopl arms and DVB-T2 adapter gate: `survey/validation-output-v3.txt` |
| `survey/nr-derate-build.sh <aff3ct-root>` | AFF3CT pin checks, NR arms and equivalence gate: `survey/nr-derate-validation.txt` |
| `survey/run-analysis.sh` | shift semantics and NR selection comparison: `survey/analysis-output-v3.txt` |
| `survey/analysis` `bootstrap-resolution` | NR resolution derivation `pilot-resolution-nr-derate.txt`; DVB-T2 endpoint stability `survey/dvb-t2-v3-endpoint-stability.txt` |
| `survey/inspect-sources.py` | `survey/source-evidence.json`: every code claim below with project, commit, path, line, verbatim text and interpretation |
| `../../bench_results/eda07788/run-dvb-t2-baselines.sh`, `run-nr-derate-baselines.sh` | bounded checkpointed campaigns under the CCX1 full-host lock |
| `../../bench_results/eda07788/reevaluate-v3.sh` | independent re-evaluation of all five v3 receipts: `reevaluation-v3.log` |
| `../../bench_results/eda07788/summarize-v3.py` | [DVB-T2 tables](../../bench_results/eda07788/tables-v3.md) and [NR tables](../../bench_results/eda07788/tables-nr-derate.md) |

Code claims cite `source-evidence.json` claim IDs in backticks. Every number
below appears in the tables, an acceptance summary or a named output file.

| Source | Pin | License | Role here |
|---|---|---|---|
| xdsopl/LDPC [Xdsopl2026] | commit `32357d8ad55a6a302c34e093759f0454e45cca56` | zero-clause BSD grant (`xdsopl-license`) | timed DVB-T2 arm |
| AFF3CT [Cassagne2019] | tag `v4.7.0`, commit `e8a65c5047262d97a15563b9edc961f69b2792cc` | MIT (`aff3ct-license`) | timed NR arm, linking the static library `c077a88b` built (SHA-256 `b9605974…`, `-O3 -march=native -funroll-loops`) |
| srsRAN_Project [Srsran2026] | tag `release_25_10`, commit `d2f4b70dda8e2c557d5b05a0ac5f92dbddda19bc` | AGPL version 3 or later (`srsran-license-grant`) | source survey; unavailable to build |

All arms are single-core release builds from rustc 1.97.0. The `native`
builds use `-C target-cpu=native`; the `conservative-portable` scalar/compiler
controls use `-C target-cpu=x86-64`. The C++ adapters are `-march=native`
builds. Every receipt records each session's host observation: `fraktaali`,
an AMD Ryzen 9 5900X on Linux 7.2.2-arch1-1 with all 24 CPUs in the mask, SMT
active and the `powersave` governor. Every confirmatory cell is
whole-consumer, single-core latency with a warm cache. The baseline is always
gf2, so a speedup below 1 in a `-gap-` cell means gf2 is faster.

## Operation mapping outcomes

| Operation | External path | Mapping | Timed |
|---|---|---|---|
| DVB-T2 §6.1.3 bit interleaving [Etsi2015] | xdsopl `PCTITL` | operation-equivalent, bit-exact | six cells, all `not-confirmatory` |
| 5G NR LLR de-rate-matching | AFF3CT `Puncturer::depuncture` | bit-exact after an adapter, in 8 of 12 configurations | six confirmatory cells |
| 5G NR rate-matching bit selection | AFF3CT `Puncturer::puncture` | AFF3CT's rule selects gf2's positions in 8/8 configurations | no: gf2 has no public bit-selection entry point |
| 5G NR circulant rotation | AFF3CT fast QC encoder, srsRAN `circ_shift_backward` | external per-frame operation, no gf2 counterpart | no |
| Arbitrary zero-fill bit shift | none found | unmatched | no |

## 1. DVB-T2 bit interleaving

### Mapping and validation

gf2's `DvbT2BitInterleaver` (`gf2-dvb-t2-interleaver`) implements ETSI EN 302
755 v1.4.1 [Etsi2015] §6.1.3. AFF3CT contains no DVB-T2 code: the recorded
negative search over `src` and `include` matches no path. Its interleaver
cores are generic families (inventory `aff3ct-interleaver-cores`). Its `User`
core applies any permutation table loaded from a file
(`aff3ct-user-interleaver-file`), so it could reproduce the output only from a
table computed outside AFF3CT. That generic gather is unmeasured.

xdsopl names the standard (`xdsopl-dvb-t2-standard`), selects its rate-1/2
table (`xdsopl-dvb-t2-table-a1`) and composes `PITL` (`xdsopl-pitl-definition`)
with the `CT8`/`CT12` column twist (`xdsopl-ct8-definition`,
`xdsopl-ct12-definition`) inside `PCTITL` (`xdsopl-pctitl-definition`,
`xdsopl-pctitl-column-twist`). `PITL` writes `out[K+360q+m] = in[K+Qm+q]`
(`xdsopl-pitl-fwd`), gf2's parity permutation (`gf2-dvb-t2-parity-perm`). The
twist reads `in[c·Nr + (R+Nr−tc) mod Nr]` (`xdsopl-ct8-fwd`), gf2's inverse
twist (`gf2-dvb-t2-twist-row`), whose offsets gf2's tests check against the
standard's table (`gf2-dvb-t2-twist-spec-test`,
`gf2-dvb-t2-twist-example-test`). xdsopl's own handler wraps `PCTITL` in a
bit-to-cell demux (`xdsopl-normal-r12-instantiation`,
`xdsopl-short-r12-instantiation`) that gf2 scopes out, so the adapter calls
`PCTITL` directly. The gate's nine `PASS` lines
(`survey/validation-output-v3.txt`) cover three checks. Canonical order and
zero tail padding hold at lengths 0, 1, 63, 64, 65, 127 and 128. Every
position of four MODCODs matches: 16/64-QAM, Normal/Short FECFRAME, Rate 1/2.
The two timed arms agree bit-for-bit.

The gf2 arms time `interleave` on a packed `BitVec` (`gf2-dvb-t2-interleave`,
`survey-gf2-arm-body`) and report one untimed `new` as setup
(`survey-gf2-arm-setup`). The external arm's timed body does four things. It
unpacks to one `int32` per bit (`survey-external-unpack`), allocates the output
(`survey-external-output`), and copies the input, because `PCTITL::fwd`
overwrites it (`xdsopl-pctitl-overwrites-input`, `survey-shim-input-copy`).
It then permutes and packs back (`survey-external-pack`), timing the unpack
and pack spans inside the windows.

### Protocol-v3 confirmation

[Receipt](../../bench_results/eda07788/2026-09-08-eda07788-dvb-t2-v3-confirmation/receipt.json)
`ab94fa3fcb4b9f54d4d236b1e80494767f8cc3e1141b063acc1d4342eeded52d`: three
sessions, 24 fresh pairs per cell, and intervals at confidence 0.999306 with
10 000 resamples.

| Cell | Speedup [interval] | gf2 faster by | Decision | Outcome |
|---|---|---|---|---|
| `qam16-r12-normal-gap-native-vs-xdsopl` | 0.4901 [0.4879, 0.4937] | 2.040 [2.026, 2.050] | regressed | not-confirmatory |
| `qam16-r12-normal-gap-portable-vs-xdsopl` | 0.4890 [0.4297, 0.4954] | 2.045 [2.018, 2.327] | regressed | not-confirmatory |
| `qam16-r12-normal-control-portable-vs-native` | 1.0018 [0.9898, 1.0098] | - | not-worse | not-confirmatory |
| `qam64-r12-normal-gap-native-vs-xdsopl` | 0.4580 [0.4510, 0.4626] | 2.183 [2.162, 2.217] | regressed | not-confirmatory |
| `qam64-r12-normal-gap-portable-vs-xdsopl` | 0.4623 [0.4443, 0.4702] | 2.163 [2.127, 2.251] | regressed | not-confirmatory |
| `qam64-r12-normal-control-portable-vs-native` | 0.9993 [0.9919, 1.0048] | - | not-worse | not-confirmatory |

`regressed` means that the upper bound lies below `1/1.1`: the external arm is
more than the 10% equivalence margin slower.

**P-20 status.** The receipt-local ledger holds four reservations. The
imported v1 confirmation and this confirmation reserve six comparisons each,
and the two v3 pilots reserve none. So m = 12 and the attempt is t = 2. The
attempt alpha is 0.05 / (2 · 3) = 0.008333, the corrected alpha is 0.008333 /
12 = 0.0006944, and 10 000 · 0.0006944 / 2 = 3.47 draws fall in each tail,
against the required 20. The endpoint-stability half of P-20 passes: the
largest endpoint shift is 0.001127 against the declared 0.04
(`survey/dvb-t2-v3-endpoint-stability.txt`). Tail support alone makes all six
cells `not-confirmatory`. The expected tail count is 125/m on attempt 1 and
41.7/m on attempt 2. A six-cell confirmation of this family could therefore
reach 20 only as a first attempt with m ≤ 6. The imported v1 reservation had
already spent that attempt.

**Controls and attribution.** The portable-versus-native intervals lie within
the declared resolution of 1. That fits `interleave` being a scalar per-bit
scatter (`gf2-dvb-t2-forward-table`, `gf2-dvb-t2-scatter-loop`,
`gf2-dvb-t2-scatter-branch`). The medians over 24 executions per arm, from the
tables, are as follows. gf2 takes 196.2 µs (16-QAM) and 196.3 µs (64-QAM). The
external arm takes 400.2 µs and 428.6 µs, of which 27.6 µs is unpack and
199.8 µs and 197.8 µs are pack. The conversion spans exceed the paired call
gap in 24 of 24 pairs on 16-QAM and in 0 of 24 on 64-QAM. Without them the
external call is 0.880 and 1.040 of the paired gf2 call. gf2's lead is the
conversion its packed layout avoids; its permutation kernel is not faster than
xdsopl's `int32` gather on 16-QAM. At face value that ratio is 1/0.880 ≈ 1.14,
below the 1.2 material-gap threshold. These decompositions are descriptive and
carry no interval.

**Disturbed executions and resolution.** Four cells contain whole executions
far above their median, for example 866.8 µs against 401.3 µs. Such slowdowns
persist through an execution, so the flag rule (a window at least twice its
execution's median) catches 1 of the 1440 windows. No sample is removed. The
cause is not identified; the receipt records one-minute load averages of
0.24, 4.15 and 3.69 at the session starts. The declared resolution 0.04 comes
from [pilot r2](../../bench_results/eda07788/2026-09-08-eda07788-dvb-t2-v3-pilot-r2/receipt.json),
whose widest relative half-width is 0.032632 (`pilot-resolution-v3-r2.txt`),
so the value is not below its pilot's observation. The first v3 pilot's
widest was 0.153611 at six pairs on the same Short-frame cell
(`pilot-resolution-v3.txt`). The confirmation contradicts that sizing in one
cell: `qam16-r12-normal-gap-portable-vs-xdsopl` reaches 0.121401 at the
stricter confidence. P-20 bounds endpoint movement between seed streams, not
width, so this does not trip it, and that cell's upper bound, 0.4954, stays far
below `1/1.1`.

**Exploratory coverage and v1 history.** The v3 pilots share the
confirmation's executables. The identity cell reads 0.9996 [0.9905, 1.0030].
The Short FECFRAME gap cells read 0.2242 and 0.2215 at six pairs, and 0.2289
[0.2215, 0.2327] in r2 at 24 pairs. The `streaming` cell reads 0.5638
[0.5618, 0.5653].

The v1 [pilot](../../bench_results/eda07788/2026-09-08-eda07788-dvb-t2-pilot/)
and [confirmation](../../bench_results/eda07788/2026-09-08-eda07788-dvb-t2-confirmation/)
remain byte-for-byte. The v1 confirmation reads 0.4266 to 0.4583 (`fail`) in
the gap cells. The two campaigns contradict each other beyond their
intervals. The v1 native intervals [0.4536, 0.4666] and [0.4289, 0.4316]
exclude the v3 points 0.4901 and 0.4580. gf2's median call moved from
186.4/186.8 µs to 196.2/196.3 µs, and the external arm's from 406.8/434.2 µs to
400.2/428.6 µs. All v1 executables differ from v3 by digest. Git navigation
shows `bit_interleaver.rs`, `bitvec.rs` and both adapters unchanged between
the launch revisions `130c33f4` and `142d0774`, while the linked timing
library and unrelated `gf2-core`/`gf2-kernels-simd` files changed.
Within-campaign intervals therefore do not bound between-build variation. The
cause is not isolated.

## 2. 5G NR rate matching and de-rate-matching

AFF3CT implements 5G NR LDPC. The puncturer, codec, encoder and decoder
factories wire the 5G path (`aff3ct-puncturer-factory`,
`aff3ct-codec-5g-wiring`, `aff3ct-codec-5g-base-graph`, `aff3ct-codec-5g-ncw`,
`aff3ct-encoder-5g-build-branch`, `aff3ct-encoder-5g-build`,
`aff3ct-decoder-5g-build`), and 97 NR matrices are pinned (inventory
`aff3ct-5g-matrix-count`).

**Parameter derivation.** AFF3CT derives the base graph from `K` and
`R = K/N` (`aff3ct-bg-selection`), while gf2 takes it from its caller. AFF3CT
uses its block-size-dependent `Kb` only to choose `Zc`
(`aff3ct-lifting-uses-kb`). Its `K_LDPC` and `N_LDPC` are the base-graph
column counts times `Zc` (`aff3ct-bg2-columns`, `aff3ct-bg1-columns`,
`aff3ct-mother-dimensions`, `aff3ct-mother-length`), which is gf2's rule
(`gf2-full-k`, `gf2-kb-for-z-selection`). The lifting tables differ only for
`560 < K ≤ 640`: AFF3CT uses 8 (`aff3ct-kb-560-640`,
`aff3ct-kb-560-640-value`) and gf2 uses 9 (`gf2-kb-560-640`). gf2 also
requires enough transmitted bits when choosing `Z` (`gf2-z-selection`).

**Bit selection.** `compare-nr-rate-matching` transcribes AFF3CT's derivation
and selection (`aff3ct-puncture-entry`, `aff3ct-puncture-filler-skip`,
`aff3ct-puncture-selection`). Over eight configurations
(`survey/analysis-output-v3.txt`), AFF3CT's rule on gf2's parameters selects
gf2's positions in 8/8. AFF3CT's own `(K, N)` derivation equals gf2's in 4/8:
the three rate-1/2 BG1 configurations become BG2, and at `K = 600` AFF3CT
lifts to `Z = 80` where gf2 lifts to `Z = 72`. AFF3CT exposes selection
publicly: `Puncturer::puncture` runs a module task that calls
`Puncturer_5G`'s protected hook (`aff3ct-puncturer-public-puncture`,
`aff3ct-puncture-task-dispatch`, `aff3ct-puncturer-5g-hook-protected`). gf2
fuses selection as a gather into the private `encode_rate_matched`
(`gf2-encode-rate-matched-private`, `gf2-transmitted-gather`). No selection
cell exists, because timing a harness re-implementation would time the
harness.

**De-rate-matching mapping.** Both sides expose the inverse LLR mapping
publicly. gf2's `prepare_llrs` (`gf2-prepare-llrs-public`) allocates `full_n`
zeros, scatters the channel LLRs through its transmitted-column table and
writes `FILLER_LLR = 15.0` into fillers (`gf2-prepare-llrs-zero-init`,
`gf2-prepare-llrs-scatter`, `gf2-prepare-llrs-filler`, `gf2-filler-llr`).
AFF3CT's public `depuncture` (`aff3ct-puncturer-public-depuncture`,
`aff3ct-depuncture-entry`) writes the selected positions, zeroes the `2·Zc`
prefix and writes +∞ into floating-point fillers
(`aff3ct-depuncture-zero-prefix`, `aff3ct-depuncture-filler-inf`); it leaves
the other positions unwritten. The adapter calls it through a C-ABI shim on
`Puncturer_5G<int32_t, float>`, constructed with the `N_cw` AFF3CT's codec
passes (`survey-nr-shim-construct`, `survey-nr-shim-depuncture`). The timed
adapter call does three things inside the whole-consumer boundary. It converts
the `&[Llr]` frame to `float` and allocates a zeroed output
(`survey-nr-adapter-zeroed-output`), runs `depuncture`
(`survey-nr-adapter-depuncture`), and converts back, writing gf2's filler value
over AFF3CT's filler range (`survey-nr-adapter-filler`). The gf2 arm times one
`prepare_llrs` call (`survey-nr-gf2-arm-body`, `survey-nr-aff3ct-arm-body`).

**Equivalence gate.** `survey/nr-derate-validation.txt` first verifies the
AFF3CT commit and the static library's digest. It then compares AFF3CT's own
derivation with gf2's over twelve configurations, the same four
non-equivalences as above:

- The 8 equivalent configurations pass on two seeded frames each. AFF3CT's raw
  output, pre-filled with NaN, holds bit-identical channel LLRs at exactly
  gf2's transmitted positions, zeros in the prefix and +∞ in the fillers. The
  positions it leaves unwritten are exactly gf2's untransmitted zeros. The
  adapter's output equals gf2's bit-for-bit at every mother-code position.
- The 4 non-equivalent configurations are recorded and not timed.

**Cells.** The six confirmatory cells span both base graphs, codes with and
without fillers, and lifting sizes 22 to 96. They are five gap cells (BG2
`Z = 22, 52` with fillers and `Z = 72` without; BG1 `Z = 96` with fillers and
`Z = 48` without) and the portable-versus-native control at BG1 `Z = 96`. The
eight-cell [pilot](../../bench_results/eda07788/2026-09-10-eda07788-nr-derate-pilot/receipt.json)
(24 pairs each, zero findings) adds an identity cell, 0.9975 [0.9948, 1.0019],
and a BG1 `Z = 44` gap cell, 0.3330 [0.3327, 0.3345].

**Resolution.** At the pilot's own alpha (0.025) the widest relative
half-width is 0.007809, the P-03 floor. The declared resolution uses the
confirmation's corrected alpha, 0.025/6, instead. There the pilot's data give
a widest half-width of 0.025312 and a largest P-20 endpoint shift of 0.017717,
both in the BG1 `Z = 96` gap cell. The larger, rounded up, is the declared
0.03 (`pilot-resolution-nr-derate.txt`). This departs from the DVB-T2
family's rule, which used the pilot's own alpha. That rule is the one the DVB-T2
confirmation contradicted with a threefold wider interval. The confirmation's
own widest relative half-width is 0.014056.

**Confirmation.**
[Receipt](../../bench_results/eda07788/2026-09-10-eda07788-nr-derate-confirmation/receipt.json)
`e1041cde377fa5742b8f613530647222bcfede7c2f47710711f6c832669de03e`: three
sessions, 24 fresh pairs per cell, zero findings. It is the family's first
attempt with m = 6, so the attempt alpha is 0.025. The corrected alpha is
0.0041667, confidence 0.995833, and 20.83 draws fall in each tail.

| Cell | Speedup [interval] | gf2 faster by | Decision | Outcome |
|---|---|---|---|---|
| `bg2-n256-k121-gap-native-vs-aff3ct` | 0.3491 [0.3472, 0.3504] | 2.865 [2.854, 2.880] | regressed | fail |
| `bg2-n1024-k400-gap-native-vs-aff3ct` | 0.3428 [0.3418, 0.3476] | 2.918 [2.877, 2.926] | regressed | fail |
| `bg2-n1440-k720-gap-native-vs-aff3ct` | 0.3197 [0.3186, 0.3208] | 3.128 [3.117, 3.139] | regressed | fail |
| `bg1-n1320-k1056-gap-native-vs-aff3ct` | 0.3263 [0.3251, 0.3274] | 3.065 [3.054, 3.076] | regressed | fail |
| `bg1-n2560-k2048-gap-native-vs-aff3ct` | 0.3292 [0.3282, 0.3312] | 3.038 [3.019, 3.047] | regressed | fail |
| `bg1-n2560-k2048-control-portable-vs-native` | 0.9992 [0.9905, 1.0031] | - | not-worse | not-material |

**Attribution.** From the tables, gf2's median call ranges from 225 ns
(`Z = 22`) to 1356 ns (`Z = 96`), and the adapter's from 645 ns to 4118 ns.
The adapter's conversion stages are small. Timed alone after the windows,
unpack takes 71-376 ns and pack 62-365 ns. With both subtracted, the rest of
the external call is still 2.28 to 2.69 times gf2's paired call (medians over
24 pairs). The gap therefore lies in AFF3CT's `depuncture` and its task
dispatch, not in the adapter. In the source, AFF3CT visits circular-buffer
positions one at a time and recomputes a modulo index for each
(`aff3ct-depuncture-scatter`), whereas gf2 scatters through a precomputed
table (`gf2-prepare-llrs-scatter`). That explanation is an unprofiled
hypothesis. The setup costs run the other way. gf2's untimed code
construction takes 1.7 ms to 46 ms, and AFF3CT's `Puncturer_5G` construction
about 50 µs, which matters to a consumer that builds codes per frame.

**srsRAN.** Its rate dematcher is unavailable. MbedTLS is missing on this
host, so srsRAN's CMake configuration stops (`survey/nr-derate-validation.txt`;
`c077a88b`'s committed configure log). It would also not be bit-exact. It
takes and returns `int8` LLRs (`srsran-dematcher-entry`, `srsran-llr-int8`),
excludes the `2·Zc` prefix from its output (`srsran-dematcher-shortened`) and
de-interleaves modulation bits (`srsran-dematcher-deinterleave`). Its rate
matcher starts at the redundancy-version offset `k0`
(`srsran-rate-matcher-select-bits`, `srsran-rate-matcher-k0`,
`srsran-rate-matcher-start`).

gf2's constructor rustdoc states +∞ for filler LLRs
(`gf2-rate-matched-doc-filler`), while the code writes 15.0. This production
documentation mismatch lies outside this survey's diff and is reported for
tracking.

## 3. 5G NR circulant rotations

AFF3CT's 5G build constructs `Encoder_LDPC_QC_fast` (`aff3ct-encoder-5g-build`,
`aff3ct-qc-fast-subclass`). Its `_encode` applies each nonzero circulant as a
cyclic-shift XOR of the `Zc`-bit input block (`aff3ct-qc-fast-encode`,
`aff3ct-qc-fast-shift-wrap`, `aff3ct-qc-fast-shift-body`). The base class's
`std::rotate` (`aff3ct-qc-rotation`, `aff3ct-qc-rotation-call`) is not on that
path. srsRAN rotates codeblock nodes with its public `circ_shift_backward`
(`srsran-circular-shift-backward`, `srsran-ldpc-encoder-generic-rotation`,
`srsran-ldpc-encoder-rotation`). gf2 has no per-frame circulant data
operation. `CirculantMatrix::to_edges` emits sparse coordinates
(`gf2-circulant-to-edges`) that matrix construction consumes once per code
(`gf2-qc-to-edges-caller`, `gf2-ldpc-from-qc`), and the encoder applies parity
through one `matvec_transpose` (`gf2-parity-matvec`). The mapping is
inapplicable and stays out of the scorecard. Writing a gf2 rotation primitive
would be production work that this survey excludes.

## 4. Arbitrary zero-fill bit shifts

`BitVec::shift_left`/`shift_right` and their AVX2 word kernels shift by any
offset and zero-fill (`gf2-bitvec-shift-left`, `gf2-bitvec-shift-right`,
`gf2-avx2-shift-left-words`, `gf2-avx2-shift-right-words`). The surveyed
sources expose no general zero-fill bit-vector shift, and srsRAN's shifts are
circular (`srsran-circular-shift-forward`, `srsran-circular-shift-backward`).
A wrap-around shift is no substitute (`survey/analysis-output-v3.txt`). 148
shifts over lengths 0, 1, 63, 64, 65, 127, 128, 4096 and 64 800 match an
independent zero-fill reference, and all 88 nonzero offsets differ under
rotation, with offsets through `len+1` (`survey-shift-offsets`). The result
stays explicitly unmatched.

## Falsified, contradicting and negative results

- An earlier revision of this report made four claims that the pinned sources
  contradict:
  - AFF3CT sets `K_LDPC = Kb·Zc`, so the projects rate-match different BG2
    mother codes. The derivations differ only in base-graph choice and the
    `560 < K ≤ 640` entry.
  - Neither project exposes bit selection. AFF3CT exposes `puncture` and
    `depuncture`.
  - AFF3CT's 5G circulant operation is `Encoder_LDPC_QC`'s `std::rotate`. The
    5G path is `Encoder_LDPC_QC_fast`.
  - srsRAN is AGPL-3.0-only. Its file grant covers version 3 or later. The
    analysis binary that printed mismatched `Kb` values and a +20 filler value
    is also corrected.
- Contradicting the first survey: AFF3CT implements 5G NR, and gf2's selection
  agrees with it.
- The v1 DVB-T2 headline, a 2.2-2.3x gf2 lead, is superseded. The v1 and v3
  intervals are disjoint.
- All six DVB-T2 v3 cells are `not-confirmatory`, and one interval is three
  times the declared resolution.
- Four NR configurations are non-equivalent, the srsRAN arm is unavailable,
  and AFF3CT has no DVB-T2 interleaver. No substitute operation was timed in
  their place.

## Evidence lifecycle

Each family keeps an append-only ledger. Every line binds the SHA-256 of its
predecessor, and each receipt snapshots the prefix through its reservation.
`dvb-t2-bit-interleave-baselines-trial-ledger.jsonl` holds four reservations;
the first is the retrospective v1 import, derived in
`trial-ledger-v1-import.json`. `nr-llr-derate-matching-trial-ledger.jsonl`
began empty and holds the NR pilot and confirmation. `producing-inputs.json`
and `producing-inputs-nr-derate.json` name the producing closures that the
receipts snapshot under `inputs/producing/`. All five v3 receipts
re-evaluate as accepted with the evaluator merged from main
(`reevaluation-v3.log`), and their receipt and summary bytes are unchanged.
Each confirmation's execution log is byte-identical to the runner's canonical
log.

## Criterion outcomes

| Criterion | Status | Evidence |
|---|---|---|
| REQ-01 | MET | Accepted v3 pilots and confirmations for both families pin the contract, protocol, schema, addendum, ledger prefix and producing closure. Release builds, full-host lock and checkpointed sessions. Negative, `fail`, `not-material` and `not-confirmatory` outcomes are recorded. With no production change, before/after evidence does not apply. |
| REQ-02 | MET | Three sources are pinned with licenses. Operation, layout, standards mapping and backend evidence are in `source-evidence.json`. Arm build identities are in the receipts; the AFF3CT static library's digest is verified. The AFF3CT 5G modules are scalar C++ (negative search). Arbitrary shifts are unmatched. |
| REQ-03 | MET | Zero-fill versus wrap and offsets through `len+1` (`analysis-output-v3.txt`). Canonical order, tails and the §6.1.3 permutation (`validation-output-v3.txt`). NR transmitted, punctured, filler and untransmitted positions checked exactly (`nr-derate-validation.txt`). Conversion and initialization costs are inside every timed call and reported per arm. |
| REQ-04 | MET | Frozen addenda: five DVB-T2 (v1 pilot and confirmation, v3 pilot, r2, confirmation) and two NR (pilot, confirmation). Internal and external receipts include scalar/compiler controls in both families. Inapplicable mappings are recorded: circulant rotation, bit selection, shifts, four NR configurations and srsRAN. |

## Recommendations

- Keep xdsopl `PCTITL` as the epic's DVB-T2 §6.1.3 comparator and AFF3CT
  `Puncturer_5G::depuncture` as its NR de-rate-matching comparator.
- The whole-consumer NR encoder comparison, tracked by `12fdeb5b`, would pit
  gf2's public `BlockEncoder::encode` (`gf2-nr-block-encoder`,
  `gf2-nr-block-encoder-encode`) against AFF3CT's public `encode` on
  `Encoder_LDPC_QC_fast` plus `puncture` (`aff3ct-encoder-public-encode`).
  Its configurations must derive the same base graph and lifting on both
  sides, and a codeword-agreement gate must pass before timing, because gf2
  places bits through an RREF-derived column mapping
  (`gf2-rref-column-mapping`).
- Profile gf2's per-bit scatter in DVB-T2 `interleave`. The falsifiable
  follow-up is whether a branch-free or word-level form beats the 16-QAM
  external residual.
- Correct gf2's filler-LLR rustdoc to match `FILLER_LLR`.
- Re-survey circulant rotations if gf2 gains a per-frame rotation primitive.

## Citations

- [Cassagne2019] — AFF3CT.
- [Xdsopl2026] — xdsopl/LDPC.
- [Srsran2026] — srsRAN_Project.
- [Etsi2015] — ETSI EN 302 755 v1.4.1 (DVB-T2).
