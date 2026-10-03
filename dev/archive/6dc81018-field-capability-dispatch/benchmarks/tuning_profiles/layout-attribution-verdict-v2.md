# Layout-attribution amendment v2: the coverage-repairing ensemble axes

This document amends the layout ensemble's axes, and nothing else, for the next
measured post-cutover receipt of epic `6dc81018`. It is filed by issue
`9162956b` under owner decision DEC-G of 2026-08-21, and it is committed before
any measured run under it exists, so the rule cannot be shaped by the numbers it
governs.

[`layout-attribution-verdict-v1.md`](layout-attribution-verdict-v1.md) stands as
written and stays authoritative. **This document changes none of its text.** It
is read together with v1: everything v1 fixes that this document does not name
is what governs. [`selector-non-regression-plan-v1.md`](selector-non-regression-plan-v1.md)
and [`across-build-control-arm-v1.md`](across-build-control-arm-v1.md) stand
beside both.

τ_cell stays 5 %. τ_set stays 2 %. The pinned set keeps all thirty-four cells.
The schema token `selector-non-regression-v1` is not bumped. The attribution
margin stays three standard errors. No value any of the three standing documents
predeclares moves.

## A1. What sends this document here

[`2026-08-20-post-cutover-receipt-3.md`](2026-08-20-post-cutover-receipt-3.md)
§"The two cells that fail coverage" records v1 §6.4's coverage precondition
failing at two of the thirty-four pinned cells:

| Cell | `s_R` at K = 128 | Floor `σ̂`/2 | Fraction of the floor reached |
|---|---:|---:|---:|
| `bit_backend/or_inplace/words=1` | 0.047448 | 0.059077 | 80.3 % |
| `bit_backend/xor_inplace/words=1` | 0.038850 | 0.049556 | 78.4 % |

v1 §6.6 fixes the consequence in advance: "The ensemble is not a model of the
layout variation the record measures, and no larger K repairs that. The run
reports which cells fail and by how much, and the ensemble's axes are the
subject of a tracked amendment before the next measured run." This is that
amendment.

## A2. What the record's own layout variation is

`σ̂` is defined by v1 §6.4 from the natural pair of v1 §8: the committed baseline
build of revision `0c072d73ca65cf50af98b8c4b61ed876f8218df6`, made in the main
checkout, and receipt 2's control rebuild of the same revision, made in
`.agents/worktrees/control-0c072d73`. Those two binaries are the whole empirical
content of the coverage floor.

[`ensemble-axes-pilot-receipt.md`](/dev/active/9162956b/ensemble-axes-pilot-receipt.md)
§2 measures what separates their placement. The bench target embeds its
`CARGO_MANIFEST_DIR` as a string literal in `.rodata`; the two checkouts'
manifest directories differ in length by thirty-five characters; `.rodata`
therefore ends at a different address and **the whole program image below it is
translated**. Building the same revision under a manifest path of the main
checkout's length and under one of the control worktree's length reproduces it:
every one of the 736 common text symbols moves, all by one constant of 32 bytes,
every symbol's address changes modulo 64 and modulo 4096, and no symbol moves
relative to any other.

**The record's layout variation is a translation of the image. v1 §3.1's axes
pad inside it and never translate it.** Worse, they suppress translation: the
receipt's §3 measures `-C llvm-args=-align-all-functions=<A>` raising `.text`'s
own section alignment to as much as 128 bytes, which rounds a 32-byte
translation away and pins every function's offset inside its cache line. The
receipt's §4.5 measures the consequence cell by cell: at
`bit_backend/or_inplace/words=1` the same translation axis produces `s_R` of
0.064526 among members at `A = 0` and 0.039968 among members at `A = 4`–7.

That is why the amended enumeration replaces the alignment axes rather than
adding to them.

## A3. The amended enumeration

Member `j`, for `j` from 0 to K−1:

    E(j) = ⌊j / 2⌋                        G(j) = (⌊j / 4⌋ + j) mod 2

| Axis | Option | Levels | What it does |
|---|---|---|---|
| E | `-C link-arg=-Wl,--build-id=0x<hexadecimal string of 2·(20 + 32·E) zeros>` | 0–127 | Lengthens the `.note.gnu.build-id` payload to `20 + 32·E` bytes, translating the loaded image by `32·E` bytes |
| G | `-C link-dead-code` | 0–1 | Retains dead code, rearranging the live code inside the image |

A level of 0 omits its option, so the member with `E = G = 0` builds under empty
`RUSTFLAGS` and is the ordinary build, exactly as v1 §3.1 has it, and v1 §7.1's
recorded single-build comparison keeps the member it names. Member `j` records
`--execution j+1`, as v1 §3.1 and §3.2 fix.

**K = 256.**

**Axis E changes placement and provably nothing else.** The `.note.gnu.build-id`
note sits ahead of `.rodata` and `.text` in the image, so lengthening it
translates both.
The pilot receipt §3 records the extracted `.text` and `.rodata` as
byte-identical to the ordinary build's at every level of E, with the whole image
moved `32·E` bytes, verified at E = 1, 3, 7, 63 and 127. It is the axis's
*length* that is the level: a build-id of the default length changes the
binary's SHA-256 and moves no symbol at all, exactly as v1 §6.6 found for
`--sort-section=name`.

**Axis G rearranges rather than translates.** `-C link-dead-code` keeps code the
linker would drop, which moves live code relative to live code — the second way
two natural builds of one program differ — while leaving `.text` aligned to 16
so that a translation lands exactly where it is asked to.

**The two axes together give 256 distinct placements.** The pilot receipt §6
records all 256 members as 256 distinct binaries at 256 distinct `.text` page
offsets, so no member repeats another's placement. That is the standard v1 §6.6
sets when it rejects an axis that "would hold each of the 128 placements twice
rather than 256 placements".

**Continuing the enumeration past K.** v1 §3.5 replaces a member that fails to
build by "continuing the enumeration past K to the next member of the dropped
member's index parity". The formulas above are defined for every `j ≥ 0`, with
`E(j) = ⌊j / 2⌋` unbounded, so the continuation is mechanical and a build
failure cannot become a choice about which layouts the verdict sees.

**What that continuation costs, stated in advance.** Axis E enumerates the 128
distinct 32-byte offsets of a 4,096-byte page, and axis G doubles them, so the
enumeration provides exactly 256 distinct placements and no more. A replacement
member at `j ≥ 256` is a distinct binary whose translation exceeds one page and
whose *page offset* therefore repeats an earlier member's — a weaker draw than a
fresh placement, because a whole-page shift changes no cache-line offset and no
L1i set index. A session that uses one records it, names the dropped member and
its replacement, and re-runs §A4's verification over the enumeration it actually
used. §A8's single retry exists so that the transient build failures the pilot
observed do not reach this rule.

## A4. Why the construction of v1 §3.1 and §3.2 is preserved

v1 §3.2's parity property exists for one purpose, which it states: "Splitting
the ensemble on that parity splits it evenly on every axis", so that the
half-split
null "measures across-build dispersion rather than a difference between two
unlike halves". This amendment establishes that balance **on the realized
placement** rather than on a sum of levels, which is the stronger statement, and
the pilot receipt §6 verifies each of the following by enumeration over all 256
members from the build log, before any timed window:

- **256 distinct members**, and 256 distinct `.text` page offsets.
- **The halves are matched on the cache-line offset.** A member's `.text`
  address is `base(G) + 32·E`, so its offset inside a 64-byte line takes four
  values; each half of the member-index parity split holds exactly 32 members at
  each of the four.
- **The halves are matched on the L1 instruction-cache set index.** The two
  multisets of `(address >> 6) mod 64` are equal, each covering all sixty-four
  sets twice.
- **The halves are balanced on each axis**: exactly one member at each of the
  128 levels of E, and sixty-four at each of the two levels of G.
- **Every split runs over `j`, never over the recorded execution index**, as v1
  §3.2 requires.

This is not a formality. The pilot receipt §5 records an earlier candidate
enumeration whose axes were each marginally balanced and whose halves
nonetheless occupied *disjoint* sets of cache-line offsets, and measures the
half-split null it produced: 0.876003 at `bit_backend/or_inplace/words=1`, far
outside τ_cell. The property above is what that measurement forced.

## A5. What computes the preconditions, and what it does not need

v1 §6.5's `--layout-audit` mode computes v1 §6.4's four preconditions on the two
arms' committed CSVs. **This amendment needs no change to it and makes none.**

- It accepts an arm of exactly 128 or 256 members
  (`ENSEMBLE_SIZES` in `crates/gf2-core/benches/selector_non_regression.rs:62`),
  and K = 256 is one of them.
- It reads the parity split from `execution − 1` (`member_index` and
  `even_member`, same file, lines 1338 and 1344), which the amended enumeration
  records exactly as v1 does.
- It requires both arms to carry the same member indices, which they do.

So the coverage, precision, half-split-null and decorrelation machinery is
computable unchanged, the recording path and the emitted columns are the code
they were, the schema token stays where it is, and per
`@/inv/behavioral-evidence-validity` every committed receipt stays valid
evidence across this amendment. The amendment lives entirely in which
`RUSTFLAGS` the session's build phase sets.

## A6. What the pilots verified before this document was written

[`ensemble-axes-pilot-receipt.md`](/dev/active/9162956b/ensemble-axes-pilot-receipt.md)
is the committed receipt: a build-time study and three timed pilots, each under
a protocol whose decision rule was fixed before its first timed window, building
and measuring the reference revision only. Two of the three read REJECT and are
recorded with their numbers; the third measures the complete 256-member
reference ensemble of §A3.

- **Pilot 1** tested v1's alignment axes with the translation axis added, and
  reads REJECT: `s_R` reaches 0.049124 against a floor of 0.059077 at
  `bit_backend/or_inplace/words=1` and 0.042778 against 0.049556 at
  `bit_backend/xor_inplace/words=1`. Its thirty-two-member group varying the
  translation axis **alone** clears every one of the thirty-four floors, and its
  thirty-two members of v1's own enumeration reproduce receipt 3's committed
  reference-arm binaries bit for bit and fail at exactly the two cells receipt 3
  failed at.
- **Pilot 2** tested the two axes of §A3 under a different half-split
  assignment, measuring the complete 256-member reference ensemble. Coverage,
  precision and drift hold; the half-split null fails at four cells and the
  calibration guard fails on one contaminated execution. It reads REJECT and is
  recorded with its numbers, and its build-time facts are what fixed `G`.
- **Pilot 3** measures the complete 256-member reference ensemble of §A3 and
  reads **ADOPT** on all six of its predeclared gates.

| Reading | Requirement | Pilot 3 |
|---|---|---|
| Coverage, per cell | `s_R(c) ≥ σ̂(c)/2` at all thirty-four | no failure; tightest ratio 1.17 |
| Coverage, whole set | RMS `s_R` ≥ RMS `σ̂` = 0.033878 | 0.046937 |
| Half-split null | inside ±5 % per cell, ±2 % on the geometric mean | widest 1.022538; geometric mean 1.000845 |
| Precision, projected | margin ≥ 3 at every cell and at the set | narrowest 4.849; set margin 15.931 |

At the two cells this amendment exists to repair:

| Cell | Floor `σ̂`/2 | `s_R` at K = 128 (receipt 3) | `s_R` under this enumeration | Fraction of the floor |
|---|---:|---:|---:|---:|
| `bit_backend/or_inplace/words=1` | 0.059077 | 0.047448 | 0.069084 | **116.9 %** |
| `bit_backend/xor_inplace/words=1` | 0.049556 | 0.038850 | 0.069835 | **140.9 %** |

The pilots measure the reference arm only. Decorrelation needs two arms and is
decided by the measured session's own audit, as v1 §6.4 and §6.5 have it.

## A7. Why K is 256

v1 §3.4 sizes K from the precision precondition against the across-build
dispersion the record measures, and the same rule sizes it here against the
amended ensemble's own measured dispersion. The precondition is
`3 · √(s_R² + s_C²) / √K ≤ ln(1.05)` at every cell.

Read with the measured `s_R` of §A6 and receipt 3's committed candidate arm
unchanged, the widest cell is `bit_backend/xor_inplace/words=16` at
`√(s_R² + s_C²) = 0.160989`, which needs `K ≥ 98`. Read with the candidate arm's
dispersion scaled cell by cell by the factor the amended ensemble raises the
reference arm's by — the conservative reading, since the candidate revision's
own layout sensitivity is not measured until the session runs — the widest cell
is `bit_backend/not_inplace/words=8` at 0.199034, which needs `K ≥ 150`.

K = 128 satisfies the first and not the second. **K = 256 satisfies both**, and
the pilot's measured margins at K = 256 are 4.849 at the narrowest under the
plain reading and 3.922 under the scaled one, both above three.

Two other things put K at the same number. It is what the enumeration provides
in distinct placements — axis E gives 128 translations, axis G doubles them, and
the pilot records 256 distinct `.text` page offsets across the 256 members — and
it is the size v1 §6.5's audit accepts above 128, so the amended enumeration
needs no change to what checks it.

**There is no rung above this one.** v1 §6.6 doubles K to 256 when precision or
the half-split null fails, and states what lies beyond: "Beyond K = 256 the run
reports that the per-cell rule is not decidable at τ_cell against this host's
across-build dispersion, and that demonstration goes to the owner of epic
`6dc81018`." This amendment starts at K = 256, so a precision or half-split-null
failure under it is that demonstration, and it goes to the owner rather than to
a larger ensemble. A coverage or decorrelation failure keeps v1 §6.6's other
branch: a further tracked amendment of the axes before any further measured run.

## A8. The session at K = 256

- **512 builds and 512 timed executions**, as v1 §5.3 budgets the ladder's
  second rung. The pilot's own build phase took 3,554 s of compiler time for 256
  members of one arm, so 512 builds is about two hours outside the lock; its
  timed phase ran 296 executions in 3,241 s, so 512 executions is about 94
  minutes in one lock session.
- **The build phase may build into a scratch target directory outside the
  checkout and delete it between members.** Receipt 3 records 420 MiB of
  artifacts per member pair and no pruning, which at 512 builds does not fit
  this host. The pilot receipt's Provenance section establishes that the target
  directory does not enter the binary — the ordinary build reproduces
  `c7be7a87d7a01d3c2e297919033a99237afe8939f3fce147802ecd86df32c710`
  byte-for-byte from a target directory outside the checkout — while the
  *manifest* directory does, which is §A2's mechanism. Each checkout keeps its
  own manifest directory: the reference arm builds in
  `.agents/worktrees/control-0c072d73` and the candidate arm in the main
  checkout, as receipt 3 has it.
- **A member that fails to build is retried once before v1 §3.5 applies.** The
  pilot receipt records two transient `rustc` internal compiler errors in its
  first build phase of 126 distinct builds, both the panic `active query job
  entry` while building `criterion`, and both succeeded on an immediate retry
  under identical flags; the second build phase of 256 builds had none. A member
  that fails a second time is dropped and replaced under v1 §3.5.
- **v1 §5.4's recording rule is unchanged**: each binary is copied out and
  hashed before the next member is built, each arm appends one CSV whose
  `execution` column carries the member index plus one, `--output` takes an
  absolute `/tmp` path checked absent beforehand, and both files are copied into
  this directory afterwards with their SHA-256 recorded.
- **v1 §5.2's ordering is unchanged**: member `j` runs its two arms adjacently,
  reference first when `j` is even and candidate first when `j` is odd.
- **v1 §5.1's sampling is unchanged**: one execution of one repetition per
  build, at `--target-ms 250`, with the plan's equivalence probes before every
  execution.

## A9. What this amendment does not touch

- **v1 in full.** Its §1 bounds, §2 statement of what the amendment changes, §4
  two arms, §5 session, §6.1 statistic, §6.2 rules, §6.3 attribution argument,
  §6.4 preconditions, §6.5 audit, §6.6 consequences, §7 declared readings, §8
  baseline side, §9 list of what plan v1 and control-arm v1 keep, and §10
  standing all stand as written. What is read through this document is §3.1's
  axis table, §3.4's choice of K, and §6.6's fourth axis.
- **The three standing receipts and the two standing plans.** None is modified,
  superseded, re-run or adjusted here.
- **Receipt 3.** It stands as taken, with its `RESULT: FAIL` verdict, its
  `RESULT: FAIL` audit and its falsification record intact. This amendment does
  not reopen it, does not recompute it, and claims no different reading of it.
  Its two failing cells are the measurement this document answers, and its
  ensemble arms remain the committed evidence `σ̂`, `s_C` and every projection
  here are read from.
- **Every predeclared value.** τ_cell, τ_set, the pinned set, the schema token,
  the three-standard-error margin, the `--target-ms 250` protocol, the pinned
  host, the wrapper, the affinity, the governor and the toolchain.
- **v1 §7's declared readings.** All four combinations of verdict and
  preconditions, and the six further readings, govern the next session exactly
  as v1 fixes them. In particular v1 §7.6 — "No predeclared value moves for any
  outcome" — governs this amendment too: it is written before the run and is not
  revised by it.

## A10. Standing of this document

This rule is written before the measurement it governs. At the commit that adds
it, the receipts in the repository under the `selector-non-regression-v1` schema
token are the procedure verification of `e8fe47f5`, the pre-cutover baseline of
`278acf3a`, and the three post-cutover receipts of `50b47eae`; no run under this
amendment exists, and no candidate arm of the amended ensemble has been built or
measured. The pilots of `9162956b` build and measure the reference revision
only, run the audit on nothing, and establish no verdict.

**v1 gains no pointer to this document.** v1 §9 has each document it amends gain
a pointer section naming it, and both standing plans carry one. This amendment
adds none, because it changes none of v1's text. Whether v1 should gain a
pointer to it is the owner's call, and it is the one editorial question this
document leaves open.

The owner of epic `6dc81018` approves this text before the next measured session
is dispatched. Until that approval is recorded, no session runs under it.
