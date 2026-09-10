# Shift and coding-permutation baselines

> **Diátaxis Type:** Reference (research findings)

## Result

The protocol-v3 confirmation `confirmation-v3-eda07788-20260908t194030z` is
accepted and qualifies for nothing: all six confirmatory cells are
`not-confirmatory` under P-20, with 3.47 expected bootstrap draws per tail
against the required 20. They carry the only timed comparison this
survey supports, DVB-T2 bit interleaving. In all four comparator-gap cells
gf2's whole-consumer call is faster than the xdsopl/LDPC adapter. The speedups
of medians are 0.4580 to 0.4901 over 24 pairs, so at the point estimates gf2
is 2.040 to 2.183 times faster. The native and conservative-portable gf2
builds cannot be told apart. The gap is the external arm's representation
conversion: without it, the external call is 0.880 (16-QAM) and 1.040
(64-QAM) of gf2's. The other surveyed operations reach no timed cell. 5G NR
LLR de-rate-matching is exposed publicly by both gf2 and AFF3CT and remains
unmeasured. No production code changes.

## Question

Which of gf2's arbitrary-offset bit shifts, quasi-cyclic circulant rotations,
DVB-T2 bit interleaving and 5G NR rate matching have a reproducible,
operation-equivalent open-source external baseline? Where both sides expose
one, what do committed receipts say about the gap? Where one does not, what
exactly is missing, and on which side?

## Method and evidence

The [measurement contract](../1a379447-zen3-cpu-performance/measurement-contract.md)
and [protocol version 3](../f547c394/protocol.md) govern all timed work. Six
committed commands produce every artifact:

| Command | Output |
|---|---|
| `survey/fetch-build.sh` | pinned xdsopl checkout, three arm executables, adapter gate: `survey/validation-output-v3.txt` |
| `survey/run-analysis.sh` | shift semantics and NR selection comparison: `survey/analysis-output-v3.txt` |
| `survey/inspect-sources.py` | `survey/source-evidence.json`: every code claim below with project, commit, path, line, verbatim text and interpretation |
| `../../bench_results/eda07788/run-dvb-t2-baselines.sh pilot-v3\|pilot-v3-r2\|confirmation-v3` | bounded checkpointed campaigns under the CCX1 full-host lock |
| `../../bench_results/eda07788/reevaluate-v3.sh` | independent re-evaluation of the three v3 receipts: `reevaluation-v3.log` |
| `../../bench_results/eda07788/summarize-v3.py` | `tables-v3.md`, the numerical projection of the receipts |

Code claims cite `source-evidence.json` claim IDs in backticks. Every number
below appears in [the evidence tables](../../bench_results/eda07788/tables-v3.md),
an acceptance summary or a named output file.

| Source | Pin | License | Role here |
|---|---|---|---|
| xdsopl/LDPC [Xdsopl2026] | commit `32357d8ad55a6a302c34e093759f0454e45cca56` | zero-clause BSD grant (`xdsopl-license`), copied into `survey/xdsopl-pin.txt` | timed external arm |
| AFF3CT [Cassagne2019] | tag `v4.7.0`, commit `e8a65c5047262d97a15563b9edc961f69b2792cc`, `conf` submodule `ecae10cd0375f12febe2f5513f8ca39f2540c640` | MIT (`aff3ct-license`) | source survey; not built here |
| srsRAN_Project [Srsran2026] | tag `release_25_10`, commit `d2f4b70dda8e2c557d5b05a0ac5f92dbddda19bc` | AGPL version 3 or later (`srsran-license-grant`) | source survey |

The timed arms come from one `fetch-build.sh` run. Each flavour builds the
Rust side and the C++ shim at one architecture level. Toolchains are rustc
1.97.0 and g++ 16.2.1, recorded in the receipt and its `launcher.log`:

| Arm | Build identity | Flags |
|---|---|---|
| gf2 `DvbT2BitInterleaver::interleave` | `native` | `-C target-cpu=native` |
| gf2 `DvbT2BitInterleaver::interleave` | `conservative-portable` (scalar/compiler control) | `-C target-cpu=x86-64` |
| xdsopl `PCTITL` adapter | `external` | `-C target-cpu=native`, `g++ -O3 -march=native -std=c++17` |

Every receipt carries the host observation of each session. The host is
`fraktaali`: an AMD Ryzen 9 5900X on Linux 7.2.2-arch1-1 with all 24 CPUs in
the mask, SMT active and the `powersave` governor.

## Operation mapping outcomes

| Operation | External path | Mapping | Timed |
|---|---|---|---|
| DVB-T2 §6.1.3 bit interleaving [Etsi2015] | xdsopl `PCTITL` | operation-equivalent, validated bit-exact | six confirmatory cells, all `not-confirmatory` |
| 5G NR rate-matching bit selection | AFF3CT `Puncturer::puncture` | AFF3CT's rule selects gf2's positions in 8/8 configurations | no: gf2 has no public bit-selection entry point |
| 5G NR LLR de-rate-matching | AFF3CT `Puncturer::depuncture` | public on both sides; filler value and unwritten positions need an adapter | no: feasible, unmeasured |
| 5G NR circulant rotation | AFF3CT fast QC encoder, srsRAN `circ_shift_backward` | external per-frame operation, no gf2 counterpart | no |
| Arbitrary zero-fill bit shift | none found | unmatched | no |

## 1. DVB-T2 bit interleaving

### Mapping and validation

gf2's `DvbT2BitInterleaver` (`gf2-dvb-t2-interleaver`) implements ETSI EN 302
755 v1.4.1 [Etsi2015] §6.1.3: parity interleaving followed by column twisting.
AFF3CT contains no DVB-T2 code; the recorded negative search over `src` and
`include` matches no path. Its interleaver cores (inventory
`aff3ct-interleaver-cores`) are generic families. Its `User` core applies any
permutation table loaded from a file (`aff3ct-user-interleaver-file`). That
core could reproduce the DVB-T2 output only from a table computed outside
AFF3CT. It is an unmeasured generic gather, not a standards implementation.

xdsopl names the standard (`xdsopl-dvb-t2-standard`), selects its rate-1/2
table (`xdsopl-dvb-t2-table-a1`) and composes `PITL` (`xdsopl-pitl-definition`)
with the `CT8`/`CT12` column twist (`xdsopl-ct8-definition`,
`xdsopl-ct12-definition`) inside `PCTITL` (`xdsopl-pctitl-definition`,
`xdsopl-pctitl-column-twist`). `PITL` writes `out[K+360q+m] = in[K+Qm+q]`
(`xdsopl-pitl-fwd`), which is gf2's parity permutation
(`gf2-dvb-t2-parity-perm`). The twist functor reads
`in[c·Nr + (R+Nr−tc) mod Nr]` (`xdsopl-ct8-fwd`), which is gf2's inverse twist
(`gf2-dvb-t2-twist-row`). gf2's own tests check its twist offsets against the
standard's table (`gf2-dvb-t2-twist-spec-test`,
`gf2-dvb-t2-twist-example-test`). xdsopl's own handler wraps `PCTITL` in a
`BITL` bit-to-cell demux (`xdsopl-normal-r12-instantiation`,
`xdsopl-short-r12-instantiation`) that gf2's module scopes out, so the adapter
calls `PCTITL` directly.

`survey/validation-output-v3.txt` comes from the gate `fetch-build.sh` runs
against the timed build. Its nine `PASS` lines cover three checks. Canonical
little-endian order and zero tail padding hold at lengths 0, 1, 63, 64, 65,
127 and 128. All positions of four MODCODs match: 16-QAM and 64-QAM, Normal and
Short FECFRAME, Rate 1/2. The two timed arms agree bit-for-bit on seeded
frames. The executable digests it prints equal the receipts' arm digests.

### Cells and conversion boundary

Every cell is whole-consumer, single-core latency, warm cache. The gf2 arms
call `interleave` on a packed `BitVec`, output allocation included
(`gf2-dvb-t2-interleave`, `survey-gf2-arm-body`). They report zero pack and
unpack costs and report one untimed `new` as `setup` (`survey-gf2-arm-setup`).
The external arm unpacks
to one `int32` per bit (`survey-external-unpack`) and allocates an `int32`
output (`survey-external-output`). Its shim copies the input, because
`PCTITL::fwd` overwrites its input buffer (`xdsopl-pctitl-overwrites-input`,
`survey-shim-input-copy`). It then applies `PCTITL::fwd` and packs the result
back (`survey-external-pack`). The unpack and pack spans are timed inside the
measured windows. The baseline is always gf2. The candidate is xdsopl in
`-gap-` cells and the native build in `-control-` cells.

The 6-, 12- and 24-CPU core arms are not meaningful here, and none was
fabricated. Both sides permute one frame per call with no intra-frame
parallelism, and neither exposes a multi-frame batch entry point. A multicore
cell would time a thread pool that exists only in the harness. The exploratory
`streaming` cell covers working sets beyond cache instead.

### Protocol-v3 confirmation

[Receipt](../../bench_results/eda07788/2026-09-08-eda07788-dvb-t2-v3-confirmation/receipt.json)
SHA-256 `ab94fa3fcb4b9f54d4d236b1e80494767f8cc3e1141b063acc1d4342eeded52d`:
three sessions, 24 fresh pairs per cell, and bootstrap intervals at confidence
0.999306 with 10 000 resamples.

| Cell | Speedup [interval] | gf2 faster by | Decision | Outcome |
|---|---|---|---|---|
| `qam16-r12-normal-gap-native-vs-xdsopl` | 0.4901 [0.4879, 0.4937] | 2.040 [2.026, 2.050] | regressed | not-confirmatory |
| `qam16-r12-normal-gap-portable-vs-xdsopl` | 0.4890 [0.4297, 0.4954] | 2.045 [2.018, 2.327] | regressed | not-confirmatory |
| `qam16-r12-normal-control-portable-vs-native` | 1.0018 [0.9898, 1.0098] | - | not-worse | not-confirmatory |
| `qam64-r12-normal-gap-native-vs-xdsopl` | 0.4580 [0.4510, 0.4626] | 2.183 [2.162, 2.217] | regressed | not-confirmatory |
| `qam64-r12-normal-gap-portable-vs-xdsopl` | 0.4623 [0.4443, 0.4702] | 2.163 [2.127, 2.251] | regressed | not-confirmatory |
| `qam64-r12-normal-control-portable-vs-native` | 0.9993 [0.9919, 1.0048] | - | not-worse | not-confirmatory |

In a comparator-gap cell `regressed` means that the interval's upper bound
lies below `1/1.1`: the external arm is slower than gf2 by more than the 10%
equivalence margin. It is the opposite of a comparator lead.

**P-20 status.** The receipt-local ledger prefix holds four reservations. The
imported v1 confirmation and this confirmation reserve six comparisons each;
the two v3 pilots reserve none. So m = 12 and the attempt number is t = 2.
The attempt alpha is 0.05 / (2 · 3) = 0.008333, and the corrected alpha
0.008333 / 12 = 0.0006944 gives 10 000 · 0.0006944 / 2 = 3.47 expected
bootstrap draws per tail. P-20 requires 20, so every cell is
`not-confirmatory` regardless of its endpoint stability. The tables file
recomputes this accounting from the ledger snapshot and checks it against the
acceptance summary. Under the frozen resamples the expected tail count is
125/m on attempt 1 and 41.7/m on attempt 2. A six-cell confirmation of this
family can reach 20 only as a first attempt with m ≤ 6. The imported v1
reservation had already spent that first attempt, and another attempt would
lower the count further.

**Controls.** The portable-versus-native intervals, [0.9898, 1.0098] and
[0.9919, 1.0048], lie inside the ±10% equivalence margin and within the
declared resolution (0.04) of 1. The native architecture level buys nothing
measurable. That is consistent with gf2's `interleave` being a scalar per-bit
scatter: one table entry and one bit test per input bit
(`gf2-dvb-t2-forward-table`, `gf2-dvb-t2-scatter-loop`,
`gf2-dvb-t2-scatter-branch`).

**Attribution.** The tables give per-call medians over the 24 executions per
arm of the native gap cells. gf2 takes 196.2 µs (16-QAM) and 196.3 µs
(64-QAM). The external arm takes 400.2 µs and 428.6 µs, of which 27.6 µs is
unpack and 199.8 µs and 197.8 µs are pack. The external conversion spans
exceed the whole paired call gap in 24 of 24 pairs on 16-QAM and in 0 of 24 on
64-QAM. After subtracting them, the external call is 0.880 (16-QAM) and 1.040
(64-QAM) of the paired gf2 call (medians). That residual still includes the
adapter's two `int32` buffer allocations and copies. gf2's whole-consumer lead
comes from the conversion its packed layout avoids, and its permutation kernel
is not faster than xdsopl's `int32` gather on 16-QAM. These decompositions are
descriptive and carry no interval. Even at face value, the implied 16-QAM
factor, 1/0.880 ≈ 1.14, is below the family's 1.2 material-gap threshold.
Setup (one `DvbT2BitInterleaver::new`, about 0.80-0.83 ms) is reported apart
and never amortized.

**Disturbed executions and resolution.** Four cells contain whole executions
far above their median, for example an external execution of 866.8 µs against
a median of 401.3 µs. Flagged windows, those at or above twice their own
execution's median, flag 1 of the 1440 windows, because these slowdowns
persist through an execution. No sample is removed. The receipt records
one-minute load averages of 0.24, 4.15 and 3.69 at the three session starts
under the exclusive lock. The cause is not identified. The declared resolution
is 0.04, taken from
[pilot r2](../../bench_results/eda07788/2026-09-08-eda07788-dvb-t2-v3-pilot-r2/receipt.json),
whose only cell has a widest relative half-width of 0.032632
(`pilot-resolution-v3-r2.txt`). The declared value is not below the pilot's
observation. The first v3 pilot's widest value was 0.153611 at six pairs on
the same Short-frame cell (`pilot-resolution-v3.txt`), which r2 re-measured at
24 pairs. The confirmation contradicts that sizing in one cell: the
`qam16-r12-normal-gap-portable-vs-xdsopl` interval has a relative half-width
of 0.121401, three times the declared resolution, at a stricter confidence.
The other five cells are at most 0.0390. P-20's resolution test bounds how far
the interval endpoints move between two bootstrap seed streams, not the
interval's width ([protocol, bootstrap numerical resolution](../f547c394/protocol.md)),
so this width does not trip it. The cell's upper bound, 0.4954, remains far
below `1/1.1`.

**Exploratory coverage.** The v3 pilots use the same executables as the
confirmation and support no decision. The identity cell reads 0.9996
[0.9905, 1.0030]. The Short FECFRAME gap cells read 0.2242 and 0.2215 at six
pairs, and 0.2289 [0.2215, 0.2327] in r2 at 24 pairs. The external arm's fixed
conversion cost dominates harder at a quarter of the frame. The `streaming`
cell reads 0.5638 [0.5618, 0.5653], a smaller gf2 lead once the working set
rotates through eight banks.

### Protocol-v1 history

The v1 [pilot](../../bench_results/eda07788/2026-09-08-eda07788-dvb-t2-pilot/)
and [confirmation](../../bench_results/eda07788/2026-09-08-eda07788-dvb-t2-confirmation/)
are preserved byte-for-byte and were accepted by their receipt-local v1
evaluator. Protocol v3 supersedes them for the current decision. The v1
confirmation reads 0.4266 to 0.4583 in the gap cells (`fail`) and 0.9949 and
0.9932 in the controls (`not-material`).

The two campaigns contradict each other beyond their intervals. The v1 native
gap intervals, [0.4536, 0.4666] and [0.4289, 0.4316], exclude the v3 points
0.4901 and 0.4580, and the v3 intervals exclude the v1 points. gf2's per-call
median moved from 186.4 µs and 186.8 µs (v1) to 196.2 µs and 196.3 µs (v3);
the external arm moved from 406.8 µs and 434.2 µs to 400.2 µs and 428.6 µs.
All three v1 executables differ from the v3 executables by digest. Git
navigation shows `bit_interleaver.rs`, `bitvec.rs` and both adapter sources
unchanged between the launch revisions `130c33f4` and `142d0774`. The timing
library linked into the arms and unrelated `gf2-core` and `gf2-kernels-simd`
files did change. Within-campaign intervals therefore do not bound
between-build variation. The cause is not isolated. Code generation or layout
of the rebuilt executables is an untested hypothesis. The v3 pilot, which
shares the v3 executables, agrees with the v3 confirmation.

## 2. 5G NR rate matching and de-rate-matching

AFF3CT implements 5G NR LDPC. Its base graph comes from `K` and `R = K/N`
(`aff3ct-bg-selection`), and 97 NR matrices are pinned (inventory
`aff3ct-5g-matrix-count`). The puncturer, codec, encoder and decoder factories
wire the 5G path together (`aff3ct-puncturer-factory`, `aff3ct-codec-5g-wiring`,
`aff3ct-codec-5g-base-graph`, `aff3ct-codec-5g-ncw`,
`aff3ct-encoder-5g-build-branch`, `aff3ct-encoder-5g-build`,
`aff3ct-decoder-5g-build`).

**Parameter derivation.** AFF3CT uses its block-size-dependent `Kb` only to
choose `Zc` (`aff3ct-lifting-uses-kb`). `K_LDPC` and `N_LDPC` are the
base-graph column counts times `Zc` (`aff3ct-bg2-columns`, `aff3ct-bg1-columns`,
`aff3ct-mother-dimensions`, `aff3ct-mother-length`). That is gf2's `full_k`
rule (`gf2-full-k`, `gf2-kb-for-z-selection`). The lifting tables differ only
for `560 < K ≤ 640`, where AFF3CT uses 8 (`aff3ct-kb-560-640`,
`aff3ct-kb-560-640-value`) and gf2 uses 9 (`gf2-kb-560-640`). gf2 takes the
base graph from its caller. When choosing `Z` it also requires enough
transmitted bits (`gf2-z-selection`).

**Selection.** `compare-nr-rate-matching` transcribes AFF3CT's derivation and
selection rule (`aff3ct-puncture-entry`, `aff3ct-puncture-filler-skip`,
`aff3ct-puncture-selection`). It recovers gf2's positions through the public
`prepare_llrs`. Over eight configurations (`survey/analysis-output-v3.txt`):

- AFF3CT's rule on gf2's parameters selects gf2's positions in 8/8.
- AFF3CT's own `(K, N)` derivation equals gf2's in 4/8. The three rate-1/2
  BG1 configurations select BG2 in AFF3CT. At `K = 600` AFF3CT lifts with
  `Kb = 8` to `Z = 80`, where gf2 lifts to `Z = 72`.
- With the base graph fixed to gf2's, 7/8 derivations agree.

**Entry points.** AFF3CT exposes both directions publicly.
`Puncturer::puncture` and `Puncturer::depuncture` run module tasks that call
`Puncturer_5G`'s protected hooks (`aff3ct-puncturer-public-puncture`,
`aff3ct-puncturer-public-depuncture`, `aff3ct-puncture-task-dispatch`,
`aff3ct-puncturer-5g-hook-protected`). The puncture task takes one integer
per bit and the depuncture task one value per LLR. These modules are scalar
C++; the negative search finds no MIPP or intrinsic code.
`dev/bench_results/c077a88b/v3-preparation/build-identity.json` records a
build of the same AFF3CT commit.

- Bit selection has no gf2 counterpart. gf2 fuses it as a gather into the
  private `encode_rate_matched` (`gf2-encode-rate-matched-private`,
  `gf2-transmitted-gather`). Re-implementing it in the harness would time the
  harness, so no cell exists.
- LLR de-rate-matching is public on both sides. gf2's `prepare_llrs`
  (`gf2-prepare-llrs-public`) allocates `full_n` zeros, scatters the channel
  LLRs and writes `FILLER_LLR = 15.0` into filler positions
  (`gf2-prepare-llrs-zero-init`, `gf2-prepare-llrs-filler`, `gf2-filler-llr`).
  AFF3CT's `_depuncture` (`aff3ct-depuncture-entry`) writes the selected
  positions and zeroes the `2·Zc` prefix (`aff3ct-depuncture-zero-prefix`). It
  writes +∞ into floating-point filler positions
  (`aff3ct-depuncture-filler-inf`) and leaves the other unselected positions
  unwritten. After an adapter zero-fills the AFF3CT output and maps the filler
  value, the operations are equivalent. The adapter's costs belong in the
  cell. This cell is feasible and unmeasured. It needs an AFF3CT build and its
  own addendum, pilot and confirmation.

srsRAN's rate matcher selects bits in a separate `select_bits` that starts at
the redundancy-version offset `k0` (`srsran-rate-matcher-select-bits`,
`srsran-rate-matcher-k0`, `srsran-rate-matcher-start`). It stays pinned as a
second candidate.

gf2's constructor rustdoc states +∞ for filler LLRs
(`gf2-rate-matched-doc-filler`), while the code writes 15.0. This production
documentation mismatch lies outside this survey's diff and is reported for
tracking.

## 3. 5G NR circulant rotations

AFF3CT's 5G build constructs `Encoder_LDPC_QC_fast` (`aff3ct-encoder-5g-build`,
`aff3ct-qc-fast-subclass`). Its `_encode` applies each nonzero circulant as a
cyclic-shift XOR of the `Zc`-bit input block into the parity block
(`aff3ct-qc-fast-encode`, `aff3ct-qc-fast-shift-wrap`,
`aff3ct-qc-fast-shift-body`). The base class's `std::rotate` of generator
blocks (`aff3ct-qc-rotation`, `aff3ct-qc-rotation-call`) is not on that path.
srsRAN rotates codeblock nodes with its public `circ_shift_backward`
(`srsran-circular-shift-backward`, `srsran-ldpc-encoder-generic-rotation`,
`srsran-ldpc-encoder-rotation`).

gf2 has no per-frame circulant data operation. `CirculantMatrix::to_edges`
emits sparse coordinates for one block (`gf2-circulant-to-edges`); matrix
construction consumes them once per code (`gf2-qc-to-edges-caller`,
`gf2-ldpc-from-qc`). The encoder applies parity through one dense
`matvec_transpose` (`gf2-parity-matvec`). Timing a one-time coordinate
generator against a per-frame kernel would not compare one operation. Writing
a gf2 rotation primitive is production work this survey excludes. The mapping
is recorded as inapplicable and kept out of the scorecard.

## 4. Arbitrary zero-fill bit shifts

`BitVec::shift_left` and `shift_right` and their AVX2 word kernels shift by
any offset and zero-fill the vacated bits (`gf2-bitvec-shift-left`,
`gf2-bitvec-shift-right`, `gf2-avx2-shift-left-words`,
`gf2-avx2-shift-right-words`). The surveyed sources expose no general
zero-fill bit-vector shift. Their permutation operations are fixed-purpose
interleavers and puncturers, and srsRAN's shifts are circular
(`srsran-circular-shift-forward`, `srsran-circular-shift-backward`).
`survey/analysis-output-v3.txt` shows that a wrap-around shift is not a
substitute. 148 shifts over lengths 0, 1, 63, 64, 65, 127, 128, 4096 and
64 800 match an independent zero-fill reference. All 88 nonzero offsets give
a different result under rotation. The offsets reach `len+1`
(`survey-shift-offsets`). This result stays explicitly unmatched.

## Falsified, contradicting and negative results

- The previous revision stated that AFF3CT sets `K_LDPC = Kb·Zc`, so the two
  projects rate-match different BG2 mother codewords. The pinned source
  contradicts this (§2): the derivations differ only in base-graph choice and
  the `560 < K ≤ 640` table entry. The analysis binary that printed the
  mismatched `Kb` values is corrected.
- It stated that neither project exposes bit selection as an entry point. AFF3CT
  exposes `puncture` and `depuncture` publicly.
- It placed AFF3CT's 5G circulant operation in `Encoder_LDPC_QC`'s
  `std::rotate`. The 5G factory builds `Encoder_LDPC_QC_fast`. It also cited
  the encoder factory's header listing as a construction leg.
- It labelled srsRAN AGPL-3.0-only, but the file grant covers version 3 or
  later. The analysis comment named a +20 filler LLR, but the code writes 15.0.
- Kept from earlier revisions: AFF3CT does implement 5G NR, and gf2's
  selection agrees with it. Both contradict the first survey's claims.
- The v1 headline of a 2.2-2.3x gf2 lead is superseded, and the v1 and v3
  intervals are disjoint (§1).
- All six v3 cells are `not-confirmatory`. One confirmation interval is three
  times the declared resolution.
- AFF3CT exposes no DVB-T2 interleaver, so no generic core was measured in its
  place. The NR de-rate-matching cell is unmeasured.

## Evidence lifecycle

The family ledger `dvb-t2-bit-interleave-baselines-trial-ledger.jsonl` chains
four reservations, each binding the SHA-256 of its predecessor line. Its first
line is the retrospective import of the v1 confirmation, derived in
`trial-ledger-v1-import.json`. `producing-inputs.json` names the producing
closure that each receipt snapshots under `inputs/producing/`. The three v3
receipts re-evaluate as accepted with the evaluator merged from main
(`../../bench_results/eda07788/reevaluation-v3.log`). Every receipt and
non-summary file is unchanged across that evaluation, and the regenerated
summaries are byte-identical to the committed ones.

## Criterion outcomes

| Criterion | Status | Evidence |
|---|---|---|
| REQ-01 | MET | Accepted v3 pilot, pilot r2 and confirmation pin the contract, protocol, schema, addendum, ledger prefix and producing closure. Release builds, full-host lock and checkpointed sessions; negative and not-confirmatory outcomes recorded. No production change, so no before/after evidence applies. |
| REQ-02 | MET | Three sources pinned with licenses. Operation, layout, standards mapping and backend evidence are in `source-evidence.json`; the AFF3CT 5G modules are scalar C++, and `c077a88b` records a build of the same commit. The timed arm's build identity is in the receipts. Arbitrary shifts are recorded as unmatched. |
| REQ-03 | MET | Zero-fill versus wrap and offsets through `len+1` (`analysis-output-v3.txt`). Canonical order, tails and the §6.1.3 permutation (`validation-output-v3.txt`). Conversion costs are inside every timed cell and reported per arm. |
| REQ-04 | PARTIAL | Five frozen DVB-T2 addenda (v1 pilot and confirmation, v3 pilot, r2 and confirmation), internal and external receipts, and scalar/compiler controls. Circulant rotation, bit selection and shifts are recorded as inapplicable or unmatched. NR LLR de-rate-matching is applicable and unmeasured. |

## Recommendations

- Keep xdsopl `PCTITL` as the epic's comparator for DVB-T2 §6.1.3 bit
  interleaving.
- Measure NR LLR de-rate-matching as its own family: gf2 `prepare_llrs` against
  AFF3CT `depuncture` with the adapter in §2, and srsRAN as a second arm.
- A whole-consumer NR encoder cell stays supported as a separate family. It
  would pit gf2's public `BlockEncoder::encode` (`gf2-nr-block-encoder`,
  `gf2-nr-block-encoder-encode`) against AFF3CT's public `encode` on
  `Encoder_LDPC_QC_fast` plus `puncture` (`aff3ct-encoder-public-encode`). Its
  configurations must be ones where both derive the same base graph and
  lifting. It needs a codeword-agreement gate before timing, because gf2
  places bits through an RREF-derived column mapping
  (`gf2-rref-column-mapping`).
- Profile gf2's per-bit scatter in `interleave`. The falsifiable follow-up is
  whether a branch-free or word-level form beats the 16-QAM external residual.
- Correct gf2's filler-LLR rustdoc to match `FILLER_LLR`.
- Re-survey circulant rotations if gf2 gains a per-frame rotation primitive.

## Open questions

- Whether gf2 should expose rate-matching bit selection or a circulant
  rotation as an API is a production design question outside this survey.
- Whether the de-rate-matching family is required for REQ-04 or scoped out is
  a tracking decision for the epic.

## Citations

- [Cassagne2019] — AFF3CT.
- [Xdsopl2026] — xdsopl/LDPC.
- [Srsran2026] — srsRAN_Project.
- [Etsi2015] — ETSI EN 302 755 v1.4.1 (DVB-T2).
