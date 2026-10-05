# Calibration plan for the core kernel selectors (jit:63bad95d)

> **Diátaxis Type:** Research

This plan is for review. It freezes no addendum, queues no campaign and changes
no threshold. It rests on three generated records and one ledger:

- the [route inventory](route-inventory.md) (`survey/make-route-inventory.py`),
  cited by section;
- the [calibration budget](calibration-budget.md)
  (`survey/make-calibration-budget.py`), which holds every count, corrected
  alpha, tail-draw value and minute estimate this plan refers to;
- the planned families and cells,
  [`survey/planned-families.json`](survey/planned-families.json), which the
  budget record tabulates;
- [`survey/source-evidence.json`](survey/source-evidence.json), whose claim
  identifiers appear in backticks.

The [measurement contract](../measurement-contract.md) and
[protocol version 4](../../f547c394/protocol.md) govern every timed cell. The
external comparators are M4RI [AlbrechtBard2026] and Bitshuffle
[Bitshuffle2026].

## 1. Selectors and their disposition

The inventory's [evidence table](route-inventory.md#evidence-in-hand-per-selector)
gives, per selector, the families that contrast its two sides, their
confirmations and the outcome documents that state each disposition. This
plan adds the calibration disposition.

| Selector | Owner today | Evidence class | Disposition in this issue |
|---|---|---|---|
| `bit_matrix.matvec_simd_min_words` | core section, compile time; the measured owner omits it | exploratory | **Candidate.** `4337c02e` confirms the value; this issue reconfirms the profile on held-out strides (§4, §6). |
| `PRODUCTION_PREFERENCE` (transpose block lane) | constant of `gf2-kernels-simd` | confirmatory; the confirmation does not qualify | **Candidate**, subject to the first open decision (§8). |
| `popcount-comparator-kernels` (carry-save and scalar `POPCNT` routes) | no selector exists | confirmatory; neither confirmation qualifies | Retained as recorded. A crossover selector is a new mechanism (§8, third decision). |
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
  bit-backend sweep calls the two backends (`calibration-bit-arm`); the scalar
  lane has no public entry today (`matvec-scalar-lane-private`). The change
  touches the producer's behavior token and counts, the independent validator
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

The epic that built this tooling is archived, so these changes have no open
owner. They are listed under the open decisions (§8) and none is made here.

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
library at one level; the conservative level of both comparators is committed
evidence (the comparator confirmation of `1d4fd63d` and the M4RI confirmation
the [mid-range synthesis](../../2037941f-profile-and-optimize-mid-range-buffer-operations/mid-range-findings.md)
cites). The polynomial host-targeting campaigns define their tuned level by an
instruction-set flag instead, as the `rustflags` of their arms record in
[`workload-routes.jsonl`](workload-routes.jsonl); §8 lists the choice.

## 4. Overlap with `4337c02e`

`4337c02e` stays a separate family that feeds this issue.

- It owns the allocated whole-`matvec` cells at the strides whose exploratory
  rows favour the scalar lane and at their neighbours, their frozen margins
  and the confirmation. Its outcome is a threshold value, or no change.
- This issue owns the profile that carries the value, the mechanism by which
  the value reaches the tuning system (§2, §8) and the reconfirmation on
  strides `4337c02e` does not measure (§6).
- The two families ask different questions on different workloads, each on
  its own ledger; the budget record's ledger table shows what the existing
  allocated-`matvec` ledger still admits.
- One edit installs the value: the baked constant with its measured owner,
  made in this issue. `4337c02e` changes no selector constant, which keeps its
  REQ-05 (one source of the threshold) and this issue's REQ-11 consistent.

Ordering:

1. The held-out strides of `63bad95d-profile-holdout-matvec` are fixed by the
   approval of this plan, before `4337c02e` freezes its addendum; `4337c02e`
   declares none of them.
2. `4337c02e` runs its pilot and confirmation and publishes its outcome.
3. This issue freezes its families against that outcome, builds the profile
   and measures.

If `4337c02e` records no change, the matvec families of §6 do not run and the
conservative threshold is recorded as retained.

## 5. Calibration evidence and the profile

The candidate profile differs from the conservative table in at most two
selectors:

- the matvec threshold `4337c02e` confirms;
- the transpose lane the lane-selection confirmation of `1d4fd63d` measured as
  its candidate, if the first decision of §8 admits it.

The lane-selection confirmation is used as recorded. Its receipt does not
qualify, its family's ledger admits no further cell (budget record, ledger
table), and this plan repeats none of its cells. The transpose holdout family
asks a different question, on workloads that family did not measure: whether
the profile that carries the lane helps or leaves unharmed the transpose
entry point's other workloads. Its outcome, not a re-reading of the earlier
receipt, decides adoption.

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
| `63bad95d-profile-holdout-transpose` | selector calibration | The profile's transpose lane, on block counts, shapes and a multicore streaming run the lane-selection family did not measure. |
| `63bad95d-profile-dispatch-overhead` | selector calibration | Non-regression of small inputs at entry points whose selection the profile leaves unchanged. |
| `63bad95d-native-ladder` | consumer family | The native build against the portable profile build, one primary workload per entry-point group. |
| `63bad95d-transpose-external-ladder` | kernel family | The gap to M4RI and Bitshuffle at the tuned and native levels. |
| `63bad95d-matvec-external-ladder` | consumer family | The gap to the matched M4RI operation at the tuned and native levels. |

Rules common to the families:

- **Pilot.** Each family has one exploratory pilot of its non-holdout cells at
  the pair count `planned-families.json` declares; the canonical freezer
  `dev/active/c7113c5a/survey/freeze-confirmation.py` derives the confirmation
  addendum from the committed pilot receipt.
- **Holdout.** Holdout cells are declared in a holdout record before the
  pilot runs and attached by the freezer; no sample of a holdout workload
  exists before the confirmation.
- **Objectives.** A cell is an improvement cell only where committed
  confirmatory evidence records the same operation improved; whole consumers
  in which the changed kernel is a minority of the work, and every small
  input, are non-regression cells. The matvec holdout family fixes each
  cell's objective at freeze from the decisions `4337c02e` confirms, by the
  rule its entry in `planned-families.json` states.
- **Margins.** Each family freezes its worthwhile-effect and equivalence
  margins from its own pilot, strictly above one plus the pilot-derived
  resolution, with a rationale tied to the consumer.
- **Comparator cells** select nothing; they report the gap at an equal build
  level.
- **Core arms.** The entry points are leaf kernels without an internal worker
  pool, so cells run on one core; the streaming transpose holdout adds the
  six-core arm, as the lane-selection family did.

## 7. Adoption and preserved conservative values

- A selector enters the committed profile only if every non-exploratory cell
  of its holdout family passes and the dispatch-overhead family passes.
- A selector whose holdout family does not qualify keeps its conservative
  value; the receipt is committed as the outcome.
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

## 8. Open decisions

1. **Transpose lane ownership.** Either the core section gains a lane field
   (§2), or `PRODUCTION_PREFERENCE` stays a kernel-crate constant under a
   named convergence exception and the holdout family decides its value. The
   exception is the smaller change and keeps the selection in one place; the
   field makes the lane host-specific.
2. **Matvec threshold mechanism.** Either the producer gains the sweep (§2)
   and a full core campaign publishes the owner, or the confirmed value
   changes the conservative constant for every host. The second contradicts
   the meaning of the conservative table when the evidence is one host's.
3. **Carry-save crossover.** A crossover selector for the carry-save count
   routes needs two new selector fields and a resolver change, and both
   families that measured the routes are closed without a qualifying
   confirmation. This plan does not include it.
4. **Tuned level.** §3 defines `tuned-portable` by selectors at the portable
   instruction-set level. The polynomial host-targeting campaigns define the
   same identity by an instruction-set flag.
5. **CI coverage of the build without `simd`.** The
   [fallback record](fallback-verification.md#what-ci-executes) states that no
   CI step runs the contract on that build.
