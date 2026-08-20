# Layout-attribution amendment to the selector non-regression verdict v1

This document predeclares the verdict comparison for the next post-cutover
receipt of epic `6dc81018`, so that a pinned cell's verdict attributes movement
to the cutover's behaviour rather than to across-build code layout. It is filed
by issue `972e2b88` under owner decision DEC-E of 2026-08-20, and it is
committed before any further measured run exists, so the rule cannot be shaped
by the numbers it governs.

[`selector-non-regression-plan-v1.md`](selector-non-regression-plan-v1.md) is
frozen and stays authoritative, and
[`across-build-control-arm-v1.md`](across-build-control-arm-v1.md) stands
beside it. This document adds one thing: it fixes what stands on each side of
plan §5's comparison. The comparison itself, its statistic, its tolerances and
its pinned set are the ones those documents already fix.

τ_cell stays 5 %. τ_set stays 2 %. The pinned set keeps all thirty-four cells.
The schema token `selector-non-regression-v1` is not bumped.

## 1. The bounds this amendment answers to

[`2026-08-20-post-cutover-receipt-2.md`](2026-08-20-post-cutover-receipt-2.md)
§Falsification record measures three quantities on this host, under this
wrapper, affinity, governor, toolchain and protocol.

- **Session-to-session variation of one fixed binary is small.** The second
  session's control build is bit-identical to the first session's, both hashing
  to `c7be7a87d7a01d3c2e297919033a99237afe8939f3fce147802ecd86df32c710`.
  Comparing the two control arms therefore compares one binary across two
  sessions: `RESULT: PASS`, geometric mean 0.999314, every one of the
  thirty-four cells inside ±1.9 %, thirty-one of them inside ±0.8 %.
- **Across-build variation of behaviourally identical code is large.** The
  second session's control arm, a rebuild of the baseline revision
  `0c072d73ca65cf50af98b8c4b61ed876f8218df6`, lands outside ±5 % of the baseline
  at five cells — `bit_backend/or_inplace/words=1` at 0.846119,
  `bit_backend/xor_inplace/words=1` at 0.869215,
  `polynomial/div_rem_auto/dividend=4096/divisor=1024` at 0.888089,
  `bit_backend/xor_inplace/words=4` at 0.947790 and
  `bit_backend/and_inplace/words=1` at 1.059625 — the same five cells and the
  same directions as the first session's control arm. The baseline's own
  §Falsification record measures the same effect against a third build, with
  nine cells outside ±5 % and the widest at 18.1 %.
- **The effect reaches verdict-tripping cells.** Five polynomial cells —
  `polynomial/mul/len=33`, `len=64` and `len=256`, and
  `polynomial/mul_fast/len=32` and `len=64` — stand 6.6 % to 7.8 % above the
  baseline in the gated comparison although no polynomial code changes between
  the compared revisions, and they sit between 1.065069 and 1.079903 against
  the same session's control build as well.

Plan §4's falsification record states the consequence before either
post-cutover session ran: "at those cells a cutover that adds no cost can trip
the per-cell rule, and one that adds a real cost can be masked." The second
receipt observes it happening.

**What that leaves undecidable.** The verdict statistic is a ratio of two
binaries. That ratio carries the cutover's cost and the layout difference of
the particular pair of binaries compared, and the second receipt measures the
second of those at a magnitude that reaches and exceeds τ_cell at single cells.
One pair of builds cannot separate them, whichever pair it is.

## 2. What the amendment changes

The comparison of plan §5 is a rule over two receipts. This amendment fixes
what those two receipts are: each side becomes an **ensemble** of builds of one
revision rather than one build of it, so that across-build layout enters the
statistic as a sampled quantity with a measured dispersion instead of as one
unmeasured draw.

Nothing else moves. The statistic stays plan §5's pooled nanoseconds per call,
the per-cell rule stays τ_cell = 5 %, the set rule stays τ_set = 2 % on the
geometric mean of the thirty-four ratios, the pinned set stays §2's thirty-four
cells, the run protocol's `--target-ms 250` and its equivalence probes stay,
and the comparison mode that computes the verdict is unchanged code.

## 3. The layout ensemble

### 3.1 What a member is

A member of the ensemble is a set of `RUSTFLAGS` that changes where code is
placed and leaves what the code does alone. Three axes, all stable `rustc`
codegen options at the MSRV:

| Axis | Option | Levels |
|---|---|---|
| A | `-C llvm-args=-align-all-functions=<A>` | 0–7 |
| B | `-C llvm-args=-align-all-nofallthru-blocks=<B>` | 0–3 |
| C | `-C llvm-args=-align-all-blocks=<C>` | 0–3 |

A level of 0 omits its option, so the member with A = B = C = 0 builds under
empty `RUSTFLAGS` and is the ordinary build. Each option only aligns: it pads
between functions or basic blocks, which changes where the program's
instructions sit and leaves which instructions it executes alone.

Member `j`, for `j` from 0 to K−1, is

    A(j) = ⌊j / 16⌋                        B(j) = ⌊(j mod 16) / 4⌋
    C(j) = ((j mod 4) + A(j) + B(j)) mod 4

and it records `--execution j+1`, so the execution index names the member.
K = 8 × 4 × 4 = 128 members, all distinct.

### 3.2 Why the enumeration is written this way

The rotation in C(j) makes A(j) + B(j) + C(j) congruent to `j` modulo 2, so a
member's level sum has the parity of its **member index** `j`. Splitting the
ensemble on that parity splits it evenly on every axis: each half holds eight
members at each level of A and sixteen at each level of B and of C.

Every split and balance this document states runs over `j`, never over the
recorded execution index. Member `j` records `--execution j+1`, so the two
parities are opposite, and reading a half off the execution column means
reading `j = execution − 1` first. §6.4's half-split is defined that way and
the harness computes it that way, so it measures across-build dispersion rather
than a difference between two unlike halves of the ensemble.

### 3.3 What is verified before this document is committed

Observed at this revision on `fraktaali` under `rustc 1.95.0 (59807616e
2026-04-14)`, building `--bench selector_non_regression -p gf2-core --features
simd`:

- Every axis is accepted at the MSRV and moves the binary. The levels sampled
  build distinct bench binaries at distinct sizes, growing the ordinary build
  by at most 6.7 % at A = 7, 2.3 % at B = 3 and 9.2 % at C = 3. Levels past the
  table's bounds stay outside the ensemble because they bloat the binary rather
  than move it: `-align-all-nofallthru-blocks=7` grows it by 73.2 %. A member
  that turns out to leave this crate's placement alone shows up as reduced
  dispersion, which §6.4's coverage precondition reads.
- The member that perturbs most, A = 7, B = 3, C = 3, builds and reports
  `self-check PASS` with `simd_min_words=8`, so the harness's straddle
  properties and the pinned set survive the perturbation.
- One member's build costs 14 s to 15 s of wall clock on this host, which §5.3
  budgets from.

### 3.4 Why K is 128

K is set by the precision precondition of §6.4 against the widest across-build
dispersion the record measures. The second receipt's control arm stands at
0.846119 of the baseline at `bit_backend/or_inplace/words=1`, so one build pair
estimates that cell's across-build dispersion at
`|ln 0.846119| / √2 = 0.118178` in log units. Requiring
`3 · √2 · 0.118178 / √K ≤ ln(1.05)` gives K ≥ 106, and 128 is the smallest
ensemble the enumeration of §3.1 provides above it. At K = 128 the same
estimate puts the tolerance 3.30 layout standard errors above 1 at that cell.

That estimate rests on one build pair, which is why §6.4 recomputes the
dispersion from the ensemble's own K builds and §6.6 carries a ladder: the
record sizes K in advance, and the session's own measurement decides whether
that size sufficed.

### 3.5 A member that fails to build

If a member fails to build in either checkout, it is dropped from the ensemble
in **both** arms and replaced by continuing the enumeration past K to the next
member **of the dropped member's index parity**, so the ensemble keeps exactly
K members and keeps the balanced halves of §3.2. The receipt records the
dropped member, its flags, the failure, and its replacement. The rule is
mechanical, so a build failure cannot become a choice about which layouts the
verdict sees.

The count and the balance are what §6.5's audit enforces: it accepts an arm of
exactly K = 128 or exactly K = 256 members whose halves are equal on member
parity, and it requires both arms to carry the same member indices. An arm of
any other shape is a different ensemble, so the audit refuses it by name
instead of computing a margin whose K it does not know.

## 4. The two arms

- **Reference arm.** The pre-cutover baseline revision
  `0c072d73ca65cf50af98b8c4b61ed876f8218df6`, the revision every row of
  [`2026-08-19-pre-cutover-baseline.md`](2026-08-19-pre-cutover-baseline.md)
  records, built once under each member of the ensemble.
- **Candidate arm.** The post-cutover revision under test, built once under each
  member of the ensemble.

Both arms measure the same pinned set, carry the same schema token, install no
tuning profile, and run in one lock session under
`dev/scripts/ccx1-bench-flock.sh` on CPUs 6–11 with `GF2_BENCH=1`, at
`--target-ms 250`, under the MSRV toolchain with the `simd` feature.

**The arms differ in the harness file as well as in `crates/gf2-core/src/`.**
The reference revision predates the `--layout-audit` mode of §6.5, so the two
checkouts' copies of `crates/gf2-core/benches/selector_non_regression.rs`
differ. That mode is reached only through its flag and runs no measurement, so
the recorded rows, the timed windows, the pinned set and the protocol constants
are the same on both sides; what the difference produces is placement, which is
the quantity this ensemble samples. The receipt records both checkouts' harness
SHA-256 so the difference is visible rather than assumed away.

## 5. The session

### 5.1 Sampling

Each build records **one execution of one repetition**. The pinned set's
thirty-four cells each contribute one calibrated 250 ms window per build, so an
arm holds K windows per cell against the twenty-five that plan §3's five
executions of five repetitions produce.

The budget moves onto the build axis because that is where the dispersion is.
The second receipt records within-execution dispersion of 0.02 %–0.30 % and
across-execution dispersion of 0.058 %–0.143 % at the two cells it examines
execution by execution, and 2.864 % as its control arm's widest across-execution
figure, against an across-build effect in the same session that reaches 15 %.
One repetition per build also dilutes the process-level excursions that receipt
records: its `bit_backend/xor_inplace/words=7` cell has one execution of five
standing 55 % above the other four, which moves that cell's five-execution
pooled rate by 7.6 % — the receipt's 1.148518 against 1.068910 without it — and
would move a K = 128 pooled rate by 0.3 %.

Every execution runs the plan's equivalence probes before its first timed
window, so every one of the 2K builds has its cells checked for computing the
right answers, and a failed probe aborts the run.

### 5.2 Order

Member `j` runs its two arms adjacently, reference first when `j` is even and
candidate first when `j` is odd. Each arm therefore holds exactly K/2 of the
first positions and K/2 of the second, so the mean slot position of the two
arms is equal and no warm-up or thermal trend over the session lands on one arm.

### 5.3 Budget

At K = 128: 256 builds and 256 timed executions.

- **Builds** happen before the lock is taken, at 14 s to 15 s each (§3.3), so
  about 64 minutes, and no compilation happens under the lock. Each binary is
  copied out of the target directory to a staging path outside the repository
  before the next member overwrites it, and its SHA-256 is recorded.
- **Timed work** is 34 × 250 ms per execution. The second receipt's slot table
  records 45 s between the starts of consecutive five-repetition executions
  against 42.5 s of timed work, so about 2.5 s per execution sits outside the
  timed windows and an amended execution costs about 11 s. 256 executions is
  about 47 minutes, inside one lock session.

The ladder of §6.6 doubles K to 256, which is 512 builds (about 2 h 8 min) and
512 executions (about 94 minutes under the lock). Both rungs are one lock
session.

### 5.4 Recording

Each arm writes one CSV, appended across its K builds, with the build's member
index in the `execution` column. The staged binary runs with its working
directory at its own checkout, so each row records that arm's revision, and
`--output` takes an absolute `/tmp` path checked absent beforehand, so no
in-repository file exists while the run is in flight and every row records
`source_dirty=false`. Both files are copied byte-for-byte into this directory
afterwards and their SHA-256 recorded, per plan §3 step 2.

**The row schema is unchanged.** No column is added and the token
`selector-non-regression-v1` is not bumped, so the committed baseline and both
committed post-cutover receipts stay valid and comparable. The `execution`
column keeps its meaning — one separate process — and gains the ensemble's
reading of it: under this amendment that process is also a distinct build, and
the receipt's provenance table maps each index to its member's flags and its
binary's SHA-256.

## 6. The verdict

### 6.1 The statistic

For cell `c` and member `λ`, a build contributes its recorded windows. An arm's
statistic for `c` is plan §5's: the sum of `elapsed_ns` over every recorded
window of that cell in that arm divided by the sum of `calls`. Write it
`T_R(c)` for the reference arm and `T_C(c)` for the candidate arm. The verdict
ratio is

    ρ(c) = T_C(c) / T_R(c)

Because every window is calibrated to the same 250 ms target, the arm statistic
weights each build by its call count, which makes it the harmonic mean of the
per-build rates. Both arms are pooled the same way over the same ensemble, so
the transform stands on both sides of the ratio. The estimand is the cutover's
cost at that cell averaged over the ensemble of layouts.

### 6.2 The rules

Unchanged from plan §4 and §5, applied to ρ(c):

- **Per cell, τ_cell = 5 %.** A cell fails when ρ(c) exceeds 1.05.
- **Whole set, τ_set = 2 %.** The set fails when the geometric mean of the
  thirty-four ρ(c) exceeds 1.02.

Both must hold. The comparison is the §5 mode, unmodified, run on the two arms'
committed CSVs:

```sh
cargo +1.95.0 bench -p gf2-core --features simd --bench selector_non_regression -- \
  --compare dev/benchmarks/tuning_profiles/<reference-arm>.csv \
  --against dev/benchmarks/tuning_profiles/<candidate-arm>.csv
```

It applies its own schema, cell-set and per-cell identity preconditions to both
files, so an arm whose cells changed identity fails as
`RESULT: FAIL (selector identity mismatch)` rather than on a tolerance, exactly
as it does for a single-build pair.

### 6.3 Why the verdict attributes

The layout difference between the two arms is no longer one fixed draw. Each
member places the two revisions' code differently, so ρ(c) averages the layout
ratio over K members, and its layout component falls as 1/√K. The procedure
measures that component rather than assuming it: §6.4's preconditions compute
the ensemble's own dispersion at every cell, in the same session, from the same
rows, and hold the tolerance against it.

**The assumption underneath, and what tests it.** Averaging over the ensemble
estimates the cutover's cost as long as the layout ratio itself averages to 1
over the members — as long as the members place the two revisions' code
independently rather than reproducing one fixed difference at every member. The
procedure holds that assumption to measurement rather than to argument.
§6.4's decorrelation precondition reads whether the paired ratio scatters
across the members at all, and its coverage precondition reads whether the
ensemble moves an arm as far as one natural rebuild moves it. An ensemble
failing either is reported as failing rather than read as a verdict, and §6.6
says what happens then.

### 6.4 The attribution preconditions

Write `s_R(c)` and `s_C(c)` for the sample standard deviation, over the K
builds of an arm, of the natural logarithm of that build's pooled rate at cell
`c`; `p(c)` for the same statistic applied to the paired logarithm
`ln(t_C(c,λ) / t_R(c,λ))`; and `σ̂(c)` for the across-build dispersion one
natural rebuild records, `|ln r(c)| / √2`, where `r(c)` is the ratio the
committed control arm of the second receipt shows against the committed
baseline at that cell.

- **Coverage.** `s_R(c) ≥ σ̂(c) / 2` at every cell, and the root mean square of
  `s_R` over the pinned set is at least the root mean square of `σ̂`. An
  ensemble that moves an arm less than one natural rebuild moved it would
  understate the variance the verdict has to survive. The factor of two carries
  the sampling error of `σ̂`: one build pair estimates a dispersion as `|Z|`
  times it, whose median is 0.674, so an ensemble standing more than a factor
  of two below `σ̂` at a cell is tamer there rather than unlucky.
- **Decorrelation.** The root mean square of `p` is at least the root mean
  square of `s_R`. Layouts drawn independently in the two arms give
  `p = √(s_R² + s_C²)`, about √2 times an arm's own dispersion; requiring one
  times it admits correlation up to about a half and rejects an ensemble that
  moves the two arms together, which would leave one fixed layout difference
  that no averaging removes.
- **Precision.** The layout standard error of ρ(c) is
  `se(c) = √(s_R(c)² + s_C(c)²) / √K`, and the precondition is
  `ln(1.05) ≥ 3 · se(c)` at every cell. Taking the arms as independent is the
  conservative reading, since layout the two arms share only shrinks `se`. At
  three standard errors a cost-free cutover trips the per-cell rule with
  probability about 0.1 % at a cell under a log-normal model, and a few percent
  over the thirty-four. The set rule carries the same margin against
  `ln(1.02)`, with the set-level standard error computed from each build's mean
  displacement over the pinned set.
- **Half-split null.** The reference arm's builds split on the parity of the
  member index (§3.2), and the verdict statistic is computed between the two
  halves. Both halves hold behaviourally identical code, so the true ratio is
  1, and this is the verdict's own statistic evaluated under a known-zero
  effect at half the ensemble size — a strictly noisier configuration than the
  verdict's. The precondition is that it stays inside ±τ_cell at every cell and
  inside ±τ_set on the geometric mean, read two-sided because layout runs in
  both directions while the verdict is one-sided.

### 6.5 What computes them

The bench target's `--layout-audit` mode, added with this document:

```sh
cargo +1.95.0 bench -p gf2-core --features simd --bench selector_non_regression -- \
  --layout-audit dev/benchmarks/tuning_profiles/<reference-arm>.csv \
  --layout-candidate dev/benchmarks/tuning_profiles/<candidate-arm>.csv \
  --compare dev/benchmarks/tuning_profiles/2026-08-19-pre-cutover-baseline.csv \
  --against dev/benchmarks/tuning_profiles/2026-08-20-post-cutover-control-arm-2.csv
```

Under `--layout-audit` the receipt pair names the two committed receipts whose
ratio defines `σ̂`, and it is **required**: coverage is a precondition of the
verdict, so without the pair the audit refuses by name —
`RESULT: FAIL (natural pair missing)` — rather than reporting a result that
skipped one of its own preconditions. The mode prints one line per cell carrying the reference
arm's across-build spread, `s_R`, `s_C`, `p`, `se`, the achieved margin
`ln(1.05) / se`, the half-split null ratio, `σ̂`, and a per-cell verdict; then
the set-level margin and null, the coverage comparison, the decorrelation
comparison, and `RESULT: PASS` or `RESULT: FAIL`. It exits non-zero on failure.

It refuses to audit files that do not describe the predeclared ensemble of the
pinned set, naming the class on the `RESULT:` line as the §5 mode does: a
schema mismatch, a row recording `source_dirty=true` or an execution index
below one, an arm spanning two revisions, a member that is not the pinned set,
a cell whose recorded identity moves across members, a degenerate cell, a
member count that is neither 128 nor 256, halves unbalanced on member parity,
arms carrying different member indices, a missing receipt pair, or a receipt
pair that is not the pinned set.

The mode reads committed CSVs and writes none. It adds no measurement behaviour
and changes none: the recording path, the emitted columns, the timing, the
sampling and the `--compare` mode are the code they were, so per
`@/inv/behavioral-evidence-validity` every committed receipt stays valid
evidence across this change and the schema token stays where it is.

### 6.6 When a precondition fails

A failing precondition means the session established no attributable verdict.
It is reported as `RESULT: FAIL` by the audit, the epic's gate stays unmet, and
no passing comparison is claimed from that session. It can only withhold a
verdict; it can never turn a failing comparison into a passing one.

- **Precision or the half-split null fails.** K doubles to 256 by extending the
  enumeration with a fourth axis, `-C link-arg=-Wl,--sort-section=name` at two
  levels D. Member `j` from 0 to 255 then takes `D(j) = ⌊j / 128⌋`, takes A and
  B from `j mod 128` by §3.1's formulas, and takes
  `C(j) = ((j mod 4) + A(j) + B(j) + D(j)) mod 4`, which keeps the level sum
  congruent to `j` modulo 2 and so keeps the parity property of §3.2. Beyond
  K = 256 the run reports that the per-cell rule
  is not decidable at τ_cell against this host's across-build dispersion, and
  that demonstration goes to the owner of epic `6dc81018`.
- **Coverage or decorrelation fails.** The ensemble is not a model of the
  layout variation the record measures, and no larger K repairs that. The run
  reports which cells fail and by how much, and the ensemble's axes are the
  subject of a tracked amendment before the next measured run.

The session that raises any of these stands as taken. It is recorded with its
numbers, and it is not re-run at the same K until it agrees.

## 7. Every reading, declared in advance

Read the verdict of §6.2 and the audit of §6.4 together. All four combinations
are declared here, before any of them exists.

| Verdict | Attribution preconditions | Reading |
|---|---|---|
| PASS | all hold | The cutover's cost at every pinned cell stands inside τ_cell and the set inside τ_set, attributably. `50b47eae` REQ-01 is met by that receipt. |
| FAIL | all hold | The cutover's cost exceeds a tolerance, attributably. The verdict stands, the excursion is recorded with its contradiction, and its rework is tracked. |
| PASS | one or more fail | No verdict. The gate stays unmet; §6.6 governs what happens next. A passing comparison under an unmet precondition is not a pass. |
| FAIL | one or more fail | No attributable verdict, and the gate stays unmet either way. §6.6 governs. |

Further readings, each fixed here:

1. **The unamended single-build comparison is recorded.** The receipt runs the
   §5 comparison of the committed 2026-08-19 baseline against the candidate
   arm's ordinary build alone — member A = B = C = 0 — and records its per-cell
   lines and its `RESULT:`. It carries no verdict standing.
   - Agreeing with the amended verdict is evidence that the ordinary build's
     layout draw is unremarkable at the pinned cells.
   - Disagreeing is the case this amendment exists for: the receipt records
     every cell where the two differ, together with `s_R` and `σ̂` at those
     cells, as the measurement of how much one draw moved the old verdict.
     Neither outcome changes the amended verdict, in either direction.
2. **The candidate arm's own across-build dispersion is recorded.** `s_C(c)`
   materially above `s_R(c)` at a cell says the cutover made that cell's cost
   more layout-sensitive. It is recorded evidence about the change and enters
   no verdict.
3. **A cell whose own dispersion inside a receipt exceeds τ_cell is recorded as
   noise-dominated**, with its numbers, per plan §7. The tolerance is not
   widened for it and the cell is not dropped.
4. **A FAIL is preserved.** Per control-arm §4.2 and
   `@/inv/falsification-preserved` the record carries every tripping cell with
   both pooled rates and its ratio, and the rework the excursion triggers is
   tracked rather than deferred.
5. **Re-running until the comparison agrees stays forbidden.** Plan §7 requires
   a measurement that contradicts the plan's noise assumption to be recorded
   rather than accommodated, and a receipt kept because it is the one that
   agrees accommodates it. Each session stands as taken.
6. **No predeclared value moves for any outcome.** Not τ_cell, not τ_set, not
   the pinned set, not the schema token, not the ensemble, not the margin of
   three standard errors.

## 8. The baseline side of the comparison

`50b47eae` REQ-01 reads: "A committed receipt compares post-cutover
measurements against the baseline with full provenance and shows the
predeclared tolerance holds for the pinned set."

**Which measurements are the baseline side.** The reference arm of §4: the K
builds of revision `0c072d73ca65cf50af98b8c4b61ed876f8218df6` measured in the
receipt's own session, at the pinned set of plan §2, under the protocol of plan
§3, carrying the `selector-non-regression-v1` schema token, with
`source_dirty=false` on every row and plan §6's provenance rows recorded for
each build, its binary's SHA-256 included.

**Why that satisfies the wording.** "The baseline" is the pre-cutover state the
epic pinned, and
[`2026-08-19-pre-cutover-baseline.md`](2026-08-19-pre-cutover-baseline.md) is
what pins it: it fixes the revision, `0c072d73`, together with the set, the
protocol and the tolerance the comparison is judged against. The reference arm
is a measurement of that same revision, that same set, that same protocol and
that same schema token, on the same host under the same wrapper, affinity,
governor and toolchain. What the amendment changes is the precision of the
baseline side — K builds of the baseline revision instead of one — and not its
identity, its revision, its cells or its protocol. A receipt taken this way
compares post-cutover measurements against the baseline, with fuller provenance
than a single-build side carries, and it shows whether the predeclared
tolerance holds for the pinned set at the predeclared values.

The committed baseline receipt keeps its standing as the across-session record
of that revision, and it carries two roles in the amended run: with the second
receipt's control arm it supplies `σ̂` for §6.4's coverage precondition, and it
is the baseline side of the recorded single-build comparison of §7.1.

**Disposition of the standing receipts.** All three stand as taken. None is
re-run, superseded, or adjusted by this document or by the run it governs.

- [`2026-08-19-pre-cutover-baseline.md`](2026-08-19-pre-cutover-baseline.md) —
  stands as taken, with its own falsification record intact.
- [`2026-08-20-post-cutover-receipt.md`](2026-08-20-post-cutover-receipt.md) —
  stands as taken. Its `RESULT: FAIL` remains the record of what the cutover
  measured before `c42720ce`.
- [`2026-08-20-post-cutover-receipt-2.md`](2026-08-20-post-cutover-receipt-2.md)
  — stands as taken. Its `RESULT: FAIL` remains the record of what the same
  unmodified procedure measured after `c42720ce`, and its Falsification record
  is the evidence this amendment is built on.

Both failing comparisons remain what they are. This amendment does not reopen
them, does not recompute them, and claims no different reading of them: it
governs the next measured run and nothing already taken.

## 9. What plan v1 and control-arm v1 keep

- **Plan §2, the pinned cells.** All thirty-four stand as `e8fe47f5` pins them.
  No cell is added, dropped or re-bracketed, and every build of both arms
  measures that same set.
- **Plan §3, the run protocol.** The prepared host, the lock wrapper, CPUs 6–11,
  `GF2_BENCH=1`, `--target-ms 250`, the MSRV toolchain, the `simd` feature, no
  tuning profile installed, the controlled fixture alignment, the equivalence
  probes, and the `/tmp`-then-copy rule for raw files all govern this run. §5.1
  states the one allocation this amendment fixes differently — one execution of
  one repetition per build, against five of five per receipt — and states the
  measurement that decides it.
- **Plan §4, the tolerance.** τ_cell stays 5 % and τ_set stays 2 %, at their
  predeclared values. Neither is widened to admit any excursion, and neither is
  tightened.
- **Plan §5, the comparison rule.** The statistic, the preconditions and the
  `RESULT:` semantics are the ones it fixes, computed by the same unchanged
  mode.
- **Plan §7, falsification and re-pinning.** Noise-dominated cells are recorded
  with their numbers rather than dropped, and a moved conservative default
  re-pins the set and bumps the token rather than being compared across.
- **The schema token.** `selector-non-regression-v1` is not bumped. Receipts
  under different tokens do not compare, so bumping it would discard the
  baseline.
- **Control-arm v1.** Its §2 control arm, its §3 list of what plan v1 keeps and
  its §4 reading of a tripping comparison stand as written. The reference arm
  of §4 here is that control arm generalised from one rebuild to K, and §7's
  readings keep §4.4's asymmetry: nothing recorded beside the verdict converts
  a FAIL into a PASS.
- **The falsified statements.** Plan §4's derivation and confirming
  measurement, and both post-cutover receipts' records, stay as written, each
  carrying its own contradiction, per `@/inv/falsification-preserved`.

Neither standing document loses a line to this one. Each gains a pointer
section naming this amendment; that pointer states no rule.

## 10. Standing of this document

This rule is written before the measurement it governs. At the commit that adds
it, the receipts in the repository under the `selector-non-regression-v1` schema
token are the procedure verification of `e8fe47f5`, the pre-cutover baseline of
`278acf3a`, and the two post-cutover receipts of `50b47eae`; no run under this
amendment exists, and no ensemble arm has been built or measured. The
`--layout-audit` mode is committed with this document and its behaviour is
fixed by its tests before any ensemble is recorded.

`50b47eae` depends on `972e2b88`, so its next measured session runs after this
document is committed. Sibling issue `2a85f728` eliminates the per-call cost of
the bit-backend selection boundary; whether that mechanism brings the pinned
comparison inside the tolerance is decided by that session, under the procedure
as this document amends it, and not by either issue.
