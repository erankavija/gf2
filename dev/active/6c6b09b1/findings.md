# Byte-field vector and matrix comparison arms

> **Diátaxis Type:** Explanation

Research findings for issue `6c6b09b1`. The survey establishes reproducible,
operation-equivalent external baselines for gf2's GF(2^8) vector and matrix
consumer entry points, without changing any production kernel.

## Question

Do M4RIE, GF-Complete and ISA-L provide operations genuinely equivalent to
`FieldVec::axpy` and to the batched matrix cells gf2 exposes, and what does a
current, protocol-conforming baseline measurement of gf2 against those arms
show?

The question matters because `FieldVec` and `FieldMatrix` consumers do not
imply byte-packed region semantics. Whether a byte-region representation is
worth designing depends on how large the gap is against libraries that already
have one, measured on operations that are actually the same operation.

## Methodology

### Pinned arms

Three external libraries carry the comparator arms.
[`survey/fetch-build.sh`](survey/fetch-build.sh) holds the pins and
re-verifies every one of them on each run: a version plus SHA-256 for the
tarball sources, a branch or tag plus the commit it resolves to for the two
git sources. [`arm-provenance.txt`](arm-provenance.txt), produced by
[`survey/arm-provenance.sh`](survey/arm-provenance.sh), records what those
pins produced on this host, digests included, and every receipt carries a
copy of it beside its samples.

| Arm | Pinned revision | License |
|---|---|---|
| M4RIE | release `20250128`, over M4RI `20260122` | GPL-2.0-or-later |
| GF-Complete | `ceph/gf-complete` commit `a6862d10c9db` | BSD-3-Clause |
| ISA-L | `intel/isa-l` tag `v2.32.1`, commit `7c3479e0a9da` | BSD-3-Clause |

The licenses are load-bearing. gf2 is MIT (`LICENSE-MIT`). GF-Complete and
ISA-L are BSD-3-Clause and could become production dependencies; M4RIE and
the M4RI it builds on are GPL-2.0-or-later, so M4RIE is a comparator this
survey links into its own harness and never a candidate for a gf2 dependency.
That constraint holds whatever the matrix cells measure.

The gf2 arm consumes `gf2-core` by path with its default features plus
`simd`, which is what gates the runtime kernel dispatch. It does not enable
`parallel`: none of `FieldVec::axpy`, `field::matrix::gemm` or
`gf2m::batch::batch_mul` has a rayon path, so the feature would change the
dependency graph and nothing that is measured.

Both sides are built for this processor rather than for a portable baseline.
The C libraries and the shim compile with `-O3 -march=native`, which GCC
resolves to `znver3` here; the gf2 arm compiles with `-C target-cpu=native`
at `opt-level=3` with LTO and one codegen unit. Timing a native build against
a portable one would measure the codegen target instead of the two
implementations.

Which arithmetic backend a library selects is a runtime property, so
[`survey/backend_provenance.c`](survey/backend_provenance.c) reads it out of
the loaded binaries rather than out of documentation. It follows ISA-L's
multibinary dispatch slot and the PLT hop behind it to the kernel address,
reads GF-Complete's selected `gf_t` function pointers, and interposes M4RIE's
own PLT-bound entry points to count the recursion a product actually takes.
`arm-provenance.sh` resolves the reported addresses against each library's
symbol table, because the kernels of interest are file-local symbols that no
dynamic-symbol lookup can name.

This host carries AVX2 and no AVX-512, and the record shows what follows.
ISA-L runs `gf_vect_mad_avx2` for region multiply-accumulate and
`ec_encode_data_avx2` for the generator encode. GF-Complete's default `w=8`
region kernel is `gf_w8_split_multiply_region_sse`, the same kernel its
explicit `split-table-simd` variant selects, so the two variants are one arm
for region work and differ only in their scalar multiply. M4RIE multiplies
elements through the 256x256 lookup table `_gf2e_mul_table`, and its Strassen
cutoff for GF(2^8) is n = 512, so every matrix cell in this survey reaches its
product through exactly one `_mzed_mul_newton_john` call.

### Field identity

Both arms of every cell work in one field, and no cell mixes two. Thirteen
cells use GF(2^8) modulo the full reduction polynomial `0x11D`, that is
$x^8 + x^4 + x^3 + x^2 + 1$; the fourteenth uses `0x11B` against the one
comparator that offers it. The polynomial is not a free choice; it is what
makes a cell admissible at all. Rows naming an external file cite it inside
that library's pinned source tree, which `survey/fetch-build.sh` stages
outside the repository.

| Implementation | Polynomial | Source |
|---|---|---|
| gf2 runtime `Gf2mField::gf256()` | `0x11D` | `crates/gf2-core/src/gf2m/field.rs:850` |
| gf2 catalogue `PrimitivePolynomialDatabase::standard(8)` | `0x11D` | `crates/gf2-core/src/primitive_polys.rs:139` |
| gf2 compile-time `Gf2mWide` aliases `Gf2m8`, `G8`, `WinoGf2m8`, `TriGf2m8` | `0x11B` | `crates/gf2-core/src/field/expr.rs:2508`, `crates/gf2-core/src/field/matrix.rs:4890` |
| ISA-L | `0x11D`, compiled in | pinned ISA-L `erasure_code/ec_base.c:178` reduces with `0x1d` |
| GF-Complete | `0x11D` by default, any polynomial through `gf_init_hard` | pinned GF-Complete `src/gf_w8.c:2306` |
| M4RIE | any polynomial through `gf2e_init` | pinned M4RIE `m4rie/gf2e.h:80` |

ISA-L is the binding constraint: it implements `0x11D` and nothing else, and
the adapter refuses any other polynomial rather than silently substituting its
own (`survey/byte_field_ext.c`, `bfx_init`). M4RIE and GF-Complete accept the
polynomial as a parameter, so `0x11D` is available on every arm and `0x11B`
on two of the three. gf2 stores GF(2^8) elements as
canonical polynomial-residue bits, bit $i$ carrying the coefficient of $x^i$,
which is the same encoding all three externals use, so the byte values are
directly comparable.

### The 0x11B and 0x11D mismatch

`0x11B` ($x^8 + x^4 + x^3 + x + 1$, the AES polynomial) and `0x11D` are
different fields on the same byte carrier. The conformance run exhibits a
witness: $2 \cdot 128$ is $29$ under `0x11D` and $27$ under `0x11B`
(`conformance/externals.txt`, `conformance/gf2-side.txt`). They are isomorphic
as abstract fields but not equal as represented fields, so a product computed
under one is simply wrong under the other.

The two polynomials differ in a way that reaches implementations, not only
field elements. Both are irreducible, but `x` generates the multiplicative
group only under `0x11D`: its order is 255 there and 51 under `0x11B`. An
implementation that multiplies through a log and antilog table keyed on `x`
is therefore correct under `0x11D` and wrong under `0x11B` unless it chooses
a different generator, which is exactly why the conformance run checks a full
`0x11B` multiplication table for every arm that accepts the polynomial rather
than assuming the parameter is honoured.

gf2 contains both. The runtime `Gf2mField` family and the primitive-polynomial
catalogue use `0x11D`; the compile-time `Gf2mWide` GF(2^8) aliases use `0x11B`.
The survey measures both sides rather than declaring one of them out of
scope. The `0x11D` side carries thirteen cells because that is the side every
arm implements. The `0x11B` side carries one, `axpy-gfcomplete-11b-l2-1core`,
because GF-Complete takes the polynomial as a parameter and its `0x11B`
multiplication table is validated against the independent reference before the
cell is timed. ISA-L has no arm there: `bfx_init` refuses the polynomial with
its reason, so the survey records an absent comparator instead of comparing
incompatible fields. M4RIE also accepts `0x11B` and is validated there, and no
cell of its own was declared for it; that is a gap in coverage rather than in
availability.

No basis conversion is performed anywhere in this survey, and none is needed:
every cell asks both of its arms for the same polynomial. M4RIE ships
`conversion_cling8.c` and related basis-change machinery, which stays unused
for that reason.

### Operation mapping

The mapping below is the core of the survey. A matrix multiply is not a region
multiply-XOR, and the two are kept in separate cells throughout.

| gf2 entry point | Semantics | ISA-L | GF-Complete | M4RIE |
|---|---|---|---|---|
| `FieldVec::axpy(&a, &x)` | `y[i] += a * x[i]`, one coefficient reused across the region, accumulating into `y` | `gf_vect_mad(len, 1, 0, tbls, src, dest)` | `multiply_region.w32(gf, src, dest, a, bytes, add=1)` | `mzed_add_multiple_of_row(A, ar, B, br, x, 0)` |
| region assign `y[i] = a * x[i]` | same coefficient reuse, no accumulation | `gf_vect_mul(len, tbl, src, dest)` | `multiply_region.w32(..., add=0)` | none |
| `gf2m::batch::batch_mul` | `out[i] = x[i] * y[i]`, every pair distinct, no coefficient reuse | **none** | elementwise `multiply.w32` loop | **none** |
| `field::matrix::gemm(A, B)`, square | dense `C = A * B` | **none** | **none** | `mzed_mul(C, A, B)` |
| `field::matrix::gemm(G, D)`, `G` is `rows x k` | generator encode `C[r][i] = sum_j G[r][j] * D[j][i]` | `ec_encode_data(len, k, rows, tbls, data, coding)` | **none** | `mzed_mul` at that shape |
| `FieldVec::dot_product` | inner product `sum_i x[i] * y[i]` to a single field element | **none** | none | none |

Four points in that table are load-bearing.

**ISA-L's `gf_vect_dot_prod` is not `FieldVec::dot_product`.** The names
collide and the operations do not. `gf_vect_dot_prod` computes, for each byte
position $i$ independently, $\sum_j a_j \cdot \mathrm{src}_j[i]$ across
`vlen` source regions with a fixed coefficient vector. It produces a region.
`FieldVec::dot_product` contracts two vectors to one field element. Reporting
the former as a comparator for the latter would compare a different operation,
so no cell does.

**No external arm offers arbitrary pairwise multiplication except
GF-Complete, and only through its scalar entry point.** ISA-L's entire region
API is built around one coefficient expanded into a table; `gf_mul` is a
scalar helper, not a region kernel. M4RIE's region kernels are all
fixed-coefficient or matrix shaped. The adapter returns
`BFX_ERR_UNSUPPORTED` for both rather than looping their scalar multiply and
calling the result a region arm.

That leaves the pairwise cell with a comparator that is itself a loop over
`gf_t.multiply.w32`, one indirect call per element, because GF-Complete has
no vectorised elementwise product of two regions either. The cell is
admissible — it is the fastest arm GF-Complete has for that operation — but
its result says gf2 is faster than a per-element indirect call, not that gf2
is faster than a region kernel. No region kernel for this operation exists in
any of the three libraries.

**Neither ISA-L nor GF-Complete carries a dense GF(2^8) matrix type.** M4RIE is
the only matrix comparator. ISA-L's `ec_encode_data` is a genuine matrix
product, but at a specific shape: a small generator matrix times a few very
wide data regions. That shape gets its own cell rather than being presented as
a square GEMM comparator.

**Accumulation is XOR.** The field has characteristic 2, so `+=` in every arm
above is `^=`, and all four axpy forms agree on that.

**Overlap.** Every arm requires source and destination to be distinct.
`FieldVec::axpy` takes `&mut self` and `&Self`, so aliasing is a compile
error; the C adapters take distinct Rust borrows, which the FFI wrapper's
safety comments record.

### Representation and conversion

The representations differ far more than the operations do, and that is the
survey's main finding.

| Arm | GF(2^8) element storage | Bytes per element |
|---|---|---|
| gf2 `FieldVec<Gf2mElement>` | `Vec<Gf2mElement>`, each `{ value: u64, params: Arc<FieldParams> }` | 16, plus an atomic refcount touched per element clone |
| ISA-L, GF-Complete | `unsigned char *` | 1 |
| M4RIE `mzed_t` | `e` bits per element packed into `mzd_t` words | 1 |

`Gf2mElement_<u64>` is defined at `crates/gf2-core/src/gf2m/field.rs:206`. A
`FieldVec<Gf2mElement>` therefore occupies sixteen times the footprint of the
byte region it represents, and every element-wise operation moves an `Arc`
alongside the value. This is a property of the general runtime field
abstraction, not a defect: `FieldVec` is generic over `FiniteField` and must
carry runtime field parameters for fields whose modulus is not known at
compile time. It does mean `FieldVec` is not a byte-region type, which is
exactly what the issue set out to establish.

`gf2m::batch::batch_mul` carries a second representation gap of its own: it
reads and writes `u64` lanes, one element per lane, so its kernel-isolated
cell already moves eight times the bytes of the byte region it stands for,
before any `FieldVec` appears. Its comparator reads the byte region directly.
The pairwise cell therefore times the same arithmetic over two different
memory footprints, which is what a byte-region consumer faces today and what
a byte-packed `batch_mul` would remove.

Cells therefore come in two metric kinds. A **kernel-isolated** cell times the
call on operands already in the arm's own representation. A **whole-consumer**
cell starts and ends at the shared byte region, so its window includes packing,
the kernel, table preparation and unpacking. Every arm reports its conversion
components separately in the receipt (`setup_ns`, `pack_ns`, `unpack_ns`,
`batch_fill_ns`, `dispatch_ns`), so the composition of a whole-consumer cost is
visible and not merely asserted.

Table preparation differs by arm and is charged where it happens:

- ISA-L expands one coefficient into a 32-byte nibble table with
  `ec_init_tables`, once per coefficient. A whole-consumer cell pays it per
  call, matching a consumer that applies a fresh coefficient to each region.
- GF-Complete expands the coefficient inside each region call; its one-off
  cost is the field construction in `gf_init_hard`, charged to `setup_ns`.
- M4RIE takes the coefficient as an argument; its tables come from `gf2e_init`,
  charged to `setup_ns`.
- gf2 takes the scalar directly and has no table to prepare, so its
  `batch_fill_ns` is zero for the region cells. For the generator-encode cell
  the analogue is converting the generator matrix, which happens once per
  coefficient set, and is charged to `batch_fill_ns`.

`dispatch_ns` is zero for every arm: none performs an implementation-selection
step outside the kernel call, and gf2's per-call `try_simd_axpy` check is
inside the timed window already.

### Correctness before timing

No cell is timed before its adapter agrees with an independent oracle. The
oracle is a shift-and-reduce scalar multiply written twice, once in C
(`bfx_ref_mul`) and once in Rust (`reference_mul` in `gf2_conformance.rs`),
sharing no code with any library under test.

Four validation stages run before the first timed window, all driven by
`dev/bench_results/6c6b09b1/run-byte-field.sh`:

1. **Shared field-law suite.** `cargo nextest run -p gf2-core --all-features -E
   'test(field::axiom_tests::test_gf2_8_field_axioms)'` runs the canonical
   `test_field_axioms` harness over `Gf2mField::gf256()`, the exact field the
   arms use. This survey calls the shared suite rather than restating it.
2. **External library conformance** (`conformance/externals.txt`). Full
   65536-entry multiplication tables per backend, region multiply-XOR and
   multiply-assign over the byte boundary lengths 0, 1, 31, 32, 63, 64, 65,
   127, 128 and 4096, arbitrary pairwise multiplication, dense matmul with a
   pack and unpack round trip, generator encode, and the `0x11B`/`0x11D`
   distinctness witness.
3. **FFI wrapper validation** (`conformance/ext-wrapper.txt`). The same
   operations through the Rust wrapper the arms actually call.
4. **gf2 adapter validation** (`conformance/gf2-side.txt`). `FieldVec::axpy`,
   `batch_mul`, square and rectangular `gemm`, and the byte-to-`FieldVec` round
   trip, each against the independent reference at the boundary lengths.

Every stage exits non-zero on the first mismatch, so a failure stops the
campaign before it starts. The same pre-timing phase writes
`arm-provenance.txt`, so what the receipt says about the arms was observed
before the arms were timed, not reconstructed afterwards.

### Measurement protocol

Measurements follow the frozen Zen 3 benchmark protocol
(`dev/active/f547c394/protocol.md`, version 1) and the epic's measurement
contract, both pinned by content digest into every receipt. gf2 is the
baseline arm and the external library the candidate in every cell, so the
estimator, the ratio of medians, reads above one as a gap in the comparator's
favour. Cells use the `comparator-gap` objective.

Two properties of this family are worth stating plainly.

**The family selects nothing for production.** It is a feasibility study
establishing a current before measurement. A receipt qualifies for production
selection only when every non-exploratory cell passes; this family expects
cells that do not, and retains them.

**A cell where gf2 wins records `fail` under the `comparator-gap`
objective.** That outcome name means no material gap exists in the
comparator's favour. It is a result the family keeps, not an omission.

Timed sessions run under `dev/scripts/ccx1-bench-flock.sh --full-host`, which
holds the canonical CCX1 mutex exclusively and leaves the processor
unpartitioned, so the runner resolves the core arm's CPU ids itself inside the
affinity mask it observes. The campaign is bounded and resumable: the plan
caps a session at two cells for the three-cell pilot and seven for the
fourteen-cell confirmation, and the runner then pauses, so the mutex returns
to the other workers on this host between chunks and a resumed session never
repeats a completed cell. The chunk size follows what this host costs: a
measured cell takes seconds, while one acquisition of the mutex waited
minutes, so smaller chunks would spend more of the host's time queueing than
they return to it.

The pilot and the confirmation are separate receipts. The pilot is
exploratory, supports no adoption decision, and exists to measure this
family's resolution; `survey/freeze-confirmation.py` reads that resolution out
of the committed pilot receipt, freezes the confirmatory addendum around it,
and pins the pilot by digest as the confirmation's resolution evidence.

### Cells, and the arms that do not exist

The confirmatory family declares fourteen cells. Region multiply-accumulate is
measured at four footprints of the shared byte region — 4 KiB, 128 KiB, 8 MiB,
and a streaming cell whose eight rotating fixture banks make the touched set
sixteen times one 2 MiB region — and against all three comparators at 128 KiB.
Dense matrix multiply is measured at n = 64, 128 and 256. One whole-consumer
cell mirrors the region case and one mirrors the matrix case. The generator
encode and arbitrary pairwise multiplication take one cell each, and one
further region cell works in `0x11B`.

Those footprints name the byte region both arms start from, which is the only
footprint the two share. The same 4 KiB region is 64 KiB of
`FieldVec<Gf2mElement>`, so gf2's working set leaves each cache level well
before the comparator's does. That asymmetry is the survey's subject rather
than a flaw in the cell design, and the whole-consumer cells price it
directly.

No cell declares a six-, twelve- or twenty-four-way arm. Every operation
measured here is single-threaded on both sides: `FieldVec::axpy`,
`field::matrix::gemm` and `gf2m::batch::batch_mul` expose no parallel form,
and neither do ISA-L's region entry points, GF-Complete's region kernels or
M4RIE's matrix layer. A multicore arm would compare two harness thread pools
rather than two library implementations, so this family records the reason
and omits the arm instead of fabricating samples for it.

## Findings

### What is measured and what is frozen

One receipt carries every number below: the exploratory pilot
`pilot-6c6b09b1-20260908t092546z` at
[`dev/bench_results/6c6b09b1/2026-09-08-6c6b09b1-byte-field-pilot/`](../../bench_results/6c6b09b1/2026-09-08-6c6b09b1-byte-field-pilot/),
receipt SHA-256 `b15308aad470be693d0f0afff09fd418c074fde58230fdb25ae8ef3fab928425`.
The independent acceptance tool reports `Accepted qualifies=false findings=0`.
It pins the protocol (`4cc897ed…`), the measurement contract (`9f3c7563…`),
the addendum schema (`44dd132a…`) and the frozen pilot addendum
(`23252109…`) by content digest, along with both arm executables. Its source
provenance covers the protocol tooling only: when it ran, the runner pinned one
hard-coded manifest, so the pilot carries a
[`survey-inputs.sha256`](../../bench_results/6c6b09b1/2026-09-08-6c6b09b1-byte-field-pilot/survey-inputs.sha256)
listing the arm sources beside it. The runner now takes a manifest argument, so
[`producing-inputs.json`](producing-inputs.json) puts this survey's arm and
validation sources inside the campaign's own content identity, and the
confirmation records them there rather than in a file of its own.

The pilot is labeled exploratory and supports no adoption decision. It sizes
the confirmation and it is the resolution evidence the confirmation cites.

The confirmatory family is frozen and unmeasured.
[`addendum-byte-field-arms.json`](addendum-byte-field-arms.json) declares the
fourteen cells, derives a measurement resolution of `0.02` from the pilot's
largest relative confidence-interval half-width (`0.0151`), sets the material
gap threshold at `1.25`, and pins the pilot receipt by SHA-256 as the evidence
for both. Its cells carry fresh seeds (21 to 34) rather than the pilot's 11 to
13, so the confirmation re-measures rather than reusing pilot samples.

That campaign is held. The rework of `f547c394` is landing version 2 of the
protocol these receipts are governed by, and a version-1 confirmatory receipt
would be re-run under version 2 before acceptance. The campaign is scripted end
to end (`dev/bench_results/6c6b09b1/run-byte-field.sh confirmation`), its plan
generates for all fourteen cells, and re-running
`survey/freeze-confirmation.py` regenerates the addendum against the amended
protocol, so the re-run is one command and no manual step.

### Measured results

Host `fraktaali`: AMD Ryzen 9 5900X, Linux 7.2.2-arch1-1, SMT active, all 24
logical CPUs on the `powersave` governor, 51 GiB available. Sessions ran under
the exclusive CCX1 mutex through `dev/scripts/ccx1-bench-flock.sh --full-host`
with the wrapper's `inherited-fd-and-independent-flock-conflict` hold evidence
journalled. Every cell resolved to CPU 0 and every arm child observed CPU 0.

Each cell contributes 6 counterbalanced pairs; each execution reports 5 windows
against a 100 ms target, so 30 windows per arm and 60 per cell. A pair value is
the median of its 5 windows, and the cell estimate is the ratio of the two arms'
medians over the 6 pairs, with a nearest-rank percentile bootstrap interval over
whole-pair resampling, 10000 resamples, at 95% per-comparison confidence
(Bonferroni over one comparison, because a pilot declares no confirmatory cell).

gf2 is the baseline arm and the external library the candidate in every cell, so
the estimate is the factor by which the external library is faster than gf2, and
a value below one is a cell gf2 wins.

Every table in this section is rendered from the committed receipt by
[`survey/summarize-receipt.py`](survey/summarize-receipt.py), so the prose
cannot drift from the evidence it cites. Re-running
`survey/summarize-receipt.py dev/bench_results/6c6b09b1/2026-09-08-6c6b09b1-byte-field-pilot --conversion`
reproduces them.

| Cell | gf2 ns/call | comparator ns/call | ratio of medians | 95% interval | outcome |
|---|---|---|---|---|---|
| `axpy-isal-l2-1core` | 1,340,308 | 2,855 | 469.45 | [468.67, 471.49] | pilot |
| `matmul-m4rie-n128-1core` | 6,840,282 | 408,023 | 16.76 | [16.64, 17.04] | pilot |
| `pairwise-gfcomplete-l2-1core` | 133,237 | 221,098 | 0.60 | [0.60, 0.61] | pilot |

Relative confidence-interval half-widths: `axpy-isal-l2-1core` 0.0030,
`matmul-m4rie-n128-1core` 0.0119, `pairwise-gfcomplete-l2-1core` 0.0151. The
maximum, 0.0151, is what the confirmatory addendum rounds up to its frozen
measurement resolution of 0.02.

The same medians expressed as rates, dividing each cell's declared size by the
median above:

| Cell | Rate unit | gf2 | Comparator |
|---|---|---:|---:|
| `axpy-isal-l2-1core` | 128 KiB region per call | 97.8 MB/s | 45.9 GB/s |
| `matmul-m4rie-n128-1core` | 128^3 multiply-accumulates per call | 307 MMAC/s | 5.14 GMAC/s |
| `pairwise-gfcomplete-l2-1core` | 131072 products per call | 984 Melem/s | 593 Melem/s |

The `selected_path` each arm reported inside the receipt names what actually
ran, so a reviewer can confirm the two sides of a cell are the operations the
mapping claims:

| Cell | Arm | selected path |
|---|---|---|
| `axpy-isal-l2-1core` | gf2 | `gf2-core/FieldVec::axpy/Gf2mElement/poly=0x11D` |
| `axpy-isal-l2-1core` | ext-isal | `isa-l/runtime-dispatched-simd/2.32.1/poly=0x11D` |
| `matmul-m4rie-n128-1core` | gf2 | `gf2-core/field::matrix::gemm/Gf2mElement/poly=0x11D` |
| `matmul-m4rie-n128-1core` | ext-m4rie | `m4rie/newton-john/20250128/poly=0x11D/mzed_mul` |
| `pairwise-gfcomplete-l2-1core` | gf2 | `gf2-core/gf2m::batch::batch_mul/u64-lanes/poly=0x11D` |
| `pairwise-gfcomplete-l2-1core` | ext-gfcomplete | `gf-complete/default/ceph/gf-complete@a6862d10c9db/poly=0x11D/pairwise` |

The renderer's conversion-cost table is empty for this receipt, which is the
projection of the fact that all three cells are kernel-isolated.

**The region gap is a representation gap, and it is nearly three orders of
magnitude.**
ISA-L is 469x faster at the same logical operation over the same 128 KiB of
field elements. The 128 KiB the comparator holds in bytes is 2 MiB as
`FieldVec<Gf2mElement>`, sixteen bytes per coefficient plus an atomic refcount
touched per element, so gf2's working set leaves L2 while ISA-L's sits inside
it. gf2 sustains 97.8 MB/s of logical region against ISA-L's 45.9 GB/s. This
is the survey's central measurement and it confirms the issue's premise: the
general runtime field abstraction is not a byte-region type, and the cost of
that is not a constant factor a tuned kernel would close.

**The matrix gap is 16.8x and is a different kind of gap.** Both arms run a
dense GF(2^8) product at n = 128 with the same asymptotics; M4RIE reaches its
through one `_mzed_mul_newton_john` call over bit-sliced `mzed_t` rows, which
the provenance record confirms is the path taken at every measured dimension
because the Strassen cutoff for GF(2^8) is n = 512. gf2 reaches its through
`Gf2mElement` arithmetic on a dense `FieldMatrix`. The gap is an order of
magnitude smaller than the region gap because a GEMM already amortises
element overhead across n operations per load.

**gf2 wins the pairwise cell, and the result is preserved.** gf2's
`gf2m::batch::batch_mul` is 1.66x faster than GF-Complete's fastest arbitrary
pairwise arm, so the cell records no material gap in the comparator's favour.
gf2 achieves this while moving eight times the bytes: `batch_mul` reads and
writes `u64` lanes at one element per lane, so its 131072 products move 3 MiB
of lane traffic against the comparator's 384 KiB of byte traffic. The cell says
gf2 is faster than a per-element indirect call, and it does not say gf2 is
faster than a region kernel, because no region kernel for this operation exists
in any of the three libraries. GF-Complete's arm here is a loop over
`gf_t.multiply.w32`, one indirect call per element, which is the fastest thing
GF-Complete has for it.

### Validated mappings, and the equivalents that do not exist

Every mapping below was validated against an independent shift-and-reduce
oracle written twice, in C (`bfx_ref_mul`) and in Rust (`reference_mul`),
sharing no code with any library under test, before the first timed window
opened. 39 external checks with 0 failures
([`conformance/externals.txt`](conformance/externals.txt)), 8 gf2-side checks
with 0 failures ([`conformance/gf2-side.txt`](conformance/gf2-side.txt)), the
full wrapper surface through the arms the campaign actually calls
([`conformance/ext-wrapper.txt`](conformance/ext-wrapper.txt)), and the shared
`test_gf2_8_field_axioms` suite over `Gf2mField::gf256()`
([`conformance/field-laws.txt`](conformance/field-laws.txt)).

Validated as genuinely the same operation:

- `FieldVec::axpy` against ISA-L `gf_vect_mad`, GF-Complete
  `multiply_region.w32` with `add=1`, and M4RIE
  `mzed_add_multiple_of_row`. One coefficient reused across the region,
  accumulation by XOR because the characteristic is 2, source and destination
  distinct on every arm.
- `field::matrix::gemm` square against M4RIE `mzed_mul`.
- `field::matrix::gemm` at generator shape against ISA-L `ec_encode_data`.
- `gf2m::batch::batch_mul` against a GF-Complete elementwise `multiply.w32`
  loop.

Recorded as missing rather than substituted:

- **No dense GF(2^8) matrix type in ISA-L or GF-Complete.** Both report
  `unsupported … the backend carries no dense GF(2^8) matrix type` for dense
  matmul. M4RIE is the only matrix comparator this survey has.
- **No region kernel for arbitrary pairwise multiplication anywhere.** ISA-L
  and M4RIE report `unsupported … the backend's region API assumes one reused
  coefficient`. The adapter returns `BFX_ERR_UNSUPPORTED` instead of looping
  their scalar multiply and presenting the result as a region arm.
- **No generator-matrix encode in GF-Complete or M4RIE.**
- **No region multiply-assign in M4RIE**, and no free-standing region API at
  all; its row form is checked separately and is what the axpy mapping uses.
- **ISA-L rejects region lengths below its backend minimum** (`len=1`), which
  the conformance run records as a note rather than a failure.
- **`gf_vect_dot_prod` is not `FieldVec::dot_product`** and no cell pretends
  otherwise. The former produces a region; the latter contracts two vectors to
  one field element. No arm offers the latter, so no cell exists for it.

The matrix multiply and the region multiply-XOR are in separate cells
throughout, with separate comparators, and neither substitutes for the other.

### The two fields, and how the measured cells honour the mismatch

[The 0x11B and 0x11D mismatch](#the-0x11b-and-0x11d-mismatch) above establishes
that the two are different fields on the same byte carrier, and why the
conformance run checks a full 65536-entry multiplication table for every arm
that accepts the polynomial rather than trusting the parameter. The run carries
the witness on both sides, `2 * 128` giving `29` under `0x11D` and `27` under
`0x11B` in `conformance/externals.txt` and `conformance/gf2-side.txt`
independently. What the measured cells do about it is this.

Every measured cell asks both of its arms for the same polynomial, and no cell
mixes two. All three pilot cells declare `poly=285` (`0x11D`), which the receipt
carries in each cell's case and in each arm's `selected_path`. No basis
conversion is performed anywhere in this survey, and none is needed; M4RIE's
`conversion_cling8.c` stays unused for that reason.

ISA-L is the binding constraint: it compiles `0x11D` in and `bfx_init` refuses
any other polynomial with its reason, so the conformance run records
`unsupported isa-l under 0x11B` rather than comparing incompatible fields.
GF-Complete and M4RIE take the polynomial as a parameter and both are validated
under `0x11B`.

The confirmatory family carries one `0x11B` cell,
`axpy-gfcomplete-11b-l2-1core`, against the comparator that offers it. It is
declared and unmeasured, like the other thirteen. Its gf2 arm is the runtime
`Gf2mField` constructed with that modulus, the same type the `0x11D` cells
measure, because that is what a `FieldVec` or `FieldMatrix` consumer uses. The
compile-time `Gf2mWide` GF(2^8) aliases are where `0x11B` appears in gf2's own
API surface, and no cell measures them: they are a separate type with no
`FieldVec` consumer path.

### Limitations of this evidence

Four of the defects the independent methodology review of `f547c394` found in
protocol version 1 reach this family. Three bear on this evidence and one does
not.

**The flagged-window rule pools both arms, and this family reproduces the
consequence directly.** `receipt.rs:1315` collects every window of both arms
into one vector and flags against the pooled median, so
`axpy-isal-l2-1core` reports 19 of 60 windows flagged, a fraction of 0.317
against a `max_flagged_fraction` of 0.10. Measured per arm the count is 0 of 30
on each side: the gf2 windows span a factor of 1.024 and the ISA-L windows a
factor of 1.019. The mechanism is arithmetic. With equal window counts and
fully separated arms the pooled median is the midpoint of the fast arm's
maximum and the slow arm's minimum, so the threshold becomes `fast_max +
slow_min`, which for this cell is 1 339 288 ns against a slow-arm minimum of
1 336 387 ns, 0.2% above it. Nineteen of the thirty gf2 windows land above a
threshold set essentially at their own minimum. The other two cells flag 0 of
60 for the same reason inverted: their comparators are slow enough
(432 479 ns and 140 323 ns) that `fast_max` exceeds the slow arm's absolute
spread. The flagged count in this receipt therefore measures arm separation
rather than instability. The condition is `fast_max < slow_max - slow_min`:
the pooled threshold falls inside the slower arm's own window spread exactly
when the faster arm's slowest window is smaller than that spread. That
predicate reproduces the flagged count of all three cells, true at 470x
separation and false at 16.8x and at 1.7x, so it is a statement about
separation measured against the slower arm's dispersion rather than about the
host. For arms as stable as these, spreads of 1.02, it starts to bite above
roughly a fortyfold separation; an arm with a 17% spread would trip it at
sixfold. Under a confirmatory role `axpy-isal-l2-1core` would be recorded
`unstable` and re-run on that basis alone. The confirmatory family's ISA-L
cells compare arms separated by hundreds, so each will need the same per-arm
recomputation before anything is concluded about its stability; how many trip
the pooled rule depends on their measured spreads and is not known from this
receipt. This receipt is exploratory, so nothing in it was gated on the count.

**The receipt binds only the last session's host observation.** The campaign
resumed once, and `benchmark-ab-runner.rs:216` binds `hostname;cpus=<affinity>`
as the campaign's host identity while finalization keeps a single `host` block.
That block is the session-2 observation at 09:39:51Z, load average
1.39/3.17/5.23. Cells `axpy-isal-l2-1core` and `matmul-m4rie-n128-1core` were
measured in session 1, whose observation was 09:36:45Z at load average
2.30/4.84/6.13, and the receipt does not carry it. Both observations survive in
the committed execution log as `driver-diagnostic` records, so the evidence is
recoverable, but the receipt alone understates the host variation across two
thirds of this campaign's cells. The identity string also carries no governor,
SMT or capability state, so a change in those between sessions would not be
caught by the resume check.

**The prior-confirmation trial ledger is not independently append-only**
(`receipt.rs:1565`). Both addenda in this family declare
`prior_confirmatory_trials: 0` with an empty `prior_trials`, so the ledger
currently protects nothing here. It becomes load-bearing when the confirmation
re-runs under version 2 and a second attempt has to append to it.

**The cold-cache calibration defect does not reach this family.**
`timing.rs:176` runs the workload to calibrate the call count before the first
timed window, so a cell declaring `cache_state: cold` is not cold. No cell in
either addendum declares `cold`: all three pilot cells and thirteen of the
fourteen confirmatory cells declare `warm`, and the fourteenth declares
`streaming`.

Two further limitations are properties of what has been run, not of the
protocol.

**No conversion, table-preparation, setup or output cost has been measured.**
All three pilot cells are `kernel-isolated` with `conversion_costs_included:
false`, and `conversion` is `null` in every pair record, so no `setup_ns`,
`pack_ns`, `unpack_ns`, `batch_fill_ns` or `dispatch_ns` figure exists yet. The
two `whole-consumer` cells that price these are declared in the confirmatory
addendum and unmeasured. The measured 469x region gap is therefore a
kernel-to-kernel comparison; the cost a byte-region consumer actually pays,
including packing into and out of `FieldVec`, is larger and is not yet
quantified.

**The `0x11B` region paths are validated one level below the cell that will
time them.** Under `0x11B` the conformance run checks the full 65536-entry
multiplication table on both sides, which establishes the arithmetic, but the
region paths themselves — gf2's `FieldVec::axpy` and GF-Complete's
`multiply_region.w32` — are checked only under `0x11D`. GF-Complete builds its
region tables from the polynomial it is given, so the region path is the level
where a polynomial-specific defect would live. The confirmatory cell
`axpy-gfcomplete-11b-l2-1core` therefore needs that check added to both
conformance binaries before it is timed, which is a pre-flight step for the
held campaign rather than a defect in what has been measured: no `0x11B` cell
has run.

**Coverage is three cells, not fourteen.** One cache regime (128 KiB, L2), one
matrix dimension (n = 128), one comparator per operation, one field. The L1,
L3 and streaming regimes, the n = 64 and n = 256 dimensions, the generator
encode, the cross-comparator axpy cells, the `0x11B` cell and both
whole-consumer cells are declared and unmeasured.

The confirmation will also report wider intervals than the pilot at the same
sample size: fourteen confirmatory cells raise the Bonferroni per-comparison
confidence from 0.95 to 0.99643.

### Criterion-by-criterion outcome

**REQ-01 — measurement contract, pinned versions, negative outcomes:
PARTIAL.** The pilot receipt pins the committed measurement contract
(`9f3c7563…`), the frozen protocol (`4cc897ed…`), the addendum schema
(`44dd132a…`) and the cell addendum (`23252109…`) by content digest, was
produced by release builds finished before timing under the exclusive lock with
runtime-observed topology and affinity, and passes the independent acceptance
tool. This survey changes no production kernel, so no before/after evidence is
owed. The negative outcome is preserved and reported: gf2 wins
`pairwise-gfcomplete-l2-1core` and the cell is retained with its interval. What
is missing is the confirmatory receipt, which is the current baseline this
criterion ultimately wants; it is held for protocol version 2.

**REQ-02 — pins, licenses, compiler targeting, selected backends: MET.**
[`arm-provenance.txt`](arm-provenance.txt) records M4RIE `20250128` over M4RI
`20260122` (GPL-2.0-or-later), GF-Complete `ceph/gf-complete@a6862d10c9db`
(BSD-3-Clause) and ISA-L `v2.32.1` at commit `7c3479e0a9da` (BSD-3-Clause),
each with the tarball SHA-256 or resolved commit that `survey/fetch-build.sh`
re-verifies on every run, plus the SHA-256 of each license file and the SHA-256
of every shared object the external arm links. Compiler targeting is recorded
as observed: `cc (GCC) 16.2.1 20260810` with `-std=c11 -O3 -march=native`
resolving to `znver3`, against `rustc 1.97.0` with `-C target-cpu=native` at
`opt-level=3`, LTO, one codegen unit.

The selected arithmetic backends are **read out of the loaded binaries at run
time, not out of the configure flags or a version string**. This is what
distinguishes the record from a claim.
[`survey/backend_provenance.c`](survey/backend_provenance.c) resolves each
library's live function pointer and reports the address it holds:
it walks ISA-L's multibinary dispatch slot and the PLT stub behind it to the
kernel address, reads GF-Complete's selected `gf_t` function-pointer fields
directly out of the initialised field structure, and interposes M4RIE's own
PLT-bound `_mzed_mul_strassen`, `_mzed_mul_newton_john` and `_mzed_mul_naive`
entry points to count which recursion an actual product takes.
[`survey/arm-provenance.sh`](survey/arm-provenance.sh) then resolves each
reported address against the library's symbol table with `nm --defined-only`,
because these kernels are file-local symbols that `dladdr` cannot name. The
resulting record names the kernels this host actually selects:
`gf_vect_mad_avx2`, `gf_vect_mul_avx`, `ec_encode_data_avx2` and
`gf_vect_dot_prod_avx2` for ISA-L; `gf_w8_split_multiply_region_sse` and
`gf_w8_default_multiply` for GF-Complete's default `w=8` variant;
`_gf2e_mul_table` for M4RIE element multiply, with `mzed_mul` observed taking
the Newton-John path at n = 64, 128 and 256 against a measured Strassen cutoff
of 512. The host is recorded as `avx2=1 avx512f=0`, which is what selects
those kernels.

Existing comparator harnesses and the existing M4RIE build provenance are
reused where they apply, and the pins were re-verified rather than assumed:
`fetch-build.sh` re-checks every digest and resolved commit on each run, and
the conformance suite establishes suitability of each pinned source for the
operations mapped to it before any of it is timed.

**REQ-03 — honest operation mapping: MET.** The mapping table in
[Operation mapping](#operation-mapping) states, for every gf2 entry point, the
field polynomial, the coefficient-reuse pattern, the accumulation semantics
(XOR, characteristic 2) and the overlap requirement (source and destination
distinct on every arm; `FieldVec::axpy` makes aliasing a compile error). Every
mapping is validated against the independent oracle before timing. Missing
equivalents are recorded as missing with the backend's own reason, in
`conformance/externals.txt` and in the table's `none` entries: no dense matrix
type in ISA-L or GF-Complete, no arbitrary-pairwise region kernel anywhere, no
generator encode in GF-Complete or M4RIE, no `dot_product` equivalent at all. A
matrix multiply is never presented as a region multiply-XOR: the two live in
separate cells with separate comparators, and `gf_vect_dot_prod` is explicitly
rejected as a comparator for `FieldVec::dot_product`.

**REQ-04 — validate before timing, price what the consumer pays: PARTIAL.**
The validation half is met and is a hard gate in the launcher: four stages
(shared field-law suite, external conformance, FFI wrapper, gf2 adapter) run
before the first timed window, each exiting non-zero on the first mismatch, and
their outputs are copied into the receipt directory: 39 external checks and 8
gf2-side checks with 0 failures, the wrapper surface agreeing with the
reference on every backend, and the shared axiom test passing. The distinction
between arbitrary pairwise multiplication and fixed-coefficient reuse is
honoured in the cell design and measured: `pairwise-gfcomplete-l2-1core` and
`axpy-isal-l2-1core` are separate cells against separate comparators, and
the receipt's `selected_path` records which entry point each arm took. The cost
half is unmet in measurement: conversion, table preparation, setup and output
costs are declared in two `whole-consumer` cells and in the per-arm
`setup_ns`/`pack_ns`/`unpack_ns`/`batch_fill_ns`/`dispatch_ns` reporting, and
neither cell has run. The measured numbers are kernel-isolated only.

**REQ-05 — frozen addenda, baseline receipts, cache regimes, field mismatch:
PARTIAL.** Both family addenda are frozen and committed:
[`addendum-byte-field-arms-pilot.json`](addendum-byte-field-arms-pilot.json),
snapshotted by exact bytes into the pilot receipt, and
[`addendum-byte-field-arms.json`](addendum-byte-field-arms.json), which derives
its numeric settings from the committed pilot receipt and pins it by SHA-256.
The `0x11B`/`0x11D` mismatch is documented with a measured witness, the absence
of an ISA-L `0x11B` arm is recorded with the library's own reason instead of a
substituted comparison, and no basis conversion is used or needed because both
arms of every declared cell take the same polynomial. Coverage is the shortfall:
one baseline receipt exists and it is exploratory, covering three cells at one
size and one cache regime. The fourteen admissible cells across L1, L2, L3 and
streaming regimes, three matrix dimensions, both metric kinds and both fields
are declared and frozen but unmeasured, pending protocol version 2.
