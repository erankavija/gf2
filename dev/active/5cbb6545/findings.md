# Population count and fused bit reductions on the Ryzen 9 5900X

> **Diátaxis Type:** Explanation

Survey for `5cbb6545`. It evaluates population-count and fused
AND-population-count candidates for `gf2-core`, retains the established
production routes, and adds two directly testable carry-save kernels to
`gf2-kernels-simd`. The
[measurement contract](../1a379447-zen3-cpu-performance/measurement-contract.md)
and the [shared protocol](../f547c394/protocol.md) at
[version 4](../f547c394/amendment-v4.md) — the version every receipt pins,
snapshots and is evaluated under — govern every receipt.

This report states no measured value. Each conclusion points to its source: a
section of the generated [receipt tables](../../bench_results/5cbb6545/tables.md),
written "tables § heading", with a row named by its cell ID; a receipt or
summary field; or a named file. Speedup intervals are the acceptance tool's at
the corrected alpha; per-call times, rates and cycles per byte in the tables are
medians over a cell's pairs and are descriptive, as tables § Method and
§ Rates state. Running `dev/active/5cbb6545/survey/summarize.py` from the
repository root reproduces the tables byte for byte. Code and instruction claims
cite [`survey/source-evidence.json`](survey/source-evidence.json) by claim ID in
backticks; each entry holds the path, line, verbatim text and reason.

## Question and answer

Which population-count and fused-reduction route is fastest at each input size
this host serves, against the established AVX2 nibble lookup, scalar `POPCNT`,
Harley-Seal carry-save accumulation [Mula2018] and pinned libpopcnt
[Libpopcnt2026]? Where does a fused reduction earn its place in a consumer?

1. **The exploratory sweep finds a carry-save crossover at 256 words, but the
   confirmation does not qualify the family for production.** The
   unfused carry-save kernel is slower than the established per-vector lookup at
   every swept width up to 128 words and clears this family's worthwhile margin
   first at 256 words (tables § Cells: `popcount-sweep`, rows
   `sweep-popcount-w16-csa` through `sweep-popcount-w128-csa`, decision
   `regressed`; § Cells: `popcount-sweep2`, row `sweep2-popcount-w256-csa`,
   decision `improved`, against `sweep2-popcount-w224-csa`, decision
   `not-worse`). The fused candidate crosses earlier and is already materially
   faster there (§ Cells: `fused-sweep2`, rows `sweep2-and-w192-csa` and
   `sweep2-and-w256-csa`). Those exploratory cells do not establish a canonical
   cutover, and no exact 255/256/257 confirmation exists. The confirmations
   contain passing cells at `popcount-w1024-dispatch`,
   `popcount-w16384-dispatch`, `and-w512-fused`, `and-w4096-fused` and
   `matvec-1024x16384`, but each family also has a non-passing cell and therefore
   records `qualifies: false` (§ Campaigns and § Cells). P-20 makes that
   family-level result controlling, so the production selector stays unchanged.
2. **A fused direct comparator avoids a temporary buffer and second pass.** The
   candidate fused route beats the
   two-pass route a `gf2-core` consumer has without it, allocation and second
   pass included, by the widest margin in either family (§ Cells:
   `fused-confirmation`, row `and-w4096-vs-two-pass`, decision `improved`), and
   the probes of that cell show where the two-pass cost sits (§ Conversion
   costs).
3. **The scalar `POPCNT` kernel wins nothing a consumer can reach, and that
   region keeps the count it has.** Called directly, the kernel is faster than
   the established sub-threshold count at every swept width below eight words
   (§ Cells: `popcount-sweep`, rows `sweep-popcount-w1-scalar-popcnt` through
   `sweep-popcount-w7-scalar-popcnt`, decision `improved`). Reached the way a
   consumer reaches it, through the bundle's function pointer, it loses: the
   confirmation records `popcount-w4-dispatch` as `regressed`, outcome **fail**
   (§ Cells: `popcount-confirmation`). Under REQ-10 that region retains its
   established implementation, and the production resolver returns the scalar
   backend's portable count below the bit-backend SIMD threshold
   (`ops-scalar-retained`, `scalar-count-ones`).
4. **The residual candidate gap to libpopcnt is not material.** In the
   candidate-producing confirmation, libpopcnt leads the cache-resident count
   by less than this
   family's material-gap threshold: the confirmation records
   `popcount-w16384-vs-libpopcnt` as `not-worse`, outcome **not-material**
   (§ Cells: `popcount-confirmation`). Against Muła's reference the surveyed gap
   is `inconclusive` in both directions at both widths (§ Cells:
   `popcount-sweep`, rows `sweep-popcount-w1024-vs-mula-avx2-harley-seal` and
   `sweep-popcount-w16384-vs-mula-avx2-harley-seal`). The aspirational REQ-07 of
   the epic — meeting the fastest compatible measured implementation — is
   therefore met against Muła and falsified against libpopcnt, by a gap this
   family's own rule reports and leaves.
5. **Two cells establish no non-regression, making their families
   nonqualifying.**
   `and-w128-fused` and `matvec-1024x4096` are `inconclusive` at the frozen
   equivalence margin (§ Cells: `fused-confirmation`), while
   `popcount-w4-dispatch` is `regressed` in the other family. The production
   tree retains the established scalar route below the SIMD threshold and the
   established nibble routes at every SIMD width. § Clock observation names the
   route each observed width takes after that retention.
6. **The two confirmations measure arms their pilots did not.** Only the
   confirmations time the library arms through a consumer's indirect call; every
   sweep and pilot times a devirtualised copy. The arm digests differ between
   each confirmation and its pinned pilot (tables § Campaigns, column *Arms*;
   § Numerical resolution and tail support, column *Arms match*). This is why
   conclusion 3 reverses what the sweep found, and it is why no pilot estimate in
   this survey is an independent earlier sample of a confirmed cell. The
   consequences are in [Arm fidelity](#arm-fidelity).

## What production takes, and the rule that decided it

| Width, in `u64` words | Route the tree takes | Rule and evidence |
|---|---|---|
| below the bit-backend SIMD threshold | the scalar backend's portable count (`ops-scalar-retained`, `scalar-count-ones`) | REQ-10: a no-win region retains its established implementation. Tables § Cells: `popcount-confirmation`, row `popcount-w4-dispatch`, decision `regressed`, outcome **fail** |
| at and above that threshold | the established AVX2 nibble lookup (`ops-nibble-resolver`, `ops-route-report`, `legacy-bundle-popcount`) | P-20 and the measurement contract: `popcount-confirmation` records `qualifies: false`, so passing cells do not authorize a partial family adoption |
| below the bit-backend SIMD threshold, fused consumers | the scalar fused fallback (`backend-simd-threshold`) | REQ-10: the automatic route retains its established implementation |
| at and above that threshold, fused consumers | the established fused nibble kernel (`ops-fused-nibble-resolver`, `matvec-established-fused`) | P-20 and the measurement contract: `fused-confirmation` records `qualifies: false`; the SIMD matrix path already counts the row intersection without a temporary buffer |
| direct comparator calls only | scalar `POPCNT` and the unfused and fused carry-save kernels (`bundle-scalar-popcnt-comparator`, `bundle-csa-comparator`, `bundle-fused-csa-comparator`) | Negative results remain reproducible without placing a nonqualifying candidate in an automatic production route |

The candidate kernels stay in the bundle as measured direct comparators that no
automatic resolver selects (`bundle-scalar-popcnt-comparator`,
`bundle-csa-comparator`, `bundle-fused-csa-comparator`), so the conformance suite
and this survey keep exercising them. The exact 255/256/257 regression cases in
that suite guard the absence of an unsupported production cutover; they are
untimed behavioral checks, not receipt evidence for a crossover.

Neither family qualifies for production selection by the tool's own flag: both
confirmations record `qualifies: false` (tables § Campaigns, column
*Qualifies*), because each holds at least one cell the evaluator did not pass.
No per-cell override exists in the contract or protocol. Production therefore
retains the established implementation over the full automatic-dispatch domain.

## Arm fidelity

A `gf2-core` consumer reaches a kernel through a function pointer the bundle
loads once into process-wide state, so every call is indirect and no caller sees
through it. An arm built in the survey's own link-time-optimised unit does see
through it: the optimiser resolves each bundle field to its kernel and emits a
direct call. The survey's library arms therefore place an optimisation barrier
around the bundle, so that an arm spelling a library route measures that route
rather than a devirtualised copy (`survey/gf2-side/src/arms.rs`, the `simd_fns`
barrier).

Only the two confirmations measure the arms with that barrier. Every sweep and
every pilot, including the two pilots the confirmation addenda pin as resolution
evidence, measures arms without it; the arm digests in tables § Campaigns,
column *Arms*, separate the two groups, and § Numerical resolution and tail
support records the mismatch per confirmation. Three consequences follow, and
none is repaired by re-measuring, because P-20 admits no further confirmatory
cell in either family
([Family accounting](#family-accounting-and-what-a-second-attempt-admits)):

- The confirmations are the deciding trials and they measure the call shape a
  consumer has. Their family-level nonqualification permits no production
  adoption.
- A pilot estimate is not an independent earlier sample of the confirmed cell of
  the same name. Pilot and confirmation agreement is therefore not evidence in
  this survey, and the tables state no agreement row.
- Where the two disagree, both stand. The sharpest case is
  `popcount-w4-dispatch`: the pinned pilot's derivation record lists it as this
  family's widest relative half-width and as an improvement
  ([`pilot-resolution-popcount.txt`](pilot-resolution-popcount.txt)), while the
  confirmation records it as `regressed` (tables § Cells:
  `popcount-confirmation`). The direction is what the barrier changes, and the
  sweep shows the same route winning when called directly (§ Cells:
  `popcount-sweep`). Production retention follows the confirmation's
  family-level result.

P-03 is satisfied as it is written: resolution evidence is a distinct,
digest-matched pilot receipt of the same family whose widest relative half-width
supports the declared resolution, which the acceptance tool verified for both
confirmations (tables § Campaigns, column *Verdict*, and § Numerical resolution
and tail support). The declared resolution bounds the bootstrap's own endpoint
stability rather than the arms' behaviour, and no cell of either confirmation is
recorded `not-confirmatory` (same section, last column). The protocol says in the
same breath that a stable endpoint cannot repair an unrepresentative experiment;
that caution is the reason this section exists.

## Evidence

### Receipts

Tables § Campaigns lists every committed receipt under
`../../bench_results/5cbb6545/`, in the order
[`run-count-campaign.sh`](run-count-campaign.sh) runs its stages, with the family
attempt its own frozen ledger prefix makes it, the reserved comparisons, the
corrected alpha, the P-20 tail support, the frozen resolution and margins, the
verdict, the acceptance finding count and the arm digests. Each receipt directory
holds `receipt.json`, `acceptance-summary.{json,md}`, `plan.json`,
`launcher.log`, `execution.log`, the checkpoint manifest and the `inputs/`
snapshots of protocol, contract, addendum schema, family addendum, frozen ledger
prefix and producing inputs. The family ledgers are
[`popcount-route-selection-family-ledger.jsonl`](../../bench_results/5cbb6545/popcount-route-selection-family-ledger.jsonl)
and
[`fused-count-consumers-family-ledger.jsonl`](../../bench_results/5cbb6545/fused-count-consumers-family-ledger.jsonl).

The addenda `addendum-{popcount,fused}-v4-{sweep,sweep2,pilot,confirmation}.json`
beside this file were frozen before their campaigns. Each confirmation addendum
was derived from its committed pilot receipt by the canonical freezer
`dev/active/c7113c5a/survey/freeze-confirmation.py`, which pins that receipt by
path and SHA-256, sets the resolution from its widest relative half-width and
raises a margin that does not strictly exceed one plus that resolution; the
derivation records are
[`pilot-resolution-popcount.txt`](pilot-resolution-popcount.txt) and
[`pilot-resolution-fused.txt`](pilot-resolution-fused.txt), and the fused record
names the margin it raised. Tables § Numerical resolution and tail support checks
that each record pins the same pilot receipt and digest as its addendum.

Every cell of every receipt ran on one core under the protocol's shared window
settings, with no flagged window (tables § Cells, columns *Flagged*, and the
`resolved_cpus` field of each summary). Each § Cells header carries its
campaign's toolchain, host, kernel and the load average at its host observation.
The selected operations are leaf count functions with no internal worker pool:
popcount handles one buffer, fused AND-popcount handles one pair, and the matrix
consumer invokes that fused function once per row. Six-core, twelve-core, or
twenty-four-logical-CPU arms would therefore measure an added scheduling layer,
not another route implemented or selected by this issue; multicore orchestration
belongs to the higher-level consumer that batches independent calls.
[`producing-inputs.json`](producing-inputs.json) names every file whose bytes the
receipts snapshot, and
[`make-producing-inputs.py`](make-producing-inputs.py) writes it.

### Arms, pins and builds

One release binary serves every arm and selects the route from `GF2_COUNT_ARM`,
so a measured ratio attributes to the route rather than to two builds; each
receipt's `arms` map holds the description, environment and executable digest of
every arm it names.

| Arm | Runs | Role |
|---|---|---|
| `legacy-dispatch` | the route `ops::popcount` takes without this issue: the scalar backend below the threshold, the bundle's nibble lookup at and above it | popcount baseline |
| `resolved-dispatch` | the route selected by the candidate-producing receipt snapshot, resolved for the buffer's width | popcount candidate; the receipt snapshot is evidence, not the retained production selection |
| `nibble-lut`, `scalar-popcnt`, `csa` | the bundle's `avx2_popcnt`, `popcnt_words` and `avx2_popcnt_csa` called directly, with no threshold | internal controls, the sweeps' crossover arms |
| `compiler-count-ones` | the portable `u64::count_ones` loop | internal control, named only by the smoke stage |
| `libpopcnt` | libpopcnt v4.2 `popcnt()` with its own CPUID dispatch, compiled `-O3` [Libpopcnt2026] | external arm |
| `mula-avx2-harley-seal` | sse-popcount `popcnt_AVX2_harley_seal` [Mula2018], which loads through `const __m256i*` and so takes only vector-aligned windows | external arm |
| `and-legacy-fused` | the bundle's `avx2_and_popcnt`, the route a fused consumer takes without this issue | fused baseline |
| `and-resolved-fused` | the route selected by the candidate-producing receipt snapshot | fused candidate; the receipt snapshot is evidence, not the retained production selection |
| `and-csa-fused` | the bundle's `avx2_and_popcnt_csa` called directly | internal control |
| `and-two-pass` | the public route without a fused kernel: temporary, `and_inplace`, `popcount`, all inside every call | whole-consumer comparator |
| `and-scalar-control` | a single-pass portable `(a & b).count_ones()` loop | internal control, named only by the smoke stage |
| `matvec-legacy`, `matvec-resolved` | the row loops in the baseline and candidate-producing receipt snapshots, output allocation included | whole-consumer arms |

The smoke stage names every arm identity once through the real runner, so the
wire contract between runner and arm child is established by an execution rather
than by reading code; its receipt is a throwaway by design and only its ledger
and launcher log are committed
([`smoke-family-ledger.jsonl`](../../bench_results/5cbb6545/smoke-family-ledger.jsonl),
[`smoke-launcher.log`](../../bench_results/5cbb6545/smoke-launcher.log)). Two
arms are named there and by no committed cell, as their rows above state.

Each launcher log records the external compile commands and compiler versions on
its *external build* line. The external sources live under the primary checkout's
`.agents/ext/5cbb6545` and are shared; the receipts pin what was built from them.

## Routes from the public entry points to the kernels

`BitVec::count_ones` uses the established backend dispatch for the buffer it
holds (`bitvec-count-ones`). `ops::resolve_popcount` and
`ops::resolve_and_popcount` expose the same scalar-below-threshold and
nibble-at-or-above-threshold selection for callers that hoist a function
pointer out of a loop (`backend-simd-threshold`, `ops-nibble-resolver`,
`ops-fused-nibble-resolver`). `BitMatrix::matvec` uses scalar row parity below
that boundary; its SIMD path loads the bundle once and calls the established
fused nibble kernel for each row (`matvec-entry`, `matvec-established-fused`).

The only automatic boundary is the bit-backend SIMD threshold
(`backend-simd-threshold`). There is no carry-save tuning field or 256-word
selector. Scalar `POPCNT` and the two carry-save kernels remain bundle
comparators reachable through direct function-pointer fields, but no production
resolver selects them (`bundle-scalar-popcnt-comparator`,
`bundle-csa-comparator`, `bundle-fused-csa-comparator`). Every kernel route is
reachable only through the non-default `simd` feature of `gf2-core`
(`core-simd-optional`); without it, and on a host whose bundle is absent, every
width takes the scalar count.

The route each width resolves to is observed rather than inferred:
`ops::popcount_route` and `and_popcount_route` report it at run time
(`ops-route-report`), the shared suite asserts the retained route at 255, 256
and 257 words, and the arm verifier records it at receipt widths including 256
([`validation-arms.txt`](validation-arms.txt), the *gf2 route* lines). Every
receipt pair records the route its candidate-producing executable took in
`selected_path` (tables § Cells, column *Route*). Those committed receipt values
remain unchanged as provenance for the negative family verdicts.

## Input coverage (REQ-07)

The sweeps cover the sizes and regimes, and their rows are in tables § Cells:
`popcount-sweep`, `popcount-sweep2`, `fused-sweep` and `fused-sweep2`.

- **Small inputs and the vector boundary**: one, two, four and seven words
  against the sub-threshold route, and eight, twelve and sixteen words against
  the vector kernel (`sweep-popcount-w{1,2,4,7}-scalar-popcnt`,
  `sweep-popcount-w{8,12,16}-nibble-vs-scalar`).
- **The carry-save block boundary**: one block is sixteen AVX2 vectors, or
  sixty-four words and 512 bytes (`csa-block-vectors`, `csa-block-stride`). The
  sweeps cover one-quarter, one-half and one through six blocks, including the
  exact boundary (`sweep-popcount-w{16,32,64,96,128}-csa`,
  `sweep2-popcount-w{160,192,224,256,320,384}-csa`, and the fused counterparts).
- **Cache regimes**: a 2 KiB buffer that fits L1, a 128 KiB buffer that exceeds
  it, a 1 MiB buffer, and a streaming cell whose working set rotates through the
  eight fixture banks of the protocol's streaming state
  (`sweep-popcount-w1m-streaming-csa`, cache state `streaming`). The protocol
  states that rotation and the declared working-set size are established while
  eviction from any level is not; at that size the carry-save route's advantage
  falls to `not-worse`, so the gain it buys is a cache-resident effect.
- **Alignment offsets**: a window three words past a vector boundary
  (`sweep-popcount-w1024-off3-csa`), with the full offset matrix in the
  conformance run. Both carry-save kernels load unaligned
  (`csa-unaligned-loads`), so no alignment precondition reaches a caller; Muła's
  reference does have one and refuses the misaligned windows, which
  [`validation-arms.txt`](validation-arms.txt) counts.
- **Bit patterns**: all-one and all-zero words beside random ones
  (`sweep-popcount-w1024-{ones,zeros}-csa`). The carry-save decision is the same
  for all three, as a data-independent kernel requires.
- **External references on identical buffers**: libpopcnt and Muła at 1024 and
  16384 words
  (`sweep-popcount-w{1024,16384}-vs-{libpopcnt,mula-avx2-harley-seal}`), and
  libpopcnt again as a confirmatory comparator cell.

## Correctness (REQ-08)

[`run-validation.sh`](run-validation.sh) records both correctness groups in
[`validation.json`](validation.json), and the campaign launcher refuses to take
the benchmark mutex unless that record passes, so no timed window precedes it
(tables § Correctness coverage).

The shared suite `crates/gf2-core/tests/popcount_routes.rs` holds every route the
library can take to one reference count over the same buffers: the scalar
backend, both resolvers, and each bundle function pointer called directly. Its
lengths bracket the empty buffer, the sub-vector widths, the repository's
0/1/63/64/65 word boundaries and the carry-save block boundary, on random,
all-zero and all-one data, and it checks the fused routes over the shorter
operand, `BitVec::count_ones` for canonical bit indexing and zero tail padding,
and `BitMatrix::matvec` parity across strides that change the route. The survey's
own verifier additionally covers the external arms, their byte-length tails,
Muła's alignment precondition and the consumer arms against an independent
byte-table count; both raw outputs carry the case count of every group.

Exact counts hold at every width, so the three tiers of tail the kernels carry
are exercised rather than argued: the per-vector remainder loop
(`csa-vector-remainder`), the word tail of each vector kernel (`csa-word-tail`,
`lut-word-tail`) and the sub-vector widths that never enter a vector loop at all.

The established fused reduction already avoids a consumer temporary:
`BitMatrix::matvec` calls the fused bundle kernel directly
(`matvec-established-fused`). The `and-w4096-vs-two-pass` candidate cell measures
that whole-consumer benefit, but it does not override the fused family's
`qualifies: false` result. `run-validation.sh` also audits that neither automatic
entry point selects the comparator pointers and that no removed carry-save
tuning selector remains, and checks that the source-evidence generator exactly
reproduces its committed artifact (tables § Correctness coverage).

## Rates and consumer results (REQ-09)

Tables § Rates gives per-call latency, useful bytes per second and cycles per
byte for every arm of every confirmatory cell. The cycles-per-byte column is an
estimate at an observed clock rather than a counted cycle total: the receipts
count no cycles, so [`survey/observe-clock.py`](survey/observe-clock.py) runs one
arm child per confirmatory cell under `perf stat` and records cycles over
task-clock, with the fraction of the child its timed windows occupy (§ Clock
observation). That record establishes a clock and no comparison; it runs the
candidate-producing revision recorded in the observation. Its `selected_path`
values describe that revision and do not state the final production selection.

Consumer results carry their dispatch and setup costs. The whole-consumer cells
declare their conversion costs included, and tables § Conversion costs lists
every probe of every such cell and arm: the two-pass arm's per-call temporary
allocation, the matrix-vector arms' fixture construction, and the backend
selection one call performs. The per-call latency of a whole-consumer cell is
therefore a whole product, output allocation included, not a kernel in isolation.

The sweeps describe size-dependent candidate performance, and the confirmations
test fresh pairs at the corrected alpha. Neither confirmation qualifies its
family, so the canonical tuning mechanism gains no selector and production
retains the established SIMD threshold alone (`tuning-retained-selector`). The
exploratory 256-word crossover is reported but not promoted to a production
boundary.

## Emitted instructions, register lifetimes and dependency chains (REQ-10)

The claims below are lines of the committed asm artefacts beside the kernel
sources, `crates/gf2-kernels-simd/src/x86/asm/{popcount,avx2}.asm.txt`, whose
headers name the revision and toolchain each was recorded from.

- **The carry-save block loop trades lookups for bitwise triples.** One iteration
  loads sixteen vectors and advances 512 bytes (`csa-block-loop`,
  `csa-block-stride`, `csa-block-vectors`), folds them through carry-save adders,
  and looks up only the weight-16 register, so the block costs one nibble lookup
  where the established route costs sixteen. The established route's loop, by
  contrast, runs one lookup and one accumulator addition per 32 bytes
  (`lut-vector-loop`, `lut-accumulator`).
- **The accumulator set costs register lifetime.** Five carry-save registers and
  the running total live across the whole block loop, and the kernel spills six
  callee-saved registers to hold them (`csa-callee-saved-spill`). The per-vector
  kernel keeps one accumulator and spills none. This explains the exploratory
  regressions below 256 words: a buffer that runs few blocks pays the prologue,
  the spill and the remainder handling without amortising the saved lookups,
  which is what the sweep measures as a regression at every width up to 128 words
  (tables § Cells: `popcount-sweep`).
- **Three tiers of tail.** Below one block the carry-save kernel reaches only its
  per-vector remainder loop, which is the same nibble lookup the established
  route uses (`csa-vector-remainder`); below one vector both kernels fall to a
  word tail that counts with the SWAR multiply rather than `POPCNT`
  (`csa-word-tail`, `lut-word-tail`).
- **The fused kernel materialises nothing.** It ANDs the second operand as it
  loads the first, inside the same carry-save block (`fused-and-on-load`), so the
  fused route has no temporary buffer to allocate and no second pass to run,
  which is the difference `and-w4096-vs-two-pass` measures.
- **The scalar `POPCNT` kernel is a five-instruction loop with one dependency
  chain.** One `POPCNT` per word accumulates into a single register, with no
  unrolling and no second accumulator (`popcnt-words-symbol`,
  `popcnt-words-loop`). Called directly it beats the established count at every
  width below the threshold; reached through the bundle's pointer it does not
  (tables § Cells: `popcount-sweep` against `popcount-confirmation`, row
  `popcount-w4-dispatch`). At four words the indirect call and the resolution
  around it are a large share of the work, and this loop has no
  instruction-level parallelism to offset them, while the count it would replace
  is a single inlined expression over the same four words
  (`scalar-count-ones`).

The scalar `POPCNT` candidate therefore does not replace the established scalar
count below the bit-backend SIMD threshold. The carry-save families do not
qualify, so neither carry-save comparator enters automatic dispatch at any
width. The exploratory crossover explains a measured pattern; it is not a
canonical production cutoff.

## Family accounting and what a second attempt admits

Both families are on their first confirmatory attempt: each ledger holds one entry
that reserves comparisons, and each confirmation reserves six (tables
§ Campaigns, columns *Attempt* and *m*). `trial_ledger::attempt_alpha` divides
the frozen family-wise alpha by `attempts * (attempts + 1)`, counting the ledger
entries that reserved at least one comparison, and the acceptance tool divides
that by the reserved comparisons of the prefix, so the first attempt's corrected
alpha is the family-wise alpha over twelve, and P-20's tail support at ten
thousand resamples clears twenty expected draws per tail (tables § Campaigns,
columns *alpha_c* and *Draws per tail*, and § Numerical resolution and tail
support).

A second attempt of either family admits no confirmatory cell at all. Its attempt
count is two, so its attempt alpha is the family-wise alpha over six; the prefix
already reserves six comparisons, so a second attempt reserving `n` cells is
corrected by six plus `n`. P-20 requires ten thousand resamples times the
corrected alpha over two to reach twenty expected draws per tail, which bounds
the corrected alpha from below by four thousandths; a second attempt would need
the reserved comparisons of its prefix to stay under three to meet that bound,
and its prefix alone is six, so even one additional cell falls short by more than
a factor of three, and `receipt.rs` records every cell of such a campaign
`not-confirmatory`. The protocol's own rule follows: a family whose ledger
history admits no confirmatory cell records that as a preserved outcome, and no
non-confirmatory "confirmation" is launched. Independently of the alpha, the
ledger decoder rejects a second reservation of a candidate identity already
attempted at the same protocol version, so the same candidate could not be
re-confirmed even if the arithmetic allowed it.

This is why the arm-fidelity mismatch is reported rather than repaired, and why
the two `inconclusive` cells stay inconclusive.

## What is not established

- No statement here holds for another host, another toolchain or a build with ISA
  flags. Every receipt is a conservative portable build on the host its
  `receipt.json` records.
- The streaming cell establishes rotation through the declared working set, not
  eviction from any cache level; the protocol says so, and no cache-miss counter
  is read anywhere in this survey.
- The cycles-per-byte column converts a measured time at an observed clock. No
  cycle total of a timed window is counted.
- Pilot and confirmation estimates are not two samples of one experiment in this
  survey; see [Arm fidelity](#arm-fidelity).
- Nothing here measures an unfused consumer that hoists
  `resolve_popcount` out of a loop over equal widths. The matrix consumer uses
  the established fused bundle function directly.

## Criteria

| Criterion | Where it is met |
|---|---|
| REQ-01 (contract, receipts, before/after evidence, negative outcomes preserved) | Every receipt pins the contract, protocol and addendum by path and digest (tables § Method, § Campaigns); the baseline arm of every dispatch cell is the route without this issue; the `fail`, `not-material` and `inconclusive` outcomes are reported as recorded, above and in § Cells |
| REQ-07 (candidates against pinned libpopcnt and Muła on identical buffers, over sizes, boundaries, cache regimes, offsets and patterns) | [Input coverage](#input-coverage-req-07); the external arms in § Cells: `popcount-sweep` and the confirmatory comparator cell |
| REQ-08 (exact counts, canonical indexing, zero tail padding, fused reduction only where it replaces a consumer's buffer or pass) | [Correctness](#correctness-req-08); tables § Correctness coverage |
| REQ-09 (cycles per byte, useful bytes per second, per-call latency, consumer results with dispatch and setup costs; winners determined and independently confirmed through the canonical tuning mechanism) | [Rates and consumer results](#rates-and-consumer-results-req-09); tables § Rates, § Conversion costs, § Clock observation, § Campaigns; the independent confirmation outcome is nonqualification, so no new selector is installed |
| REQ-10 (register lifetimes, accumulator chains, scalar tails, emitted instructions; no-win regions retain their established implementation) | [Emitted instructions](#emitted-instructions-register-lifetimes-and-dependency-chains-req-10); the retained regions in [What production takes](#what-production-takes-and-the-rule-that-decided-it) |
| REQ-02 of the epic (measured outcomes for bit reductions; production selection includes only qualifying candidates) | [What production takes](#what-production-takes-and-the-rule-that-decided-it) reports `qualifies: false` for both confirmations and retains the established production selection over the complete automatic-dispatch domain |
| REQ-06 of the epic (zero tail padding, unsafe-kernel isolation, canonical abstractions, deterministic seeded execution) | The unsafe kernels stay confined to `gf2-kernels-simd` and are reached through the bundle; tail and indexing semantics are in [Correctness](#correctness-req-08); every workload word comes from the protocol's seeded generator, as each cell's `workload.seed` records |

Every `cites:` label of the issue appears here: [Libpopcnt2026] and [Mula2018].
