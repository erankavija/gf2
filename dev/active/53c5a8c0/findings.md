# Carry-less multiplication crossovers and wide polynomial competitiveness

> **Diátaxis Type:** Explanation

This report explains what the two frozen families of issue `53c5a8c0` measure on
this Zen 3 host, what each outcome means for the code the study did not change,
and which selector recommendation the evidence supports. Every quantitative
statement below is a conclusion with a pointer to the generated table row,
acceptance summary or receipt that holds the figure; no measured number is
copied into this prose.

The external comparator is the gf2x library [GfTwoX2026]. Its exact source
revision, build configuration and loaded object are pinned by each polynomial
receipt's arm record, projected in the *Arm executables* table of
`dev/bench_results/53c5a8c0/tables.md`.

## The answer

1. **The batched carry-less paths are not materially worse at any size this
   study measures, and no length gate is warranted for any of them.** The crossover family's
   confirmation records the batched raw product, the dispatched dot product at
   sixty-four elements and the batched reduced element-wise product as passes,
   and the dispatched dot product at eight elements as not material inside the
   frozen equivalence margin. Its long holdout cell passes in the same
   direction. See the *Family `gf2m-clmul-crossover`* confirmation table of
   `dev/bench_results/53c5a8c0/tables.md`.

2. **There is no measured length range in which the per-element dot product is
   the better path.** The short holdout cell, whose arms are swapped by declaration before
   any pilot ran, asks whether the per-element dot product is *not worse* at
   sixteen elements. It regresses and is recorded as a fail. A length gate below
   the threshold would have relied on exactly that hypothesis, so the
   recommendation adds no gate and retains the dispatched path at every measured
   length.

3. **gf2's long polynomial product beats the pinned gf2x build at the two widths
   a wide carry-less kernel covers and loses sharply at the measured widths
   above them.** Four-word and nine-word products are recorded as
   regressions of the gf2x candidate, which in this family's frozen direction
   means gf2 is faster; the sixteen-word product is recorded as a pass, which
   means gf2x is faster there. See the *Family
   `wide-polynomial-competitiveness`* confirmation table.

4. **No cell of either family qualifies for production selection.** Both
   confirmations are accepted with `qualifies` false, stated on each table's
   Source line. The study changes no multiplication path and adopts no gf2x
   dependency.

5. **The study falsifies one documented claim preserved by its source snapshot.** The
   receipt-local batch-module source carries advice about the length below which the batch
   kernel's setup overhead outweighs its throughput advantage. The
   confirmation's eight-element reduced batch cell passes in favour of the
   batched path, which contradicts that advice at eight elements, and the
   pilot's thirty-two- and thousand-element cells extend the same direction. The
   module states the measured boundary and cites the cell, corrected in its own
   commit with the receipts as its evidence. The source-evidence ledger records
   the claim as it stands at the commit the study measured, which is what makes
   the contradiction checkable.

## What each cell measures, and what it does not

The five operations are never averaged together and never substituted for one
another. Their definitions are frozen once in
`dev/active/53c5a8c0/survey/cells.py`, which both the addendum generator and the
plan generator read, so a cell's identity, workload, metric kind and arm
assignment cannot drift between the two.

- **Raw batch** (`raw-batch-*`) is `count` independent unreduced
  sixty-four-by-sixty-four-bit carry-less products. The metric kind is
  kernel-isolated. **No external library exposes this as one call.** The
  crossover family's raw-batch cells therefore compare two gf2 entry points, not
  gf2 against a comparator.
- **Reduced element-wise batch** (`field-batch-mul-*`) is the whole-consumer
  element-wise product of `count` field element pairs, every result reduced.
- **Dot product** (`field-dot-*`) is the whole consumer: building both
  `FieldVec` operands, extracting their element values, the carry-less products,
  the XOR accumulation, the single Barrett reduction and the extraction of the
  field value. Both arms pay the identical packing.
- **Long polynomial product** (`poly-*`) is the unreduced product of two
  `words`-word polynomials over the binary field. This is the operation an
  external long-product library performs, so these are the only
  operation-equivalent comparator cells.
- **Whole-consumer wide field product** (`wide-field-*-composed`) is that long
  product followed by its Barrett reduction. The name records the composition:
  gf2x supplies no field reduction, so its arm composes one gf2x product with
  gf2's own wide Barrett reducer for the same field, which is what a consumer
  replacing only the product stage would pay.

Two cells are composed rather than equivalent, and say so in their identifiers
and in their addendum entries:

- **`raw-batch-1024-composed`** issues one single-word gf2x product per pair
  against one gf2 batch call over the whole vector. gf2x exposes no independent
  batch entry point, so its per-call cost is inside every product. The cell
  records what a consumer reaching gf2x through its public interface pays and
  **states no gf2x basecase rate.**
- The two **`wide-field-*-composed`** cells include the field-reduction and
  representation costs on both sides, which the addendum marks through
  `conversion_costs_included`.

Which conversion costs a cell includes is visible per cell in the *Untimed
conversion diagnostics* tables, which report setup, packing, one complete call,
the reduction or product extraction, and the runtime capability detection, each
as a descriptive median with no interval.

## The baseline is the pinned current implementation

Neither family measures a proposed implementation. The crossover family's two
arms are two entry points of the same pinned executable, selected by an
environment variable, so a measured ratio attributes to the entry point rather
than to two builds; the *Arm executables* table shows the two crossover arms
sharing one executable digest and differing only in `GF2_CROSSOVER_PATH`. The
polynomial family's gf2 arm is the public owned long product and the public wide
field product of the same pinned tree.

Each receipt pins the complete producing-input snapshot and the exact executable
digests under which its raw samples were collected. The *Arm executables* table
projects those identities. The release assembly under
`dev/active/53c5a8c0/survey/asm/` is disassembled from those pinned executable
bytes rather than from a later build: each artefact names the executable and its
matching receipt digest in its own banner. `dev/active/53c5a8c0/survey/build-arms.sh`
remains the release build command; documentation-only source changes do not
invalidate the receipt's pinned behavioral evidence even when they change Rust
binary metadata and therefore the digest of a later build.

The predicate-fix evidence this study retains rather than assuming is the
accepted receipt of issue `1d0da41f`, whose confirmation establishes which
raw-batch lane the default detection selects. The lane every crossover arm
reports at run time is in the *Per-arm call time and selected path* tables and
is the sequential carry-less lane, not a vector lane; the study assumes no
speedup from that repair and measures the lane the host actually publishes.

## Crossover family outcomes

Every outcome below is exactly as the evaluator recorded it, fails included, in
the *Family `gf2m-clmul-crossover`* section of
`dev/bench_results/53c5a8c0/tables.md`.

| Cell | Role | Recorded outcome | What it establishes |
|---|---|---|---|
| `raw-batch-8` | confirmatory | pass | The batched raw product is materially faster at the shortest batch measured. |
| `field-dot-8` | confirmatory | not material | The two dot-product paths are indistinguishable inside the frozen equivalence margin at eight elements. |
| `field-dot-64` | confirmatory | pass | The dispatched dot product is materially faster at sixty-four elements. |
| `field-batch-mul-8` | confirmatory | pass | The batched reduced product is materially faster at eight elements, which contradicts the module's rustdoc advice about short batches. |
| `field-dot-16-holdout` | holdout, arms swapped, non-regression | **fail, regressed** | The per-element dot product is materially *worse* at sixteen elements, so a gate that selected it there would lose. |
| `field-dot-512-holdout` | holdout, in the grid's direction | pass | The dispatched dot product is materially faster at five hundred and twelve elements, so retaining it above the threshold is supported. |

The exploratory pilot and the resolution pilot extend the same directions across
the rest of the grid, including the streaming cells and the thousand-element
reduced batch; their cells are recorded as `pilot` and take no part in a
decision. Their rows are in the same section.

## Polynomial family outcomes

| Cell | Role | Recorded outcome | What it establishes |
|---|---|---|---|
| `wide-field-256-composed` | confirmatory | pass | Composing one gf2x product with gf2's wide reducer is materially faster than gf2's own composed whole-consumer product at the narrower field. |
| `wide-field-571-composed` | confirmatory | not material | At the wider field the two composed consumers are indistinguishable inside this family's frozen equivalence margin. |
| `poly-4w` | confirmatory | **fail, regressed** | gf2's four-word long product is faster than gf2x's. |
| `poly-9w` | confirmatory | **fail, regressed** | gf2's nine-word long product is faster than gf2x's. |
| `poly-16w` | confirmatory | pass | gf2x's sixteen-word long product is materially faster than gf2's. |
| `raw-batch-1024-composed` | confirmatory | **fail, regressed** | One gf2 batch call is faster than the same number of separate gf2x calls. |

A fail in this family is not a defect of the study: the family's frozen
direction makes gf2 the baseline and gf2x the candidate, so a regressed
candidate is gf2 winning. The outcomes are reported under the names the
evaluator assigned them, and their meaning is stated rather than the label being
softened.

The pilot's two widest cells, at sixty-four and two hundred and fifty-six words,
are not carried into the confirmation. The reason is the tail-support bound
recomputed below, and the selection is recorded in
`dev/active/53c5a8c0/resolution-polynomial.txt` before the confirmation ran.
Their pilot values stay in the pilot receipt and appear in the same section of
the tables.

## Why `poly-16w` changes direction sharply

The factor is a **real implementation crossover in gf2 at its kernel-coverage
boundary.** It is not an artefact of the cell's definition and not an arm
defect. Three independent pieces of evidence establish this.

**The arm reports the path.** The gf2 arm's runtime-observed identity for the
sixteen-word cell is the portable scalar lane, while the four- and nine-word
cells report a vector kernel. See the *Selected path* column of the *Row
inventory* table in `dev/bench_results/53c5a8c0/profile/profile.md` and the same
column of the confirmation's *Per-arm call time and selected path* table. The
dispatch is const-generic in the operand word count and reaches a kernel at four
and nine words only; every other width runs the portable schoolbook. The source
evidence for the dispatch and for the public entry point is in
`dev/active/53c5a8c0/survey/source-evidence.json`.

**The assembly carries no carry-less multiply at sixteen words.** The committed
artefact `dev/active/53c5a8c0/survey/asm/long-product.asm.txt` disassembles the
sixteen-, sixty-four- and two-hundred-and-fifty-six-word monomorphisations of
the arm's long product beside the two wide kernels. Its derived annotation
blocks, counted by `dev/active/53c5a8c0/survey/annotate-asm.py`, are projected
into the *Release assembly of the measured gf2 paths* table of
`dev/bench_results/53c5a8c0/tables.md`: the two kernels carry vector carry-less
multiplies and close no loop, while each width above nine carries no carry-less
multiply instruction at all and closes several loops. The trailing-zero count and
clear-lowest-set-bit instructions in those bodies are the scalar bit-serial
product's set-bit walk.

Identification of a monomorphisation is not a guess from a body's size or its
address order: `dev/active/53c5a8c0/survey/dump-asm.py` resolves each width
through the dispatch the executable itself performs, reading the jump table the
width-matching code indexes and following its tail-call stub, and records that
route in the artefact beside the body.

**The counters agree.** In the *Instruction throughput and branch behaviour*
table of `profile.md` the sixteen-word gf2 row retires branches per operation on
the scale of thousands, which is what a schoolbook over hundreds of word
products, each walking the set bits of a random operand, costs; the four- and
nine-word gf2 rows retire branches per operation on the scale of ones. Their
dispatch-token stall counters in the *Load and store traffic and dispatch-token
stalls* table are near zero, so the cost is a latency chain rather than a
scheduler resource. The *Cycle attribution by symbol* table puts essentially all
of the sixteen-word row's cycles in the arm's long-product body rather than in a
kernel.

**Consequence.** The cell's receipt stands exactly as recorded. Nothing is
contradicted, because the cell measures the operation it declares on the path a
consumer at sixteen words actually reaches. It preserves and attributes the gap
in gf2's own kernel coverage without selecting an unmeasured algorithm.

## Why `raw-batch-1024-composed` changes direction

This factor is a **composed-cost artefact of the cell's own definition**, which
the cell declares before it runs. gf2x exposes no independent batch entry point,
so the comparator arm issues one library call per single-word product while the
gf2 arm issues one batch call for the whole vector. The gf2 side amortises its
dispatch and loop setup across the whole vector; the gf2x side pays a library
call, its length dispatch and its scratch-pool handling on every product.

The profile separates the two costs. In the *Cycle attribution by symbol* table
of `profile.md` the gf2x row's cycles land in the library's own entry point and
its callees, and the *Load and store traffic and dispatch-token stalls* table
gives the per-operation load and store traffic of the two rows. The cell
therefore reports what a consumer reaching gf2x through its public interface
pays, and it states no gf2x basecase rate, because this measurement cannot
separate that rate from the per-call cost.

**Consequence.** The receipt stands as recorded and the cell keeps its
`-composed` name. It is evidence about an interface, not about gf2x's basecase.

## Why the sixteen-element holdout regresses, and what that means for the gate

The dispatched dot product consults **only** its compile-time chunk extent; no
branch consults the vector length. That extent sizes three stack scratch
buffers, so every call reserves the same frame whatever the vector length. The
release assembly of the dispatched consumer in
`dev/active/53c5a8c0/survey/asm/crossover.asm.txt` shows that frame, summed
across the page-sized reservations its stack-probe sequence performs, in the
*Stack frame bytes* column of the *Release assembly of the measured gf2 paths*
table; it is the largest frame of any measured body.

The profile says what that frame costs. In the *Cycle attribution by symbol*
table of `dev/bench_results/53c5a8c0/profile/profile.md` the dispatched
sixteen-element row spends a large share of its cycles inside the host C
library's vectorised memory-set routine, which is the scratch buffers being
zeroed on every call. Naming that routine is not a reading of the address:
`perf` reports an offset because the routine is a local symbol the object
publishes no dynamic entry for, and
`dev/active/53c5a8c0/survey/resolve-symbols.py` resolves the offset against that
object's own symbol table, recording the object path, its digest and the covering
symbol in `symbol-resolution.json`. The resolved routine appears in the sampled
short and crossover dispatched rows and in no per-element row; it falls below
the report threshold in the longest dispatched row. That pattern is consistent
with a fixed per-call cost whose share shrinks as useful work grows.

That fixed cost is why a short dot product is not a clear win for the dispatched
path: at eight elements the confirmation records the two paths as
indistinguishable inside the equivalence margin. It is **not** a reason to select
the per-element path at sixteen. The holdout measures that hypothesis directly
and it fails, because the per-element consumer pays one carry-less multiply and
one field reduction **per element** while the dispatched consumer pays one
reduction for the whole vector. The *Cycle attribution by symbol* table shows
that split directly: the largest share of the per-element sixteen-element row's
cycles falls in the carry-less Barrett reduction, and the dispatched row's share
there is far smaller. The *Untimed conversion diagnostics* rows for the two holdout
cells carry the same reduction cost per side.

**What the recommendation retains near sixteen elements.** No gate. The
dot-product gate does not exist in the mechanism today, the evidence supports
adding none, and the established dispatched path is retained at every measured
length, sixteen included.

## Measurement resolution and its check

Each confirmation declares an `effect.measurement_resolution` derived by the
canonical freezer from a committed pilot receipt pinned by path and digest. The
*Measurement resolution* table of `dev/bench_results/53c5a8c0/tables.md`
recomputes, for each confirmation, the widest relative bootstrap half-width its
pinned pilot observed, and checks three things: that the pinned digest is that
pilot's receipt digest, that the declared resolution is at or above the widest
observed half-width, and that the frozen equivalence margin is strictly above one
plus that resolution. All three hold for both families.

The crossover family's resolution comes from a **second** pilot run at the
confirmatory pair count rather than from the first, because the first pilot's
widest cell left the declared equivalence margin indistinguishable from
measurement noise at the confirmation's corrected confidence. The polynomial
family reaches the same situation and resolves it the other way: its equivalence
margin is replaced by the smallest two-decimal value strictly above one plus its
resolution, recorded with its reasoning in
`dev/active/53c5a8c0/resolution-polynomial.txt`. Both derivation records sit
beside their addenda and are pinned into the confirmations they produced; the
crossover family's is `dev/active/53c5a8c0/resolution-crossover.txt`.

The holdout declaration `dev/active/53c5a8c0/survey/holdout-cells.json` is
committed before the crossover pilot runs and is carried into the confirmation
addendum by the canonical freezer, through the freezer's own `--holdout-cells`
argument. Its two sizes appear in no pilot cell and its fixtures use seeds no
pilot cell uses, so no sample of either holdout took part in choosing a
threshold. Both cells' directions are fixed in that declaration before any pilot
result exists; the *Holdout declaration* table projects them.

## The tail-support bound, and why no further confirmatory attempt helps

The protocol's numerical-resolution rule requires at least twenty bootstrap
replicates in each interval tail. The *Family accounting and P-20 tail support*
tables recompute the whole chain from each receipt's own pinned ledger prefix
rather than trusting a number: the family-wise alpha from the frozen addendum,
the reserved comparisons from the ledger, the attempt count from the ledger
entries that reserve any, the attempt alpha from the sequential attempt budget,
the corrected alpha, and the expected replicates per tail.

Both families reserve six comparisons on their first attempt, and both tables
show the tail condition holding with a margin under one replicate. Six is the
largest reservation that holds it, which is why the polynomial family carries six
of its eight pilot cells into its confirmation.

Each table also computes the bound on a **further** attempt from the same
arithmetic, because the sequential attempt budget shrinks the attempt alpha as
the attempt count rises. The rows headed *t for a further attempt*, *attempt
alpha then* and *m admitted then* show that a second confirmatory attempt on
either family admits only two comparisons. A six-cell confirmation therefore
cannot be repeated, and there is no reservation under which the six recorded
outcomes could be re-tested together. This is why no cell of either family is
re-run and every committed receipt stands as recorded: the alternative is not a
better measurement but a smaller one answering a different question.

## Selector and crossover recommendations

REQ-05 asks for qualifying selector and crossover recommendations for the
canonical tuning mechanism. The evidence supports none, and says so with the rows
rather than by silence.

**The dot product.** The only field-vector key the canonical mechanism carries is
the chunk extent `field_vec.dot_chunk_len`; that selector family's constructor
takes the one field, so the mechanism carries **no** dot-product length threshold
to set. The source-evidence ledger records both the family and its constructor.
The confirmation and both holdouts establish that no length threshold should be
added: the dispatched path is indistinguishable at eight elements, materially
faster at sixty-four and at five hundred and twelve, and the per-element path is
materially worse at sixteen. **Recommendation: retain the dispatched dot product
at every length, add no gate, and change no tuning value.**

**The chunk extent.** No cell of this study varies `field_vec.dot_chunk_len`, so
the study measures no alternative value for it and recommends no change. The
profile and the release assembly do establish what the current extent costs a
short call, namely the largest stack frame of any measured body, reserved on
every call whatever the vector length. That observation does not establish an
alternative extent and therefore does not support setting one here. The extent
reaches production as a compiled constant through the bake
mechanism, so installing a runtime tuning profile does not move it; a
recommendation about it would be a recommendation about that mechanism.

**The raw batch and the reduced element-wise batch.** The dispatched paths win at
every measured size, from eight elements through the streaming cells, so no
threshold is needed and none is recommended. The batch entry point selects its
kernel on the field degree alone; no branch consults the batch length, which the
source-evidence ledger records with the search that found no counterexample.
**Recommendation: retain the dispatched batch paths and the degree-based
selection unchanged.** The one change this evidence establishes is documentary,
stated in the answer above and made in its own commit.

**The long polynomial product.** The comparator cells recommend no production
change and adopt no dependency. gf2's product is faster at the two widths its
kernels cover and materially slower at the measured widths above them, because
above nine words it has no kernel. Closing that is a new production algorithm, a
wide carry-less kernel for more than nine words or a recursive split over the
existing kernels, which this issue explicitly does not select or implement.

**No-win domains retained.** The two not-material cells, `field-dot-8` and
`wide-field-571-composed`, establish parity rather than a direction at their
sizes. The established path is retained at both, and the rows that say so are
those confirmation rows in the tables.

Because no recommendation changes a tuning value or a code path, this issue
carries no before-and-after production benchmark: there is no production change
whose effect a receipt would have to bracket. The receipts are the evidence for
the recommendation to retain.

## Profiler attribution of the measured paths

`dev/active/53c5a8c0/survey/run-profile.sh` records the profile session in
`dev/bench_results/53c5a8c0/profile/`, and
`dev/active/53c5a8c0/survey/summarize-profile.py` projects it into
`dev/bench_results/53c5a8c0/profile/profile.md`. The session answers where each
path's cycles go; it is not an acceptance comparison and no receipt depends on it.
Its generated `validation.json` pins every session input by digest and records the
complete row/counter/report matrix, zero lost samples and the terminal session.

Its shape matters for reading the numbers. The counted process is `profile-arm`,
which issues a fixed number of logical calls of one frozen cell's operation on
that cell's frozen fixture and does nothing else, so a counter divided by the
process's operation count is a per-operation figure of the path rather than of a
window protocol. Every row's case is checked against the frozen cell grid before
the session starts, by `dev/active/53c5a8c0/survey/check-profile-cases.py`. Each
row's call count is derived by the script from its own two-point calibration,
never typed. Each counter ratio is the median over the session's repetitions with
the order-statistic interval of those repetitions, and each symbol's share of a
row's listed cycle samples carries a Wilson interval over the exact listed
sample count. The
launcher invokes the canonical host mutex per row; this is a source-level property
of the profile protocol rather than an inferred host condition.

Four attributions come out of it, each with its table in `profile.md`:

- **Dependency chains.** The instruction-throughput table separates the paths
  whose per-operation cost is a serial chain from those whose cost is work. The
  gf2 rows above nine words retire few instructions per cycle while walking
  thousands of branches per operation, and their dispatch-token stall counters
  are near zero, which places them on a latency chain rather than on a scheduler
  resource.
- **Instruction throughput.** The same table's instructions- and
  macro-ops-per-cycle columns give each path's achieved width, and the kernel
  rows' carry-less multiply counts in the *Release assembly* table give the work
  those cycles do.
- **Spills and frame traffic.** The memory table's per-operation load and store
  counts, read beside the *Stack frame bytes*, *Frame stores* and *Frame loads*
  columns of the *Release assembly* table, separate a body's declared scratch
  from compiler spills. The dispatched dot-product consumer's frame is its
  declared chunk buffers, which its source states, and the *Cycle attribution by
  symbol* table shows the cost of zeroing them; the wide kernel at four words
  reserves no frame at all, so its loads and stores are operands rather than
  spills.
- **Packing, extraction and reduction conversion overhead.** The receipts' own
  *Untimed conversion diagnostics* tables carry these per cell and per side, and
  `dev/active/53c5a8c0/survey/arms/src/bin/stage-diagnostic.rs` decomposes the
  dot-product consumer into packing, extraction, products, accumulation and the
  single reduction, refusing to emit a record unless the reconstructed stages
  compose to the value the consumer returns. The profile adds the cost the
  consumer pays before any of those stages, which is the zeroing of the chunk
  scratch.

**Bounded unrolling or fusion experiments.** The profile justifies none within
this issue. The one body whose cost the profile attributes to an algorithm rather
than to a schedule is the portable long product above nine words, and unrolling a
bit-serial set-bit walk does not shorten its dependency chain; what that body
needs is a kernel, which is a new production algorithm and out of scope. The
dot-product consumer's short-call cost is a frame the chunk extent declares,
which is a tuning question rather than an unrolling one. Neither is attempted
here, and neither is queued as an exploratory cell.

## Criteria

| Criterion | Where it is satisfied |
|---|---|
| REQ-01 correctness, reproducible release benchmarks, comparison validity, adoption, negative outcomes preserved | Each receipt pins the contract, protocol, addendum, producing-input snapshot and executable identities, projected on each table's Source line and *Arm executables* table; correctness is the *Correctness validation* table; every fail and not-material cell is reported above under the name the evaluator gave it. |
| REQ-02 raw batches and dot products across the frozen grid, costs separated, pinned implementation, predicate-fix receipt retained as evidence | The crossover family's confirmation and both pilots; the *Untimed conversion diagnostics* tables; the two arms sharing one pinned executable in *Arm executables*; the selected lane in *Per-arm call time and selected path*, which is the lane `1d0da41f` establishes rather than an assumed speedup. |
| REQ-03 operation-equivalent wide polynomial products against the pinned gf2x build with its observed basecase, raw cells without an external equivalent recorded honestly, composed costs stated | The polynomial family's confirmation and pilot; the gf2x build identity and loaded-object digest in *Arm executables*; the `-composed` cells' declarations and the sections above that state what each composes. |
| REQ-04 release assembly and profiler attribution of dependencies, throughput, spills and conversion overhead; bounded experiments only where justified | `dev/active/53c5a8c0/survey/asm/` with its dump and annotation generators, projected in *Release assembly of the measured gf2 paths*; `dev/bench_results/53c5a8c0/profile/profile.md`; the four attributions and the experiment decision in the section above. |
| REQ-05 qualifying selector and crossover recommendations with independent holdout measurements, established path retained in no-win domains, additional algorithm change tracked separately | The recommendation section above; the two declared holdout cells and their recorded outcomes; the retained rows named for both not-material cells. No additional production algorithm is selected or changed by this issue. |
