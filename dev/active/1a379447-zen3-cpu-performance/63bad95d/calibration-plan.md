# Calibration plan for the core kernel selectors (jit:63bad95d)

> **Diátaxis Type:** Research

The core campaign this plan declares is published;
[`campaign-findings.md`](campaign-findings.md) states what it decides, and the
plan below is the plan those findings leave. It rests on four generated
records and one ledger:

- the [route inventory](route-inventory.md) (`survey/make-route-inventory.py`),
  cited by section;
- the [calibration budget](calibration-budget.md)
  (`survey/make-calibration-budget.py`), which holds every count, corrected
  alpha, tail-draw value and minute estimate this plan refers to;
- the [campaign tables](campaign-tables.md) (`survey/make-campaign-tables.py`),
  which hold every published selector value and sweep figure;
- the planned families and cells,
  [`survey/planned-families.json`](survey/planned-families.json), which the
  budget record tabulates and from which `survey/make-family-addenda.py`
  projects the pilot addenda and the holdout declaration
  [`survey/dispatch-holdout-cells.json`](survey/dispatch-holdout-cells.json);
- [`survey/source-evidence.json`](survey/source-evidence.json), whose claim
  identifiers appear in backticks.

The [measurement contract](../measurement-contract.md) and
[protocol version 4](../f547c394/protocol.md) govern every timed cell. The
external comparator is M4RI [AlbrechtBard2026].

## 1. Selectors and their disposition

The inventory's [evidence table](route-inventory.md#evidence-in-hand-per-selector)
gives, per selector, the families that contrast its two sides, their
confirmations and the outcome documents that state each disposition. This
plan adds the calibration disposition.

| Selector | Owner today | Evidence class | Disposition in this issue |
|---|---|---|---|
| `bit_matrix.matvec_simd_min_words` | core section, compile time; the measured owner states it | producer sweep; exploratory under the protocol | Retained at the conservative value: the sweep is non-monotone and `4337c02e` confirms no value (§5, §7). |
| `PRODUCTION_PREFERENCE` (transpose block lane) | constant of `gf2-kernels-simd` | confirmatory; the confirmations do not qualify | Retained at its value and standing. The non-qualifying confirmations are the recorded outcome (§8). |
| `popcount-comparator-kernels` (carry-save and scalar `POPCNT` routes) | no selector exists | confirmatory; neither confirmation qualifies | Retained as recorded. The exploratory carry-save crossover is a limit of this calibration (§8). |
| `bit_backend.simd_min_words` | core section, compile time; the measured owner states it | producer sweep; confirmatory, not qualifying | **The one candidate.** The measured owner states a value below the conservative one and the baked constant mirrors it; the dispatch-overhead family of §6 reconfirms it (§5, §7). |
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
| Calibrate candidate selectors using the frozen protocol | The dispatch-overhead family of §6, a selector-calibration family whose pilot and confirmation measure the profile's one changed baked selector; the lane-selection confirmation of `1d4fd63d` and the rejected threshold confirmation of `4337c02e` are the recorded protocol outcomes for the two lanes the profile leaves conservative. |
| Independently reconfirm the resulting profile | The holdout cells of that family, which no pilot and no producer sweep measures. |
| Commit the versioned profile with producing-tool behavioral identity and linked receipts | An owner envelope of the offline tuning system and the baked constants that pin it. |

What the offline tuning system does for this issue:

- **The matvec threshold is a swept field.** The producer sweeps
  `bit_matrix.matvec_simd_min_words` as a retained threshold
  (`calibration-matvec-field`). The field is selected at compile time, so its
  arms pin the two lanes through `BitMatrix::matvec_with_route`
  (`calibration-matvec-arms`, `matvec-lane-entry`), as the bit-backend sweep
  calls the two backends (`calibration-bit-arm`). The producer publishes only
  when measured and omitted fields partition its fixed inventory
  (`calibration-inventory-closed`), and the independent validator carries the
  field (`validator-matvec-grid`). [`matvec-sweep-design.md`](matvec-sweep-design.md)
  states the design.
- **A campaign is declared.** [`campaign-declaration.json`](campaign-declaration.json)
  names `63bad95d`; [`premeasurement-protocol.md`](premeasurement-protocol.md)
  amends the executed seam protocol by reference, and the producing manifest
  is generated. The launcher and driver are unchanged
  (`calibration-launcher-declaration`). The campaign re-measures every field
  the producer sweeps and publishes a new owner envelope, a complete envelope
  and a receipt; one session's budget is in the
  [budget record](calibration-budget.md#planned-families).
- **The baked constants follow the owner.** The baked constants pin the
  published owner by digest (`tuning-baked-owner-digest`) and mirror the
  values it states, the matvec threshold included (`baked-matvec-owner-test`,
  `baked-measured-owner-test`).

What stays outside that system:

- **A transpose lane in the profile.** No codec field exists. The kernel crate
  cannot read a `gf2-core` selector (crate dependency direction), so ownership
  by the core section needs a preference argument on the kernel crate's
  detection, as the raw-batch lane has (`clmul-batch-default-lane`), a new
  baked field, a codec schema change and a categorical sweep in the producer.
- **Anything the producer measures on another statistic.** The producer's
  crossover rule is its own; it does not produce protocol receipts. This plan
  does not ask it to.

The transpose lane stays outside the profile (§8).

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
against `tuned-portable`. The plan holds no comparator-gap cell: the profile
selects the conservative lane on the routes the committed comparator
confirmations measure (the M4RI confirmation the
[mid-range synthesis](../2037941f-profile-and-optimize-mid-range-buffer-operations/mid-range-findings.md)
cites), so those receipts are the comparator evidence at the profile's
selectors.

`tuned-portable` here is defined by the profile's selectors at the portable
instruction-set level. The polynomial host-targeting campaigns give the same
arm identity another meaning: an instruction-set flag and no profile, as the
`rustflags` of their arms record in
[`workload-routes.jsonl`](workload-routes.jsonl). The two are different
levels, and a result of one is no result of the other.

## 4. Overlap with `4337c02e`

`4337c02e` is a separate family on its own ledger: the allocated
whole-`matvec` cells at the strides whose exploratory rows favour the scalar
lane and at their neighbours. This issue owns the producer sweep by which a
threshold reaches the tuning system and the one edit that would install a
value, the baked constant with its measured owner; `4337c02e` changes no
selector constant, which keeps its REQ-05 (one source of the threshold) and
this issue's REQ-11 consistent.

`4337c02e` records no change: its confirmation receipt is rejected by
acceptance and it installs no value. The matvec families therefore do not run
(the plan record's exclusion table), the holdout declaration
[`survey/matvec-holdout-cells.json`](survey/matvec-holdout-cells.json) stays
committed and unused, and the conservative threshold is retained.

## 5. Calibration evidence and the profile

The profile is the measured owner of this issue's campaign. Among the baked
constants it differs from the conservative table in one selector,
`bit_backend.simd_min_words`; the [selector table](campaign-tables.md#selectors-per-owner)
gives every value and the [sweep tables](campaign-tables.md#bit-backend-threshold-sweep)
the producer's evidence for it. The dispatch-overhead family of §6 measures
the profile build against the conservative build on both sides of the two
thresholds, with holdout cells at operation and length pairs the producer does
not time.

The owner states the matvec threshold at its conservative value
([matvec lane sweep](campaign-tables.md#matvec-lane-sweep)).

The owner also restates every other field the producer sweeps, because one
campaign measures the whole core section. Those fields are read from an
installed profile at run time and route entry points outside this epic's
families; this plan neither calibrates nor reconfirms them.
[`campaign-findings.md`](campaign-findings.md) names the one whose value
differs between this owner and the owner of the seam campaign.

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
| `63bad95d-profile-dispatch-overhead` | selector calibration | Non-regression of small inputs of the bit-backend entry points, the profile build against the conservative build, on both sides of the two thresholds. |
| `63bad95d-native-ladder` | consumer family | The native build against the portable profile build, one primary workload per entry-point group. |

The harness is `survey/profile-arms`: one executable built once per build
level, which refuses a request for a level it is not, installs the pinned
owner on the profile levels and reports the routes it observes.
`dev/bench_results/63bad95d/run-profile-families.sh` builds, smokes, launches
and freezes the campaigns.

Rules common to the families:

- **Pilot.** Each family pilots its non-holdout cells at the pair count
  `planned-families.json` declares, within the pilot trials per cell its
  pilot addendum's search budget declares; the canonical freezer
  `dev/active/1a379447-zen3-cpu-performance/c7113c5a/survey/freeze-confirmation.py` derives the confirmation
  addendum from the committed receipt of the pilot trial it names. The
  dispatch-overhead family takes a second trial; the
  [supersession record](campaigns/63bad95d-profile-dispatch-overhead-confirmation-superseded.md)
  of the confirmation frozen from its first states why.
- **Holdout.** Holdout cells are declared in a holdout record before the
  pilot runs and attached by the freezer; no sample of a holdout workload
  exists before the confirmation.
- **Objectives.** Every cell is a non-regression cell. The improvement claim
  for the bit-backend threshold is the producer's crossover decision; the
  family shows the profile build is not worse.
- **Margins.** Each family freezes its worthwhile-effect and equivalence
  margins from its own pilot, strictly above one plus the pilot-derived
  resolution, with a rationale tied to the consumer.
- **Core arms.** The entry points are leaf kernels without an internal worker
  pool, so every cell runs on one core.

The budget record's exclusion table states why the plan holds no matvec
family and no transpose family: the profile selects the conservative lane on
both routes, so a tuned arm is the conservative arm that the committed
comparator confirmations measure. The native ladder keeps a non-regression
cell of each entry point.

## 7. Adoption and preserved conservative values

- The matvec threshold keeps its conservative value: the producer's sweep is
  non-monotone and the protocol confirmation of `4337c02e` is rejected, so no
  single boundary expresses a result.
- The baked bit-backend threshold mirrors the measured owner. It is retained
  when no confirmatory or holdout cell of the dispatch-overhead confirmation
  records `fail`, and takes the conservative value otherwise, with each
  receipt committed as the outcome; the family's addendum freezes this rule.
- A production change carries its before and after cells: a cell's baseline
  arm is the conservative build and its candidate arm the profile build, on
  the same workload.
- After the profile is committed, the route witnesses of the fallback record
  are run again, with the profile build added, and the inventory is
  regenerated, so the routes the production consumers execute are observed.

## 8. Decisions

1. **Transpose lane.** `PRODUCTION_PREFERENCE` keeps its value and its
   standing as a kernel-crate constant. It is no calibration candidate, and
   the non-qualifying confirmations of `1d4fd63d` and `04b85d10` are the
   recorded outcome. The plan holds no cell that selects a lane.
2. **Matvec threshold.** The offline tuning producer sweeps
   `bit_matrix.matvec_simd_min_words` as a retained threshold, and the full
   core campaign publishes the measured owner that states it.
3. **Carry-save crossover.** No selector is added. The exploratory crossover
   of the carry-save count routes is a recorded limit of this calibration.
4. **Tuned level.** `tuned-portable` is defined by profile selectors (§3).
5. **Build without `simd` in CI.** The non-simd `gf2-core` configuration of
   `316150fd` runs the dispatch contract.
