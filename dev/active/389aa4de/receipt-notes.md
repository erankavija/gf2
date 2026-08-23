# Prepared record updates for issue `389aa4de` (REQ-03)

REQ-03 requires that no committed record still states
`polynomial.karatsuba_min_degree` is unsweepable. This file holds the prepared
text for every such record, unapplied. Nothing here is committed to the
documents it names.

Apply after the measured calibration run of this issue exists and its receipt is
committed: three of the four blocks cite that receipt by path, written below as
`<new receipt>`.

## Rule the blocks follow

Append-only. Committed records under `dev/active/` and `dev/benchmarks/` carry
citations anchored to the tree they were written against, so an amendment is
appended and changes no pre-existing line
(`dev/active/6dc81018-field-capability-dispatch/handoff.md:57`). The section
shape follows the precedents at
`dev/benchmarks/tuning_profiles/across-build-control-arm-v1.md:179` and
`dev/benchmarks/tuning_profiles/selector-non-regression-plan-v1.md:314`.

Nothing in `2026-08-20-host-calibration.md` is rewritten, and in particular the
emitted document quoted byte-for-byte at `:293` and every measured figure stay
as taken. `@/inv/behavioral-evidence-validity` keeps them valid: they record
what one binary, identified by its `binary_sha256`, measured on one host, and
this issue re-measures none of it. What the amendment supersedes is that
receipt's forward-looking claim that the field cannot be swept — not its record
of what was swept.

## Records that state the limitation

| Record | Anchor | Statement |
|---|---|---|
| `dev/benchmarks/tuning_profiles/2026-08-20-host-calibration.md` | `:338`–`:367` (F2) | "no grid point offers both arms and §2.7 rule 1 cannot be executed for this field" |
| `dev/active/6dc81018-field-capability-dispatch/handoff.md` | `:26` (DEC-B6) | "`karatsuba_min_degree` is not sweepable (both arms private …)" |
| `dev/active/7d824b2f/design.md` | `:427`–`:431`, `:455`–`:461` | names the steering technique as one `389aa4de` builds, and cites the field's omission as the precedent for seventeen non-sweepable fields |
| `crates/gf2-core/benches/tuning_calibration.rs` | module docs, `# Arm reachability` | **already rewritten** by this issue's own commits; listed so the inventory is complete |

## Block A — append at the end of `dev/benchmarks/tuning_profiles/2026-08-20-host-calibration.md`

```markdown
## Amendment — issue `389aa4de` (<date>)

This section is appended at the end of the receipt and changes no line above it.

F2 records `polynomial.karatsuba_min_degree` as uncalibrated at this receipt's
revision: `FieldPoly::mul` resolved its arm from the compiled-in
`KARATSUBA_THRESHOLD`, so no grid point offered both arms, and F2 names issue
`389aa4de` as the tracked owner of the gap together with the mechanism that
closes it.

That condition is discharged. The polynomial cutover `697fc55b` has `mul_impl`
resolve the threshold from `tuning::active()`, and the calibration harness now
reaches both arms at every grid point by installing a profile that forces one.
Each arm is timed in a child process, because `tuning::install` resolves the
process-wide profile once, and each child reports the arm the production
selector `gf2_core::field::poly::mul_route` picks under the installed profile
along with digests of its operands and its product, which the calibrating
process checks. `<new receipt>` records that sweep and the value it selected.

The field is therefore sweepable and no longer omitted from an emitted profile.
Every figure above, the emitted document quoted under §Emitted profile, and
F2's account of what this run measured stand as taken.
```

## Block B — append as a bullet to the current session section of `dev/active/6dc81018-field-capability-dispatch/handoff.md`

```markdown
- **DEC-B6 discharged by `389aa4de`**: `karatsuba_min_degree` is sweepable
  after the polynomial cutover. Both arms come off `FieldPoly::mul` when a
  child process installs a profile forcing the threshold and the production
  route reporter confirms the arm, so an emitted profile now states a measured
  value for the field instead of omitting it. `<new receipt>` records the
  sweep.
```

## Block C — append at the end of `dev/active/7d824b2f/design.md`

Recommended rather than required by REQ-03's wording: §5.1 condition 2 states
the mechanism in the future tense and §5.2 cites the field's omission as a
current precedent, and `@/inv/present-tense-prose` makes both stale once the
sweep lands.

```markdown
## Amendment — issue `389aa4de` (<date>)

This section is appended at the end of the document and changes no line above
it.

§5.1 condition 2 names the profile-steered technique as one issue `389aa4de`
builds. It is built: `crates/gf2-core/benches/tuning_calibration.rs` reaches
both arms of a runtime threshold field by spawning one child process per arm,
each installing a profile that forces its arm before any selection boundary
runs, and each reporting the arm back through the family's production route
reporter so the parent checks it rather than assuming it. A follow-on field
that satisfies §5.1's three conditions uses that mechanism, as the section
requires, and no second steering mechanism exists.

§5.2's closing paragraph cites `karatsuba_min_degree`'s omission as the
precedent for the seventeen non-sweepable fields. Their standing rule is design
220cab0b §5 condition 5 itself, which each satisfies for the reason §5.2's
table records; `karatsuba_min_degree` is no longer an instance of it, and
`<new receipt>` records its sweep.
```

## What `<new receipt>` must carry for REQ-02

The harness prints each of these, so the receipt transcribes rather than
derives them.

- The grid and, per grid point, both arms' medians, spreads and window counts,
  with the margin and the noise band.
- The selected value with its selecting margin and band, or — when the rule
  keeps the default on a tie, a non-monotone crossover, or no winning grid
  point — the fallback reason and the default kept. §2.7 rule 3 and REQ-02 both
  ask for that case explicitly.
- The forcing policy, which the sweep header prints beside the arm names: the
  top of the admissible range for the schoolbook arm, the grid point itself for
  the Karatsuba arm.
- The omission inventory: the harness prints one row per omitted schema field
  with its inherited value and whether no sweep covers it or no grid point
  offered both arms.
- What this field's asymptotic arm is, per the section below.

## What the forced Karatsuba arm measures

The schoolbook arm forces `usize::MAX`, the top of the field's admissible
range, where no grid point routes to Karatsuba. The Karatsuba arm forces **the
grid point itself**, per owner decision DEC-B16.

`mul_karatsuba_raw` recurses on the same profile value, so the value forced
decides which algorithm is timed. Forcing the grid point makes the recursion
split once at that degree and hand its sub-operands, at about half the degree,
to the schoolbook base case — the algorithm the dispatcher runs when
`karatsuba_min_degree` is set to that grid point. Each grid point therefore
compares the two arms the selection rule chooses between at that point, so the
receipt may read the selected value as a threshold recommendation without a
recursion-shape caveat.

## Block D — DEC-B16, the mechanism the earlier records describe

Three records describe the forcing as minimal-against-maximal: the issue's own
Background, `dev/benchmarks/tuning_profiles/2026-08-20-host-calibration.md`
F2's tracked-owner paragraph at `:361`–`:367`, and
`dev/active/7d824b2f/design.md` §5.1 condition 2 at `:427`–`:431`. DEC-B16
supersedes that detail. The paragraph below carries the correction; append it
to Block A and to Block C, and apply it to the issue Background.

```markdown
Owner decision DEC-B16 fixes one detail of the mechanism these records
describe. The forcing is not minimal-against-maximal. The schoolbook arm
installs the top of the admissible range, where no grid point routes to
Karatsuba, and the Karatsuba arm installs the grid point itself.
`mul_karatsuba_raw` recurses on the same profile value, so forcing the bottom
of the range would time a Karatsuba recursion carried to its degree-0 base
case — an algorithm no threshold produces — while forcing the grid point times
the single split over schoolbook base cases that a threshold at that grid point
does produce. Both forced values stay ordinary admissible values and reserve no
sentinel.
```

### Evidence for DEC-B16

Both readings were measured on the same binary and host. The numbers below come
from unpinned smoke runs on a contended host — no lock wrapper, three windows,
minimum-of-three reported — so they are evidence of a direction, not of a
crossover, and no receipt may cite them as a measurement.

Forcing the bottom of the range put the Karatsuba arm behind the schoolbook arm
at **every** grid point, by 12.3x at degree 4 falling to 2.9x at degree 256, so
the §2.7 rule kept the conservative default for want of a winning grid point.

Forcing the grid point reverses it:

| Operand degree | schoolbook ns | karatsuba ns | karatsuba / schoolbook |
|---:|---:|---:|---:|
| 16 | 346.6 | 373.3 | 1.077 |
| 31 | 1288.8 | 1083.2 | 0.841 |
| 32 | 1267.8 | 1204.3 | 0.950 |
| 64 | 5053.8 | 4087.8 | 0.809 |
| 128 | 19260.6 | 15504.5 | 0.805 |

The Karatsuba arm loses at 16 and wins from 31 upward, so a crossover exists
between those degrees. That agrees with what `2026-08-20-host-calibration.md`
recorded from the dispatcher curve alone at `:188`–`:193`: `FieldPoly::mul`
cost 1248.166 ns at degree 31 on the schoolbook arm and 1135.357 ns at degree
32 on the Karatsuba arm, a step **down** for one more coefficient, which that
receipt noted as consistent with the crossover lying below 32 while claiming
no crossover from it. The measured run decides the value.
