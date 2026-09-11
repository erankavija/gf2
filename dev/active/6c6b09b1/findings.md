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
in three families with three ledgers. The protocol-v3 pilots of all three
families are built, frozen and queued for the benchmark window; every measured
result, every confirmation and the list of gf2's losing cells wait on those
receipts. The superseded protocol-v1 pilot is kept as history. No production
code changes.

## Question and evidence

`FieldVec` and `FieldMatrix` consumers hold field elements, not byte regions.
The survey asks which gf2 consumer operations have an equivalent in M4RIE,
GF-Complete and ISA-L once field polynomial, coefficient reuse, accumulation
and overlap are matched; how far current gf2 is from those libraries on this
Ryzen 9 5900X under the
[measurement contract](../1a379447-zen3-cpu-performance/measurement-contract.md)
and [protocol version 3](../f547c394/protocol.md), per cache regime,
kernel-isolated and as a whole byte-region consumer; and which cells gf2 loses,
as the baseline for the feasibility issue `19513245`. The issue excludes
adoption and any production change, so every cell is a `comparator-gap` cell
with the external library as candidate: with gf2 as baseline, a speedup of
medians below one means gf2 is faster.

| Command | Output |
|---|---|
| `survey/fetch-build.sh` | the pinned libraries, built from verified pins under the primary checkout's `.agents/ext/6c6b09b1/` |
| `survey/stage-externals.sh` | that prefix, checked against `survey/ext-prefix.sha256` and copied into `target/6c6b09b1-ext/` |
| `../../bench_results/6c6b09b1/run-byte-field-v3.sh build` | the C shim, conformance and provenance tools, both arm executables, the runner, and `conformance-v3/`: correctness evidence, `arm-provenance.txt`, `build-record.txt` |
| `../../bench_results/6c6b09b1/run-byte-field-v3.sh plan\|window CAMPAIGN` | the v3 campaigns; each receipt's `launcher.log` records every session command |
| `survey/make-addenda-v3.py`, `survey/make-plan-v3.py`, `survey/make-producing-inputs-v3.py` | the pilot addenda, the runner plans and the producing closure `survey/producing-inputs-v3.json` |
| `survey/make-source-evidence.py` | `survey/source-evidence.json`: every code claim below, cited by claim ID in backticks |
| `survey/make-ledger-origin-v3.py` | `../../bench_results/6c6b09b1/v3-*-ledger-origin.json` |
| `survey-analysis tables` (`survey/analysis/`) | [the generated tables](../../bench_results/6c6b09b1/tables.md) |

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
`-O3 -march=native` (`conformance-v3/arm-provenance.txt` § Compiler
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
header before a build. `conformance-v3/arm-provenance.txt` records the pins,
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
`conformance-v3/externals.txt` and `conformance-v3/gf2-side.txt`). A 0x11B
consumer could use GF-Complete or M4RIE, whose full 0x11B multiplication tables
pass against the independent oracle (`conformance-v3/externals.txt`), or gf2
through `Gf2mField::new(8, 0x11B)` (`conformance-v3/gf2-side.txt`); ISA-L
refuses 0x11B. No pinned library offers a change of polynomial basis, since
M4RIE's conversion module changes storage layout within one field
(`m4rie-conversion-scope`), so a 0x11B consumer reaching ISA-L would need an
isomorphism adapter that this survey neither builds nor times. The
protocol-v1 design declared a 0x11B cell on the premise that gf2's
compile-time GF(2^8) aliases were a shipped 0x11B field; they are test code,
and the v3 design has no such cell.

The rustdoc of `gf256()` names x^8+x^4+x^3+x+1 and calls the field the one
"used in AES" (`gf2-gf256-doc-polynomial`, `gf2-gf256-doc-aes`) while the code
builds 0x11D, and the catalogue comment calls the five-term polynomial a
trinomial. Both are documentation defects in gf2-core, reported for tracking;
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
  (`conformance-v3/arm-provenance.txt`), an inline lookup in a 256 by 256 table
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
(`survey/producing-inputs-v3.json` lists `conformance-v3/`):

- `conformance-v3/externals.txt`: every backend through the C shim against an
  independent shift-and-reduce oracle (`bfx_ref_mul`): full multiplication
  tables; region multiply-accumulate at the byte boundary lengths and at the 4
  KiB and 128 KiB cell lengths; the M4RIE row form; pairwise products; dense
  products at boundary and cell dimensions and at the encode shape; the ISA-L
  encode; and the 0x11B refusal, tables and distinctness witness.
- `conformance-v3/ext-wrapper.txt`: the region, pairwise, product and encode
  paths through the Rust wrapper the external arm calls.
- `conformance-v3/gf2-side.txt`: both gf2 representations through the arm's
  own conversions, `FieldVec::axpy` for every coefficient, `gemm` at boundary
  and cell shapes on the route the arm records, `batch_mul`, and the 0x11B
  field, against an oracle written without gf2-core.
- `conformance-v3/field-laws-element.txt` and
  `conformance-v3/field-laws-wide.txt`: the shared field-law suite,
  `test_field_axioms`, over `Gf2mField::gf256()` (gf2-core's
  `test_gf2_8_field_axioms`) and over `Gf2mWide<1, Gf256x11d>`
  (`survey/field-laws`).

Each file ends in its check count with no failure, or in its pass line.

## Families and campaigns

Three questions, three families, each with its own append-only ledger created
before its first v3 campaign:

- `byte-field-region-axpy` ([pilot addendum](addendum-v3-region-axpy-pilot.json)):
  fixed-coefficient region multiply-accumulate per cache regime (4 KiB, 128 KiB
  and 8 MiB warm; 2 MiB regions rotated through eight banks), kernel-isolated
  and whole-consumer, for both gf2 representations. The pilot also compares
  ISA-L with GF-Complete and M4RIE, and the two representations with each other.
- `byte-field-matrix-product` ([pilot addendum](addendum-v3-matrix-product-pilot.json)):
  dense products at n = 64, 256 and 512, which spans M4RIE's switch to its
  bitsliced Karatsuba path (the `mzed_mul` lines of
  `conformance-v3/arm-provenance.txt`), and the encode shape against ISA-L,
  kernel-isolated and whole-consumer.
- `byte-field-pairwise-control` ([pilot addendum](addendum-v3-pairwise-control-pilot.json)):
  arbitrary pairwise products against each library's per-byte multiply at 4 KiB
  and 128 KiB, kernel-isolated and whole-consumer.

Every cell is single-core: none of the measured calls has a parallel form in
these builds, so a 6-, 12- or 24-CPU arm would time a harness thread pool; each
addendum states this. Each ledger opens with the retrospective zero-comparison
restatement of the protocol-v1 pilot, which measured one cell of each
operation; the origin records derive it and list the two v1 launches that
stopped before a campaign opened
(`../../bench_results/6c6b09b1/v3-*-ledger-origin.json`). The frozen v1
confirmation never ran, so no family has spent a comparison: each family's v3
confirmation is its first attempt, which P-20's twenty expected tail draws at
the corrected alpha allow for at most six confirmatory cells.

The pilots are exploratory, with the launcher's `PILOT_PAIRS` pairs per cell,
and are queued for the benchmark window; their receipts will be
`../../bench_results/6c6b09b1/v3-r1-<family>-pilot/`. Each family's
confirmation is then frozen from its own pilot receipt: a measurement
resolution at or above the pilot's widest relative bootstrap half-width
(`survey-analysis resolution`, the quantity P-03 recomputes), margins strictly
above one plus that resolution, the fastest measured external arm per cell,
and fresh seeds. A family whose honest confirmatory cell set exceeds six cells
runs those cells and reports them `not-confirmatory`.

## Results

### Protocol-v3 pilots

Pending the benchmark window. The tables file gains one section per receipt
when `survey-analysis tables` is rerun.

### Protocol-v1 pilot (history)

The v1 pilot `pilot-6c6b09b1-20260908t092546z` is immutable, superseded
evidence and decides nothing (tables § `pilot-6c6b09b1-20260908t092546z`). It
measured only the `Gf2mElement` representation, one kernel-isolated cell per
operation, built with a Rust newer than the MSRV. Within those limits it points
where the v3 design looks: ISA-L's region multiply-accumulate was ahead of
`FieldVec::axpy` by more than two orders of magnitude (`axpy-isal-l2-1core`),
M4RIE's product by about an order of magnitude (`matmul-m4rie-n128-1core`), and
gf2's `batch_mul` was ahead of GF-Complete's per-element loop
(`pairwise-gfcomplete-l2-1core`). Its flagged-window counts follow v1's pooled
rule, which version 3 replaced by an execution-local rule
([amendment](../f547c394/amendment-v3.md)).

## Baseline for 19513245

The feasibility issue `19513245` receives, per family, the current gap of both
gf2 GF(2^8) representations to the fastest measured external arm, per cache
regime and with and without conversion costs, and the list of cells gf2 loses.
Both wait on the v3 receipts. The v1 history already marks region
multiply-accumulate and dense products as losing operations for `Gf2mElement`,
and pairwise products as one where gf2 leads, against per-element comparators
only.

## Status against the criteria

- REQ-01: the contract, protocol and addendum pins, the launcher and the
  ledgers are in place; the v3 receipts wait on the window.
- REQ-02: met; see Arms and pins.
- REQ-03: met; see One field and Operation mapping.
- REQ-04: validation met (Correctness before timing); measured conversion,
  table-preparation, setup and output costs wait on the window.
- REQ-05: pilot addenda frozen and the field mismatch documented; the
  confirmation addenda and baseline receipts wait on the pilots.
