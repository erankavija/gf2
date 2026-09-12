# Population-count and fused-reduction baselines

> **Diátaxis Type:** Explanation

Survey for `26465e6c` on the Ryzen 9 5900X. It changes no production kernel or
library. The
[measurement contract](../1a379447-zen3-cpu-performance/measurement-contract.md)
and the [shared protocol](../f547c394/protocol.md) at
[version 3](../f547c394/amendment-v3.md) — the version every receipt pins,
snapshots and is evaluated under — govern every receipt.

This report states no measured value. Each conclusion points to its source: a
section of the generated [receipt tables](../../bench_results/26465e6c/tables.md),
written "tables § heading", with a row named by its workload key, cell ID or
contrast; a receipt or summary field; or a named file. Speedup intervals are
the acceptance tool's at the corrected alpha; every other interval in the
tables is the generator's 95% percentile bootstrap, descriptive and not
family-corrected, as tables § Method states. Running
`dev/active/26465e6c/survey/summarize-receipts.py` from the repository root
reproduces the tables byte for byte. Code claims cite
[`survey/source-evidence.json`](survey/source-evidence.json) by claim ID in
backticks; each entry holds the commit, path, line and verbatim text.

## Question and answer

Is gf2's production population-count dispatcher
(`gf2_core::kernels::ops::popcount`) the fastest measured arm for each workload
class, against libpopcnt v4.2 [Libpopcnt2026], Muła's AVX2 Harley-Seal
reference [Mula2018] and three labelled internal controls? Does gf2's fused
AVX2 AND-popcount beat the routes a consumer has without it?

1. **Population count: the dispatcher is the fastest measured arm for no
   workload.** In every row of tables § Speedups by workload:
   `v3-popcount-confirmation` some alternative's interval lies wholly above 1,
   and at every workload but the streaming one some alternative clears the
   material-gap threshold (decision `improved` in § Cells of the same receipt).
   § Fastest arm of that receipt names the fastest arm per workload and the
   dispatcher's time over it (column *Dispatcher / fastest*). The losses have
   three causes: a slow scalar route below eight words, a fixed per-call
   dispatch cost, and no carry-save kernel.
2. **Fused AND-popcount: confirmed faster than both alternatives in every
   cell.** Every row of tables § Cells: `v3-and-popcnt-confirmation` records
   decision `regressed` and outcome `fail`: the single-pass scalar control and
   the two-pass public gf2-core route are both slower than the fused kernel by
   more than the equivalence margin.
3. **The protocol-v1 result that Muła beats gf2 at 512 B is reversed**: the
   v1 harness charged gf2's arms a per-call cost the C arms did not pay.

The population-count confirmation is the family's second attempt. With the
imported v1 comparisons, its cumulative comparison count leaves fewer expected
bootstrap draws per tail than the twenty P-20 requires (tables § Campaigns, row
`v3-popcount-confirmation`, columns *Attempt*, *m* and *Draws per tail*), so
every cell is `not-confirmatory`, as the frozen addendum's `family.description`
states. Its numbers are descriptive baselines, and the independent pilot
reproduces nearly every estimate (tables § Pilot and confirmation agreement).
The AND family is on its first attempt, which meets P-20, and its cells are
confirmatory (row `v3-and-popcnt-confirmation`).

## Evidence

### Receipts

Tables § Campaigns lists every v3 receipt, each under
`../../bench_results/26465e6c/v3-*/`, with its family accounting, the widest
relative half-width of each pilot, the resolution each confirmation froze from
its pilot, the margins, the outcomes and the acceptance findings. Each receipt
directory holds `receipt.json`, `acceptance-summary.{json,md}`,
`launcher.log`, `execution.log`, `instruction-observation.txt` and the input
snapshots. The family ledgers are
[`v3-popcount-family-ledger.jsonl`](../../bench_results/26465e6c/v3-popcount-family-ledger.jsonl)
and
[`v3-and-popcnt-family-ledger.jsonl`](../../bench_results/26465e6c/v3-and-popcnt-family-ledger.jsonl);
the popcount ledger opens with the imported v1 reservation, and the AND ledger
opens empty because its v1 confirmation was frozen and never run
([`ledger-genesis.json`](ledger-genesis.json)).

The addenda `addendum-{popcount,and-popcnt}-v3-{pilot,confirmation}.json`
beside this file were frozen before their campaigns; each confirmation names
its pilot receipt by path and SHA-256 and states its family accounting. Each
resolution derivation
([`pilot-resolution-v3-popcount.txt`](pilot-resolution-v3-popcount.txt),
[`pilot-resolution-v3-and-popcnt.txt`](pilot-resolution-v3-and-popcnt.txt))
lists every pilot interval's relative half-width and ends with the widest, the
frozen resolution and the operative margins. The consumer-benefit margins are
`MATERIAL_GAP` and `EQUIVALENCE` in `survey/make-addenda.py`; a confirmation
raises a margin to the next hundredth when it does not strictly exceed one plus
the resolution, and the margin's rationale in its `effect` block says so. Both
popcount margins rose; of the AND margins only the equivalence margin did.

The first AND pilot, `v3-and-popcnt-pilot`, is rejected on P-11 (tables
§ Campaigns; its § Cells lists the `invalid` cell). An operator stop ended its
first session inside cell `and-popcnt-w4096-vs-scalar-control`: that session
has no exit line in the receipt's `launcher.log`, and its `execution.log`
starts the cell without completing it. After the second session paused, the
launcher loop started no third session; a re-invoked `run` did. The second
session had started the interrupted cell again, which P-11 rejects. Under
protocol v3 only a stop at a cell boundary, at a cell-budget pause or between
sessions, resumes without repeating a cell; protocol v4 lifts that restriction
by journalling the abandoned attempt (`bdc507a3`,
[amendment](../f547c394/amendment-v4.md)). These receipts pin v3 and are
evaluated under the rules they pin, so the rejection stands. The receipt stays
published and is not resolution evidence:
`v3-and-popcnt-pilot-r2` re-measured the same frozen pilot addendum as the
second of the two pilot trials per cell it allows (`search_budget`). Every cell
of every receipt ran on one core under the protocol's shared window settings,
and no window is flagged (column *Flagged* in each § Cells; `resolved_cpus` in
each summary). The launcher
[`run-campaign.sh`](../../bench_results/26465e6c/run-campaign.sh) records the
exact commands and states the resume condition for its operators, and
[`survey/producing-inputs.json`](survey/producing-inputs.json) names every file
whose bytes the receipts snapshot.

### Arms, pins and builds

| Arm | Build | Runs | Role |
|---|---|---|---|
| `production-dispatch` | conservative-portable | `ops::popcount` (`gf2-popcount-dispatcher`): scalar below 8 words, AVX2 nibble lookup from 8 (`gf2-simd-threshold-default`) | popcount baseline |
| `nibble-lut` | conservative-portable | gf2's `avx2_popcnt` through its function pointer, no threshold | internal control |
| `scalar-popcnt` | conservative-portable | scalar loop forced onto `POPCNT` by a runtime-checked `target_feature` function | internal control |
| `compiler-count-ones` | conservative-portable | portable `u64::count_ones` loop | internal control |
| `libpopcnt` | external | v4.2 `popcnt()` [Libpopcnt2026] (BSD-2-Clause, `libpopcnt-license`), host gcc with `-O3` as its README recommends (`libpopcnt-readme-o3`), own CPUID dispatch (`libpopcnt-cpuid-cached`) | external arm |
| `mula-avx2-harley-seal` | external | sse-popcount `popcnt_AVX2_harley_seal` [Mula2018] (two-clause BSD, `mula-license-binary-clause`), host g++ with the Makefile's AVX2 flags (`mula-makefile-avx2`) | external arm |
| `and-fused` | conservative-portable | `avx2_and_popcnt` from `gf2_kernels_simd::detect()`, as `BitMatrix::matvec_simd` calls it | AND baseline |
| `and-scalar-control` | conservative-portable | single-pass `(a & b).count_ones()` loop | internal control |
| `and-two-pass` | conservative-portable | temporary copy, `ops::and_inplace`, `ops::popcount`, all inside every call | public gf2-core route, whole consumer |

The libpopcnt tag and both upstream commits are `pins` in
`source-evidence.json`; its `vendored_files` records that every vendored file
equals the pinned checkout byte for byte. One Rust release binary serves every
arm (`[profile.release]` in `survey/gf2-side/Cargo.toml`: thin LTO, no ISA
flags) and links the vendored C and C++ objects its `build.rs` compiles; the
binary prints the exact compile commands and compiler versions, which every
launcher log records on its *external build* line. Every arm therefore runs the
same fixture and the same timed loop, with one computed indirect call per timed
call (instruction records, § Shared timed loops). Toolchain and host are in
each receipt section's header lines in the tables. The AND family ran a
rebuilt arm executable (tables § Campaigns, column *Arm executable*): before
its first campaign, the AND arm's cost probes were corrected to time only the
step each names. Random workload words come from SplitMix64 [Steele2014] as
tuning-campaign-support 0.1.0 `src/abtest.rs` implements it, seeded per cell as
each § Cells *Workload* column lists; the right AND operand takes the left
seed plus the offset tables § Method states, and all-one and all-zero words
use no seed. CPU 0's cache sizes and sharing are in the instruction records'
§ Cache geometry of CPU 0 (sysfs): the 2 KiB workload is L1-resident, the
128 KiB workload L2-resident, and the 64 MiB streaming set exceeds the L3.

### Operation equivalence (REQ-03)

Before every campaign `popcount-verify`
(`survey/gf2-side/src/bin/popcount-verify.rs`) checked each arm against an
independent byte-table count: every arm over the matrix of sizes, word offsets,
bit patterns and seeds its `SIZES`, `STREAMING_SIZE`, `SEEDS` and `patterns`
define; the external byte-length entries over every tail shorter than one
vector; gf2's bit-length tail semantics through `BitVec::count_ones` over
`BIT_LENGTHS` (`gf2-bitvec-count-ones`, `gf2-from-words-precondition`); the
fused AND arms over the same matrix; and both arms of every planned cell on its
exact case.
Every group passes in every v3 receipt's `launcher.log`, whose *PASS* lines
give the case counts. Muła refuses every misaligned window (the log's
*refused* line; `mula-aligned-dereference`), so the misaligned workload
declares no Muła cell. Neither library exposes a fused AND or XOR reduction
(`libpopcnt-single-buffer-entry`, `mula-single-buffer-registry`), and gf2 has
no fused XOR kernel, so the AND family has no external arm. The two-pass arm
allocates its temporary and runs both passes inside each timed call.

### Observed instructions and thresholds (REQ-04)

Static facts from the measured binaries' records
([popcount](../../bench_results/26465e6c/v3-popcount-confirmation/instruction-observation.txt),
[AND](../../bench_results/26465e6c/v3-and-popcnt-confirmation/instruction-observation.txt);
disassembler named in each). The AND record comes from a later observer that
also lists `imul`, the SWAR population-count masks and every function holding
a `popcnt` instruction; it records the dispatcher with the same size and
comparisons as the popcount record.

- gf2 takes AVX2 from 8 words: the dispatcher's comparison is `cmp $0x7`
  (`gf2-simd-threshold-default`), and `popcount-verify` observes the scalar
  route at 7 words and AVX2 at 8 (each launcher log's *gf2 dispatch* lines).
  Below the threshold the dispatcher inlines the scalar backend's `count_ones`
  loop (`gf2-scalar-count-ones`) as an unrolled SWAR count (AND record, § gf2
  production dispatcher: `imul`, the word-count comparisons and the SWAR
  masks). The `count_ones` control, called directly, vectorises four words per
  iteration with SSE2 `psadbw` (§ compiler count_ones control, trip mask
  `$0xffffffffffffffe0`). The arm description the plans record for this
  control, "the lowering gf2's scalar backend runs" (`survey/families.py`),
  therefore does not hold for the dispatcher's scalar route.
- No gf2 function contains a `popcnt` instruction: the exact-mnemonic scan
  finds them only in libpopcnt's `popcnt()` and in the forced-POPCNT control
  (AND record, § Every function containing `popcnt`). This agrees with sibling
  survey `04b85d10` that gf2's scalar count has no `POPCNT`, and it accounts
  for the `POPCNT` counts that survey reports in `avx2_popcnt` and
  `avx2_and_popcnt`: here both kernels hold no `popcnt` instruction, and the
  number of their disassembly lines that merely contain the substring (the
  symbol header and branch targets named after the function) equals those
  counts, as the same section lists.
- libpopcnt counts with scalar `POPCNT` below 96 B (`libpopcnt-avx2-threshold`,
  `cmp $0x5f`), a per-vector nibble lookup up to 1 KiB
  (`libpopcnt-medium-loop`) and an AVX2 Harley-Seal loop from 1 KiB
  (`libpopcnt-harley-seal-threshold`, `cmp $0x3ff`). Its capability word here
  has POPCNT and AVX2 but not AVX-512 VPOPCNTDQ (each launcher log's
  *libpopcnt get_cpuid()* line; the selected paths), so its compiled AVX-512
  branch is never taken (`libpopcnt-avx512-branch`).
- Muła's carry-save loop takes 16 vectors (512 B) per iteration
  (`mula-csa-trip`) and loads with `vmovdqa` (`mula-aligned-dereference`),
  hence its alignment precondition.
- gf2's `avx2_popcnt` looks up every vector and has no carry-save stage
  (`gf2-avx2-nibble-lookup`; its record lists `vpshufb` and `vpsadbw` but no
  `vpor`); both Harley-Seal loops fold 16 vectors through `vpand`/`vpor`/`vpxor`
  carry-save adders before one lookup [Mula2018] (§ libpopcnt popcnt_avx2 and
  § Mula AVX2_harley_seal::popcnt).

The measured times turn where the instructions say (tables § Workload
contrasts: `v3-popcount-confirmation`, descriptive): the dispatcher is faster
on 64 bytes than on 32 while every other arm is slower (row `w8 / w4`); Muła is
faster on 512 bytes than on 480, where its carry-save loop engages, while every
other arm is slower (row `w64 / w60`); and across its 96-byte threshold
libpopcnt's time rises least of all arms (row `w12 / w8`).

## Population-count results

Pointers in this section name rows of the confirmation's tables: § Speedups by
workload: `v3-popcount-confirmation` gives every alternative's speedup over the
dispatcher by workload key, § Fastest arm: `v3-popcount-confirmation` the
fastest arm, its runner-up and the dispatcher's gap to it (descriptive), and
§ Cells: `v3-popcount-confirmation` each cell's decision against the frozen
margins. Its *Decision improved* list names gf2's losing cells: every cell
where an alternative clears the material-gap threshold. Every outcome is
`not-confirmatory`; the pilot receipt's matching sections are the independent
earlier sample.

**Below eight words the scalar route loses to every arm but Muła.** At `w4`
the nibble-lut, scalar-popcnt, compiler-count-ones and libpopcnt cells are
`improved` and the Muła cell is `not-worse`, and the forced scalar `POPCNT`
loop is the fastest arm (§ Fastest arm, row `w4`). The dispatcher runs the unrolled SWAR count because
the portable build has no `POPCNT`; the same `count_ones` source called
outside the dispatcher vectorises and is materially faster
(`popcount-w4-vs-compiler-count-ones`), and so is gf2's own AVX2 kernel on
four words (`popcount-w4-vs-nibble-lut`). The 8-word threshold is too high on
this host. The tuning table holds a 4-word value compiled only under
`--cfg gf2_tuning_baked` (`gf2-baked-threshold`); nothing below 4 words was
measured.

**At 64 and 96 B gf2's kernel is the fastest arm and the dispatcher costs it
the win.** § Fastest arm names `nibble-lut` at `w8` and `w12`, and both
`nibble-lut` cells are `improved`. libpopcnt, still on scalar `POPCNT` at
64 B, and Muła are slower than the dispatcher there (speedups below 1 in rows
`w8` and `w12`). The advantage of calling gf2's kernel directly shrinks as the
buffer grows and is gone at 128 KiB (§ Speedups by workload, column
*nibble-lut*, rows `w8` through `w16384`), the signature of a fixed per-call
cost: the size test, the lazily initialised backend load and one more indirect
call (`gf2-simd-threshold-compare`, `gf2-simd-backend-lazylock`,
`gf2-simd-popcount-indirect`).

**From 480 B to 128 KiB libpopcnt is the fastest arm** (§ Fastest arm, rows
`w60` through `w16384`), and every libpopcnt cell in that range is `improved`,
misaligned and constant patterns included. Its runner-up is gf2's kernel
called directly up to 1 KiB and, on aligned buffers, Muła from 2 KiB (column
*Runner-up*). From 1 KiB both external arms run a carry-save loop and gf2 has
none: Muła crosses the dispatcher between 1 KiB and 2 KiB (§ Speedups by
workload, column *mula-avx2-harley-seal*, rows `w128` and `w256`) and is
`improved` at 128 KiB (`popcount-w16384-vs-mula-avx2-harley-seal`). At 480 and
512 B libpopcnt runs its per-vector lookup (`libpopcnt-medium-loop`), gf2's
algorithm, and is still ahead of gf2's kernel called directly (§ Fastest arm,
rows `w60` and `w64`, column *Runner-up / fastest*, descriptive); the static
evidence does not explain that residual.

**Streaming data closes the gaps.** On the 64 MiB set, twice the L3
(`w1m-streaming`), libpopcnt and Muła remain faster than the dispatcher but
below the material-gap threshold (decision `not-worse`), and gf2's kernel
called directly ties it (§ Speedups by workload, row `w1m-streaming`). Muła's
sustained rate is § Fastest arm, row `w1m-streaming`, column *GB/s*.

**Alignment and bit patterns barely matter.** No alignment or bit-pattern
contrast comes near the material-gap threshold (§ Workload contrasts:
`v3-popcount-confirmation`, rows `w256-off24 / w256`, `w64-ones / w64` and
`w64-zeros / w64`); the largest is libpopcnt's on the misaligned window.

## Fused AND-popcount results

Tables § Cells: `v3-and-popcnt-confirmation` holds the confirmatory cells at
32 B, 32 KiB and streaming operand sizes, with the fused kernel as every cell's
baseline, so a speedup below 1 means the fused kernel is faster. Every cell's
corrected interval lies below the reciprocal of the frozen equivalence margin
(decision `regressed`, outcome `fail`): the single-pass scalar control and the
two-pass public route are each confirmed slower than the fused kernel by more
than that margin at every size. The scalar control has no `POPCNT` either; it
vectorises with SSE2 like the `count_ones` control (AND record, § AND scalar
control).

The two-pass route pays for a temporary copy, a separate AND pass and a
separate count inside every call. § Conversion-cost probes:
`v3-and-popcnt-confirmation` gives the copy's and the two backend selections'
costs as descriptive medians, measured outside the timed windows that already
contain them: at 32 KiB the copy alone is a large share of a fused call (row
`and-popcnt-w4096-vs-two-pass`, arm `and-two-pass`, against that cell's
baseline in § Cells), while the selections sit at the probe's
integer-nanosecond resolution. The copy probe repeats bank 0's copy, so in the
streaming cell it is a cache-warm lower bound. `BitMatrix::matvec_simd`
already uses the fused kernel (`gf2-matvec-uses-fused`), but gf2-core keeps
the kernel bundle private (`gf2-simd-fns-private`) and its public vector API
offers no fused AND-count, so a gf2-core consumer pays the two-pass cost unless
it takes the kernel from `gf2_kernels_simd::detect()` itself
(`gf2-kernels-detect-public`).

## History and contradictions

The protocol-v1 receipts (`../../bench_results/26465e6c/2026-09-0*`), addenda
and harness ([`superseded/v1/`](superseded/v1/)) stay byte-for-byte as
history. Tables § History: protocol-v1 confirmation
`2026-09-08-26465e6c-popcount` sets each cell of the accepted v1 confirmation
beside the v3 cell with the same size, offset, pattern and arm pair. Among the
v1 cells that passed, the v3 counterparts reverse both Muła cells at 512 B,
whose v3 intervals lie wholly below 1; shrink libpopcnt's all-one 512 B gap,
which stays `improved`; keep libpopcnt's 128 KiB gap, a little wider in v3;
and widen the nibble-lookup gap at 32 B. The v1 scalar-`POPCNT` cell at 64 B,
`not-material` in v1, is `improved` in v3.

The v1 gf2 arms reduced the bank index with a runtime modulus and matched the
arm name as a string inside every timed call, while the C arms selected banks
without a division and were built `-march=native` (`v1-harness-bank-modulo`,
`v1-harness-string-dispatch`, `v1-c-harness-bank`, `v1-external-march-native`).
A fixed cost on gf2's side inflates an external arm's small-buffer speedup and
compresses the ratio between two gf2 arms, which is the pattern above; at
128 KiB, where a per-call cost is negligible against the call, the v1 result
persists. The v1 finding that libpopcnt and Muła beat gf2 materially, where
their v1 cells passed, is falsified for Muła at 512 B and stands for
libpopcnt.

Re-evaluated with the current acceptance tool on scratch copies, every v3
summary reproduces byte for byte and every v1 receipt but one reproduces every
verdict, decision and outcome
([`reevaluation.txt`](../../bench_results/26465e6c/reevaluation.txt), written by
`survey/reevaluate.sh`). The exception is the 2026-09-07 v1 pilot, which v1
had already superseded: the tool now rejects it on P-05 because its snapshot
lacks the `Cargo.lock` its receipt pins, a file the v1 branch never committed.
The other v1 snapshots hold a `Cargo.lock` with the pinned digest; no result
here rests on that pilot.

## Baseline for 5cbb6545

The optimization issue for population count and fused bit reductions can use
these receipts as its pre-change baseline. Each item is a falsifiable
follow-up:

1. **Selection threshold.** Lower `SIMD_MIN_WORDS_DEFAULT` from 8
   (`gf2-simd-threshold-default`): gf2's AVX2 kernel is materially faster than
   the scalar route on 4 words (`popcount-w4-vs-nibble-lut`). Sweep 1 to 8
   words; this survey measured nothing below 4.
2. **Scalar `POPCNT`.** A runtime-detected `POPCNT` scalar kernel in
   gf2-kernels-simd: the forced `POPCNT` loop is the fastest arm at 32 B
   (§ Fastest arm, row `w4`; `popcount-w4-vs-scalar-popcnt`).
3. **Per-call dispatch.** Direct kernel calls are materially faster at 64 and
   96 B (`popcount-w8-vs-nibble-lut`, `popcount-w12-vs-nibble-lut`); resolve
   the kernel once per consumer or specialise small sizes.
4. **Carry-save kernel.** libpopcnt is materially faster from 1 KiB to 128 KiB
   and Muła at 128 KiB (`popcount-w128-vs-libpopcnt`,
   `popcount-w256-vs-libpopcnt`, `popcount-w16384-vs-libpopcnt`,
   `popcount-w16384-vs-mula-avx2-harley-seal`): an AVX2 Harley-Seal loop from
   about 1 KiB is the measured opportunity.
5. **480 and 512 B residual.** libpopcnt's per-vector loop, gf2's algorithm,
   is materially faster than the dispatcher (`popcount-w60-vs-libpopcnt`,
   `popcount-w64-vs-libpopcnt`) and ahead of gf2's kernel called directly
   (§ Fastest arm, rows `w60` and `w64`, descriptive); profile both loops
   before attributing it.
6. **Streaming and alignment.** Leave them: no streaming cell reaches the
   material-gap threshold (the `popcount-w1m-streaming-*` cells), and
   misalignment moves no arm's time by an amount near it (§ Workload contrasts,
   row `w256-off24 / w256`).
7. **Fused AND.** Keep `avx2_and_popcnt`; a public fused AND-popcount would
   spare gf2-core consumers the two-pass route, which the confirmation finds
   slower at every size (the `and-popcnt-*-vs-two-pass` cells).

gf2's losing cells are listed by identifier on the *Decision improved* line of
tables § Cells: `v3-popcount-confirmation`. They are the cells above: at 32 B
against every arm but Muła, at 64 B against gf2's kernel and the forced
`POPCNT` loop, at 96 B against gf2's kernel, from 480 B to 128 KiB against
libpopcnt on every pattern and alignment, and at 128 KiB against Muła.

## Criteria

| Criterion | Status | Evidence |
|---|---|---|
| REQ-01 contract, frozen protocol and addendum, negative outcomes | MET | Every v3 receipt (tables § Campaigns) pins the contract, protocol, schema, frozen addendum, ledger prefix and producing inputs; the current `benchmark-acceptance` accepts each with no error finding except the interrupted first AND pilot, which it rejects on P-11 and which stays published (`reevaluation.txt`). Losing, not-confirmatory and rejected results and the v1 history are preserved. No production change, so no before/after pair is due. |
| REQ-02 pinned external revisions and backends; labelled internal controls | MET | libpopcnt v4.2 and sse-popcount at the commits in `source-evidence.json` `pins`, vendored byte-identical with their licences (`vendored_files`; `libpopcnt-license`, `mula-license-binary-clause`); backends observed per execution (each § Selected paths in the tables) and in the instruction records; nibble-lookup, scalar `POPCNT` and `count_ones` controls in both popcount receipts. |
| REQ-03 identical buffers, lengths, tails and counts; matching fused reductions; consumer-equivalent timings | MET | `popcount-verify` passes every group in every v3 launcher log; Muła's refusal of misaligned windows is recorded there; no external fused AND or XOR reduction exists; the two-pass arm times its temporary and both passes. |
| REQ-04 frozen addenda and receipts covering the workload classes; emitted instructions observed | MET | The frozen v3 addenda and receipts cover 32 to 96 B, the 480/512 B carry-save and 1 KiB boundaries, a 24-byte misalignment, all-one and all-zero data, L1- and L2-resident and streaming sizes; every threshold is read from the instruction records. |

## Limits

- The population-count confirmation cannot be confirmatory under the frozen
  family history. At its corrected alpha the bootstrap leaves too few
  expected tail draws for stable endpoints, and the tool notes P-20 on every
  cell (tables § Campaigns, row `v3-popcount-confirmation`, columns *Draws per
  tail* and *Findings*). The arm-median and cross-cell intervals are
  descriptive and not family-corrected.
- No profiler ran: attributions rest on the static instruction records and on
  the direct-kernel comparisons, and the 480 and 512 B residual stays open.
- Every cell is single-core because every arm is a single-threaded leaf
  kernel. Cold first-use series are not measured: nanosecond calls cannot
  resolve them above timer overhead.
- Nothing below 4 words was measured, and libpopcnt's AVX-512 path was not
  exercised because the host lacks AVX-512 VPOPCNTDQ (each launcher log's
  *host* line).
- The conversion probes are single-shot (`setup_ns`) or cache-warm means
  (`pack_ns`, `dispatch_ns`) at integer-nanosecond resolution and are
  descriptive.
