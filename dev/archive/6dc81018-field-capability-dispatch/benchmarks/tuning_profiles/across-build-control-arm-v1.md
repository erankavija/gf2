# Across-build control arm for the post-cutover comparison v1

This document predeclares how an excursion in the gated post-cutover comparison
of epic `6dc81018` is adjudicated. It is filed by issue `51058f8e` under owner
decision DEC-C of 2026-08-19, and it is committed before any post-cutover
measurement exists, so the rule cannot be shaped by the numbers it governs.

[`selector-non-regression-plan-v1.md`](selector-non-regression-plan-v1.md) is
frozen and stays authoritative. This document adds one measurement to
`50b47eae`'s session and one rule for reading it. It adds no term to the
verdict.

## 1. The assumption the baseline falsifies

The plan's §4 grounds τ_cell = 5 % on within-build dispersion. Its derivation
takes the across-execution coefficients of variation of 0.045 %–0.5 % recorded
by
[`batched-f3-avx2-provenance-fixed.md`](/dev/benchmarks/permanent_campaign/batched-f3-avx2-provenance-fixed.md)
§Dispersion for its two well-behaved backends, and its confirming measurement
takes the two procedure-verification cohorts, whose widest per-cell deviation is
1.91 % and whose geometric mean is 1.000810, giving τ_cell a factor of 2.6 over
the worst observed cell and τ_set a factor of 25 over the observed set
statistic. Both bound the spread of a single build: the provenance dispersion is
five executions of one bench binary, and the two cohorts run one bench binary
back to back inside a single lock session (plan §4, falsification record).

The pre-cutover baseline of `278acf3a` measures the across-build spread instead.
[`2026-08-19-pre-cutover-baseline.md`](2026-08-19-pre-cutover-baseline.md)
§Falsification record compares that baseline, revision `0c072d73`, against the
verification cohorts' `1d8a289a` on the same host under the same wrapper,
affinity, governor, toolchain and protocol, and nine of the thirty-four pinned
cells fall outside ±5 %:

- The widest is `bit_backend/or_inplace/words=1`, where the baseline's 4.157592
  ns/call stands 18.1 % above cohort A's 3.520309, which is 3.6 × τ_cell, with
  cohort B agreeing to 0.01 %.
- The geometric mean of the thirty-four ratios is 0.977297 against cohort A and
  0.978088 against cohort B.
- The shifts run in both directions: `bit_backend/and_inplace/words=1` is 6.0 %
  the other way from its three scalar siblings.
- Both cohorts reproduce the per-cell split, seven of the nine cells to within
  0.1 %, while every cell's own across-execution dispersion in the baseline
  stays under 4.67 %, so the effect tracks the binary rather than run-time host
  state.

That measurement establishes that the shift tracks the binary; it does not
isolate which change in the binary produces it. The one placement mechanism the
plan names is excluded, because both runs use the harness that lays each
bit-logical cell's fixture banks on 64-byte boundaries and asserts it before
timing, per plan §3.

**The consequence for the gated comparison.** The comparison τ_cell gates — this
baseline against `50b47eae`'s post-cutover receipt — is necessarily across two
builds, so the inference from within-build dispersion does not carry to it. At
those cells a cutover that adds no cost can trip the per-cell rule, and one that
adds a real cost can be masked (plan §4; receipt §Falsification record).

## 2. The predeclared control arm

`50b47eae`'s session measures a control build alongside the post-cutover build.

- **What is built.** A rebuild of the pre-cutover baseline revision
  `0c072d73ca65cf50af98b8c4b61ed876f8218df6`, the revision every row of the
  baseline receipt records.
- **How it runs.** The plan's §3 protocol unmodified: `GF2_BENCH=1` on the
  prepared host, `dev/scripts/ccx1-bench-flock.sh` holding
  `/tmp/gf2-ccx1.lock`, CPUs 6–11, five fresh executions of five repetitions at
  `--target-ms 250`, the MSRV toolchain, and no tuning profile installed. The
  harness carries the same `selector-non-regression-v1` schema token, so the arm
  is comparable against the baseline under §5's preconditions.
- **When it runs.** In the same session as the post-cutover build, so both arms
  see one host state.
- **What is recorded.** The control arm's per-cell ratios against the baseline
  receipt's raw windows,
  [`2026-08-19-pre-cutover-baseline.csv`](2026-08-19-pre-cutover-baseline.csv),
  on the pooled ns/call statistic of §5, for all thirty-four cells; together
  with the §6 provenance rows, including the control build's own bench binary
  SHA-256. A ratio below 1 is recorded and read the same as one above, because
  §1's effect runs in both directions while §5's verdict is one-sided.

Producing those ratios with the §5 comparison mode is the expected mechanism.
Its verdict column and its `RESULT:` line carry no standing for this pair, the
same way the baseline receipt records its eligibility check without a verdict
(receipt §Comparison eligibility).

What the arm bounds depends on whether the rebuild reproduces the baseline's
bench binary SHA-256
`acbfd9c49325b4063b7a419114e05bacd6b98299bbde45fc89016f0d162db0fe`, and both
readings are declared here rather than chosen once the hash is known. An equal
hash means the arm measures the baseline's own binary, and its ratios bound
session-to-session variation of that binary. A different hash means the arm
carries an across-build component of the kind §1 records.

## 3. What plan v1 keeps

Plan v1 is unmodified. Nothing here re-pins, widens, or supersedes it.

- **§2, the pinned cells.** All thirty-four cells stand as `e8fe47f5` pins them,
  fourteen bit-backend and twenty polynomial. No cell is added, dropped, or
  re-bracketed, and the control arm measures that same set.
- **§3, the run protocol.** Unchanged, and it governs the control arm as well as
  the post-cutover arm.
- **§4, the tolerance.** τ_cell stays 5 % and τ_set stays 2 %, at their
  predeclared values. Neither is widened to admit the across-build spread of §1.
- **§5, the comparison rule.** The verdict is that rule applied to this baseline
  against `50b47eae`'s post-cutover receipt: pooled ns/call per cell, judged by
  τ_cell and τ_set, under the schema, cell-set and per-cell identity
  preconditions it names. That comparison is the whole verdict.
- **The schema token.** `selector-non-regression-v1` is not bumped. Receipts
  under different tokens do not compare (§7), so bumping it would discard the
  baseline; the baseline instead stays valid and comparable.
- **The baseline.** `2026-08-19-pre-cutover-baseline.md` stands as taken. It is
  not re-run, not superseded, and not adjusted.
- **The control arm.** It enters no verdict. No control-arm ratio changes a
  per-cell verdict, the geometric mean, or the `RESULT:` line of the gated
  comparison, in either direction.
- **The falsified statements.** Plan §4's derivation and confirming measurement
  stay as written, each carrying its pointer to the falsification record, per
  `@/inv/falsification-preserved`.

The only edit this document makes to plan v1 is the pointer to it in §4's
falsification record. That pointer states no rule.

Owner decision DEC-C records the two rejected alternatives. Option A applies the
procedure literally with no concurrent evidence to adjudicate a spurious trip.
Option C re-pins to a v2 comparison rule, which bumps the schema token and
discards the baseline. Option B, written down here, keeps the plan frozen and
adds the evidence beside it.

## 4. Reading the control arm when the comparison trips

The comparison trips when §5 fails a cell against τ_cell or fails the set
against τ_set. The reading is fixed here, before any such number exists.

1. **The verdict stands as §5 computes it.** A failing comparison is a failing
   comparison, and `50b47eae` REQ-01 is unmet until a receipt shows the
   tolerance holds.
2. **The excursion is recorded with its contradiction and its tracked rework**,
   per `50b47eae` REQ-02 and `@/inv/falsification-preserved`: the committed
   record carries every tripping cell with both pooled rates and its ratio, and
   the rework the excursion triggers is tracked rather than deferred.
3. **The control arm's same-cell ratio is the evidence that rework weighs.** It
   is a quantitative, concurrent input into the tracked work:
   - A tripping cell where the control arm deviates by a comparable magnitude in
     the same direction is evidence that an across-build component of the kind
     §1 records is present at that cell.
   - A tripping cell where the control arm sits near 1 is evidence that the
     excursion at that cell is attributable to the cutover.
   - A τ_set failure is read the same way against the control arm's geometric
     mean over the thirty-four cells.
4. **A trip is never dismissed by the control arm alone.** The control arm
   cannot convert a `FAIL` into a `PASS`, widen τ_cell or τ_set, drop a cell
   from the pinned set, or close `50b47eae` on a comparison that fails. Its
   ratios enter the record and the rework, never the verdict.
5. **Re-running until the comparison agrees stays forbidden.** Plan §7 requires
   a measurement that contradicts the plan's noise assumption to be recorded
   with the contradiction rather than accommodated, and a receipt kept because
   it is the one that agrees with the tolerance accommodates it. A tripping run
   stands as taken, as the baseline stands as taken against its own
   contradicting measurement.

The arm is symmetric with respect to the verdict. It is recorded in
`50b47eae`'s receipt whether or not the comparison trips, and a passing
comparison is not reopened by it.

## 5. Standing of this document

This rule is written before the measurement it governs. At the commit that adds
this document, the only receipts in the repository under the
`selector-non-regression-v1` schema token are the procedure verification of
`e8fe47f5` and the pre-cutover baseline of `278acf3a`, both taken from
pre-cutover builds. `50b47eae` depends on `51058f8e`, so its post-cutover
session runs after this document is committed.

The document is registered against `51058f8e` and is reachable from the plan:
§4's falsification record, which names `51058f8e` as the owner of the
consequence, links here.

## Amendment — issue `972e2b88` (2026-08-20)

This section is appended after §5 and changes no line above it.

The control arm of §2 measures the across-build component at one rebuild and
enters no verdict. Two post-cutover receipts have now recorded it landing
outside ±5 % at five cells, in the same cells and directions in both sessions.

Owner decision DEC-E of 2026-08-20 carries the consequence into the verdict
itself, by the predeclared amendment
[`layout-attribution-verdict-v1.md`](layout-attribution-verdict-v1.md) that
issue `972e2b88` files. Its reference arm is this document's control arm
generalised from one rebuild to an ensemble of builds, and its readings keep
§4.4's asymmetry: nothing recorded beside the verdict converts a `FAIL` into a
`PASS`.

Nothing here moves for it. §2's control arm, §3's list of what plan v1 keeps
and §4's reading of a tripping comparison stand as written, and τ_cell and
τ_set stay at 5 % and 2 %.
