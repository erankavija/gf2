# The 64x64 transpose lane family and the BCH bitslice conversion

> **Diátaxis Type:** Explanation

Report for `1d4fd63d` (epic `1a379447`). The [measurement contract](../1a379447-zen3-cpu-performance/measurement-contract.md)
and the [shared protocol](../f547c394/protocol.md) at [version 4](../f547c394/amendment-v4.md)
govern every receipt; each receipt pins the contract, the protocol, the
addendum schema and its family addendum by digest and commits its producing
snapshot, its journal and its checkpoints. This report carries the argument and
cites the evidence: every figure lives in the [generated tables](../../bench_results/1d4fd63d/tables.md)
(`tables.md`, projected by [`survey/summarize.py`](survey/summarize.py) from the
receipts alone), in a receipt's acceptance summary, in a frozen derivation
record, or in a claim of [`survey/source-evidence.json`](survey/source-evidence.json),
which this report cites by claim id rather than by line number. Pointers name
the `tables.md` section, block and row.

## Answer

The library gains a named family of 64x64 bit-block transpose lanes with one
dispatch, two new candidate kernels, and contract cases that every lane runs.
Two campaigns measure it.

The lane-selection family confirms that the `avx2-ymm6` candidate is materially
faster than the lane the dispatch publishes on the block kernel in isolation,
on a streaming run, and on the parity-unpack half of the BCH bit-slice
conversion, and confirms it not worse on the packing half and on the whole
dense matrix consumer (tables § Lane confirmation → Cells). Its receipt does
not qualify for production selection, because two of its six confirmatory cells
are improvement cells that record `not-material`; § [Adoption](#6-adoption)
gives the reason, the arithmetic that leaves this family no further confirmatory
attempt, and which issue owns the selector decision. The published lane is
unchanged.

The comparator family reaches the confirmatory decision the transpose question
has lacked: no material gap exists in either external comparator's favour, and
the candidate lane is materially ahead of both M4RI [AlbrechtBard2026] and
Bitshuffle [Bitshuffle2026] at the frozen kernel geometry (tables § Comparator
confirmation → Cells).

Both other candidates are rejected by the frozen rule with their rows shown:
the PSHUFB lane is behind on every workload and behind both external
comparators, and the movemask lane is behind on every workload but one
(tables § Lane ranking stage → Cells; § Comparator pilot → Cells).

## 1. The lane family and its one dispatch

`gf2_kernels_simd::transpose` owns the 64x64 block contract: under little-endian
bit indexing, bit `c` of input word `r` is the matrix entry $(r, c)$, and a
transpose writes 64 words with bit `r` of word `c` equal to it, which is
$\mathrm{output}[c][r] = \mathrm{input}[r][c]$. `TransposeLane` enumerates every
implementation of that contract (`lane-enumeration`) and `lane` is the one way
to reach a specific one (`lane-entry-point`), publishing a safe pointer only
after the runtime feature test that is each unsafe kernel's whole safety
contract (`lane-feature-gate`). `detect` resolves the production lane through
`PRODUCTION_PREFERENCE` (`production-preference`, `detect-resolves-preference`),
and that constant is the only thing an adoption edits. The PSHUFB alternative,
which used to have its own entry point, is a variant of the enum like every
other lane; there is no second dispatch and no private copy.

Two candidates join the incumbent and the portable kernel:

| Lane | Mechanism | Claim |
|---|---|---|
| `avx2-bit-twiddle` | the incumbent: four wide mask-shift-XOR stages in YMM registers over a stack copy of the block, two narrow stages in words | `incumbent-declares-scratch` |
| `avx2-ymm6` | all six stages in YMM registers, the first writing the caller's output directly, so the block is never copied to a stack scratch | `ymm6-kernel` |
| `avx2-pshufb` | 8x8 byte tiles through a `vpshufb` bit-reversal lookup | `pshufb-kernel` |
| `avx2-movemask` | an SSE byte transpose then `vpmovmskb` bit-plane extraction | `movemask-kernel` |

The movemask lane is the issue's 32x8 movemask/byte-shift candidate brought
inside the 64x64 contract: its byte transpose is the tile assembly that geometry
needs and its bit-plane extraction is the packing, and both are inside the
kernel, so every cell that times it pays them (`movemask-kernel`).

`BitMatrix::transpose` is the tiling driver at the resolved kernel
(`matrix-transpose-delegates`), and the driver takes its block primitive as an
argument (`matrix-transpose-driver`), so a contract case or a benchmark arm
pins a lane by naming it and no process-global override exists. The bit-sliced
BCH encoding reaches the transpose through one field of its bundle
(`bch-bundle-transpose-field`), which the accelerated bundle fills from the same
dispatch (`bch-bundle-uses-dispatch`).

## 2. Contracts every lane answers (REQ-08)

The cases live once and run over `TransposeLane::ALL`, so a lane added to the
enum is exercised without a second copy of them.

- `transpose::contract::assert_block_contract` covers the canonical relation
  against naive bit arithmetic on pseudorandom blocks, the zero and all-ones
  blocks, single bits at the four corners, the involution
  $(A^{\mathsf T})^{\mathsf T} = A$, and the same relation with both buffers
  held at an odd word offset inside a larger allocation, which is storage no
  vector load may assume aligned.
- `crates/gf2-core/tests/transpose_lane_contract.rs` drives the production
  tiling, output allocation, zero padding and tail mask through each lane for
  zero-sized matrices in both dimensions, non-square matrices, row and column
  counts that leave a partial word, and the 63, 64 and 65 boundary tiles, and
  checks each against the production entry point and against the involution.
  It runs with and without the `simd` cargo feature, which is what decides
  whether `BitMatrix::transpose` resolves a detected lane at all.
- `bch_encode`'s round trip bit-slices a message batch through each lane and
  reads the parity back, against the per-frame reference, across the dimension,
  redundancy and batch boundaries that exercise a partial lane group, a partial
  degree block and a partial parity word.

[`run-validation.sh`](run-validation.sh) runs all of them at the repository
MSRV and records the commands, the exit status and the test counts the harness
reported in [`validation.json`](validation.json); the launcher refuses to start
a timed campaign unless that record passed.

## 3. Equivalent bit mapping and the frozen geometry (REQ-07)

Every arm of every comparator cell reads the same fixture, 64 words of
SplitMix64 [Steele2014] seeded at the cell's seed, and writes the transpose
under the mapping § 1 states.
[`survey/verify-bit-mapping.py`](survey/verify-bit-mapping.py) computes that
transpose by naive bit arithmetic and compares it against every lane of
`gf2_kernels_simd::transpose` and against both external arms at five seeds,
writing [`survey/bit-mapping-report.json`](survey/bit-mapping-report.json); the
launcher runs it before any timed comparator campaign, so no cell times two
routes that compute different bits.

The frozen geometry is 6fb89a3c's: one 64x64 block per call into a preallocated
output, no tail and no adapter, because one block of 64 eight-byte elements is
the canonical gf2 layout for both comparators. Adapter costs are zero there and
are not hidden. The geometries that do pay an adapter, 63 and 65 rows, are
6fb89a3c's own consumer cells, and that survey carries them with the padding,
packing and plane-unpacking it measured. The tile shapes, tails and matrix
dimensions of both families are frozen in their addenda (`cells`, each cell's
`workload.size`) before either campaign launched.

The external arms are not rebuilt from new sources. They are the executables
6fb89a3c pinned, reproduced by its committed `fetch-build.sh` and accepted only
when byte-identical to the digests its `build-evidence.json` records, which
[`survey/check-external-arms.py`](survey/check-external-arms.py) decides and
[`survey/external-arm-check.json`](survey/external-arm-check.json) records. Its
warm-pass and child-framing verification therefore carries over unchanged, and
the comparator identities are that survey's.

## 4. Scratch traffic against compiler spills (REQ-10)

`crates/gf2-kernels-simd/src/x86/asm/transpose.asm.txt` is the release
disassembly of all four AVX2 lanes with an annotation appended by
[`survey/annotate-asm.py`](survey/annotate-asm.py), which counts each symbol's
stack frame, block copies, frame stores and loads and mnemonic mix from those
bytes alone and is idempotent, so the artefact reproduces on re-run. Frame
traffic alone does not say whether a routine declares a buffer or the compiler
spilled; the declaration in the source settles it, and the two read together
give:

- The incumbent declares a 64-word local and copies the block into it and back
  out, which the annotation counts as block copies rather than as individual
  stores (`incumbent-declares-scratch`, `asm-incumbent-block-copy`). This is
  intentional scratch, the classification 04b85d10 recorded for it.
- The candidate declares no buffer and copies no block
  (`asm-ymm6-no-block-copy`). Its remaining frame traffic belongs to no declared
  local, so it is register pressure: the six-stage recursion holds more live
  YMM values than the incumbent's four-stage one, and the compiler spills some
  of them. Removing an intentional block-sized scratch and paying a smaller
  frame instead is the mechanism behind the block cell's confirmed gain; the
  annotation's frame rows carry both sides of that trade.
- The movemask lane declares a byte-plane array (`movemask-declares-planes`),
  the family's second intentional scratch, which is the packing its geometry
  needs.

The annotation's per-symbol rows carry the counts.

## 5. Design and results of the two campaigns

### 5.1 The lane-selection family

One arm executable serves every arm and selects its lane from
`GF2_TRANSPOSE_LANE` (`arm-selects-lane-by-environment`), so the arms share a
build, a fixture generator, a warm pass and a timing loop and differ only in
the kernel under measurement. The baseline of every cell is whatever `detect`
publishes (`arm-baseline-is-production-dispatch`), which is the pinned
pre-change implementation REQ-09 requires; the candidate is one named lane
reaching the consumers through the same abstraction the production kernel does.
Each arm applies the declared warm policy, one untimed pass of the timed body
over every bank before calibration (`arm-warm-pass`). The arm enables no
`test-support` feature, so the measured code is what a consumer of these crates
compiles.

Each candidate is measured on the six workloads REQ-09 names: the block kernel
alone over an L2-resident run and over a streaming run on a six-core arm; the
two halves of the BCH bit-slice conversion, which are one lane group's whole
message absorbed through the transpose and the bit-sliced recurrence
(`absorb-block-transposes`) and the reduced register read back as packed
per-frame parity (`unpack-parity-transposes`), both on the mother code of the
DVB-T2 short frame resolved from 04b85d10's committed code registry; and the
whole `BitMatrix::transpose` consumer at a dense and a partial-tile geometry,
whose output allocation, tile assembly and tail mask are inside the timed call.
A seventh cell is an identity control that pins the whole bit-sliced BCH batch
encode both halves sit inside: both of its arms run the production family entry
point, which resolves its own block kernel, so it measures no lane difference
(`arm-control-reports-dispatched-lane`).

The family runs in four stages, each accepted with the finding count on its
`tables.md` heading:

| Stage | Receipt | What it decides |
|---|---|---|
| Smoke | [`transpose-lane-smoke`](../../bench_results/1d4fd63d/2026-09-13-1d4fd63d-transpose-lane-smoke/acceptance-summary.md) | the child protocol, case decoding, warm pass, affinity resolution and acceptance agree on the wire; its own family and ledger, and no conclusion rests on it |
| Lane ranking | [`transpose-lane-pilot`](../../bench_results/1d4fd63d/2026-09-13-1d4fd63d-transpose-lane-pilot/acceptance-summary.md) | which candidate a confirmation can reach |
| Selected lane | [`transpose-lane-selected`](../../bench_results/1d4fd63d/2026-09-13-1d4fd63d-transpose-lane-selected/acceptance-summary.md) | the measurement resolution a confirmatory sample has |
| Confirmation | [`transpose-lane-confirmation`](../../bench_results/1d4fd63d/2026-09-13-1d4fd63d-transpose-lane-confirmation/acceptance-summary.md) | the lane decision, on fresh samples |

The ranking stage runs every candidate on every workload. Its outcomes select
`avx2-ymm6` and reject the other two, and every one of its rows stays
(tables § Lane ranking stage → Cells). The selected stage repeats that lane's
cells and the control at the confirmatory pair count on the same sizes, seeds
and fixtures, because a resolution observed at half the confirmatory sample is
wider than the one a confirmation has. The confirmation is frozen from that
stage's committed receipt by the canonical freezer, which pins it by path and
digest, derives the resolution from its own intervals and refuses a margin the
resolution does not admit; [`confirmation-derivation.txt`](confirmation-derivation.txt)
is that derivation and names the one cell whose bimodal per-execution values
set the resolution and the margin that replaces the one the pilot declared
(tables § Resolution of the lane confirmation).

The confirmation's six cells decide: the block kernel and the streaming run
pass, the parity unpack passes, the partial-tile consumer passes its
non-regression rule, and the packing half and the dense whole-matrix consumer
record `not-material` (tables § Lane confirmation → Cells). No cell regresses.

Why the two `not-material` cells read that way is arithmetic, not noise. The
whole bit-slice conversion is a minority share of the whole bit-sliced BCH
batch encode, and what the candidate lane removes from a batch is a small part
of that share; the Amdahl ceiling on what any faster conversion can give that
consumer follows and is labelled an estimate (tables § Conversion share of the
whole BCH consumer). Inside the packing half the transpose is one block
operation against a bit-sliced recurrence over the code's whole redundancy, and
inside the dense matrix consumer the outer tiling loop, the output allocation
and the cache traffic bound the call, which is what 04b85d10 attributed there.
A faster block kernel moves only its own share of each; the family's frozen
worthwhile threshold applies to the complete conversion cost, so both record
not-material.

### 5.2 The comparator family

`transpose-lane-vs-external` is a separate scientific question with its own
ledger: the lane gf2 publishes is decided by the selection family, and this
family asks only whether a material gap separates a lane from an external
comparator. The baseline of each cell is a gf2 lane and the candidate the
external arm, the direction 6fb89a3c uses, so a speedup above one means the
external arm is ahead.

| Stage | Receipt | What it decides |
|---|---|---|
| Pilot | [`external-pilot`](../../bench_results/1d4fd63d/2026-09-13-1d4fd63d-external-pilot/acceptance-summary.md) | every lane against both comparators; the family's resolution |
| Confirmation | [`external-confirmation`](../../bench_results/1d4fd63d/2026-09-13-1d4fd63d-external-confirmation/acceptance-summary.md) | the candidate lane against both comparators, on fresh samples |

The pilot puts all four lanes against both comparators and every cell but the
PSHUFB ones records gf2 materially ahead (tables § Comparator pilot → Cells).
The confirmation, frozen by the same canonical freezer with
[`external-confirmation-derivation.txt`](external-confirmation-derivation.txt)
as its record, carries the two cells P-20's tail-support bound admits and
records `fail` for both: no material gap exists in either comparator's favour,
and the candidate lane is materially ahead of each (tables § Comparator
confirmation → Cells).

This is the confirmatory decision the transpose comparison has lacked.
6fb89a3c's own confirmation is withdrawn under its warm-pass defect and its
corrected re-measurement is exploratory, and its ledger admits no further
confirmatory attempt; its § 8 follow-up 3 asks for a question narrow enough to
carry at most two comparisons, and this family is that question. Its scope is
narrower than 6fb89a3c's in exchange: the kernel geometry only, where no
adapter exists, with the 63 and 65 consumer geometries left to that survey.

## 6. Adoption

`PRODUCTION_PREFERENCE` is unchanged and the published lane is the incumbent.
The reason is the rule, not the measurement.

The protocol grants `qualifies` only to a receipt whose every non-exploratory
cell passes, and the contract selects a production candidate only when the
improvement, correctness and non-regression rules pass on the declared dispatch
domain. Two of the six confirmatory cells are improvement cells that record
`not-material`, so the lane confirmation does not qualify (tables § Lane
confirmation, *Qualifies for production selection*).

That is a consequence of how this family froze those two cells, and the defect
is the objective they carry rather than the result they produced. The packing
half and the dense whole-matrix consumer are cells where the transform is a
minority of the measured work by construction, so a confirmed non-inferiority
is the strongest outcome either can give; declaring them `improvement` rather
than `non-regression` makes that outcome read as a failure to improve. The
comparable sibling receipt of this epic, `1c602857`, declares exactly that
distinction and qualifies. The assignment was frozen before the trial and stays
as it is.

No further confirmatory attempt exists for this family. Its ledger already
holds one non-exploratory reservation, so a second attempt spends the smaller
budget $\alpha/[t(t+1)]$ at $t = 2$ over a cumulative comparison count that
includes the first attempt's, and P-20's floor of twenty expected draws per
bootstrap tail is reached at no cell count (tables § Tail support and remaining
budget of the lane-selection family, which recomputes both rates from the
ledger the confirmation pins). A renamed family is a different scientific question, not a way to
retry this one. So the choice is between adopting on the evidence as recorded
and never adopting. That choice belongs to `63bad95d`, which calibrates the
production kernel selectors through the canonical tuning mechanism and reads
this family's receipts as its evidence: the
evidence says the candidate is materially faster on the block kernel, on a
streaming run and on the parity unpack, confirmed not worse everywhere else it
is measured, materially ahead of both external comparators, and correct on every
contract case; the rule says the receipt does not qualify.

The lane stays in the family either way, tested, documented, reachable through
`transpose::lane` and covered by the committed asm artefact, so adoption is one
constant's edit whenever it is authorized, and a production change would then
owe the before/after evidence REQ-01 requires. No production change is adopted
here, so that clause is not engaged.

## 7. Rejected candidates and preserved outcomes

- **The PSHUFB lane is rejected.** It is behind the incumbent on every workload
  of the ranking stage and behind both external comparators in the comparator
  pilot (tables § Lane ranking stage → Cells; § Comparator pilot → Cells). Its
  `vpshufb` bit reversal is followed by a scalar bit loop that assembles the
  transposed tiles, which the artefact's annotation shows as the only lane
  whose instruction mix is a byte shuffle over a frame it also copies. It stays
  in the family as a named lane and an asm artefact rather than being deleted,
  because the issue names it as a candidate and its rows are the evidence that
  rejected it.
- **The movemask lane is rejected.** It is behind on every workload but the
  packing half, where it records not-worse (tables § Lane ranking stage →
  Cells). Its tile assembly and packing are inside the kernel, so this is the
  complete conversion cost of that geometry and not a kernel figure with the
  adapter removed.
- **Two confirmed not-material cells stay.** The packing half and the dense
  whole-matrix consumer are reported exactly as the evaluator recorded them.
- **The identity control measures no lane difference**, by construction, and
  its interval contains one (tables § Selected-lane stage → Cells). It is
  pilot-only: a confirmatory reservation for it would spend one of the six
  comparisons P-20 admits on a cell that cannot decide anything, which is the
  rationale [`confirmation-derivation.txt`](confirmation-derivation.txt)
  records for dropping it.
- **The bit-sliced family is not what the conservative profile selects.** Under
  it no batch length admits the bit-sliced encoding family
  (`bitslice-min-batch-conservative`, `bitslice-family-admission`), so the
  consumer the conversion sits inside is reached by installing a profile or by
  naming the family, which is what the control cell does. Whether to admit it
  at a production batch length is 04b85d10's ranked entry 2, a tuning-profile
  question this issue's kernel work does not settle.

## 8. Where this conversion sits among comparable libraries

The bit-slice conversion exists because a BCH shift register is serial per
frame, so the parallelism a batch offers is across frames, and moving a batch
into and out of that layout is a transpose. AFF3CT's `Encoder_BCH_inter`
[Cassagne2019] is the same design: the same recurrence advanced across a SIMD
wave of frames, with the batch transposed in and out by a reorderer. That is
what makes the two halves measured here the right unit rather than the block
kernel alone. The generic binary BCH library of the Linux kernel [Djelic2011]
takes the other branch: a table-driven remainder over packed bytes, one frame at
a time, which pays no conversion at all and therefore gains nothing from a
faster block transpose. 4e732b56's survey pins both and states their generator
agreement; this report adds no measurement against either and cites them for
where the conversion belongs, not for a figure.

## 9. Criteria

| Criterion | Status | Evidence |
|---|---|---|
| REQ-01 contract | MET; no production change is adopted, so the before/after clause is not engaged | six accepted receipts pin the contract, protocol v4, the schema and their family addendum by receipt-local digest and commit their producing closure, journals and checkpoints; both confirmations use fresh samples with resolution evidence from a distinct pilot pinned by digest; correctness precedes timing through `run-validation.sh` and `validation.json`; negative, rejected and not-material outcomes are retained (§ 6, § 7) |
| REQ-07 comparison | MET | the equivalent bit mapping is defined and checked for every arm before timing (§ 3; `survey/bit-mapping-report.json`); the geometry is frozen in both addenda and pays no adapter, with the adapter-paying geometries left to the survey that measured them; all four lanes are compared against M4RI [AlbrechtBard2026] and Bitshuffle [Bitshuffle2026] in the comparator pilot and the candidate confirmatorily (§ 5.2) |
| REQ-08 relation | MET | the shared block, matrix and bit-slice round-trip contracts run over every lane, covering zero-sized, non-square, unaligned and partial-tile cases under little-endian bit indexing (§ 2) |
| REQ-09 measurement | MET | isolated latency, streaming throughput, packing, unpacking and the whole BCH conversion consumer, each against the pinned pre-change implementation, through the established runner, acceptance tool and lock wrapper (§ 5.1); `tables.md` projects every cell |
| REQ-10 attribution and selection | MET; two candidates rejected, the third not selected | the asm annotation separates the incumbent's declared scratch and the movemask lane's declared byte planes from the candidate's spills (§ 4); every candidate reaches the consumers through the shared abstraction and none through a private path (§ 1); the PSHUFB and movemask lanes are rejected by the frozen rule with their rows shown (§ 7); the remaining candidate's selection is blocked by the rule § 6 states, not waived |

## 10. Follow-ups

1. **Adopting the candidate lane** is the open decision § 6 states, and
   `63bad95d` owns it: that issue calibrates the production kernel selectors
   through the canonical tuning mechanism. This family can produce no
   qualifying receipt, the evidence it did produce is complete, and the change
   is one constant.
2. **Admitting the bit-sliced BCH encoding family at a production batch length**
   is 04b85d10's ranked entry 2 and a tuning-profile question; this issue
   measures the conversion inside that family but selects no profile, and
   `63bad95d` owns the profile the production build installs.
3. **Word-wise systematic codeword assembly** is 04b85d10's ranked entry 1. Its
   share of the BCH consumer is far larger than the conversion's (tables
   § Conversion share of the whole BCH consumer bounds what a faster conversion
   can give), so it is the lever that consumer has left. It is a packing change
   in `gf2-coding` rather than a transpose lane, and `8e53235b` owns it.
4. **The 63 and 65 consumer geometries against the external comparators** stay
   with 6fb89a3c, whose ledger admits no further confirmatory attempt for them;
   a confirmatory decision there needs a new family narrow enough to carry at
   most two comparisons, as this issue's comparator family is for the kernel
   geometry; `1362381c` records it among the remaining limits.
