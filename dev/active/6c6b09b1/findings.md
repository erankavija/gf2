# Byte-field vector and matrix comparison arms

> **Diátaxis Type:** Explanation

## Result

The survey maps gf2's GF(2^8) vector and matrix consumer entry points onto the
operations M4RIE [Mfourrie2026], GF-Complete [GfComplete2026] and ISA-L
[IsaL2026] genuinely share with them, fixes the one field every compared arm
implements, and validates every adapter before timing. The three pinned
libraries and gf2's runtime GF(2^8) field all use the polynomial 0x11D; gf2
ships no GF(2^8) field modulo 0x11B, so no cell uses 0x11B and none needs a
basis conversion. A region multiply-accumulate, a matrix product and an
arbitrary pairwise product are different operations, and the survey keeps them
in three families with three ledgers. Each family carries a protocol-v4 pilot
over its whole declared breadth and a protocol-v4 confirmation over the six
cells the protocol's confirmatory budget admits; all six receipts are
committed, every declared cell of every one of them is measured, and the
evaluator accepts each (verdict and finding count on each receipt's header
lines in the generated tables). gf2 loses every confirmed region
multiply-accumulate cell and every confirmed dense-product cell to the pinned
libraries at the family confidence, and leads four of the six confirmed
pairwise-control cells; the generated tables carry each figure. The superseded
protocol-v1 pilot is kept as history. No production code changes.

## Question and evidence

`FieldVec` and `FieldMatrix` consumers hold field elements, not byte regions.
The survey asks which gf2 consumer operations have an equivalent in M4RIE,
GF-Complete and ISA-L once field polynomial, coefficient reuse, accumulation
and overlap are matched; how far current gf2 is from those libraries on this
Ryzen 9 5900X under the
[measurement contract](../1a379447-zen3-cpu-performance/measurement-contract.md)
and [protocol version 4](../f547c394/protocol.md), per cache regime,
kernel-isolated and as a whole byte-region consumer; and which cells gf2 loses,
as the baseline for the feasibility issue `19513245`. The issue excludes
adoption and any production change, so every cell is a `comparator-gap` cell
with the external library as candidate: with gf2 as baseline, a speedup of
medians below one means gf2 is faster.

| Command | Output |
|---|---|
| `survey/fetch-build.sh` | the pinned libraries, built from verified pins under the primary checkout's `.agents/ext/6c6b09b1/` |
| `survey/stage-externals.sh` | that prefix, checked against `survey/ext-prefix.sha256` and copied into `target/6c6b09b1-ext/` |
| `../../bench_results/6c6b09b1/run-byte-field-v4.sh build` | the C shim, conformance and provenance tools, both arm executables, the runner, and `conformance-v4/`: correctness evidence, `arm-provenance.txt`, `build-record.txt` |
| `survey/smoke-arms.sh` | every arm and every case operation carried through the real runner on a throwaway family under `target/`, before any campaign is queued |
| `../../bench_results/6c6b09b1/run-byte-field-v4.sh plan\|window CAMPAIGN` | the v4 campaigns; each receipt's `launcher.log` records every session command. A campaign that completes holding no cell with paired executions fails the job (exit 4) instead of passing as a clean run; a cell the host makes inapplicable is named, not failed |
| `../../bench_results/6c6b09b1/run-byte-field-v4.test.sh` | the launcher's outcome guards over synthesized journals and receipts, alongside the real failed v3 stage and the committed v1 receipt |
| `survey/make-addenda.py`, `survey/make-plan-versioned.py`, `survey/make-producing-inputs.py` | the pilot addenda, the runner plans and the producing closure `survey/producing-inputs-v4.json` |
| `survey/make-confirmation-addenda.py` and its `.test.sh` | the three confirmation addenda, frozen from the committed pilot receipts, and the guards that keep that freeze reproducible |
| `survey/make-source-evidence.py` | `survey/source-evidence.json`: every code claim below, cited by claim ID in backticks |
| `survey/make-ledger-origin-v3.py` | `../../bench_results/6c6b09b1/v3-*-ledger-origin.json` |
| `survey/make-void-attempt-record.py` | `../../bench_results/6c6b09b1/v3-r1-void-attempts.json`: what each ledger attempt that measured no cell actually did |
| `survey-analysis tables` and `survey-analysis resolution` (`survey/analysis/`) | [the generated tables](../../bench_results/6c6b09b1/tables.md) and each family's pilot-derived measurement resolution |

This report states no measured value. Each conclusion points to its source: a
section of [the generated tables](../../bench_results/6c6b09b1/tables.md),
written "tables § heading" with the row named by its cell ID; a receipt or
evidence file; or a claim ID. The interval methods are stated in the tables
file and in `survey/analysis/src/tables.rs`.

The host is `fraktaali`, a Ryzen 9 5900X; each receipt records its CPU model,
kernel, governors, SMT state and CPU mask per session. Timed work runs only
under `dev/scripts/ccx1-bench-flock.sh --full-host`, builds and checks under
`scripts/cargo-budget.sh`. Arms, runner and acceptance tool are Rust 1.95
release builds; the libraries and the shim are built by GCC with
`-O3 -march=native` (`conformance-v4/arm-provenance.txt` § Compiler
targeting). Seeds: each campaign seed in the launcher drives the
counterbalanced pair order and the acceptance bootstrap through
`Xoshiro256StarStar` seeded by `SplitMix64` [BlackmanVigna2021] [Steele2014],
both in tuning-campaign-support 0.1.0
(`dev/tools/tuning-campaign-support/src/abtest.rs`); each cell's workload seed
in its addendum generates the operands with that crate's `SplitMix64` through
`OperandStream` (`survey/arm-common/src/lib.rs`); the C conformance binary uses
its own SplitMix64 stream with the fixed seed set in `main` of
`survey/byte_field_conformance.c`.

## Arms and pins

| Arm | Pin | Licence (evidence) | Selected backend on this host |
|---|---|---|---|
| ISA-L [IsaL2026] | tag and commit in `survey/fetch-build.sh` | BSD-3-Clause (`isal-license-clause-3`) | runtime-dispatched AVX2 region and encode kernels; table preparation and `gf_mul` are its base C code |
| GF-Complete [GfComplete2026] | Ceph-mirror commit in `survey/fetch-build.sh` | BSD-3-Clause (`gfcomplete-license-clause-3`) | default `w = 8` configuration: SSE split-table region kernel, `gf_w8_default_multiply` for single elements |
| M4RIE [Mfourrie2026] over M4RI [AlbrechtBard2026] | release tarballs by version and SHA-256 in `survey/fetch-build.sh` | GPL-2.0-or-later (`m4rie-license-header`, `m4rie-license-version`, `m4rie-license-readme`; M4RI `m4ri-license-header`, `m4ri-license-version`, the label sibling survey `6fb89a3c` records) | table multiply `_gf2e_mul_table`; `mzed_mul` recursion recorded per shape; no OpenMP (`m4ri-no-openmp`) |

`stage-externals.sh` re-verifies every pin and every installed library and
header before a build. `conformance-v4/arm-provenance.txt` records the pins,
licence-file digests, compiler targeting, the digests of the libraries the
external arm links, and the kernels each library selects, read from the loaded
binaries by `survey/backend_provenance.c` (§ Selected arithmetic backends,
observed at run time). gf2 is MIT-licensed (`LICENSE-MIT`); GF-Complete and
ISA-L are permissive, while M4RIE and M4RI are GPL-licensed and serve here only
as comparators.

## One field: 0x11D

GF(2^8) modulo 0x11D = x^8+x^4+x^3+x^2+1 is the only field all compared arms
share, and it is gf2's own:

- `Gf2mField::gf256()`, the runtime field every `Gf2mElement` consumer gets,
  builds 0x11D (`gf2-gf256-polynomial`), and the primitive-polynomial
  catalogue's degree-8 entry is 0x11D (`gf2-catalogue-degree-8`).
- The compile-time `Gf2mWide` takes its polynomial from a caller-supplied
  configuration (`gf2-wide-config-trait`). Every GF(2^8) configuration in gf2's
  crates sits in a test module (`gf2_gf256_wide_config_scan` in the source
  evidence), and the tests and the gemm benchmark use 0x11B
  (`gf2-wide-test-config-0x11b`, `gf2-wide-bench-config-0x11b`). gf2 therefore
  ships no GF(2^8) field modulo 0x11B. The survey declares its own 0x11D
  configuration, `Gf256x11d` (`survey/gf2-side/src/workload.rs`), which runs the
  same `Gf2mWide` code with a different reduction constant.
- ISA-L compiles 0x11D in (`isal-polynomial`) and the shim refuses any other
  polynomial for it. GF-Complete defaults to 0x11D
  (`gfcomplete-default-polynomial`) and M4RIE takes the polynomial as a
  parameter (`m4rie-init`); the shim passes 0x11D to both.

0x11B and 0x11D are distinct fields on the same byte carrier: both conformance
runs exhibit a product that differs between them (the distinctness lines of
`conformance-v4/externals.txt` and `conformance-v4/gf2-side.txt`). A 0x11B
consumer could use GF-Complete or M4RIE, whose full 0x11B multiplication tables
pass against the independent oracle (`conformance-v4/externals.txt`), or gf2
through `Gf2mField::new(8, 0x11B)` (`conformance-v4/gf2-side.txt`); ISA-L
refuses 0x11B. No pinned library offers a change of polynomial basis, since
M4RIE's conversion module changes storage layout within one field
(`m4rie-conversion-scope`), so a 0x11B consumer reaching ISA-L would need an
isomorphism adapter that this survey neither builds nor times. The
protocol-v1 design declared a 0x11B cell on the premise that gf2's
compile-time GF(2^8) aliases were a shipped 0x11B field; they are test code,
and the current design has no such cell.

The rustdoc of `gf256()` names x^8+x^4+x^3+x+1 and calls the field the one
"used in AES" (`gf2-gf256-doc-polynomial`, `gf2-gf256-doc-aes`) while the code
builds 0x11D, and the catalogue comment calls the five-term polynomial a
trinomial. Both are documentation defects in gf2-core, tracked as `835f34f0`;
this issue changes no production file.

## Operation mapping

| gf2 entry point (arm) | Operation | ISA-L | GF-Complete | M4RIE |
|---|---|---|---|---|
| `FieldVec::axpy` (`element`, `wide`) | y[i] += a·x[i], one coefficient over the region (`gf2-axpy-signature`) | `gf_vect_mad` over `ec_init_tables` (`isal-vect-mad`, `isal-init-tables`) | `multiply_region.w32`, accumulating (`gfcomplete-region`) | `mzed_add_multiple_of_row` on one-row matrices (`m4rie-row-axpy`) |
| `field::matrix::gemm`, square (`element`, `wide`) | C = A·B (`gf2-gemm-entry`) | none | none | `mzed_mul` (`m4rie-mzed-mul`) |
| `field::matrix::gemm`, 4x10 by 10x65536 | C[r][i] = Σ_j G[r][j]·D[j][i] | `ec_encode_data` over `ec_init_tables` (`isal-encode`) | none | `mzed_mul` at that shape |
| `gf2m::batch::batch_mul` (`batch`) | z[i] = x[i]·y[i], no coefficient reuse (`gf2-batch-mul-entry`) | `gf_mul` per byte (`isal-gf-mul`) | `multiply.w32` per byte | `gf2e_mul` per byte (`m4rie-scalar-mul`) |
| `FieldVec::dot_product` | Σ x[i]·y[i], one element | none | none | none |

The table rests on these facts:

- Accumulation is XOR in every arm, and every arm reads a source distinct from
  its destination, which `FieldVec::axpy` enforces by its borrows
  (`gf2-axpy-signature`).
- ISA-L's region call needs at least 64 bytes (`isal-vect-mad-length`); every
  region cell is larger, and the conformance run records the rejection below
  the minimum.
- Neither ISA-L nor GF-Complete has a dense matrix type. M4RIE is the only
  square-product comparator, and ISA-L's encode is a matrix product only at the
  generator shape, which has its own cells.
- No library has a region kernel for distinct pairs. Each pairwise arm is the
  library's public single-element multiply applied per byte, labelled
  `element-multiply-per-byte` in its selected path: a log/antilog lookup for
  ISA-L (`isal-gf-mul-tables`), an indirect call for GF-Complete
  (`conformance-v4/arm-provenance.txt`), an inline lookup in a 256 by 256 table
  of 64-bit words for M4RIE (`m4rie-mul-table`, `m4rie-word`). A pairwise
  result compares gf2's batched kernel with per-element calls, not with a
  region kernel.
- ISA-L's `gf_vect_dot_prod` computes one dot product per byte position across
  source regions and returns a region (`isal-dot-prod-doc`); it is not
  `FieldVec::dot_product`, which has no equivalent, so no cell measures either.
- A matrix product and a region multiply-accumulate never share a cell or a
  family.

Within gf2, `FieldVec::axpy` runs its element loop for both GF(2^8)
representations, because only `Fp<P>` overrides the kernel hook
(`gf2-axpy-simd-hook`, `gf2-axpy-default-false`, `gf2-axpy-fp-override`,
`gf2-axpy-element-loop`). `gemm` allocates its product in every call and
transposes B first (`gf2-gemm-fresh-output`, `gf2-gemm-transpose`);
`Gf2mWide<1, _>` then takes the whole product through the dispatched GEMM
kernel or its scalar panelized fallback (`gf2-wide-gemm-kernel`,
`gf2-wide-gemm-fallback`), while `Gf2mElement` computes each output cell as a
batched dot product (`gf2-gemm-per-cell-hook`, `gf2-element-batch-dot`).
`batch_mul` reaches the dispatched batch kernel (`gf2-batch-mul-dispatch`), and
the gf2 arm enables the `simd` feature that gates dispatch
(`gf2-simd-feature`). Every gf2 execution names in its selected path the route
these branches take given the kernels it finds available at run time (tables §
Selected paths).

## Representations and consumer costs

The operations match; the representations do not. `FieldVec<Gf2mElement>`
stores a u64 value and a reference-counted field handle per element
(`gf2-element-value`, `gf2-element-params`), sixteen times the byte region it
stands for, and clones the handle for every element it creates.
`Gf2mWide<1, _>` and `batch_mul`'s lanes store one u64 per element
(`gf2-wide-words`), eight times the byte region. ISA-L and GF-Complete work on
the byte region itself; M4RIE works on its `mzed_t` matrices, which the shim
fills and reads element by element (`bfx_mat_pack` and `bfx_mat_unpack` in
`survey/byte_field_ext.c`). A cell's size names the byte region both arms start
from, so gf2's working set leaves each cache level before the comparator's
does; that asymmetry is what the cells measure.

A kernel-isolated cell times the call on operands already in the arm's
representation. A whole-consumer cell starts and ends at the byte region: gf2
converts both operands into the `FieldVec` or `FieldMatrix` it keeps and
converts the result back, `batch_mul` widens to lanes and narrows back, M4RIE
packs and unpacks its matrices, ISA-L prepares its coefficient or generator
tables in every call, and GF-Complete, which expands the coefficient inside its
region call, adds nothing. Every execution also reports its setup, packing,
unpacking and table-preparation costs, each measured outside the timing windows
as the median of repeated probes (`probe_ns` in `survey/arm-common/src/lib.rs`),
so the composition of a whole-consumer cost is visible; `dispatch_ns` is zero
because no arm selects an implementation outside its call (tables § Conversion
and setup costs, § Whole-consumer over kernel-isolated).

## Correctness before timing

The launcher's `build` step stops at the first failed check, and each
campaign's producing snapshot pins the evidence it wrote
(`survey/producing-inputs-v4.json` lists `conformance-v4/`):

- `conformance-v4/externals.txt`: every backend through the C shim against an
  independent shift-and-reduce oracle (`bfx_ref_mul`): full multiplication
  tables; region multiply-accumulate at the byte boundary lengths and at the 4
  KiB and 128 KiB cell lengths; the M4RIE row form; pairwise products; dense
  products at boundary and cell dimensions and at the encode shape; the ISA-L
  encode; and the 0x11B refusal, tables and distinctness witness.
- `conformance-v4/ext-wrapper.txt`: the region, pairwise, product and encode
  paths through the Rust wrapper the external arm calls.
- `conformance-v4/gf2-side.txt`: both gf2 representations through the arm's
  own conversions, `FieldVec::axpy` for every coefficient, `gemm` at boundary
  and cell shapes on the route the arm records, `batch_mul`, and the 0x11B
  field, against an oracle written without gf2-core.
- `conformance-v4/field-laws-element.txt` and
  `conformance-v4/field-laws-wide.txt`: the shared field-law suite,
  `test_field_axioms`, over `Gf2mField::gf256()` (gf2-core's
  `test_gf2_8_field_axioms`) and over `Gf2mWide<1, Gf256x11d>`
  (`survey/field-laws`).

Each file ends in its check count with no failure, or in its pass line.

The arms also have to speak the runner's wire. `survey/arm-common` decodes the
request `benchmark-ab-runner` writes on each child's stdin and writes the one
canonical result line it reads back, and the shared transport rejects a request
whose re-encoding differs from the bytes it received, so the field names, their
order and every wire spelling are part of the contract. The `role` field of a
request names the arm's position in the A/B pair, not the cell's sampling
classification. The `wire` tests in `survey/arm-common/src/lib.rs` pin the
contract from both ends: a request the runner emits, round-tripped, and a scan
of the runner's own `ArmRequest` declaration that fails when the runner gains,
loses or reorders a field. Those tests read declarations; `survey/smoke-arms.sh`
runs the wire. It measures a throwaway family on a throwaway ledger under
`target/`, at the smallest pair count the protocol allows, over four cells that
between them name all six arms and all four case operations, and it fails
unless every arm wrote result lines into a paired cell. It is a precondition of
queueing a campaign, because the version-3 attempts died on their first arm in
thirteen milliseconds on a wire defect that reading the code did not catch.

## Families and campaigns

Three questions, three families, each with its own append-only ledger that
carries every attempt of that question under every protocol version:

- `byte-field-region-axpy` ([pilot addendum](addendum-v4-region-axpy-pilot.json),
  [confirmation addendum](addendum-v4-region-axpy-confirmation.json)):
  fixed-coefficient region multiply-accumulate per cache regime (4 KiB, 128 KiB
  and 8 MiB warm; 2 MiB regions rotated through eight banks), kernel-isolated
  and whole-consumer, for both gf2 representations. The pilot also compares
  ISA-L with GF-Complete and M4RIE, and the two representations with each other.
- `byte-field-matrix-product` ([pilot addendum](addendum-v4-matrix-product-pilot.json),
  [confirmation addendum](addendum-v4-matrix-product-confirmation.json)):
  dense products at n = 64, 256 and 512, which spans M4RIE's switch to its
  bitsliced Karatsuba path (the `mzed_mul` lines of
  `conformance-v4/arm-provenance.txt`), and the encode shape against ISA-L,
  kernel-isolated and whole-consumer.
- `byte-field-pairwise-control` ([pilot addendum](addendum-v4-pairwise-control-pilot.json),
  [confirmation addendum](addendum-v4-pairwise-control-confirmation.json)):
  arbitrary pairwise products against each library's per-byte multiply at 4 KiB
  and 128 KiB, kernel-isolated and whole-consumer.

Every cell is single-core: none of the measured calls has a parallel form in
these builds, so a 6-, 12- or 24-CPU arm would time a harness thread pool; each
addendum states this. Each ledger opens with the retrospective zero-comparison
restatement of the protocol-v1 pilot, which measured one cell of each
operation; the origin records derive it and list the two v1 launches that
stopped before a campaign opened
(`../../bench_results/6c6b09b1/v3-*-ledger-origin.json`). Each ledger then
carries the version-3 pilot attempt, which reserved its campaign, failed on
its first arm and measured no cell
(`../../bench_results/6c6b09b1/v3-r1-void-attempts.json`); the version-4
pilot, which measured every cell it declared; and the version-4 confirmation,
the first line of the chain to spend comparisons.

### What the confirmations confirm

A pilot addendum declares only exploratory cells, so each of those three
reservations spends zero comparisons and names no candidate identity. No line
ahead of a confirmation therefore spends a comparison, and each family's
confirmation is its first confirmatory attempt: `attempt_alpha` in
`dev/tools/tuning-campaign-support/src/trial_ledger.rs` counts reservations
whose comparison count is positive, finds none, and allocates the first
attempt's budget, which Bonferroni then divides by the attempt's own
confirmatory cell count. P-20 additionally requires at least twenty expected
bootstrap draws in each tail at that corrected confidence, and the frozen
`bootstrap_resamples` fixes the draws, so a family that declares more than six
confirmatory cells would report every one of them `not-confirmatory`. Each
confirmation therefore declares six cells and leaves the rest of its pilot
breadth as exploratory evidence; `survey/make-confirmation-addenda.py` states
which six each family confirms and what they cover, and so does each
addendum's family description.

Each confirmation is frozen from its own family's committed pilot receipt
before any confirmatory trial: the measurement resolution is that pilot's
widest relative bootstrap half-width at the corrected alpha its own frozen
addendum and ledger imply, rounded up to six decimal places
(`survey-analysis resolution-freeze`, the quantity P-03 recomputes); the
resolution evidence pins that pilot receipt by content; and the cells are the
pilot's own cells with the role changed to `confirmatory`, so a confirmed cell
measures the workload, size, seed, cache state, metric kind and arms its pilot
measured. The margins are the family's own, declared with the pilot and
unchanged, and each strictly exceeds one plus its family's frozen resolution,
which the addendum validation requires. The campaign seed is fresh, so the pair
order and the bootstrap stream are; the workload seeds stay the pilot's, so the
confirmation measures the workload whose resolution it declares.
`survey/make-confirmation-addenda.test.sh` runs the real freeze over a fixture
tree and shows it writes the same bytes once a confirmation receipt, or a
pilot-labelled campaign relaunched from the confirmation addendum, sits beside
the pilot.

The confirmation receipts need one reading stated, because the outcome
vocabulary is written for adoption decisions and these cells decide no
adoption. Every cell has gf2 as baseline and the library as candidate, and the
decision rule maps an interval below the reciprocal of the equivalence margin
to `regressed`, which the receipt records as the outcome `fail` whatever the
cell's objective (`abtest::decide` and the outcome match in
`dev/tools/tuning-campaign-support/src/receipt.rs`). In a `comparator-gap` cell
that outcome means the external library is more than the equivalence margin
slower than gf2 at the family confidence: it is gf2 winning the cell, not gf2
failing it. The pairwise control is where that applies, since gf2 leads most of
its cells, so its confirmation receipt carries `fail` outcomes and does not
qualify; `qualifies` requires every confirmatory cell to pass
(the `qualifies` computation in `receipt.rs`), which on such a receipt would
mean only that every comparator is materially ahead. The region and matrix
families read the ordinary way round, because the library leads every one of
their cells.

## Results

### Protocol-v4 pilots

In each of the three pilots every declared cell starts, checkpoints and
completes exactly once across the campaign's sessions, on the launcher's
exploratory pair count with the frozen window count; each campaign ends in its
own `complete` terminal record; and
`benchmark-acceptance` recomputes each receipt to the verdict, finding count
and `qualifies` false its header lines in the tables carry, which is what an
exploratory-only receipt is
(tables § `6c6b09b1-v4-r1-region-axpy-pilot`,
§ `6c6b09b1-v4-r1-matrix-product-pilot`,
§ `6c6b09b1-v4-r1-pairwise-control-pilot`, each receipt's
`acceptance-summary.json`). No cell of any of the three is unstable: the
flagged-window counts are zero throughout (tables § Cells, the flagged-windows
column of each receipt).

**Region multiply-accumulate.** ISA-L's `gf_vect_mad` is ahead of
`FieldVec::axpy` in every cell and in every cache regime, for both gf2
representations and both metric kinds (tables §
`6c6b09b1-v4-r1-region-axpy-pilot` § Cells, the twelve `*-vs-isal` rows). The
gap is not uniform across the regimes: for `Gf2mElement` it is widest in the
4 KiB warm cell and narrowest in the 2 MiB streaming cell
(`axpy-4k-element-vs-isal` against `axpy-2m-stream-element-vs-isal`), and the
same ordering holds for `Gf2mWide`. Comparing the two gf2 representations
directly, `FieldVec<Gf2mElement>::axpy` is the faster of the two
(`axpy-128k-element-vs-wide`), which is the opposite of the dense-product
result below. Among the external arms ISA-L is the fastest at both region
sizes, GF-Complete second and M4RIE's row form last
(`axpy-4k-isal-vs-gfcomplete`, `axpy-4k-isal-vs-m4rie`,
`axpy-128k-isal-vs-gfcomplete`, `axpy-128k-isal-vs-m4rie`); that ordering is
what makes ISA-L the comparator every confirmatory region cell uses.

**Dense products.** M4RIE's `mzed_mul` is ahead of `field::matrix::gemm` at
every square dimension, for both gf2 representations, and the gap widens with
the dimension (tables § `6c6b09b1-v4-r1-matrix-product-pilot` § Cells, the
`matmul-n64`, `matmul-n256` and `matmul-n512` rows of each representation).
`FieldMatrix<Gf2mWide<1, Gf256x11d>>` is the faster gf2 representation here
(`matmul-n256-element-vs-wide`), because it takes the whole product through
the dispatched GEMM kernel while `Gf2mElement` computes each output cell as a
batched dot product (tables § Selected paths). At the generator-encode shape
ISA-L's `ec_encode_data` is ahead of both gf2 representations
(`encode-k10r4-64k-element-vs-isal`, `encode-k10r4-64k-wide-vs-isal`) and far
ahead of M4RIE at that shape (`encode-k10r4-64k-isal-vs-m4rie`), so ISA-L is
the comparator both confirmatory encode cells use.

**Pairwise control.** The control separates cleanly from the region family.
`gf2m::batch::batch_mul` is ahead of ISA-L's `gf_mul` and of GF-Complete's
`multiply.w32` applied per byte, at both sizes and in the whole-consumer cells
(tables § `6c6b09b1-v4-r1-pairwise-control-pilot` § Cells, the six
`*-vs-isal` and `*-vs-gfcomplete` rows). Against M4RIE's per-byte multiply,
the fastest of the three, the direction depends on the size: M4RIE is ahead at
4 KiB and in the 128 KiB whole-consumer cell, gf2 at 128 KiB kernel-isolated
(`pairwise-4k-batch-vs-m4rie`, `pairwise-128k-whole-batch-vs-m4rie`,
`pairwise-128k-batch-vs-m4rie`). None of those three pilot intervals clears the
family's material-gap threshold in either direction at the pilot's own
per-comparison confidence and pair count, which is why the family's
confirmation takes M4RIE and GF-Complete, the two fastest external arms, and
leaves ISA-L's per-byte multiply as exploratory evidence.

### Protocol-v4 confirmations

Each family's confirmation measures the six cells its addendum declares
confirmatory, on the launcher's confirmatory pair count with the frozen window
count, and nothing else. Every declared cell starts, checkpoints and completes
exactly once across the campaign's sessions; each campaign ends in its own
`complete` terminal record; no cell is flagged unstable, the flagged-window
counts being zero throughout; and `benchmark-acceptance` built from this tree
recomputes each receipt to the verdict, qualification and finding count
its committed acceptance summary and tables header lines carry (tables §
`6c6b09b1-v4-r1-region-axpy-confirmation`,
§ `6c6b09b1-v4-r1-matrix-product-confirmation`,
§ `6c6b09b1-v4-r1-pairwise-control-confirmation`, each receipt's
`acceptance-summary.json`). Each family ledger's confirmation line spends one
comparison per confirmatory cell, the first spend of its chain
(`../../bench_results/6c6b09b1/v3-*-family-ledger.jsonl`, the sequence-3
reservation of each).

**Region multiply-accumulate: six cells of six to ISA-L.** Every confirmed
cell passes, so ISA-L's `gf_vect_mad` is ahead of `FieldVec::axpy` by more
than the family's improvement margin at the corrected confidence: for
`Gf2mElement` in the 4 KiB, 128 KiB and 8 MiB warm cells, for `Gf2mWide` at
128 KiB, and for both representations in the 128 KiB whole-consumer twins
(tables § `6c6b09b1-v4-r1-region-axpy-confirmation` § Cells). The receipt
qualifies, which for this family means every confirmatory comparator is
materially ahead of gf2.

**Dense products: six cells of six to the libraries.** Every confirmed cell
passes: M4RIE's `mzed_mul` at n = 256 for both gf2 representations and for
both whole-consumer twins, and ISA-L's `ec_encode_data` at the generator shape
for both representations (tables §
`6c6b09b1-v4-r1-matrix-product-confirmation` § Cells). That receipt qualifies
on the same reading.

**Pairwise control: four cells of six to gf2, two inside the band.** No
confirmed cell passes, so the receipt does not qualify, and four of the six
record the outcome `fail`. Each of those four is a cell gf2 wins: the interval
lies below the reciprocal of the family's equivalence margin, which in a
`comparator-gap` cell places the library more than that margin behind gf2 at
the corrected confidence (What the confirmations confirm). They are
`gf2m::batch::batch_mul` against GF-Complete's `multiply.w32` per byte at
4 KiB, at 128 KiB and in the 128 KiB whole-consumer cell, and against M4RIE's
`gf2e_mul` per byte at 128 KiB kernel-isolated. The remaining two cells,
against M4RIE at 4 KiB and in the 128 KiB whole-consumer twin, record
`not-material`: their intervals neither clear the improvement margin nor fall
below the reciprocal of the equivalence margin, so the decision rule separates
neither arm there and the receipt states the two as measured (tables §
`6c6b09b1-v4-r1-pairwise-control-confirmation` § Cells). Of the three M4RIE
cells the pilot leaves unseparated, the confirmation therefore separates one,
the 128 KiB kernel-isolated cell, in gf2's favour, and leaves the other two
unseparated at the higher confidence as well.

### Cache regimes

The region family is the one that sweeps them. Its four regimes are 4 KiB,
128 KiB and 8 MiB warm and 2 MiB rotated through the eight fixture banks, and
each is a separate cell for each representation (tables §
`6c6b09b1-v4-r1-region-axpy-pilot` § Cells). Two things the rows show are
worth stating plainly. The gap to ISA-L does not close as the region grows out
of cache: the 8 MiB warm cell is not the narrowest for either representation,
and the narrowest is the streaming cell, whose working set rotates. And the
regime changes the gap by well under the factor that separates the two gf2
representations, so which representation a consumer holds matters more here
than which cache level its region lives in.

The confirmation covers three of the four regimes, the warm ones, and for
`Gf2mElement` covers all three (tables §
`6c6b09b1-v4-r1-region-axpy-confirmation` § Cells). The streaming regime and
the `Gf2mWide` cells at 4 KiB and 8 MiB keep the pilot's exploratory strength,
because the confirmatory budget admits six cells per family and the family
spends them on one regime sweep plus the whole-consumer twins.

The protocol's cache-state declarations are what the arms applied and the
receipts record, and they establish less than their names suggest: `warm`
establishes one untimed pass over the working set before calibration, and
`streaming` establishes rotation through the declared banks and the declared
working-set size. Neither establishes eviction from any cache level
(`../f547c394/protocol.md` § Sampling design). The regime labels in the tables
carry exactly that meaning.

### Kernel-isolated against whole-consumer

The twin cells of each shape put a number on what the byte-region round trip
costs each arm, and the tables carry the ratios with their intervals and
execution counts as descriptive figures, since the two cells of a twin run in
separate executions and no decision uses the ratio (tables §
Whole-consumer over kernel-isolated, in every protocol-v4 receipt section).
The pattern differs by family and by arm:

- In the region family the round trip is expensive for `FieldVec<Gf2mElement>`
  and nearly free for `FieldVec<Gf2mWide<1, Gf256x11d>>`, at both sizes. The
  element representation builds a reference-counted field handle per element,
  and the wide one packs a byte into a u64 lane. ISA-L pays its coefficient
  table preparation only where the kernel itself is short, which is the 4 KiB
  twin.
- In the matrix family the direction reverses: both gf2 representations change
  little between the twins, because the product dominates the conversion at
  dimension 256, while M4RIE pays a visible fraction for packing and unpacking
  its `mzed_t`. At the encode shape every arm's ratio is small.
- In the pairwise family `batch_mul`'s widen-and-narrow round trip is a
  consistent fraction of the call at 128 KiB, and the external arms, which
  work on the byte region itself, are unchanged between the twins.

A consumer choosing between gf2 and a pinned library therefore reads a
different number depending on where its data already lives, and the receipts
carry both.

### Setup, conversion and table-preparation costs

Every execution reports these costs, whether or not its cell includes them in
the timing window, so the composition of a whole-consumer cost is visible in
every cell (tables § Conversion and setup costs, in every protocol-v4 receipt
section). What they show: GF-Complete and M4RIE each pay a large one-time
setup, their table construction, which is the same order of magnitude in every
cell and which a kernel-isolated cell excludes from its window; ISA-L's setup
is small and its per-call cost is the coefficient or generator table it
prepares; gf2's element representation pays its cost in packing, which scales
with the region; gf2's wide representation and `batch_mul` pay a much smaller
packing cost; and `dispatch_ns` is zero for every arm, because no arm selects
an implementation outside its call. A consumer that would reach a pinned
library from a byte region reads its adoption cost from these columns together
with the whole-consumer twins above.

### Protocol-v1 pilot (history)

The v1 pilot `pilot-6c6b09b1-20260908t092546z` is immutable, superseded
evidence and decides nothing (tables § `pilot-6c6b09b1-20260908t092546z`). It
covers only the `Gf2mElement` representation, one kernel-isolated cell per
operation, built with a Rust newer than the MSRV. Within those limits it points
where the current design looks, and the v4 pilots agree with it on all three
operations: ISA-L's region multiply-accumulate ahead of `FieldVec::axpy`
(`axpy-isal-l2-1core`), M4RIE's product ahead of `gemm`
(`matmul-m4rie-n128-1core`), and gf2's `batch_mul` ahead of GF-Complete's
per-element loop (`pairwise-gfcomplete-l2-1core`). Its flagged-window counts
follow v1's pooled rule, which version 3 replaced by an execution-local rule
([amendment](../f547c394/amendment-v3.md)). Its committed acceptance summary
predates a field the current evaluator writes into each cell claim; the
evaluator built from the current tree reaches the same verdict, findings and
qualification on the same receipt, and the committed summary stays as it is,
because it is superseded evidence and the tables pin it by digest.

### Protocol-v3 attempts, and their pre-timing evidence

The version-3 pilot of each of the same three families measures no cell: each
arm rejects the runner's request, so each campaign ends on its first arm with a
`failed` journal record and a reserved ledger line spending zero comparisons.
That is a run that produces no data, not a falsified result; the ledgers keep
it, and it changes nothing about the three questions.

A ledger line cannot say that on its own. It records the comparisons the
addendum predeclares, and a pilot addendum declares none, so the line a void
attempt writes is the line a complete pilot writes.
[The void-attempt record](../../bench_results/6c6b09b1/v3-r1-void-attempts.json)
closes that gap from the artifacts: per family it carries the ledger line, the
addendum it pins, the journal's terminal event, the cells it completed and the
child diagnostic naming the cause, which otherwise survives only in the
uncommitted stage directory. `survey/make-void-attempt-record.py` derives it
and refuses a ledger line whose addendum digest does not resolve.

The version-3 pre-timing evidence and producing manifest do not survive under
their version-3 names, and they are not discarded: commit `d9100acd` moves them
into the version-4 generation, and git records the move as renames, five of
the eight evidence files byte for byte
(`git show d9100acd -M --name-status`). `conformance-v3/arm-provenance.txt`
and `conformance-v3/build-record.txt` become their v4 counterparts with the
provenance and digests of the executables the v4 plans name, and
`producing-inputs-v3.json` becomes `producing-inputs-v4.json` over the same
closure, which it has to, because it names generator paths the rename moves
and would otherwise no longer resolve. One file, the element field-law run, is
regenerated rather than moved, because it is a test-runner transcript that
differs between runs. Keeping a second, stale copy under the v3 name would pin
nothing: the three version-3 campaigns measure zero cells, so no receipt
cites that evidence, and the only artifacts that pin a version-3 attempt are
the three version-3 addenda, each named by digest in its ledger line, and the
ledger lines themselves. Both are retained, because the chain rejects a
removed attempt. The bytes under the version-3 names remain at `d9100acd^`.

## Baseline for 19513245

The feasibility issue `19513245` receives, per family, the current gap of both
gf2 GF(2^8) representations to the fastest measured external arm, per cache
regime and with and without conversion costs, the list of cells gf2 loses, and
the strength each cell carries: confirmatory at the family confidence for the
six cells of each confirmation, exploratory for the rest of each pilot's
breadth.

gf2 loses every cell of two of the three families:

- every region multiply-accumulate cell against ISA-L, in all four cache
  regimes, for both representations, kernel-isolated and whole-consumer
  (tables § `6c6b09b1-v4-r1-region-axpy-pilot` § Cells, the twelve `*-vs-isal`
  rows). Six of those twelve are confirmed at the family confidence (tables §
  `6c6b09b1-v4-r1-region-axpy-confirmation` § Cells); the streaming regime and
  the `Gf2mWide` 4 KiB and 8 MiB cells stay exploratory;
- every dense-product cell: the three square dimensions against M4RIE for both
  representations and their whole-consumer twins, and both encode cells against
  ISA-L (tables § `6c6b09b1-v4-r1-matrix-product-pilot` § Cells). The n = 256
  cells and both encode cells are confirmed (tables §
  `6c6b09b1-v4-r1-matrix-product-confirmation` § Cells); n = 64 and n = 512
  stay exploratory.

So 19513245 inherits a confirmed gap in both families, and in each of them the
confirmed cells fix the representation, the shape and the conversion regime the
gap is measured in.

gf2 loses no pairwise cell. It leads materially against GF-Complete's per-byte
multiply in all three confirmed GF-Complete cells and against M4RIE's at
128 KiB kernel-isolated, and the two remaining confirmed M4RIE cells separate
neither arm (tables §
`6c6b09b1-v4-r1-pairwise-control-confirmation` § Cells); the pilot's ISA-L
cells, which gf2 also leads, stay exploratory (tables §
`6c6b09b1-v4-r1-pairwise-control-pilot` § Cells). The control therefore tells
19513245 that the region and matrix gaps belong to fixed-coefficient reuse and
to dense products, not to GF(2^8) multiplication as such.

Three absences bound what 19513245 can ask for, and each is recorded rather
than filled with a different operation (REQ-03). `FieldVec::dot_product` has no
equivalent in any pinned library, so no cell measures it and the issue receives
no external baseline for it. Neither ISA-L nor GF-Complete has a dense matrix
type, so the square product has M4RIE as its only comparator, and ISA-L's
encode is a matrix product only at the generator shape. And no library has a
region kernel for distinct pairs, so every pairwise comparator is a per-element
call, which is what the control measures and all it establishes.

The field mismatch bounds the same way (REQ-05, One field: 0x11D). Every cell
works in GF(2^8) modulo 0x11D, the one field all four arms implement; gf2 ships
no GF(2^8) field modulo 0x11B, ISA-L refuses 0x11B, and no pinned library
offers a change of basis. A 0x11B byte-region consumer therefore inherits from
this survey the GF-Complete and M4RIE baselines, whose 0x11B tables the
conformance run validates, and no ISA-L baseline at all; reaching ISA-L would
need an isomorphism adapter that this survey neither builds nor times, and
whose cost 19513245 would have to measure before counting the ISA-L gap as
available to it.

Two further facts, both from the pilots' exploratory breadth, bear directly on
where an improvement would go. Neither gf2 representation dominates the other:
`Gf2mElement` is the faster region arm and `Gf2mWide` the faster dense-product
arm, in the cells that compare them directly (`axpy-128k-element-vs-wide`,
`matmul-n256-element-vs-wide`). And the conversion a byte-region consumer
would pay is a large fraction of `FieldVec<Gf2mElement>`'s region cost and a
small one of every other arm's (tables § Whole-consumer over kernel-isolated),
so a byte-region consumer's choice is not the same question as a kernel's.

## Status against the criteria

- REQ-01: met. The contract, protocol and addendum pins, the launcher and the
  ledgers are in place; the three pilot receipts and the three confirmation
  receipts are committed, each accepted by the evaluator (finding count on
  the receipt's header lines in the tables), and
  each family's ledger carries both reservations. The pairwise control's four
  `fail` cells and its `qualifies` false are recorded as the evaluator states
  them, and so is the negative version-3 outcome, with the record that says
  what it did. No production change, so no before/after evidence is due.
- REQ-02: met; see Arms and pins.
- REQ-03: met; see One field, Operation mapping and the recorded absences in
  Baseline for 19513245.
- REQ-04: met. Validation precedes timing (Correctness before timing), and
  every execution of every committed receipt reports its conversion, packing,
  table-preparation and setup costs, with the whole-consumer twins beside the
  kernel-isolated cells (tables § Conversion and setup costs, § Whole-consumer
  over kernel-isolated). The pairwise control is a separate family from the
  fixed-coefficient region family and shares no cell with it.
- REQ-05: met. The three pilot addenda and the three confirmation addenda are
  committed and frozen; the pilot receipts cover every admissible cell across
  the declared sizes and cache regimes, and each confirmation receipt covers
  the six of them its family's confirmatory budget admits; and the 0x11B/0x11D
  mismatch is documented with the reason no supported basis conversion exists
  and with what a 0x11B consumer inherits instead.
