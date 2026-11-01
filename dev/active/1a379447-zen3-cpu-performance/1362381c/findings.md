# Integrated competitive results and remaining limits

> **Diátaxis Type:** Research

Report of `@/issue/1362381c`. The [scorecard](../../../bench_results/1362381c/scorecard.md) and its [data](../../../bench_results/1362381c/scorecard.json) hold every figure; `survey/make-scorecard.py` writes them from the [workload set](workload-set.json), the [drift record](production-drift.json), the [gap register](gaps.json) and the committed receipts. Each campaign is verified from its execution log. This report states conclusions and points at rows.

## Answer

The scorecard reports each workload cell on its own row, with no aggregate row, and names no project as winner. On the integrated tree, losses and leads coexist by cell. gf2's production LDPC decoder is ahead of AFF3CT [Cassagne2019] at one worker with eight frames and behind it with one frame and at twelve physical cores. gf2's BCH encoder is ahead of AFF3CT on the mother code and behind bchlib [Bchlib2026] on the DVB-T2 outer codes. The core and decoder tuned routes each record `pass` in every confirmation cell. The aspirational fastest-open-source target holds per workload, over every other project's measured arm on that workload. It is met on some workloads and missed on others; the *Target outcome* column states each cell's outcome and its own pairwise comparison, so a cell whose workload is lost to another project reads `not met` even where gf2 leads its own comparator.

## Workload set and consumed evidence (REQ-07)

The declaration selects a campaign per registry family by the rule it states, and FI-1 to FI-4 resolve to their confirmations. Every confirmation cell's identity, workload, arm builds and core arm equal those its pilot addendum or holdout declaration froze before the pilot.

Two registry entries emit no cell, and the scorecard's *Registry entries with no cell* section gives each reason. `fd9d5416` is context only: that receipt is Criterion-based, outside protocol v4, and consumed as history under design decision D-2. The wildcard `1362381c-*-integration` matches no family, because the D-3 ruling names no integrated family beyond the four. The registry keeps both entries.

Consumed rows are evidence of the code their receipt pins, labelled with the drift class; they never measure the current tree. Under the invoker's D-3 ruling, only the BCH, core route, decoder route and AFF3CT decoder cells are remeasured. The FI-4 drift class `code-differs` rests on files `added` beside the measured packages, with the measured files `identical` (the row's `class_counts` in the drift record). The historical `fd9d5416` receipt and the protocol-v4 BCH confirmations of `8e53235b`, `6fb89a3c`, `04b85d10` and `1d4fd63d` keep their evidence.

Each receipt pins the measurement contract, frozen protocol and addendum (REQ-01), and the margin rulings stay in their derivations ([FI-1 and FI-2](../31727c57/outcome.md), [FI-3 and FI-4](../1358d6ab/outcome.md)). Production before/after evidence stays with its owning issue.

## Confirmation and comparison (REQ-08)

FI-1 measures one core because its kernels run on one thread, a reason each cell declares under design decision D-5. FI-2 covers one, six and twelve physical cores and twenty-four logical CPUs; pilot configurations outside the confirmation stay exploratory ([configuration table](../31727c57/outcome.md#pilot-configurations-and-confirmation)). Per-worker ratios include a route change: `selected_path` records the lane route at one worker with eight frames and the single-frame route at several workers, so they are not thread-scaling figures.

Batch fill is its own column. Its intervals are descriptive, and it measures decoder preparation under the shared stage probe. The tuned-arm values come from the descriptive records of [`31727c57`](../31727c57/outcome.md#tuned-arm-batch-fill), reused by import; the scorecard section *Tuned-arm batch fill of FI-2 pilot-only configurations* carries the six pilot-only cells. The confirmation receipts record no tuned-arm batch fill.

Matched-algorithm and fastest quality-compatible rows sit in separate tables. FI-3 is matched-only: the [fastest-compatible pilot](../6230e0b6/findings.md#lane-route-against-aff3ct-fastest-compatible) admits no AFF3CT mode as quality-compatible on the recorded corpus, so that comparison is consumed by identity. "Matched" does not mean numerically identical; the [investigation](aff3ct-gap-investigation.md#1-comparison-validity) lists the differences from AFF3CT.

## Bottlenecks (REQ-09)

The [accounting](../78f0aafd/accounting.md) and [profile report](../78f0aafd/report.md) carry the mechanism columns per profiled cell and mark what is unmeasured. No approach to a hardware limit is claimed: no host bandwidth or instruction and dependency bound is measured.

## Remaining gaps (REQ-10)

The [gap register](gaps.json) gives one disposition per primary cell that records a material gap or whose estimate exceeds its family's frozen threshold. Cells of the second kind keep their outcome as evaluated and are not confirmed gaps; the scorecard's *Gap register* section states each cell's basis, its disposition and the family's search budget, declared and spent. A profile citation attributes a gap and is not a disposition. Finite searches establish no absolute maximum.

- **FI-3.** All four losing cells take the single-frame route, so lane stall does not explain them. They are tracked by `@/issue/55c3e433`. The next NR experiment is pilot `@/issue/1d787ed3`. That pilot covers NR only, and DVB-T2 has no QC partition, so the next DVB-T2 experiment is `@/issue/a72f2b3f`, which compares check-update variants on both codes. The one-frame cells also cite preserved experiments `07ca8585` and `f63a2464`. The QC prototype's timings are descriptive and carry no valid verdict.
- **FI-4.** The bchlib gap is tracked by `@/issue/040bc2b8`. The profile series `78f0aafd/bch-profile/` attributes the gf2 arm's samples to per-bit codeword assembly, and bchlib steps 32 input bits through four remainder tables. The receipt's arms also differ in build: conservative-portable gf2 against bchlib at `-O3 -march=native`.
- **GF(2^8).** `@/issue/c9cb0dd2` tracks the cells. Their `selected_path` values and producing snapshots show routes without the product-table lane, whose before/after receipts ([axpy](../ad2a6a58/outcome.md), [dense product](../4c1e441f/outcome.md)) have no external arm. No current measurement pairs the shipped lane with ISA-L [IsaL2026] or M4RIE [Mfourrie2026]. The pairwise controls against GF-Complete [GfComplete2026] are consumed rows.
- **NR encode and long polynomial product.** `@/issue/be6d6f45` tracks the srsRAN [Srsran2026] and AFF3CT encode cells; `@/issue/befccab3` tracks the gf2x [GfTwoX2026] cells above nine words, where production has no carry-less kernel. [Its survey](../c7113c5a/findings.md#limits-and-follow-up) records that a gf2x dependency is not recommended, because of its licence.
- **Preserved and limited cells.** The steady-state, popcount [Libpopcnt2026] [Mula2018], public long-product API, wide-field and transpose [AlbrechtBard2026] cells point at preserved experiments, and the earlier lane matched cells at the measured limits R-01 and R-03 of the [decoder record](../6230e0b6/findings.md#residual-gaps-and-their-limits). Each register entry states which.

## Decoder quality and determinism (REQ-11)

Decoder rows publish frames, frame errors, bits, bit errors and their intervals per arm. A row whose arms state different schedule or precision settings names them, and rows declaring a fastest-compatible arm stay in their own tables. Timed calls reuse the first frames of the corpus while quality statistics cover all of it, so equal iteration counts within timed frames are unestablished; `@/issue/1d787ed3` records them.

The `determinism record` runs the batching, worker-count, fallback and resume suites through `./scripts/cargo-budget.sh --test` ([command](../../../bench_results/1362381c/determinism/run.sh), [identity](../../../bench_results/1362381c/determinism/run-identity.txt)), summarized under *Verification records* with its passed and skipped counts per suite. Skipped tests are outside this evidence. In the [fresh-checkout record](../../../bench_results/1362381c/receipt-reevaluation.json) every receipt the scorecard consumes reproduces its verdict; the record's `result.differs` lists the receipts outside the workload set that do not, each with its recorded cause.

## Target outcomes (REQ-12)

Every primary cell states `met`, `not met`, `inconclusive` or `unavailable`, with its comparator's citation key and the rule above the tables. Not-confirmatory cells read `inconclusive`, with their recorded decision kept descriptive. A published ruling on a receipt also makes its cells `inconclusive`, with the receipt's outcome kept as evaluated: [C-09](../f63a2464/corrections.md#c-09-three-confirmatory-campaigns-against-a-cap-of-one) gives family QC no valid confirmatory verdict, and the [`6fb89a3c`](../6fb89a3c/findings.md#4-results) and [`eda07788`](../eda07788/findings.md#warm-state-defect-and-re-measurement) confirmations are withdrawn for their warm-pass defect. `gaps.json` lists these rulings, and the affected rows name them. The target holds per workload: a loss to another project's measured arm on the same operation, size, consumer scope, core arm, gf2 build and gf2 path makes the cell `not met`, and an undecided comparison there makes a `met` cell `inconclusive`. Comparisons with no equivalent measurement are listed under *Unavailable comparisons*, including OpenAirInterface [OpenAirInterface2026] and srsRAN decode. No other metric stands in for one. External families also cover DVB-T2 interleaving through xdsopl [Xdsopl2026] and transpose through Bitshuffle [Bitshuffle2026].

## Claims

[Performance evidence](../../../../docs/reference/performance-evidence.md) carries claim entries for the confirmatory integrated cells; `survey/test_make_scorecard.py` checks their figures against the scorecard. Holdout cells stay in the scorecard and in the [route outcome](../31727c57/outcome.md).
