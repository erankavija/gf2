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
- The forced threshold each arm's child installed, which the sweep header
  prints beside the arm names.
- The omission inventory: the harness prints one row per omitted schema field
  with its inherited value and whether no sweep covers it or no grid point
  offered both arms.
- What this field's asymptotic arm is, per the section below.

## What the forced Karatsuba arm measures

The forced thresholds are the endpoints of the field's admissible range:
`usize::MAX` for the schoolbook arm and `1` for the Karatsuba arm, following
the mechanism the issue Background and `dev/active/7d824b2f/design.md` §5.1
condition 2 both describe. `mul_karatsuba_raw` recurses on the same profile
value, so under the forced minimum the Karatsuba arm recurses to its degree-0
base case rather than bottoming out at the threshold the sweep is choosing.

That is a different arm from the one a chosen threshold produces in
production, where a top-level Karatsuba split hands sub-operands below the
threshold to schoolbook. The receipt must say so, and must not present the
selected value as a measurement of the production recursion shape.

An unpinned smoke run of the harness on a contended host — one execution, one
repetition, one millisecond, no lock wrapper, therefore evidence of nothing
beyond the harness's mechanics — put the forced-minimum Karatsuba arm between
2.9x and 12.3x the schoolbook arm's cost at every grid point from degree 4 to
degree 256, so the rule kept the conservative default with no winning grid
point. The measured run decides what the receipt records; this note exists so
that the reading is prepared rather than discovered afterwards.
