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

Two records under the frozen protocol bear on the same boundary:

- `4337c02e` (allocated whole-`matvec`, protocol version 4). Its confirmation
  receipt is rejected by acceptance, so no row is confirmatory; its rows are
  descriptive estimates
  (`dev/active/1a379447-zen3-cpu-performance/4337c02e/outcome.md`, *Recorded
  cell outcomes*, with the figures in `tables.md` beside it). They agree with
  this sweep in direction at the ragged strides that record names for the
  scalar lane, record no material difference at the strides it lists as such,
  and contradict this sweep at the one stride where that record has the SIMD
  lane ahead and this sweep's row has the scalar lane ahead;
- the exploratory rows of `2037941f-dense-allocated-matvec`, which the
  [route inventory](route-inventory.md#evidence-in-hand-per-selector) lists
  for this selector.

The contradiction is recorded and not resolved here. The recorded strides of
neither measurement form one interval above the threshold, a retained
threshold cannot express "scalar at every stride", and no accepted protocol
confirmation exists, so the selector keeps its value. The remaining gap of the
SIMD lane at ragged strides is a kernel matter outside this issue's selector
ownership (REQ-12) and is reported to the lead.

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
sides of the two thresholds; plan §7 states how its confirmation decides
whether the baked value is retained, and F-07 states where the family stands.

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

## F-06: the native-ladder confirmation is accepted and does not qualify

`63bad95d-r1-native-ladder-confirmation` measures the six cells of its frozen
addendum, native build against tuned-portable build, and spends the family's
one confirmatory attempt
([ledger](../../../bench_results/63bad95d/63bad95d-native-ladder-ledger.jsonl),
last line). Acceptance records the verdict, the qualification and each cell's
outcome in the
[acceptance summary](../../../bench_results/63bad95d/63bad95d-native-ladder/r1-confirmation/acceptance-summary.md):
three cells pass as not worse and three are inconclusive, so the confirmation
does not qualify. No cell records a regression.

The host is contended while the campaign runs. The launcher log and the
receipt's session records state a load at start above the pilot's
([disturbance record](disturbance-record.md#host-load-each-campaign-records)).
Every cell that does not pass has a relative half-width above the measurement
resolution its addendum freezes from the pilot, and every cell that passes
stays within it
([decision table](disturbance-record.md#decisions-of-63bad95d-r1-native-ladder-confirmation)).
In each inconclusive cell the median of the raw pair ratios reaches the
decision floor (same table); the raw-pair rows above it give the pair
dispersion beside the pilot's.

The disturbance is therefore sufficient to explain all three inconclusive
outcomes and the missing qualification, and the receipt does not show whether
the native build is worse or not worse on those cells. The three
passing outcomes are reached with intervals inside the frozen resolution and
do not depend on it. The receipt stays committed as measured, and this record
claims no native-build result for the inconclusive cells.

By invoker ruling the disturbed stage is repeated once on a quiet host:
F-08 states the repeat family and where it stands.

## F-07: the dispatch-overhead family has no confirmation

`63bad95d-r2-profile-dispatch-overhead-pilot` is the second trial of the
frozen pilot addendum and is accepted as a pilot
([acceptance summary](../../../bench_results/63bad95d/63bad95d-profile-dispatch-overhead/r2-pilot/acceptance-summary.md)).
It also runs on a loaded host
([disturbance record](disturbance-record.md#host-load-each-campaign-records)).
The cell that sets the first trial's resolution is narrow in the second, and
the other two cells are wider than in the first
([interval and raw-pair tables](disturbance-record.md#63bad95d-profile-dispatch-overhead)):
each trial holds a cell whose interval is several times its counterpart's, and
the two trials disagree on which.

The canonical freezer refuses to derive a confirmation from the second trial
with the margins the pilot addendum declares; its message is kept verbatim in
[`63bad95d-profile-dispatch-overhead-r2-freeze-refusal.txt`](campaigns/63bad95d-profile-dispatch-overhead-r2-freeze-refusal.txt),
and
[`63bad95d-profile-dispatch-overhead-r2-pilot-resolution.txt`](campaigns/63bad95d-profile-dispatch-overhead-r2-pilot-resolution.txt)
gives the trial's intervals at the confirmation's corrected alpha. The margins
are not replaced: a margin above one plus that resolution exceeds the margin
of the [superseded](campaigns/63bad95d-profile-dispatch-overhead-confirmation-superseded.md)
confirmation, which is set aside for being wider than the effect the addendum
declares worthwhile.

The family therefore has no frozen confirmation, no arm smoke of one and no
queue line. The pilot addendum's `search_budget.max_pilot_trials_per_cell` is
spent by the two pilot lines of the
[ledger](../../../bench_results/63bad95d/63bad95d-profile-dispatch-overhead-ledger.jsonl),
and the family's one confirmatory attempt is unspent. The retention rule of
plan §7 for the baked bit-backend threshold reads a dispatch-overhead
confirmation, and none exists. By invoker ruling the family is repeated on a
quiet host (F-08).

## F-08: both disturbed families are repeated under their own ledgers

`63bad95d-native-ladder-v2` and `63bad95d-profile-dispatch-overhead-v2`
repeat the two families: `survey/planned-families.json` gives each the cells,
arms and settings of the family its `repeats` field names, its own ledger
and, for the dispatch family, its own holdout declaration. The description
of each pilot addendum
([native ladder](campaigns/63bad95d-native-ladder-v2-pilot.json),
[dispatch overhead](campaigns/63bad95d-profile-dispatch-overhead-v2-pilot.json))
states the repetition and cites the
[recorded load](disturbance-record.md#host-load-each-campaign-records).
The receipts of the repeated families stay committed as measured and take
no part in a repeat's decisions.

Each repeat starts at its pilot. The protocol admits resolution evidence
from the confirmation's own family alone (`protocol.md`, *Roles and
freezing*: "Missing, self-referential, cross-family, or non-pilot evidence
rejects (P-03)"), and the canonical freezer refuses a pilot receipt of
another family, so the pilot of `63bad95d-native-ladder` does not serve the
repeat's confirmation.

State of each repeat: the pilot addendum is frozen, every arm of every pilot
cell is smoked through the shared runner
([native ladder](survey/native-ladder-v2-pilot-smoke.json),
[dispatch overhead](survey/profile-dispatch-overhead-v2-pilot-smoke.json)),
and the pilot is a line of the benchmark-window queue that starts only on a
quiet host. Both ledgers are empty. No repeat has a receipt, a frozen
confirmation or an outcome; the confirmation of each is frozen from its
committed pilot receipt and measured in a later window. The native-build
result for the inconclusive cells of F-06 and the retention decision of plan
§7 wait for those confirmations.

## What the campaign does not decide

- No protocol receipt: the producer's statistic is its own, and its decisions
  are calibration evidence for the offline tuning system only (plan §2).
- The transpose lane and the carry-save count routes have no selector in the
  core section (plan §8).
- The decoder and BCH encode selectors belong to the coding-owned section
  (REQ-12).
