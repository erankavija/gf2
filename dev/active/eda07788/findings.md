# Shift and coding-permutation baselines

> **Diátaxis Type:** Reference (research findings)

## Result

Two operations have an operation-equivalent external arm, and both were
measured under protocol version 3.

- **DVB-T2 bit interleaving** against xdsopl/LDPC. Every DVB-T2 receipt, v1
  and v3, measured arms whose `warm` cells skipped the protocol's untimed pass
  over the working set before calibration, so their `warm` declarations are
  false (§1, *Warm-state defect and re-measurement*). Every warm-cell
  comparison in them is withdrawn, including the gap and its attribution to
  the adapter's representation conversion; the receipts stay byte-identical
  as history. The exploratory re-measurement, with arms that apply the
  declared cache state, is accepted with zero findings, and every comparator
  cell decides against the external arm at both MODCODs, both FECFRAME
  lengths and both cache states. Every cell is exploratory and the family
  ledger records the campaign as an attempt that reserves no comparison, so
  the re-measurement supports no confirmatory claim (§1, *Exploratory
  re-measurement outcomes*).
- **5G NR LLR de-rate-matching** against AFF3CT. The confirmation
  `nr-derate-confirmation-eda07788-20260910t170957z` is accepted with zero
  findings, and all six cells are confirmatory. gf2 leads the external arm in
  every gap cell by a wide margin, which the NR tables' *Protocol-v3
  confirmation* section carries in the `Speedup [interval]` and `gf2 faster by`
  columns of the five `-gap-` rows. The decision records `fail`, the protocol's
  token for an external arm more than the frozen equivalence margin slower
  ([confirmation addendum](addendum-nr-llr-derate-confirmation.json),
  `effect.equivalence_margin`). The gap lies in AFF3CT's de-puncturing itself,
  not in the adapter.

In the NR family the native and conservative-portable gf2 builds cannot be
told apart (NR tables, *Protocol-v3 confirmation*, row
`bg1-n2560-k2048-control-portable-vs-native`). The DVB-T2 control cells that
read the same are withdrawn, and in the re-measurement one control interval
excludes equality while both stay far inside the material-gap threshold (§1).
NR bit selection has no gf2 entry point. Circulant rotation has no gf2
counterpart, and arbitrary zero-fill shifts have no external counterpart.
No production code changes.

## Question

Which of gf2's arbitrary-offset bit shifts, quasi-cyclic circulant rotations,
DVB-T2 bit interleaving and 5G NR rate matching have a reproducible,
operation-equivalent open-source external baseline? Where both sides expose
one, what do committed receipts say about the gap? Where one does not, what is
missing, and on which side?

## Method and evidence

The [measurement contract](../1a379447-zen3-cpu-performance/measurement-contract.md)
and the [shared protocol](../f547c394/protocol.md) at
[version 3](../f547c394/amendment-v3.md) — the version these receipts pin,
snapshot and are evaluated under — govern all timed work.
Committed commands produce every artifact:

| Command | Output |
|---|---|
| `survey/fetch-build.sh` | xdsopl arms and DVB-T2 adapter gate: `survey/validation-output-v3.txt` for the receipted arms; `survey/validation-output-v3-remeasure.txt`, with the re-measurement arms' digests, through `run-dvb-t2-baselines.sh preflight-remeasure` |
| `survey/freeze-remeasure.py` | the DVB-T2 re-measurement addendum, with its role, ledger and pilot-trial derivation in the family description |
| `survey/nr-derate-build.sh <aff3ct-root>` | AFF3CT pin checks, NR arms and equivalence gate: `survey/nr-derate-validation.txt` |
| `survey/run-analysis.sh` | shift semantics and NR selection comparison: `survey/analysis-output-v3.txt` |
| `survey/analysis` `bootstrap-resolution` | NR resolution derivation `pilot-resolution-nr-derate.txt`; DVB-T2 endpoint stability `survey/dvb-t2-v3-endpoint-stability.txt` |
| `survey/inspect-sources.py` | `survey/source-evidence.json`: every code claim below with project, commit, path, line, verbatim text and interpretation |
| `../../bench_results/eda07788/run-dvb-t2-baselines.sh`, `run-nr-derate-baselines.sh` | bounded checkpointed campaigns under the CCX1 full-host lock; `remeasure-v3` runs the DVB-T2 re-measurement and resumes under its campaign identity |
| `../../bench_results/eda07788/reevaluate-v3.sh` | independent re-evaluation of every v3 receipt of both families: `reevaluation-v3.log` |
| `../../bench_results/eda07788/summarize-v3.py` | [DVB-T2 tables](../../bench_results/eda07788/tables-v3.md) and [NR tables](../../bench_results/eda07788/tables-nr-derate.md) |

Code claims cite `source-evidence.json` claim IDs in backticks. This report
carries the argument and cites the evidence: it states no measured or derived
value. Every figure lives in the two generated tables, in an acceptance summary,
or in a named output file, and each pointer here names the table section and the
row that carries it. A number appears below only when it identifies a cell,
names a workload size, names a protocol or addendum constant that fixes the
design before measurement, or belongs to a citation key, version pin or commit
id. Both table documents regenerate byte for byte from the committed receipts
through `summarize-v3.py`.

| Source | Pin | License | Role here |
|---|---|---|---|
| xdsopl/LDPC [Xdsopl2026] | commit `32357d8ad55a6a302c34e093759f0454e45cca56` | zero-clause BSD grant (`xdsopl-license`) | timed DVB-T2 arm |
| AFF3CT [Cassagne2019] | tag `v4.7.0`, commit `e8a65c5047262d97a15563b9edc961f69b2792cc` | MIT (`aff3ct-license`) | timed NR arm, linking the static library `c077a88b` built, whose digest and flags `survey/nr-derate-validation.txt` verifies |
| srsRAN_Project [Srsran2026] | tag `release_25_10`, commit `d2f4b70dda8e2c557d5b05a0ac5f92dbddda19bc` | AGPL version 3 or later (`srsran-license-grant`) | source survey; unavailable to build |

All arms are single-core release builds from the toolchain the receipts record
(both table documents, *Sessions and host*). The `native` builds use
`-C target-cpu=native`; the `conservative-portable` scalar/compiler controls use
`-C target-cpu=x86-64`. The C++ adapters are `-march=native` builds. Every
receipt records each session's host observation — host name, CPU model, kernel,
governor, SMT state, CPUs in the mask and load averages — which the same
*Sessions and host* blocks project. Every confirmatory cell is whole-consumer,
single-core latency and declares a warm cache, which the DVB-T2 arms of those
receipts did not apply (§1). The baseline is always gf2, so a speedup below 1 in
a `-gap-` cell means gf2 is faster.

## Operation mapping outcomes

| Operation | External path | Mapping | Timed |
|---|---|---|---|
| DVB-T2 §6.1.3 bit interleaving [Etsi2015] | xdsopl `PCTITL` | operation-equivalent, bit-exact | withdrawn for the warm-state defect; re-measured, every cell exploratory (§1) |
| 5G NR LLR de-rate-matching | AFF3CT `Puncturer::depuncture` | bit-exact after an adapter, in the equivalent configurations the gate records | six confirmatory cells |
| 5G NR rate-matching bit selection | AFF3CT `Puncturer::puncture` | AFF3CT's rule selects gf2's positions in every compared configuration | no: gf2 has no public bit-selection entry point |
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
`PCTITL` directly. The gate's `PASS` lines
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

### Warm-state defect and re-measurement

[Version 3](../f547c394/amendment-v3.md) of the
[shared protocol](../f547c394/protocol.md) defines a `warm` cell as one untimed
pass over the working set before calibration, and its P-17 requires the applied
cache state to match the declaration (*Sampling design*). The v3
receipts snapshot the arm sources under `inputs/producing/`. In those
snapshots, both arms' warm branch only passes a reference to the input bank
through `black_box` (`survey-v3-gf2-arm-warm-defect`,
`survey-v3-external-arm-warm-defect`): no interleave, conversion or output
allocation runs before calibration, yet each arm reports `warm` as applied.
Every DVB-T2 launcher log, v1 and v3, lists the same source digests, so the
defect covers every DVB-T2 receipt. The acceptance tool checks the state an
arm reports and cannot see it. Every warm-cell comparison in those receipts
is withdrawn, and the receipts stay byte-identical as history. The
`streaming` pilot cells applied the rotation they declared.

The survey's arms decode the request's cache state and refuse any state other
than `warm` and `streaming` (`survey-arm-cache-refusal`). Both time through
one helper (`survey-gf2-arm-timed-windows`, `survey-external-timed-windows`)
that runs the timed call once on its bank before calibration in warm cells
(`survey-arm-warm-pass`). A zero-window warm request observes that pass
(`survey-arm-warm-pass-test`), and `survey/gf2-side/tests/cache_policy.rs`
sees both executables refuse `cold` before measuring.

The [re-measurement addendum](addendum-dvb-t2-bit-interleave-v3-remeasure.json)
repeats every v3 pilot cell with those arms' executables, whose digests
`survey/validation-output-v3-remeasure.txt` records and the DVB-T2 tables'
*Arm executables* section joins to each receipt. Every cell is
exploratory. The withdrawn confirmation spent the confirmatory attempt for
both comparators, and P-20's tail support rules out a confirmatory outcome for
any further attempt of this family; the addendum's family description derives
both, with each cell's pilot-trial count. The family ledger counts the
campaign as a further reservation that spends no comparisons.
`run-dvb-t2-baselines.sh remeasure-v3` ran it in a benchmark window, and its
[receipt](../../bench_results/eda07788/eda07788-dvb-t2-v3-remeasure-r1/receipt.json)
is committed.

### Exploratory re-measurement outcomes

The campaign `remeasure-v3-eda07788-r1` is accepted with no findings. Every
cell is `measured` at its frozen pair count, no window is flagged anywhere, and
every arm reports the cache state its cell declares as applied (DVB-T2 tables,
*Protocol-v3 re-measurement with the repaired warm pass*, source line and the
`Pairs`, `Flagged windows` columns). That section carries every value,
recomputed from the receipt's raw pairs by `benchmark-acceptance`.

**What the cells decide.** Every comparator-gap cell decides `regressed`:
against both gf2 builds, at 16-QAM and 64-QAM, at Normal and Short FECFRAME
and under both `warm` and `streaming`, the external arm is more than the
equivalence margin slower, and the cell table's *gf2 faster by* column gives
the reciprocal. The identity control and both portable-versus-native controls
decide `not-worse`: neither gf2 build is more than the equivalence margin
slower than the other. One control's interval excludes equality
(`qam16-r12-normal-control-portable-vs-native`), so the builds are separable on
this operation at a distance far below the material-gap threshold, which the
withdrawn receipts' wider control intervals could not show. Every cell's
relative half-width is an order below the withdrawn confirmation's widest
(the two sections' `Relative half-width` columns).

**What the outcome licenses.** Every cell's role is `exploratory` and every
outcome is `pilot`. The family ledger records the campaign as a reservation
that spends no comparison, so the family's m and t stay where the withdrawn
confirmation left them (*Family accounting (P-20)* in both sections) and the
P-20 tail condition still fails. No cell of this family can reach a
confirmatory outcome under protocol version 3, whichever further attempt is
made, and [version 4](../f547c394/amendment-v4.md) restores none: its
per-candidate cap counts attempts per protocol version, while the family
ledger's sequential attempt budget still counts every earlier reservation, so a
further attempt spends a smaller alpha again. The re-measurement therefore
replaces no withdrawn conclusion with a confirmed one: it shows the gap
survives the repair, and confirms nothing.

**What the repair changed.** The re-measurement arms differ from the withdrawn
arms only in the cache-state policy and the timing loop they call; the rest of
the producing closure differs in comment text alone, and the toolchain is the
same. Each gf2 arm's median call nonetheless falls below what the withdrawn
receipts report, and the `streaming` cell, which runs no untimed pass in either
arm version, falls by about the same proportion (the *Per-arm call time and
conversion spans* tables of the re-measurement, of the withdrawn confirmation
and of the v3 pilots). Rebuilding the arms therefore accounts for the shift and
the warm pass does not; the pass's own effect on a one-bank working set that
calibration already touches is not separable from the rebuild. What the repair
secures is the declaration, not the speed: the state a cell applies now matches
the state its receipt records.

**Attribution.** The external arm's conversion still exceeds the paired call
gap in every 16-QAM Normal cell, including the `streaming` one, and in none of
the 64-QAM Normal or Short FECFRAME cells (*Paired conversion attribution*).
In the Short cells the external residual alone stands well above the paired
gf2 call (same table, *Median external (call - unpack - pack) / paired gf2
call* column). These decompositions are descriptive and carry no interval.

### Withdrawn protocol-v3 confirmation

This subsection records what the withdrawn receipts hold. Nothing in it is a
current finding. The DVB-T2 tables' *Protocol-v3 confirmation (withdrawn)*
section carries its receipt digest, session count, pair count, per-comparison
confidence, resample count and every cell's estimate, interval, reciprocal,
decision and outcome.

[The withdrawn confirmation](../../bench_results/eda07788/2026-09-08-eda07788-dvb-t2-v3-confirmation/receipt.json)
puts the external arm behind gf2 in all four comparator-gap cells and leaves
both portable-versus-native controls `not-worse`. Every cell's outcome is
`not-confirmatory`. `regressed` means that the interval's upper bound lies
below the reciprocal of the frozen equivalence margin
([confirmation addendum](addendum-dvb-t2-bit-interleave-v3-confirmation.json),
`effect.equivalence_margin`): the external arm is more than that margin slower.

**P-20 status.** The receipt-local ledger holds the reservations of the
imported v1 confirmation, the two v3 pilots and this confirmation. The two
confirmations reserve comparisons and the pilots reserve none, so this is the
family's second attempt, and the attempt alpha, the corrected alpha and the
expected bootstrap draws per tail follow from them: the draws fall far below
the protocol's required twenty (*Family accounting (P-20)*, whose
`tail condition` row reads fails). The endpoint-stability half of P-20 passes:
the largest endpoint shift stays well below the declared measurement resolution
(`survey/dvb-t2-v3-endpoint-stability.txt`; *Measurement resolution*). Tail
support alone makes every cell `not-confirmatory`. Because the expected tail
count scales as the attempt alpha divided by the reserved comparison count, a
six-cell confirmation of this family could have reached twenty only as a first
attempt reserving no more than its own six comparisons. The imported v1
reservation had already spent that attempt.

**Controls and attribution in the withdrawn receipts.** The
portable-versus-native intervals lie within the declared resolution of 1
(*Measurement resolution* against the two `-control-` rows). That fits
`interleave` being a scalar per-bit scatter
(`gf2-dvb-t2-forward-table`, `gf2-dvb-t2-scatter-loop`,
`gf2-dvb-t2-scatter-branch`). The medians of both arms, their unpack and pack
spans and the residual after subtracting both are in *Per-arm call time and
conversion spans*; the per-pair comparison against the paired call gap is in
*Paired conversion attribution*. In the two `-native-vs-xdsopl` cells the
external conversion exceeds the paired gap in every pair at 16-QAM and in none
at 64-QAM, and the residual leaves the external call below the paired gf2 call
at 16-QAM and above it at 64-QAM. gf2's lead is the conversion its packed
layout avoids; its permutation kernel is not faster than xdsopl's `int32`
gather on 16-QAM, where the residual ratio inverts to a lead below the frozen
material-gap threshold. These decompositions are descriptive and carry no
interval.

**Disturbed executions and resolution.** Several cells contain whole executions
far above their median, which the `Call range µs` column shows against the
`Median call µs` column. Such slowdowns persist through an execution, so the
flag rule (a window at least twice its execution's median) catches a single
window of the whole campaign (`Flagged windows` column). No sample is removed.
The cause is not identified; the receipt records the one-minute load average at
each session start (*Sessions and host*). The declared resolution comes from
[pilot r2](../../bench_results/eda07788/2026-09-08-eda07788-dvb-t2-v3-pilot-r2/receipt.json),
whose widest relative half-width it rounds up (`pilot-resolution-v3-r2.txt`),
so the value is not below its pilot's observation. The first v3 pilot's widest
came from six pairs on the same Short-frame cell and was much larger
(`pilot-resolution-v3.txt`). The confirmation contradicts that sizing in one
cell: `qam16-r12-normal-gap-portable-vs-xdsopl` reaches a relative half-width
several times the declared resolution at the stricter confidence
(*Measurement resolution*). P-20 bounds endpoint movement between seed streams,
not width, so this does not trip it, and that cell's upper bound stays far
below the reciprocal of the equivalence margin.

**Exploratory coverage and v1 history.** The v3 pilots share the
confirmation's executables (*Arm executables*). The identity cell, the two
Short FECFRAME gap cells at six pairs, the Short-frame r2 cell at the pilot
maximum and the `streaming` cell are all in *Protocol-v3 exploratory pilots
(warm cells withdrawn)*.

The v1 [pilot](../../bench_results/eda07788/2026-09-08-eda07788-dvb-t2-pilot/)
and [confirmation](../../bench_results/eda07788/2026-09-08-eda07788-dvb-t2-confirmation/)
remain byte-for-byte, projected in *Protocol-v1 history (immutable,
superseded, warm cells withdrawn)*; every v1 gap cell decides `regressed` with
outcome `fail`. The two campaigns contradict each other beyond their
intervals: each v1 `-native-vs-xdsopl` interval excludes the v3 point estimate
of the same cell, and both arms' median call moved between the campaigns
(compare the two sections' cell tables and *Per-arm call time and conversion
spans*). All v1 executables differ from v3 by digest (*Arm executables*).
Git navigation shows `bit_interleaver.rs`, `bitvec.rs` and both adapters
unchanged between the launch revisions `130c33f4` and `142d0774`, while the
linked timing library and unrelated `gf2-core`/`gf2-kernels-simd` files
changed. Within-campaign intervals therefore do not bound between-build
variation. The cause is not isolated. Both campaigns ran the defective arm
source, so the contradiction stands between two withdrawn measurements.

## 2. 5G NR rate matching and de-rate-matching

AFF3CT implements 5G NR LDPC. The puncturer, codec, encoder and decoder
factories wire the 5G path (`aff3ct-puncturer-factory`,
`aff3ct-codec-5g-wiring`, `aff3ct-codec-5g-base-graph`, `aff3ct-codec-5g-ncw`,
`aff3ct-encoder-5g-build-branch`, `aff3ct-encoder-5g-build`,
`aff3ct-decoder-5g-build`), and its NR matrix inventory is recorded
(`aff3ct-5g-matrix-count`).

**Parameter derivation.** AFF3CT derives the base graph from `K` and
`R = K/N` (`aff3ct-bg-selection`), while gf2 takes it from its caller. AFF3CT
uses its block-size-dependent `Kb` only to choose `Zc`
(`aff3ct-lifting-uses-kb`). Its `K_LDPC` and `N_LDPC` are the base-graph
column counts times `Zc` (`aff3ct-bg2-columns`, `aff3ct-bg1-columns`,
`aff3ct-mother-dimensions`, `aff3ct-mother-length`), which is gf2's rule
(`gf2-full-k`, `gf2-kb-for-z-selection`). The lifting tables differ only for
`560 < K ≤ 640`, where AFF3CT's `Kb` is one below gf2's (`aff3ct-kb-560-640`,
`aff3ct-kb-560-640-value`, `gf2-kb-560-640`). gf2 also requires enough
transmitted bits when choosing `Z` (`gf2-z-selection`).

**Bit selection.** `compare-nr-rate-matching` transcribes AFF3CT's derivation
and selection (`aff3ct-puncture-entry`, `aff3ct-puncture-filler-skip`,
`aff3ct-puncture-selection`). Over the compared configurations
(`survey/analysis-output-v3.txt`), AFF3CT's rule on gf2's parameters selects
gf2's positions in every one, its lifting rule on gf2's base graph agrees in
all but the `560 < K ≤ 640` entry, and its own `(K, N)` derivation agrees in
half: the rate-1/2 BG1 configurations become BG2, and at `K = 600` the two
projects lift differently. That file's three tally lines carry the counts.
AFF3CT exposes selection publicly: `Puncturer::puncture` runs a module task
that calls `Puncturer_5G`'s protected hook
(`aff3ct-puncturer-public-puncture`, `aff3ct-puncture-task-dispatch`,
`aff3ct-puncturer-5g-hook-protected`). gf2 fuses selection as a gather into
the private `encode_rate_matched` (`gf2-encode-rate-matched-private`,
`gf2-transmitted-gather`). No selection cell exists, because timing a harness
re-implementation would time the harness.

**De-rate-matching mapping.** Both sides expose the inverse LLR mapping
publicly. gf2's `prepare_llrs` (`gf2-prepare-llrs-public`) allocates `full_n`
zeros, scatters the channel LLRs through its transmitted-column table and
writes its `FILLER_LLR` constant into fillers (`gf2-prepare-llrs-zero-init`,
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
derivation with gf2's over the configurations it lists, and its summary line
gives the equivalent, non-equivalent and failing counts:

- Every equivalent configuration passes on two seeded frames. AFF3CT's raw
  output, pre-filled with NaN, holds bit-identical channel LLRs at exactly
  gf2's transmitted positions, zeros in the prefix and +∞ in the fillers. The
  positions it leaves unwritten are exactly gf2's untransmitted zeros. The
  adapter's output equals gf2's bit-for-bit at every mother-code position.
- The non-equivalent configurations, the same ones the parameter comparison
  separates, are recorded and not timed.

**Cells.** The six confirmatory cells span both base graphs and codes with and
without fillers: five gap cells (BG2 `Z = 22, 52` with fillers and `Z = 72`
without; BG1 `Z = 96` with fillers and `Z = 48` without) and the
portable-versus-native control at BG1 `Z = 96`
([confirmation addendum](addendum-nr-llr-derate-confirmation.json), each cell's
`workload.size`). The eight-cell
[pilot](../../bench_results/eda07788/2026-09-10-eda07788-nr-derate-pilot/receipt.json)
adds an identity cell and a BG1 `Z = 44` gap cell; the NR tables'
*Protocol-v3 exploratory pilot* section carries its pair count, findings and
every estimate.

**Resolution.** At the pilot's own alpha the widest relative half-width sits at
the P-03 floor. The declared resolution uses the confirmation's corrected alpha
instead. There the pilot's data give a wider half-width and a larger P-20
endpoint shift, both in the BG1 `Z = 96` gap cell, and the declared resolution
is the larger of the two rounded up (`pilot-resolution-nr-derate.txt`; NR
tables, *Measurement resolution*). This departs from the DVB-T2 family's rule,
which used the pilot's own alpha. That rule is the one the DVB-T2 confirmation,
withdrawn in §1, contradicted with a far wider interval. The NR
confirmation's own widest relative half-width stays inside its declared
resolution (*Measurement resolution*).

**Confirmation.** The
[confirmation receipt](../../bench_results/eda07788/2026-09-10-eda07788-nr-derate-confirmation/receipt.json)
is accepted with zero findings. It is the family's first attempt and reserves
only its own comparisons, so the attempt alpha, the corrected alpha and the
expected bootstrap draws per tail clear P-20 (NR tables, *Family accounting
(P-20)*, whose `tail condition` row reads holds). Every gap cell decides
`regressed` with outcome `fail`, and the control decides `not-worse` with
outcome `not-material` (*Protocol-v3 confirmation*, `Decision` and `Outcome`
columns).

**Attribution.** gf2's median call and the adapter's are in *Per-arm call time
and adapter stages*, per cell and per side. The adapter's conversion stages are
small: unpack and pack, timed alone after the windows, stay a minor part of the
adapter's call at every lifting size. With both subtracted, the rest of the
external call is still more than twice gf2's paired call in every gap cell
(*Paired adapter attribution*). The gap therefore lies in AFF3CT's `depuncture`
and its task dispatch, not in the adapter. In the source, AFF3CT visits
circular-buffer positions one at a time and recomputes a modulo index for each
(`aff3ct-depuncture-scatter`), whereas gf2 scatters through a precomputed
table (`gf2-prepare-llrs-scatter`). That explanation is an unprofiled
hypothesis. The setup costs run the other way: gf2's untimed code construction
dominates AFF3CT's `Puncturer_5G` construction by orders of magnitude at every
size (*Per-arm call time and adapter stages*, `Median setup µs` column), which
matters to a consumer that builds codes per frame.

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
(`gf2-rate-matched-doc-filler`), while the code writes its `FILLER_LLR`
constant (`gf2-filler-llr`). This production documentation mismatch lies
outside this survey's diff and is reported for tracking.

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
A wrap-around shift is no substitute (`survey/analysis-output-v3.txt`). Over
lengths 0, 1, 63, 64, 65, 127, 128, 4096 and 64 800, every checked shift
matches an independent zero-fill reference, and every nonzero offset differs
under rotation, with offsets through `len+1` (`survey-shift-offsets`); that
file's two `PASS` lines carry the counts. The result stays explicitly
unmatched.

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
- The v1 DVB-T2 headline is superseded, and the v3 comparisons that
  superseded it are withdrawn in turn: every DVB-T2 receipt carries the
  warm-state defect (§1).
- Every DVB-T2 v3 confirmatory cell is `not-confirmatory`, and one interval is
  several times the declared resolution (DVB-T2 tables, *Protocol-v3
  confirmation (withdrawn)* against *Measurement resolution*).
- The re-measurement's comparator-gap estimates lie outside the withdrawn v3
  intervals in most of the cells they share, and outside the `streaming`
  pilot cell's interval although that cell is not withdrawn. As between v1
  and v3 (§1), within-campaign intervals do not bound between-build
  variation. Every gap cell's decision keeps its direction, and every control
  still reads `not-worse`.
- The NR configurations the equivalence gate records as non-equivalent are not
  timed, the srsRAN arm is unavailable, and AFF3CT has no DVB-T2 interleaver.
  No substitute operation was timed in their place.

## Evidence lifecycle

Each family keeps an append-only ledger. Every line binds the SHA-256 of its
predecessor, and each receipt snapshots the prefix through its reservation.
`dvb-t2-bit-interleave-baselines-trial-ledger.jsonl` holds one reservation per
campaign of that family, listed in the tables' family accounting; the first is
the retrospective v1 import, derived in `trial-ledger-v1-import.json`, and the
last is the re-measurement's exploratory reservation.
`nr-llr-derate-matching-trial-ledger.jsonl` began empty and holds the NR pilot
and confirmation. `producing-inputs.json`, `producing-inputs-nr-derate.json`
and `producing-inputs-dvb-t2-remeasure.json` name the producing closures that
the receipts snapshot under `inputs/producing/`; the last is the DVB-T2
closure plus the re-measurement's freeze script and arm-digest record. Every
v3 receipt of both families, the re-measurement included, re-evaluates as
accepted with the evaluator merged from main (`reevaluation-v3.log`), and its
receipt, input-tree and summary bytes are unchanged. Each confirmation's
execution log is byte-identical to the runner's canonical log. Protocol version
4 replaced the shared addendum schema in place, so the re-measurement addendum
is checked against the version-3 snapshot its receipt pins; `freeze-remeasure.py`
reproduces that addendum byte for byte from the committed evidence, and
`summarize-v3.py` reproduces both table documents byte for byte from the
committed receipts.

## Criterion outcomes

| Criterion | Status | Evidence |
|---|---|---|
| REQ-01 | MET for NR; DVB-T2 withdrawn and re-measured exploratorily | Accepted v3 pilots and confirmations for both families pin the contract, protocol, schema, addendum, ledger prefix and producing closure. Release builds, full-host lock and checkpointed sessions. Negative, `fail`, `not-material` and `not-confirmatory` outcomes are recorded. The DVB-T2 receipts' `warm` declarations are false, so their comparisons fail the contract's comparison validity; the re-measurement applies the state it declares, and every one of its cells is exploratory (§1). With no production change, before/after evidence does not apply. |
| REQ-02 | MET | Three sources are pinned with licenses. Operation, layout, standards mapping and backend evidence are in `source-evidence.json`. Arm build identities are in the receipts and the tables' *Arm executables* sections; the AFF3CT static library's digest is verified. The AFF3CT 5G modules are scalar C++ (negative search). Arbitrary shifts are unmatched. |
| REQ-03 | MET | Zero-fill versus wrap and offsets through `len+1` (`analysis-output-v3.txt`). Canonical order, tails and the §6.1.3 permutation (`validation-output-v3.txt`; re-measurement arms `validation-output-v3-remeasure.txt`). NR transmitted, punctured, filler and untransmitted positions checked exactly (`nr-derate-validation.txt`). Conversion and initialization costs are inside every timed call and reported per arm in the tables' *Per-arm call time* sections. |
| REQ-04 | MET for NR; DVB-T2 withdrawn and re-measured exploratorily | Frozen addenda: five DVB-T2 (v1 pilot and confirmation, v3 pilot, r2, confirmation), the DVB-T2 re-measurement and two NR (pilot, confirmation). Internal and external receipts include scalar/compiler controls in both families; the DVB-T2 receipts are withdrawn for the warm-state defect, and the re-measurement's cells carry its controls under the repaired arms (§1). Inapplicable mappings are recorded: circulant rotation, bit selection, shifts, the non-equivalent NR configurations and srsRAN. |

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
- The re-measurement reproduces the 16-QAM Normal external residual below the
  paired gf2 call that the withdrawn receipts show, so profile gf2's per-bit
  scatter in `interleave`. The falsifiable follow-up is whether a branch-free
  or word-level form beats that residual.
- Correct gf2's filler-LLR rustdoc to match `FILLER_LLR`.
- Re-survey circulant rotations if gf2 gains a per-frame rotation primitive.

## Citations

- [Cassagne2019] — AFF3CT.
- [Xdsopl2026] — xdsopl/LDPC.
- [Srsran2026] — srsRAN_Project.
- [Etsi2015] — ETSI EN 302 755 v1.4.1 (DVB-T2).
