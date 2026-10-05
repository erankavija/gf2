# Calibration plan for the core kernel selectors (jit:63bad95d)

> **Diátaxis Type:** Research

This plan freezes no addendum, queues no campaign and changes no threshold. It
rests on three generated records and one ledger:

- the [route inventory](route-inventory.md) (`survey/make-route-inventory.py`),
  cited by section;
- the [calibration budget](calibration-budget.md)
  (`survey/make-calibration-budget.py`), which holds every count, corrected
  alpha, tail-draw value and minute estimate this plan refers to;
- the planned families and cells,
  [`survey/planned-families.json`](survey/planned-families.json), which the
  budget record tabulates, with the holdout declaration
  [`survey/matvec-holdout-cells.json`](survey/matvec-holdout-cells.json);
- [`survey/source-evidence.json`](survey/source-evidence.json), whose claim
  identifiers appear in backticks.

The [measurement contract](../measurement-contract.md) and
[protocol version 4](../../f547c394/protocol.md) govern every timed cell. The
external comparator is M4RI [AlbrechtBard2026].

## 1. Selectors and their disposition

The inventory's [evidence table](route-inventory.md#evidence-in-hand-per-selector)
gives, per selector, the families that contrast its two sides, their
confirmations and the outcome documents that state each disposition. This
plan adds the calibration disposition.

| Selector | Owner today | Evidence class | Disposition in this issue |
|---|---|---|---|
| `bit_matrix.matvec_simd_min_words` | core section, compile time; the measured owner omits it | exploratory | **The one candidate.** `4337c02e` confirms the value; this issue reconfirms the profile on held-out strides (§4, §6). |
| `PRODUCTION_PREFERENCE` (transpose block lane) | constant of `gf2-kernels-simd` | confirmatory; the confirmations do not qualify | Retained at its value and standing. The non-qualifying confirmations are the recorded outcome (§8). |
| `popcount-comparator-kernels` (carry-save and scalar `POPCNT` routes) | no selector exists | confirmatory; neither confirmation qualifies | Retained as recorded. The exploratory carry-save crossover is a limit of this calibration (§8). |
| `bit_backend.simd_min_words` | core section, compile time; the measured owner states it | confirmatory | Retained. Small-input cells of §6 guard it. |
| `raw-batch-default-lane`, `gf2m-batch-capability` | constants and capability checks | confirmatory | Retained: the confirmations keep the sequential lane and add no length gate. |
| `wide-kernel-widths`, `gf256-table-predicate`, `residual-shift-capability` | structural or capability choices | confirmatory, qualifying | Retained: each route is adopted by its own qualifying receipt. |
| `bit_matrix.transpose_simple_max_blocks`, `bit_matrix.transpose_macro_tile_blocks`, `field_vec.dot_chunk_len` | core section; the measured owner states them | none under this protocol | Retained at the measured owner's values. |

The remaining core selectors route no entry point a kernel family of this
epic measures; the inventory's
[selector table](route-inventory.md#selectors-the-tuning-sections-own) lists
them, and the [private list](route-inventory.md#thresholds-and-selectors-outside-the-tuning-sections)
gives every scanned constant outside the sections with its class and read
sites. The decoder and BCH encode selectors belong to the coding-owned section
(`coding-tuning-section`) and are outside this issue (REQ-12).

A domain where no candidate wins, or where the confirmed strides do not form
one interval, keeps its conservative value; §7 states the rule.

## 2. What "calibrate through the existing offline tuning system" means here

Two systems exist, and REQ-08 names both.

**The offline tuning system** is the core producer
`crates/gf2-core/benches/tuning_calibration.rs`, run by
`dev/scripts/tuning-extent-campaign.sh <issue>` from a committed campaign
declaration (`calibration-launcher-declaration`). One campaign measures the
whole core section afresh, publishes a versioned owner envelope whose
measurement block carries the producer's behavior token and executable digest,
composes the complete envelope and writes a receipt
(`calibration-publishes-profile-id`). The baked constants mirror the measured
owner and pin it by digest (`tuning-baked-owner-digest`,
`baked-measured-owner-test`). The measured owner's identity fields are in the
inventory record under `tuning.measured_owner`.

**The frozen protocol** is the Zen 3 protocol: paired A/B cells, a family
ledger and an acceptance tool. Its `selector-calibration` purpose requires
holdout cells (`protocol-holdout-required`), and each arm records its
RUSTFLAGS and tuning profile.

The mapping this plan uses:

| REQ-08 clause | Carried by |
|---|---|
| Calibrate candidate selectors using the frozen protocol | Protocol families: the threshold confirmation of `4337c02e` and the committed lane-selection confirmation of `1d4fd63d` are the calibration evidence; the families of §6 measure the resulting profile. |
| Independently reconfirm the resulting profile | The holdout families of §6, whose cells no calibration family measured. |
| Commit the versioned profile with producing-tool behavioral identity and linked receipts | An owner envelope of the offline tuning system and the baked constants that pin it. |

What this issue can do with the offline tuning system as it stands:

- Run a complete core campaign under its own run-identifier prefix by
  committing a campaign declaration that names `63bad95d`, with a producing
  manifest and a protocol document that amends the executed one by reference.
  The launcher, driver and validator need no change. The campaign re-measures
  every field the producer sweeps and publishes a new owner envelope, a
  complete envelope and a receipt; one session's budget is in the
  [budget record](calibration-budget.md#planned-families).
- Pin the new owner from the baked constants and the committed-profile tests.

What requires a change to that system:

- **A matvec threshold in the profile.** The codec has the field, and the
  producer does not sweep it: its field list is closed (`calibration-fields`),
  it publishes only when measured and omitted fields partition a fixed
  inventory (`calibration-inventory-closed`), and the owner codec admits a
  stated value only with its measurement. The baked value is held equal to the
  conservative one while no measured owner states it
  (`baked-matvec-conservative-test`). Installing a host-specific threshold
  therefore needs a new retained-threshold sweep in the producer. The field is
  selected at compile time, so its arms call the two lanes directly, as the
  bit-backend sweep calls the two backends (`calibration-bit-arm`), through
  `BitMatrix::matvec_with_route` (`matvec-lane-entry`). The change
  touches the producer's counts, the independent validator
  (`calibration-validator-behavior`), a protocol amendment and the campaign
  declaration.
- **A transpose lane in the profile.** No codec field exists. The kernel crate
  cannot read a `gf2-core` selector (crate dependency direction), so ownership
  by the core section needs a preference argument on the kernel crate's
  detection, as the raw-batch lane has (`clmul-batch-default-lane`), a new
  baked field, a codec schema change and a categorical sweep in the producer.
- **Anything the producer measures on another statistic.** The producer's
  crossover rule is its own; it does not produce protocol receipts. This plan
  does not ask it to.

This issue makes the first change, the matvec sweep (§8); the transpose lane
stays outside the profile.

## 3. Build configurations (REQ-09)

`planned-families.json` declares the three arms once, under `builds`.

| Arm identity | gf2 build | Selectors | Competitor targeting |
|---|---|---|---|
| `conservative-portable` | default target, `simd` | conservative table, no profile | portable build, its own runtime dispatch if it has one |
| `tuned-portable` | default target, `simd`, `--cfg gf2_tuning_baked` | the candidate profile: baked constants and the installed envelope, pinned as the arm's tuning profile | the same portable build |
| `native` | `-C target-cpu=native` added | the candidate profile | built with `-march=native` |

The three are separate arms and separate executables; no cell mixes two inside
one arm. A selector's effect is a `tuned-portable` candidate against a
`conservative-portable` baseline, so both sides share one instruction-set
level and differ in selectors alone. The native ladder compares `native`
against `tuned-portable`. Each comparator-gap cell pairs gf2 and the external
library at one level; the conservative level is committed evidence (the M4RI
confirmation the [mid-range synthesis](../../2037941f-profile-and-optimize-mid-range-buffer-operations/mid-range-findings.md)
cites).

`tuned-portable` here is defined by the profile's selectors at the portable
instruction-set level. The polynomial host-targeting campaigns give the same
arm identity another meaning: an instruction-set flag and no profile, as the
`rustflags` of their arms record in
[`workload-routes.jsonl`](workload-routes.jsonl). The two are different
levels, and a result of one is no result of the other.

## 4. Overlap with `4337c02e`

`4337c02e` stays a separate family that feeds this issue.

- It owns the allocated whole-`matvec` cells at the strides whose exploratory
  rows favour the scalar lane and at their neighbours, their frozen margins
  and the confirmation. Its outcome is a threshold value, or no change.
- This issue owns the profile that carries the value, the producer sweep by
  which the value reaches the tuning system (§2, §8) and the reconfirmation on
  strides `4337c02e` does not measure (§6).
- The two families ask different questions on different workloads, each on
  its own ledger; the budget record's ledger table shows what the existing
  allocated-`matvec` ledger still admits.
- One edit installs the value: the baked constant with its measured owner,
  made in this issue. `4337c02e` changes no selector constant, which keeps its
  REQ-05 (one source of the threshold) and this issue's REQ-11 consistent.

Ordering:

1. The strides of `63bad95d-profile-holdout-matvec` are fixed in
   [`survey/matvec-holdout-cells.json`](survey/matvec-holdout-cells.json),
   which lists the reserved and the held-out strides; `4337c02e` declares no
   cell at a reserved stride.
2. `4337c02e` runs its pilot and confirmation and publishes its outcome.
3. The producer sweep and a full core campaign publish the measured owner
   that states the threshold.
4. This issue freezes its families against that owner and measures.

If `4337c02e` records no change, the matvec families of §6 do not run and the
conservative threshold is recorded as retained.

## 5. Calibration evidence and the profile

The candidate profile differs from the conservative table, in the entry points
this epic measures, in one selector: the matvec threshold. Its calibration
evidence has two parts, the confirmation of `4337c02e` under the frozen
protocol and the retained-threshold sweep of the offline tuning producer; the
measured owner states the sweep's value, and the baked constant mirrors the
owner. The holdout family of §6 measures the profile on strides neither part
uses.

The owner also restates every other field the producer sweeps, because one
campaign measures the whole core section. Those fields route entry points
outside this epic's families, and this plan neither calibrates nor reconfirms
them.

## 6. Families and cells

The [budget record](calibration-budget.md#planned-cells) lists every planned
cell with its role, objective, metric, core arm, cache state, builds and
workload, and marks the small-input cells. Per family it gives the cell
counts, the corrected alpha, the expected draws per tail under P-20 and the
estimated minutes. Every family is a first attempt on a new ledger, and the
record's first table gives the number of cells such an attempt admits; the
generator refuses a family above it.

| Family | Purpose | What its cells decide |
|---|---|---|
| `63bad95d-profile-holdout-matvec` | selector calibration | The profile's matvec threshold, on strides and cache states `4337c02e` does not measure. |
| `63bad95d-profile-dispatch-overhead` | selector calibration | Non-regression of small inputs at entry points whose selection the profile leaves unchanged. |
| `63bad95d-native-ladder` | consumer family | The native build against the portable profile build, one primary workload per entry-point group. |
| `63bad95d-matvec-external-ladder` | consumer family | The gap to the matched M4RI operation at the tuned and native levels. |

Rules common to the families:

- **Pilot.** Each family has one exploratory pilot of its non-holdout cells at
  the pair count `planned-families.json` declares; the canonical freezer
  `dev/active/c7113c5a/survey/freeze-confirmation.py` derives the confirmation
  addendum from the committed pilot receipt.
- **Holdout.** Holdout cells are declared in a holdout record before the
  pilot runs and attached by the freezer; no sample of a holdout workload
  exists before the confirmation. The matvec declaration is committed.
- **Objectives.** Every cell of the three gf2 families is a non-regression
  cell. The improvement claim for the threshold is the confirmation of
  `4337c02e`; the holdout family shows the profile is not worse where that
  confirmation did not look.
- **Margins.** Each family freezes its worthwhile-effect and equivalence
  margins from its own pilot, strictly above one plus the pilot-derived
  resolution, with a rationale tied to the consumer.
- **Comparator cells** select nothing; they report the gap at an equal build
  level.
- **Core arms.** The entry points are leaf kernels without an internal worker
  pool, so every cell runs on one core.

The budget record's exclusion table states why the plan holds no transpose
family: the lane is no candidate, and the profile selects nothing on the
transpose route, so a tuned transpose arm is the conservative arm that the
committed comparator confirmations measure. The native ladder keeps
non-regression cells of the transpose entry point.

## 7. Adoption and preserved conservative values

- The threshold enters the baked constants only if the confirmation of
  `4337c02e` qualifies, the measured owner states a value consistent with it,
  and the holdout and dispatch-overhead families qualify.
- Otherwise the baked threshold keeps its conservative value, and each
  receipt is committed as the outcome.
- The matvec threshold is a single boundary. If the strides `4337c02e`
  confirms for the scalar lane do not form one interval that starts at the
  conservative threshold, no threshold expresses the result and the
  conservative value stays.
- A production change carries its before and after cells: each holdout cell's
  baseline arm is the route before the change and its candidate arm the route
  after it, on the same workload.
- After the profile is committed, the route witnesses of the fallback record
  are run again, with the profile build added, and the inventory is
  regenerated, so the routes the production consumers execute are observed.

## 8. Decisions

1. **Transpose lane.** `PRODUCTION_PREFERENCE` keeps its value and its
   standing as a kernel-crate constant. It is no calibration candidate, and
   the non-qualifying confirmations of `1d4fd63d` and `04b85d10` are the
   recorded outcome. The plan holds no cell that selects a lane.
2. **Matvec threshold.** The offline tuning producer gains a
   retained-threshold sweep of `bit_matrix.matvec_simd_min_words`, and a full
   core campaign publishes the measured owner. The change covers the
   producer's behavior token and counts, the independent validator, a protocol
   document that amends the executed one by reference, and the campaign
   declaration.
3. **Carry-save crossover.** No selector is added. The exploratory crossover
   of the carry-save count routes is a recorded limit of this calibration.
4. **Tuned level.** `tuned-portable` is defined by profile selectors (§3).
5. **Build without `simd` in CI.** The non-simd `gf2-core` configuration of
   `316150fd` runs the dispatch contract.
