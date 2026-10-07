# Core calibration campaign: findings (jit:63bad95d)

> **Diátaxis Type:** Research

The campaign `gf2-63bad95d-20261006t230547z-2073085` measures the complete
core selector section on the 5900X under
[`premeasurement-protocol.md`](premeasurement-protocol.md). Every value and
figure these findings refer to is a row of the generated
[campaign tables](campaign-tables.md) (`survey/make-campaign-tables.py`); the
receipt, the owner envelope and their digests are in that record's
[campaign table](campaign-tables.md#campaigns). The independent validator's
verdict on the stage and on the publication is recorded in
`dev/benchmarks/tuning_profiles/gf2-63bad95d-20261006t230547z-2073085-revalidation.log`.
The producer's figures are descriptive medians with their window counts; the
producer's rule compares a margin with a band and computes no interval.

## F-01: the matvec threshold keeps its conservative value

The owner states `bit_matrix.matvec_simd_min_words` at the conservative value
([selector table](campaign-tables.md#selectors-per-owner)). The producer's
decision is "default kept: non-monotone"
([decision table](campaign-tables.md#retained-threshold-decisions), row
`matvec_simd_min_words`): the SIMD lane's margin is above the band at the
lowest swept stride only, and negative at every other stride
([matvec lane sweep](campaign-tables.md#matvec-lane-sweep)).

Disposition under §7 of the [plan](calibration-plan.md): no single boundary
expresses the sweep, so the conservative value stays and the baked constant
equals it.

## F-02: the sweep contradicts the lane the conservative threshold selects

At every swept stride at or above the conservative threshold the scalar lane
has the lower median ([matvec lane sweep](campaign-tables.md#matvec-lane-sweep),
rows from the conservative threshold upward), while the production selector
sends those strides to the SIMD lane (`matvec-lane-entry`). The largest
deficit of the SIMD lane is the row one word below the conservative threshold.

Two records bear on the same boundary and do not agree with this sweep
stride for stride:

- the pilot of `4337c02e` (allocated whole-`matvec`, protocol version 4) is
  exploratory, and its confirmation receipt is rejected by acceptance, so the
  protocol confirms neither lane at any stride;
- the exploratory rows of `2037941f-dense-allocated-matvec`, which the
  [route inventory](route-inventory.md#evidence-in-hand-per-selector) lists
  for this selector.

The contradiction is recorded here and not resolved: the threshold grid ends
at its largest swept stride, a retained threshold cannot express "scalar at
every stride", and no accepted protocol confirmation exists. The selector
keeps its value. Whether the SIMD row-parity lane should remain on the
`matvec` route at all is outside this issue's selector ownership (REQ-12
limits it to calibrating selectors) and is reported to the lead.

## F-03: the bit-backend threshold is the profile's one changed baked selector

The owner states `bit_backend.simd_min_words` below the conservative value
([selector table](campaign-tables.md#selectors-per-owner)); the producer's
decision is a crossover at that value
([decision table](campaign-tables.md#retained-threshold-decisions), row
`simd_min_words`), with the margin above the band at every swept length from
the crossover upward
([bit-backend threshold sweep](campaign-tables.md#bit-backend-threshold-sweep),
last table).

The owner of the seam campaign `gf2-dbd8787d-20261001t230000z-2601601` states
the conservative value for the same sweep, as "default kept: non-monotone".
Its table in the same section shows, in the two interquartile-range columns,
the dispersion that decision rests on. The extent campaign `gf2-a83583e0-20260930t230000z-2728298`
states the same value as this campaign. The seam decision stays committed with
its receipt; this campaign contradicts it and agrees with the extent campaign.

Disposition: the baked constants pin this campaign's owner and mirror its
value (`tuning-baked-owner-digest`, `baked-measured-owner-test`). The default
build keeps the conservative constant. The profile-dispatch-overhead family
(plan §6) measures the profile build against the conservative build on both
sides of the two thresholds and decides whether the baked value is retained
(plan §7).

## F-04: one runtime field differs from the seam owner

`ple.scalar_base_max_cols` returns to its conservative value: no grid point
wins ([PLE scalar base sweep](campaign-tables.md#ple-scalar-base-sweep), last
table), where the seam campaign records a crossover at the top of the grid
with the margin the first table's last row gives. The field is read from an
installed profile at run time and routes `FieldMatrix::ple`, which no family
of this epic measures; the plan neither calibrates nor reconfirms it (§5).

## F-05: every other selector repeats the earlier owners

For every remaining selector the `Differs` column of the
[selector table](campaign-tables.md#selectors-per-owner) names no owner that
states another measured value: it is empty, names the conservative table, or
names an earlier owner that omits the field because its campaign does not
sweep it ([decision table](campaign-tables.md#retained-threshold-decisions),
"not swept").

## What the campaign does not decide

- No protocol receipt: the producer's statistic is its own, and its decisions
  are calibration evidence for the offline tuning system only (plan §2).
- The transpose lane and the carry-save count routes have no selector in the
  core section (plan §8).
- The decoder and BCH encode selectors belong to the coding-owned section
  (REQ-12).
