# Integrated competitive scorecard: design

> **Diátaxis Type:** Explanation

Design record of `@/issue/1362381c`, the final integrated
scorecard of epic `@/issue/1a379447` (epic REQ-05 and the aspirational REQ-07,
with REQ-01, REQ-03, REQ-04 and REQ-06 support). The
[measurement contract](../measurement-contract.md), the
[frozen protocol](../f547c394/protocol.md) version 4 with its
[amendment](../f547c394/amendment-v4.md) and the
[worker brief](../worker-brief.md) govern its evidence; this design adds no
protocol, schema or checker version.

## Decisions

Each item is ruled before the work package it names starts. The invoker
selected the recommended option of D-1, D-2, D-3 and D-6 on 2026-10-25; the
lead selected the recommended option of D-4 and D-5. The recommendation is the
lightest option that meets the criteria literally.

**D-1. The frozen workload set is not defined anywhere.** The epic
description and this issue's REQ-07 and REQ-12 refer to "the frozen workload
set" and "every primary cell", and no committed document lists either. Every
candidate definition is family-local (for example
[logical-buffer addendum](../2037941f-profile-and-optimize-mid-range-buffer-operations/logical-buffer-addendum.md),
"five primary cells"; [calibration plan](../63bad95d/calibration-plan.md),
"one primary workload per entry-point group").

- Option A (recommended): WP-1 freezes the set as a generated, committed
  declaration `workload-set.json` before any trial of this issue. A workload
  cell is every confirmatory or holdout cell of the latest accepted
  protocol-v4 (or, where no v4 confirmation exists, the latest accepted)
  confirmation of each family in the family registry below, plus the
  integrated families of this issue. A primary cell is a workload cell with an
  external comparator arm. Families that end at a pilot contribute their cells
  as exploratory rows, never as primary cells. The declaration states its rule
  and the registry, so it is a projection of committed addenda, not a
  hand list.
- Option B: a hand-selected subset per domain (smaller scorecard, but
  "complete" then rests on a selection the criteria do not name).

**D-2. The canonical BCH evidence is not complete.** The BCH receipt of
`@/issue/fd9d5416`
([receipt](../../../bench_results/fd9d5416/2026-10-02-fd9d5416-bch-performance-receipt.md))
is Criterion-based, outside protocol v4, and its issue is `InProgress` with
failed doc-review and research-review gates. Session 19 removed the
`1362381c → fd9d5416` edge.

- Option A (recommended): consume that receipt and its survey data
  (`dev/bench_results/4e732b56/`) by path and digest as historical context,
  labelled with the issue state; consume the protocol-v4 BCH confirmations of
  this epic (`8e53235b` codeword assembly, `6fb89a3c` generator matrix,
  `04b85d10` layout consumers, `1d4fd63d` bitslice cells) by identity; and
  remeasure the affected integrated BCH encode cells under protocol v4 in
  family FI-4.
- Option B: wait for `fd9d5416` to complete; this blocks the scorecard on
  work outside the epic.

**D-3. Which integrated cells "remeasure".** REQ-07 requires remeasuring the
affected BCH cells, and the Background asks that kernel speedups survive
production dispatch. The design reads "affected" as follows: a cell is
affected when the code text of a measured gf2 production package differs from
the producing-input snapshot its receipt pins. The rule compares content
identities, consistent with `behavioral-evidence-validity`; Git history plays
no part. Unaffected cells are consumed by identity. The alternative,
remeasuring every primary cell, needs roughly one family per six primary cells
(P-20 below) and every family harness rebuilt. That fails the lightest-approach
standing rule and gains nothing the invariant does not already grant.

The [drift record](production-drift.json) contradicted the premise of D-3 when
it was ruled: every receipt committed by then whose measured content resolved was
`code-differs`, and the rest pinned no production content and were `unresolvable`.
The record states each receipt's current class. The invoker therefore ruled on
2026-10-25 that consumed cells keep their evidence by identity, each labelled
with its drift class (REQ-07 invalidates evidence only for producing
measurement-behavior changes), and that the integrated families remeasure only
what the criteria name: the BCH cells (FI-4), the core and decoder routes
(FI-1, FI-2) and the AFF3CT decoder comparison (FI-3); no family remeasures
other consumed cells.

**D-4. Several integrated families instead of one.** P-20 admits at most six
confirmatory and holdout cells per first attempt. The integrated measurement
is therefore split into families that each ask one post-integration question
on a fresh ledger. These are new questions, not renamed retries of a closed
family (protocol, ledger section). Recommended: accept.

**D-5. "Where meaningful" for core arms (REQ-08).** Recommended: core-route
cells measure one-core latency, because the core kernels run on one thread,
and each such cell records that reason in its declaration. Decoder-route and
BCH parallel-batch cells measure one core, six and twelve physical cores and
twenty-four logical CPUs.

**D-6. A BCH external arm needs harness work.** The `4e732b56` external BCH
programs (`dev/active/ae03bcd0-general-bch/4e732b56/baseline-survey/`) time
themselves and do not speak the runner's arm wire contract. FI-4 wraps them as
runner arms, as the AFF3CT arms are wrapped. Recommended: accept that scope.
The alternative records the external BCH comparison as unavailable under
protocol v4, which leaves REQ-12 without a measured BCH comparison.

## Problem statement

The epic's family surveys measured kernels and consumers one family at a time,
often before later production changes landed: the lane decoder and its selectors
(`ed3d490e`, `0a77ecfc`), worker-aware routing (`d06a99ed`), word-wise BCH
codeword assembly (`8e53235b`), the BMI2 shift route (`f8dd4dde`) and the core
profile's bit-backend selector (`63bad95d`). This issue publishes one scorecard
over the frozen workload set. Integrated rows are remeasured on release builds of
the integrated tree. Consumed rows stay historical evidence of the code their
receipts pin, each labelled with its drift class (D-3). Matched-algorithm and
fastest quality-compatible comparisons are reported separately, together with a
post-integration reprofile and a disposition for every material residual gap.

## Success criteria

The issue's items are authoritative: `@/issue/1362381c/requirement/REQ-01`,
`@/issue/1362381c/requirement/REQ-07`, `@/issue/1362381c/requirement/REQ-08`,
`@/issue/1362381c/requirement/REQ-09`, `@/issue/1362381c/requirement/REQ-10`,
`@/issue/1362381c/requirement/REQ-11` and `@/issue/1362381c/requirement/REQ-12`.

## Inventory per criterion

### Family registry

The registry is the family list `survey/make-workload-set.py` projects into `workload-set.json`. Each
family's receipts, ledgers and generated tables sit under
`dev/bench_results/<issue>/` and the record directory of its owning issue.

| Domain | Families (owning issue) | External arms measured |
|---|---|---|
| LDPC decode | `ldpc-update-*` (`07ca8585`); `ldpc-steady-*` (`3be770d5`); `ldpc-matched-algorithm-alist-v1`, `ldpc-quality-compatible-alist-v1` (`c077a88b`); `ldpc-batch-*` (`ed3d490e` children); `d06a99ed-worker-aware-production-v1`; `ldpc-qc-*` (`f63a2464`) | AFF3CT [Cassagne2019] |
| NR encode, de-rate matching | `nr-rate-matched-encode-baselines-v1` (`12fdeb5b`); `nr-llr-derate-matching-baselines` (`eda07788`) | srsRAN [Srsran2026], AFF3CT |
| DVB-T2 interleave | `dvb-t2-bit-interleave-baselines` (`eda07788`); `dvb-t2-interleave-consumer-profile` (`9fb40c83`) | xdsopl [Xdsopl2026] |
| BCH | `8e53235b-bch-codeword-assembly`; `bch-genmatrix-consumer` (`6fb89a3c`); BCH cells of `bit-storage-layout-consumers` (`04b85d10`) and `transpose-lane-selection` (`1d4fd63d`); `fd9d5416` receipt (D-2) | M4RI [AlbrechtBard2026]; AFF3CT and bchlib in `4e732b56` only |
| Popcount | `popcount-baselines`, `and-popcnt-baselines` (`26465e6c`); `popcount-route-selection`, `fused-count-consumers` (`5cbb6545`) | libpopcnt [Libpopcnt2026], Muła [Mula2018] |
| Polynomial / CLMUL | `polynomial-*` (`c7113c5a`); `wide-polynomial-competitiveness`, `gf2m-clmul-crossover` (`53c5a8c0`); `ymm-clmul-dispatch` (`1d0da41f`); `public-clmul-wide-dispatch` (`1c602857`) | gf2x [GfTwoX2026] |
| Transpose | `transpose-lane-*` (`1d4fd63d`); `transpose-vs-external` (`6fb89a3c`) | M4RI, Bitshuffle [Bitshuffle2026] |
| Logical buffers, matvec | `2037941f-*` families; `logical-buffer-vs-isal` (`6fb89a3c`); `4337c02e-matvec-simd-threshold`; `63bad95d-*-matvec-vs-m4ri`; `7dbe1393-*` | ISA-L [IsaL2026], M4RI |
| $\mathrm{GF}(2^8)$ | `byte-field-*` (`6c6b09b1`); `bytefield-consumer-*` (`19513245`); `gf256-shipped-*` (`4c1e441f`, `ad2a6a58`) | ISA-L, GF-Complete [GfComplete2026], M4RIE [Mfourrie2026] |
| Core route and builds | `63bad95d-native-ladder(-v2)`, `63bad95d-profile-dispatch-overhead-v2`; `bitvec-residual-bmi2-route` (`00dd43c3`); `caller-trusted-entry-check-cost` (`bfc2ceab`) | none (build or before/after arms) |

Comparators recorded as unavailable keep their records:
OpenAirInterface [OpenAirInterface2026] ([comparator survey](../c077a88b/findings.md)),
srsRAN LDPC decode and rate dematching
([comparator survey](../c077a88b/findings.md), [eda07788 findings](../eda07788/findings.md)),
and the AFF3CT flooding SIMD modes
([decoder outcome record](../6230e0b6/findings.md)).

### Criterion inventory

| Criterion | Family evidence | Artifact of this issue |
|---|---|---|
| REQ-01 | Every family receipt pins contract, protocol and addendum (P-02, P-03). Before/after evidence for each production change is in its owning issue (`d06a99ed` outcome, `8e53235b`, `00dd43c3`, `63bad95d` F-09). | Integrated families with frozen addenda and pilot-derived resolution ([route outcome](../31727c57/outcome.md), [comparator outcome](../1358d6ab/outcome.md)); a scorecard row per cell with its pins and preserved outcome. |
| REQ-07 | Raw samples per receipt. Per-domain generated tables (e.g. [LDPC tables](../../../bench_results/ed3d490e/tables.md)). | [Scorecard](../../../bench_results/1362381c/scorecard.md), [workload set](workload-set.json), [drift record](production-drift.json) and the protocol-v4 BCH cells of FI-4. |
| REQ-08 | Pre-integration holdouts: core (`63bad95d-profile-dispatch-overhead-v2`) and decoder (`ldpc-batch-decode-selector-holdout-v1`, [outcome](../7695aaee/outcome.md)). | FI-1 and FI-2 post-integration confirmations; batch-fill latency as its own column; matched and fastest tables per domain. |
| REQ-09 | Decoder profile series ([`f434ddd3` profile](../../../bench_results/f434ddd3/profile/profile.md)), 3be770d5 steady profile, 2037941f and c04dd4ac profiles, committed assembly listings. | [Profile report](../78f0aafd/report.md) and [accounting](../78f0aafd/accounting.md). |
| REQ-10 | Per-family residual-gap sections, e.g. [mid-range residual gaps](../2037941f-profile-and-optimize-mid-range-buffer-operations/mid-range-findings.md#residual-gaps) and R-01…R-05 of the [decoder record](../6230e0b6/findings.md#residual-gaps-and-their-limits). Each family freezes its `material_gap_threshold` and `search_budget`. | [Gap register](gaps.json), projected into the scorecard with the budget spent. |
| REQ-11 | Decoder receipts carry per-frame errors, FER/BER points and frame-Hoeffding/Wilson intervals (P-18). | Quality columns in the scorecard, with matched and fastest rows apart; the [determinism record](../../../bench_results/1362381c/determinism/nextest.log). |
| REQ-12 | Per-family aspirational statements, e.g. [performance evidence](../../../../docs/reference/performance-evidence.md) and the `fd9d5416` W1/W2 table. | Target outcome per primary cell (met, not met, inconclusive, unavailable), each with its comparator citation key. |

### Dispositions of pitfalls and tasks that name this issue

| Item | Disposition |
|---|---|
| surfaced pitfall 7: the P-20 bound | Folded in: every integrated family is sized by it (below). Not-confirmatory families (`1d0da41f` v3, `26465e6c` v3 popcount, `6fb89a3c` logical/transpose, `eda07788` DVB v3, `c7113c5a` baselines) enter the scorecard as `not-confirmatory` rows with that reason. |
| surfaced pitfall 34: comparator-gap outcome inversion | Folded in: the scorecard reads comparator-gap outcomes through the `INTERPRETATION` map of `make-collection-outcome.py`, extended at its source where a family's arm order differs. |
| surfaced pitfall 50: no zero-window runner mode | Resolved: the shared `benchmark-ab-runner smoke` (`@/issue/d45aff82`) answers the validation position with one untimed dispatch; each integrated family's untimed smoke record comes from it. |
| surfaced pitfall 83: AFF3CT INTRA at half LLR scale | Outside any criterion of this issue: bug `@/issue/11bfbfd9` tracks it outside the epic. The scorecard reports the arm as not quality-compatible on the recorded corpus, as recorded. |
| `12fdeb5b` NR encoder baselines | Resolved, done: consumed by identity with its drift class. |
| `bdc507a3` resumable P-11 | Resolved by protocol v4; integrated campaigns resume under it. |
| `428f2f6b` hand-copied numbers | Resolved; the scorecard copies no figure into prose. |
| `a387825e` lock leak | Resolved on main. |
| `3e59cb9a` DVB warm pass | Resolved; the repaired v3 receipt is exploratory history and enters as such. |
| `a203a23c`, `7cdc28e9` fresh-checkout verdicts | Resolved; the [re-evaluation record](../../../bench_results/1362381c/receipt-reevaluation.json) states each receipt's fresh-checkout verdict. |
| `2c487595` schema siblings | Resolved; schema siblings exist per version. |
| `706a8f93` achieved priority | Outside any criterion: rejected under the provenance freeze. The scorecard states the host facts receipts observe and nothing more. |
| `94bbe5d7` freezer prose | Outside any criterion: rejected. Each frozen confirmation states its own family description. |
| `8275a6f8` child diagnostics | Resolved on main. |

## Integrated families

Each integrated family asks one post-integration question on its own ledger and holds a pilot, a canonical freeze and a confirmation of release builds. Its frozen addendum states its cells, `material_gap_threshold`, margins and `search_budget`; the freezer refuses margins that do not exceed one plus the resolution.

| Family | Purpose | Question | Arms | Record |
|---|---|---|---|---|
| FI-1 `1362381c-core-route-integration` | `final-integration` | Does the committed core envelope hold on the integrated tree: tuned-portable no worse than, or better than, conservative-portable, and native no worse than tuned-portable? | `63bad95d` profile harness build levels, from the family's entry in `63bad95d/survey/planned-families.json` | [route outcome](../31727c57/outcome.md#core-family) |
| FI-2 `1362381c-decoder-route-integration` | `final-integration` | Does the committed coding envelope with worker-aware routing decode no slower than the conservative selectors at every worker configuration? | tuned-portable tuned-profile arm against the conservative-portable single-frame arm | [route outcome](../31727c57/outcome.md#decoder-family) |
| FI-3 `1362381c-ldpc-aff3ct-integration` | `decoder-family`, `comparator-gap` | Is the integrated production route materially slower than AFF3CT's matched flooding NMS decoder? | production gf2 arm under the committed envelope; pinned AFF3CT matched arm. The fastest-compatible comparison is consumed by identity. | [comparator outcome](../1358d6ab/outcome.md#fi-3-1362381c-ldpc-aff3ct-integration) |
| FI-4 `1362381c-bch-integration` | `consumer-family`, `comparator-gap` | Is gf2's production BCH batch encode materially slower than the strongest external arm of the `4e732b56` survey on its workloads? | production gf2 BCH encode; AFF3CT and bchlib programs as runner arms (D-6) | [comparator outcome](../1358d6ab/outcome.md#fi-4-1362381c-bch-integration) |

### P-20 bound arithmetic

`receipt::tail_supported` requires
$B\,\alpha_c/2 \ge 20$ with $B = 10\,000$ (`MIN_TAIL_DRAWS`,
`bootstrap_resamples`). `trial_ledger::attempt_budget` gives attempt $t$ the
share $\alpha_t = \alpha/[t(t+1)]$ with $\alpha = 0.05$, counting only
reservations that spend comparisons, and the Bonferroni divisor $m$ counts
every confirmatory and holdout cell the chain holds through this attempt:

$$
\alpha_c = \frac{\alpha_t}{m}, \qquad
\frac{B}{2}\cdot\frac{\alpha_t}{m} \ge 20 \iff m \le \frac{B\,\alpha_t}{40}.
$$

On a fresh ledger ($t = 1$, no prior comparisons) $\alpha_1 = 0.025$ and
$m \le 6.25$, so each FI family admits six confirmatory and holdout cells.
Pilots are exploratory and spend nothing. A second attempt on the same ledger
($t = 2$, $\alpha_2 = 0.05/6$) admits $m \le 2.08$ cumulatively, so after a
six-cell first attempt it admits no cell: each FI family has exactly one
confirmatory attempt.

## Reprofile (REQ-09)

The [profile report](../78f0aafd/report.md) states the integrated profile series and their coverage of the primary material-gap cells; the [accounting](../78f0aafd/accounting.md) carries the mechanism columns per profiled cell. A mechanism the evidence does not measure is stated as unmeasured, and no approach to a hardware limit is claimed.

## Scorecard and report generator (REQ-07, REQ-12)

- One committed generator,
  `dev/active/1a379447-zen3-cpu-performance/1362381c/survey/make-scorecard.py`,
  reads `workload-set.json`, the drift record, the gap register and the
  committed receipts it names. It writes `scorecard.json` and `scorecard.md` into
  the results directory its argument names, `dev/bench_results/1362381c`.
- It verifies each campaign's execution log through
  `dev/scripts/verify-campaign-log.py`. It reads outcomes through
  `make-collection-outcome.py` and enumerates receipts through
  `a203a23c/select-receipts.py`; both are imported, never copied.
- It writes one row per workload cell with these columns:
  - family, cell and role;
  - receipt path and digest, labelled `consumed` or `integrated`;
  - provenance from the receipt: producing-input digest, toolchain, host
    observation;
  - selected path (`selected_path` where the arm reports it);
  - per-call latency and throughput derived from the per-execution medians,
    with the workload size;
  - memory (peak resident set or lane workspace where recorded, otherwise
    `not recorded`);
  - scaling and core arm;
  - interval, corrected alpha and pairs;
  - decision and outcome as recorded, read through the comparator-gap
    inversion;
  - comparator kind (`matched-algorithm` / `fastest-quality-compatible` /
    `whole-consumer` / `kernel-isolated`);
  - batch-fill latency as its own column;
  - FER/BER sample counts and intervals for decoder cells;
  - aspirational target outcome (`met`, `not met`, `inconclusive`,
    `unavailable` with the record that says so) with the comparator's
    citation key. The target holds per workload over every other project's
    measured arm; a published ruling on a receipt makes its cell `inconclusive`;
    `pairwise` keeps the cell's own comparison.
- No aggregate row exists. Losses stay per cell. No project is named winner.
- The gap register `gaps.json` (input, committed) maps each primary cell whose
  outcome records a material gap, or whose estimate exceeds its family's frozen
  threshold with its outcome kept as evaluated, to one disposition: a measured-limit pointer,
  a preserved-experiment pointer, or a tracked issue with its next falsifiable
  experiment and the family's spent search budget. The generator refuses to
  write when a material-gap cell has no entry. That refusal belongs to the
  scorecard's output semantics; it adds no checker version.
- Reproducibility: two consecutive runs followed by `git diff --exit-code`.
  The report `1362381c/findings.md` (Research) states conclusions with
  pointers only.
- `docs/reference/performance-evidence.md` carries claim entries only for
  confirmatory integrated cells, in its existing format.

## Evidence notes

- A consumed row is historical, labelled with its drift class; the scorecard
  never presents a `code-differs` row as a measurement of the current tree.
- Ledger paths of `12fdeb5b` and `eda07788` addenda name `dev/active/<id>/`,
  while the files live under this epic's record directory. The scorecard reads
  each ledger through the receipt's pinned prefix (P-22), never through the
  addendum path.
- Remaining gaps and their next experiments are in the
  [gap register](gaps.json) and its tracked issues; the
  [report](findings.md) states the conclusions.
