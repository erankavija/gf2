# Byte-field acceleration in the vector and matrix consumers

> **Diátaxis Type:** Explanation

## Question and evidence

`FieldVec::axpy` and the batched `FieldMatrix` operations are concrete library
consumers, and a region multiply-XOR microbenchmark does not justify a new
production API or a storage redesign for them. This assessment asks whether
byte-oriented GF(2^8) arithmetic makes those consumers faster **after every
cost the byte-oriented route adds**, and ends in a proceed or no-proceed
decision under rules frozen before the measurements.

Byte-oriented arithmetic here means what the pinned comparison libraries do:
the products of one fixed coefficient with all 256 field elements are a
256-byte table, so a multiplication by a reused coefficient is one indexed
load and an accumulation is one XOR. The assessment's central observation is
that this is a statement about *arithmetic*, not about *storage*: the table is
indexed by a field element's canonical byte whatever width the element is
stored in, so a prototype can reach it without packing anything.

The evidence has three layers, each in its own place:

- a **profile** of both routes on every consumer, which says how a cost is
  composed: the route selected at run time, allocation counts, coefficient
  reuse counts, table preparation, the conversion probes and hardware
  counters. This evidence is not published with this receipt set: its
  scheduled, resumable profile campaign and exact output path are recorded in
  [Profile completion](#profile-completion), rather than being inferred from
  the A/B receipts;
- three **A/B campaign families** under
  [protocol version 4](../f547c394/protocol.md) and the
  [measurement contract](../1a379447-zen3-cpu-performance/measurement-contract.md),
  each with a pilot and a confirmation, which decide: the
  [generated tables](../../bench_results/19513245/tables.md) carry every cell's
  paired estimate with its bootstrap interval;
- the **generated code** of both routes under
  [`survey/asm/`](survey/asm), which explains the limits the campaigns measure.

This report states no measured value. Each conclusion points to its source: a
section of the generated tables, written "tables § `<receipt>`, row `<cell>`";
a row of the profile summary, written "profile § `<section>`, row `<case>`"; a
file of committed evidence; or a claim ID in backticks, which resolves in
[`survey/source-evidence.json`](survey/source-evidence.json).

| Command | Output |
|---|---|
| `../../bench_results/19513245/run-consumer-bytefield.sh build` | the arm, the field-law binary, the runner, the acceptance and analysis tools, every correctness check and the provenance under [`conformance/`](conformance) |
| `survey/smoke-arms.sh` | every arm and every case operation carried through the real `benchmark-ab-runner` on a throwaway family under `target/`, before any campaign is queued |
| `../../bench_results/19513245/run-consumer-bytefield.sh plan\|run\|finalize\|window CAMPAIGN` | the six campaigns; each receipt's `launcher.log` records every session command |
| `survey/make-addenda.py` | the three pilot addenda |
| `../c7113c5a/survey/freeze-confirmation.py` | the three confirmation addenda, frozen from the committed pilot receipts, each with its derivation record beside it |
| `survey/make-plan.py` | the runner plans |
| `survey/make-producing-inputs.py` | [`survey/producing-inputs.json`](survey/producing-inputs.json), the producing closure every receipt pins |
| `survey/make-source-evidence.py` | `survey/source-evidence.json`: every code claim below, cited by claim ID |
| `survey/run-profile.sh` | the profile series, its host record, its counters and its summary |
| `survey/disassemble.sh` | [`survey/asm/`](survey/asm): the annotated release disassembly of both measured executables and their instruction mix |
| `../6c6b09b1/survey/analysis` (`survey-analysis tables`) | [the generated tables](../../bench_results/19513245/tables.md) |

The host is `fraktaali`, a Ryzen 9 5900X; each receipt records its CPU model,
kernel, governors, SMT state and CPU mask per session, and the profile records
the same in its own host file. Timed work runs only under
`dev/scripts/ccx1-bench-flock.sh --full-host`, builds and checks under
`scripts/cargo-budget.sh`. Every executable is a Rust 1.95 release build.
Seeds: each campaign seed in the launcher drives the counterbalanced pair order
and the acceptance bootstrap through `Xoshiro256StarStar` seeded by
`SplitMix64` [BlackmanVigna2021] [Steele2014], both in tuning-campaign-support
0.1.0 (`dev/tools/tuning-campaign-support/src/abtest.rs`); each cell's workload
seed in its addendum generates the operands with that crate's `SplitMix64`
through `OperandStream` (`../6c6b09b1/survey/arm-common/src/lib.rs`); the
profile and the validation each carry their own fixed seeds in their source.

## Reuse rather than restatement

The two arms of every cell are two routes of one executable,
[`survey/consumer`](survey/consumer), which consumes `gf2-core` by path as an
external user does. It reuses the byte-field comparison survey's committed
harness rather than restating it: `byte-field-arm-common` owns the runner's
wire contract, the timing protocol and the conversion-cost accounting, and
`byte-field-gf2-side` owns the two GF(2^8) element representations, their byte
conversions and the current consumer entry points, which are the same calls
that survey measured. The tables come from that survey's committed
`survey-analysis` generator, and the confirmation addenda from the canonical
freezer `../c7113c5a/survey/freeze-confirmation.py`. What this assessment adds
is the prototype, the consumer profile, the three families and the decision.

## The consumers as they stand

The three things the issue asks to identify — the current representation, the
coefficient reuse and the actual accelerated path — separate the consumers
sharply.

**Representation.** A `FieldVec<Gf2mElement>` element is a `u64` value beside a
reference-counted handle on the field parameters (`element-arc-handle`), and
every element the field constructs clones that handle
(`element-clones-handle`), so writing one element back costs an atomic
increment and an atomic decrement. A `FieldVec<Gf2mWide<1, _>>` element is one
`u64` word and costs nothing to construct. Neither is a byte.

**Coefficient reuse.** `FieldVec::axpy` takes one coefficient and one vector
(`axpy-signature`), so the coefficient is reused once per element of the
vector; the profile reports that count per case (profile § Allocation, reuse
and conversion, the `reuse` column). A dense product reuses each left-operand
element across a whole row of the right operand, which is why the prototype's
product is a sequence of region multiply-accumulates
(`prototype-gemm-reuse`). Arbitrary independent multiplication reuses nothing,
which is what makes it the control (`prototype-pairwise-no-reuse`).

**The accelerated path — and where there is none.** `FieldVec::axpy` consults
exactly one hook (`axpy-hook`), whose default returns false
(`axpy-hook-default-false`) and whose only override in the workspace is
`Fp<P>`'s (`axpy-hook-only-fp`). Both GF(2^8) representations therefore run
the scalar element loop (`axpy-scalar-loop`): one element clone, one element
multiply and one add-assign per element. The element multiply is not a table
lookup either: `Gf2mField::new` builds no log or antilog table
(`gf256-builds-no-tables`), so `Gf2mField::gf256()` has none and the multiply
reaches the carry-less-multiply and Barrett kernel through a function pointer,
one indirect call per element (`element-mul-clmul-route`). The `Gf2mWide`
multiply allocates its unreduced product on the heap
(`wide-mul-allocates`), so one element multiplication allocates; the profile
counts those allocations per call (profile § Allocation, reuse and conversion)
and the instruction mix counts the reference-count updates and indirect calls
the two routes contain (`survey/asm/*/instruction-mix.txt`).

`field::matrix::gemm` is the one consumer that does reach a batch kernel, and
it reaches a different one per representation. It allocates its product in
every call (`gemm-fresh-output`) and transposes the right operand in every call
(`gemm-transposes`), then consults the whole-product hook
(`gemm-whole-hook`): `Gf2mWide<1, _>` overrides it
(`wide-whole-gemm-override`) and takes the whole product through the
carry-less-multiply GEMM kernel, copying both operands into flat `u64` buffers
and the result out of a third (`wide-gemm-flattens`), while `Gf2mElement`
declines it and takes the batch dot product once per output cell
(`gemm-per-cell-hook`, `element-batch-dot-override`).

`FieldMatrix::matvec` reaches no batch kernel at all. Its one hook
(`matvec-hook`) has the same default and the same sole `Fp<P>` override
(`matvec-hook-default-false`), so it falls through to one dot product per row
(`matvec-scalar-rows`), and that dot product is a scalar multiply-and-accumulate
chain for GF(2^m) (`dot-product-scalar-chain`). `FieldVec::dot_product` reaches
the same chain. This is a finding about the current library, not about the
prototype: the operation the issue calls a batched `FieldMatrix` operation is
batched in name only for GF(2^8) outside `gemm`.

`gf2m::batch::batch_mul` dispatches to the carry-less-multiply batch kernel at
run time (`batch-mul-dispatch`), and its raw form is crate-internal
(`batch-mul-raw-internal`), so only gf2-core's own consumers reach it without
a field context. It is the strongest current GF(2^8) route in the library and
therefore the control's baseline.

## Three shapes, kept apart

The issue requires the vector-shaped, matrix-shaped and region-shaped
workloads to stay distinct rather than assuming one kernel serves all three,
and they are three families with three append-only ledgers and no shared cell:

- **vector-shaped** (`bytefield-consumer-vector`, its `axpy-*` cells): the
  operands are the `FieldVec` the consumer already holds and the window times
  the call. The prototype converts nothing here; it prepares one coefficient
  table inside the call, because the consumer passes a new coefficient with
  every call, and writes its result back through the representation the vector
  stores;
- **region-shaped** (the same family's `region-*` cells): a byte region in and
  a byte region out, so the current route pays its conversion into the
  representation and out again and the prototype works on the region itself.
  This is the shape the comparison survey measured the pinned libraries in, and
  it is a different question from the vector one: the same arithmetic, a
  different boundary;
- **matrix-shaped** (`bytefield-consumer-matrix`): dense products at three
  square dimensions, on each route's own representation and across the
  `FieldMatrix` boundary a library consumer starts and ends at. There the
  prototype is the route that pays conversion.

The control (`bytefield-consumer-control`) is a fourth question and not a
shape: arbitrary independent multiplication, which reuses no coefficient. It
exists so that any vector or matrix result can be attributed to coefficient
reuse or to byte arithmetic as such, and it shares no cell with either reuse
family.

## Comparator semantics, and the one field

Every comparison in this assessment is between two gf2 routes over one field,
so it needs no external library; the external baselines at these shapes are the
committed confirmed evidence of `6c6b09b1`, and the mapping below says which of
its cells describe which consumer. A cell of that survey and a cell of this one
are never divided into each other: they are separate campaigns with separate
families, and a quotient across them would carry no interval.

| Consumer measured here | Matching external comparator, and why only that one | Where its gap is recorded |
|---|---|---|
| `FieldVec::axpy`, region-shaped, one coefficient reused, accumulation by XOR, source and destination distinct | ISA-L `gf_vect_mad` [IsaL2026] and GF-Complete `multiply_region.w32` [GfComplete2026], the only pinned fixed-coefficient region kernels; M4RIE `mzed_add_multiple_of_row` [Mfourrie2026] at the one-row shape | `6c6b09b1` tables § `6c6b09b1-v4-r1-region-axpy-confirmation` |
| `field::matrix::gemm`, square | M4RIE `mzed_mul` only: neither ISA-L nor GF-Complete has a dense matrix type | `6c6b09b1` tables § `6c6b09b1-v4-r1-matrix-product-confirmation` |
| `gf2m::batch::batch_mul`, arbitrary pairwise | each library's single-element multiply applied per byte, because no pinned library has a region kernel for distinct pairs | `6c6b09b1` tables § `6c6b09b1-v4-r1-pairwise-control-confirmation` |
| `FieldVec::axpy`, vector-shaped in place on a `FieldVec` | none: no pinned library holds a `FieldVec`, so no external arm has this boundary | not measurable externally; recorded as an absence |
| `FieldMatrix::matvec`, `FieldVec::dot_product` | none: ISA-L's `gf_vect_dot_prod` computes one dot product per byte position across regions and returns a region, which is a different operation | not measurable externally; recorded as an absence |

Field polynomial, basis and output accumulation are preserved throughout:
every cell of this assessment and every cell of `6c6b09b1` works in GF(2^8)
modulo 0x11D, accumulates by XOR, and reads a source distinct from its
destination. There is no basis conversion anywhere, because there is only one
field.

The 0x11B incompatibility bounds what can be inherited, and it is a property
of the fields rather than of any implementation. 0x11B and 0x11D are distinct
fields on the same byte carrier. `Gf2mField::gf256()` builds 0x11D
(`gf256-polynomial`); gf2 ships no GF(2^8) field modulo 0x11B, and ISA-L
implements no polynomial but 0x11D, so a 0x11B consumer inherits GF-Complete
and M4RIE baselines from `6c6b09b1` and no ISA-L baseline at all, and reaching
ISA-L would need an isomorphism adapter that neither survey builds or times.
The prototype is not bounded that way: it builds its table from the field's own
reduction polynomial, so it serves 0x11B and 0x11D alike, and the validation
covers every byte coefficient over both. No cell measures 0x11B, because there
is no 0x11B consumer in gf2 to measure and no 0x11B external baseline to
compare against.

One documentation defect sits exactly here and is not this issue's to fix: the
rustdoc of `gf256()` names x^8+x^4+x^3+x+1, which is 0x11B
(`gf256-doc-polynomial`), and calls the field the one used in AES
(`gf256-doc-aes`), while the code builds 0x11D. It is tracked as `835f34f0`;
this issue changes no production file.

## The prototype, and its MSRV feasibility

The measured prototype is [`survey/consumer/src/table.rs`](survey/consumer/src/table.rs)
and the routes in [`survey/consumer/src/lib.rs`](survey/consumer/src/lib.rs).
A coefficient table is built by the doubling recurrence over the field's own
reduction polynomial (`prototype-table-recurrence`): 256 operations, no
gf2-core arithmetic, which is why the same code serves the validation as an
independent oracle. The routes are then:

- `axpy_region`, one indexed load and one XOR per element on a byte region,
  with the destination `&mut` and the source `&` (`prototype-region-kernel`);
- `axpy_field_vec`, the same arithmetic in place on the consumer's own
  `FieldVec`, writing back through the representation it stores
  (`prototype-in-place-write`);
- `gemm_region` and `matvec_region` over the full 256-by-256 table, 64 KiB,
  which depends on the field alone;
- `pairwise_region`, the control's route, one full-table lookup per element.

Every route is safe scalar Rust and names no intrinsic, so its MSRV
feasibility is established by the arm's own compile at Rust 1.95, recorded in
[`conformance/build-record.txt`](conformance/build-record.txt) with the
toolchain version. The generated code shows what that compiles to: the region
kernel's loop is a byte load, a table load and an XOR, twice unrolled, with no
call and no reference-count update inside it
(`survey/asm/consumer-profile/prototype-axpy-region.asm.txt`, and the zero
counts of that symbol in `instruction-mix.txt`).

A **vectorised** successor would need intrinsics, and an instruction list is
not feasibility evidence for it, so
[`survey/msrv-intrinsics.rs`](survey/msrv-intrinsics.rs) names every intrinsic
such a kernel uses — the AVX2 split-table byte shuffle and the byte gather and
scatter a `u64`-lane representation needs around it, each function carrying the
`target_feature` attribute a shipped kernel would carry — and is compiled at
the same toolchain, with the result in
[`conformance/msrv-intrinsics.txt`](conformance/msrv-intrinsics.txt). That
compile is the feasibility evidence a vectorised proposal would start from; no
cell of this assessment measures such a kernel.

## Correctness before timing

The launcher's `build` step stops at the first failed check, and the producing
manifest pins the evidence it writes:

- [`conformance/prototype-vs-consumers.txt`](conformance/prototype-vs-consumers.txt):
  every entry of every coefficient table and of the full table against a
  bit-by-bit oracle written without gf2-core, for 0x11D and 0x11B; the region
  route for every one of the 256 byte coefficients over both polynomials, with
  the zero coefficient checked for leaving the destination unchanged and the
  one coefficient for reducing to XOR; the empty, single-element,
  word-boundary and odd tail lengths on the region route, the in-place vector
  route and `FieldVec::axpy` together; regions at every source and destination
  byte offset from 0 to 7; the overlap contract; the in-place route against
  `FieldVec::axpy` for both element representations over both polynomials; the
  dense product and the matrix-vector product against `gemm` and `matvec` at
  boundary and measured shapes; the arbitrary pairwise route against
  `batch_mul`; and the backend availability this host reports at run time. The
  file ends in its pass line.
- [`conformance/field-laws.txt`](conformance/field-laws.txt) and
  [`conformance/field-laws-element.txt`](conformance/field-laws-element.txt):
  the shared field-law suite, `test_field_axioms`, over
  `Gf2mWide<1, Gf256x11d>` and `Gf2mWide<1, Gf256x11b>`
  ([`survey/field-laws`](survey/field-laws)) and over `Gf2mField::gf256()`
  through gf2-core's own `test_gf2_8_field_axioms`.

The overlap contract is the consumer's own and the prototype inherits it
rather than widening it: `FieldVec::axpy` takes `&mut self` and `&Self`
(`axpy-signature`), so the borrows forbid an aliasing call, and every prototype
route carries the same signature shape. What remains checkable is that no route
writes through its source, and the validation checks that for both.

The arms also have to speak the runner's wire.
[`survey/smoke-arms.sh`](survey/smoke-arms.sh) measures a throwaway family on a
throwaway ledger under `target/`, at the smallest pair count the protocol
allows, over five cells that between them name all six arms, all three case
operations and both metric kinds, and fails unless every arm wrote result lines
into a paired cell. It is a precondition of queueing a campaign, because a
campaign that dies on its first arm spends a benchmark window and measures
nothing, which is how `6c6b09b1` lost three of them.

## Families and campaigns

Three questions, three families, each with its own append-only ledger opened
empty before its first campaign:

- `bytefield-consumer-vector`
  ([pilot addendum](addendum-v4-vector-pilot.json),
  [confirmation addendum](addendum-v4-vector-confirmation.json)): the
  vector-shaped cells at 4 KiB, 128 KiB and 8 MiB warm and at 2 MiB rotated
  through eight banks, for both element representations, and the region-shaped
  cells at 4 KiB and 128 KiB for both;
- `bytefield-consumer-matrix`
  ([pilot addendum](addendum-v4-matrix-pilot.json),
  [confirmation addendum](addendum-v4-matrix-confirmation.json)): square
  products at n = 64, 256 and 512 for both representations, and the
  `FieldMatrix`-boundary cells at n = 256 for both;
- `bytefield-consumer-control`
  ([pilot addendum](addendum-v4-control-pilot.json),
  [confirmation addendum](addendum-v4-control-confirmation.json)): arbitrary
  pairwise products at 4 KiB and 128 KiB kernel-isolated and at 128 KiB from
  and to a byte region.

Every cell is single-core: none of the measured entry points and no part of the
prototype has a parallel form in these builds, so a 6-, 12- or 24-CPU arm would
time a harness thread pool rather than either route, and each addendum states
that rather than fabricating an arm.

Every cell is an `improvement` cell whose baseline is the current gf2-core
route and whose candidate is the prototype, so a speedup above one favours the
prototype and the protocol's ordinary reading applies throughout: `pass` means
the prototype is ahead by more than the family's worthwhile speedup at the
family confidence, `not-material` means the interval clears neither margin, and
`fail` means the prototype is more than the equivalence margin behind. No cell
is a comparator-gap cell, so no cell needs the inverted reading the byte-field
comparison survey's control receipts need.

Both arms of every cell are conservative-portable builds with no target-cpu
setting. That choice is part of the question rather than a convenience: the
carry-less-multiply kernels the current routes reach are dispatched at run time
and are present in such a build, so the baseline loses nothing, while a
`-march=native` build would give the prototype's scalar loop an
auto-vectorisation that no shipped gf2 build provides and would answer a
question about this host instead of a question about the library.

### The effect rule, frozen before measurement

The family's worthwhile speedup is 1.30 and its equivalence margin 1.15, both
declared with the pilot and unchanged by the confirmation, and both are
complexity judgements rather than measurement ones. Adopting the prototype
means gf2-core carries a second GF(2^8) multiplication implementation: a table
type with a lifecycle the consumer manages, a dispatch branch in each consumer
that uses it, and a conformance surface over every byte coefficient of every
supported polynomial. Thirty percent of the consumer's measured runtime is the
smallest return that repays a permanent second path. The complexity budget
allows four hundred added source lines and no new unsafe kernel, because a
vectorised successor would be a separate proposal with its own feasibility
evidence. Each addendum carries both rationales in full.

Each confirmation is frozen from its own family's committed pilot receipt
before any confirmatory trial, by the canonical freezer
`../c7113c5a/survey/freeze-confirmation.py`: the measurement resolution is the
pilot's widest relative bootstrap half-width at the alpha its own frozen
addendum and ledger imply, rounded up to two decimal places; the resolution
evidence pins that pilot receipt by path and SHA-256; the retained cells keep
their pilot declaration with the role changed to `confirmatory`, so a confirmed
cell measures the workload, size, seed, cache state, metric kind and arms its
pilot measured; and the freezer refuses an addendum whose margins do not
strictly exceed one plus that resolution. Each derivation record is committed
beside its addendum. The campaign seed is fresh, so the pair order and the
bootstrap stream are; the workload seeds stay the pilot's, so the confirmation
measures the workload whose resolution it declares.

The confirmatory budget is what bounds each confirmation's breadth. A pilot
addendum declares only exploratory cells, so its reservation spends zero
comparisons and each family's confirmation is its first confirmatory attempt;
the attempt budget is then divided by the attempt's own confirmatory cell
count, and P-20 additionally requires at least twenty expected bootstrap draws
in each tail at that corrected confidence. Recomputed from
`dev/tools/tuning-campaign-support/src/trial_ledger.rs` and `receipt.rs` for a
first attempt with the frozen resample count, that admits at most six
confirmatory cells. The vector and matrix families therefore confirm six of
their cells each and leave the rest of their pilot breadth as exploratory
evidence; the control family has three cells and confirms all three. Each
freezer derivation record names which cells were retained, which were dropped
and why.

## Results

### Profile: how each route's cost is composed

The profile campaign is pending its scheduled benchmark window. It measures
both routes on every consumer, including `FieldMatrix::matvec`, which no
campaign cell measures. Its generated summary will report the observed route,
wall-time session interval, exact allocations, coefficient reuse, table
preparation, conversion probes and hardware counters for the cases in
`survey/counter-cases.txt`. Until those immutable session records exist, this
report makes no profile-derived claim about a consumer's allocation, conversion
or counter share.

### Pilots

In each of the three pilots every declared cell starts, checkpoints and
completes exactly once across the campaign's sessions, on the launcher's
exploratory pair count with the frozen window count; each campaign ends in its
own `complete` terminal record; and `benchmark-acceptance` recomputes each
receipt to the verdict, finding count and `qualifies` false its header lines in
the tables carry, which is what an exploratory-only receipt is (tables §
`19513245-r1-vector-pilot`, § `19513245-r1-matrix-pilot`,
§ `19513245-r1-control-pilot`, each receipt's `acceptance-summary.json`). No
cell of any of the three is unstable: the flagged-window counts are zero
throughout (tables § Cells, the flagged-windows column of each receipt).

**The vector and region shapes.** The prototype is ahead of the current route
in every cell of the vector family, for both element representations, in all
four cache regimes and at both boundaries (tables §
`19513245-r1-vector-pilot` § Cells). The pilot separates the two shapes and the
two representations in the way the rest of the report explains: the vector
shape returns least on `Gf2mElement`, because there the prototype still pays
one reference-counted write-back per element; it returns far more on
`Gf2mWide<1, _>`, whose current multiply allocates; and the region shape
returns more than the vector shape on both, because the current route pays a
conversion there that the prototype does not. The cache regime moves the result
by far less than either of those two choices does.

**The matrix shape.** The prototype is ahead at every square dimension for both
representations and in both `FieldMatrix`-boundary cells (tables §
`19513245-r1-matrix-pilot` § Cells). The gap is much larger on the element
representation, whose current route calls the batch dot product once per output
cell and packs and folds `O(k)` values for each, than on the wide one, which
takes the whole product through the carry-less-multiply GEMM kernel. The
dimension moves the result little in either representation, and the
`FieldMatrix`-boundary cells sit close to their kernel-isolated twins, so the
conversion the prototype adds to reach a byte matrix does not eat its gain at
n = 256.

**The control.** The prototype is also ahead of `gf2m::batch::batch_mul` in
every control cell (tables § `19513245-r1-control-pilot` § Cells), by much less
than in either reuse family. This is the result the control exists to produce,
and it is more informative than the reuse hypothesis alone would have been: a
single full-table lookup per element already beats gf2's strongest current
GF(2^8) route, and coefficient reuse then multiplies that margin by more than
an order of magnitude in the region shape. Byte-oriented arithmetic is
therefore not only a reuse effect, and the reuse families' results are not
explained by reuse alone.

### Confirmations

Each family's confirmation measures the cells its frozen addendum declares, on
the protocol's confirmatory pair count, and nothing else. Every declared cell
starts, checkpoints and completes exactly once across the campaign's sessions;
each campaign ends in its own `complete` terminal record; no cell is flagged
unstable, the flagged-window counts being zero throughout; and
`benchmark-acceptance` built from this tree recomputes each receipt to the
verdict, qualification and finding count its committed acceptance summary and
tables header lines carry (tables §
`19513245-r1-vector-confirmation`, § `19513245-r1-matrix-confirmation`,
§ `19513245-r1-control-confirmation`, each receipt's
`acceptance-summary.json`). Each family ledger's confirmation line spends one
comparison per confirmatory cell, the first spend of its chain
(`../../bench_results/19513245/*-family-ledger.jsonl`, the second reservation
of each).

The vector confirmation ran in more sessions than its cells needed. The session
that measured its second cell was stopped from outside the campaign before the
cell checkpointed; the next session closed it with an `interrupted` record,
journalled one `cell-abandoned` record naming the unfinished attempt, and
measured that cell again from its first pair. The abandoned attempt's
executions stay in the execution log and enter no checkpoint and no receipt,
which the acceptance tool verifies (P-11); the campaign's reservation is the
one it already held, so the interruption spent no additional comparison.

## Release assembly attribution

The release disassembly is a source artifact, not a reconstruction from prose.
[`survey/asm/consumer-arm/index.txt`](survey/asm/consumer-arm/index.txt) and
[`survey/asm/consumer-profile/index.txt`](survey/asm/consumer-profile/index.txt)
each record the executable digest that their annotated symbols describe. Those
digests match the corresponding `consumer-arm` and `consumer-profile` entries
of [`conformance/build-record.txt`](conformance/build-record.txt), whose
checksum verification is the release-assembly attribution check. The issue
changes no production SIMD source, so `asm-artefact-present` has no SIMD source
change to pair with a crate-local assembly artifact.

## Profile completion

REQ-02 remains pending the scheduled profile campaign. Its output directory is
`dev/bench_results/19513245/r1-consumer-profile`; its append-only
`repetitions.log`, nine `rep-*/cases.json` records, host record, counter files
and generated `profile-summary.md` are the authority. The command is
`env PATH=/home/vkaskivuo/.cargo/bin:/usr/local/bin:/usr/bin dev/active/19513245/survey/run-profile.sh dev/bench_results/19513245/r1-consumer-profile`
from this worktree's root, only with `GF2_BENCH_WINDOW=1` in the benchmark
window. It is resumable and rejects a changed profile executable. Once it has
completed, regenerate the evidence table and revise this section from the
resulting structured records; do not infer profile costs from the A/B receipts.

## Decision

The qualifying confirmation receipts establish that the table prototype merits
the scheduled profile, but they do not authorize a production API, storage
redesign or implementation change. The frozen decision for this evidence set is
**no-proceed to shipping**: retain the established production routes while
REQ-02 is pending. A later positive production proposal requires the completed
profile, a reviewed library-layer design and explicitly tracked implementation
scope before any production code is shipped. The region workload remains a
separate demonstrated shape; it does not imply a `FieldVec` or `FieldMatrix`
storage redesign.
