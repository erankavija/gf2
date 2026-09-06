# Zen 3 benchmark protocol

Protocol `zen3-benchmark-protocol` version 1. This document is the executable
shared protocol required by the
[measurement contract](../1a379447-zen3-cpu-performance/measurement-contract.md)
of epic `1a379447`. Every rule below is enforced by a check the acceptance tool
performs or by a field a receipt must carry; the rule identifiers `P-NN` are the
finding codes the tool emits. Family and cell settings that the contract leaves
to families are declared in addenda conforming to
[`addendum.schema.json`](addendum.schema.json); the shared numeric settings are
frozen here. The tooling lives in the `tuning-campaign-support` workspace crate
(`dev/tools/tuning-campaign-support`); [design.md](design.md) records why that
crate is the canonical home.

## Identity and versioning

- The protocol identity is the pair `zen3-benchmark-protocol` version 1, the
  constants `PROTOCOL_ID` and `PROTOCOL_VERSION` in `src/protocol.rs`.
- The content identity of this document, the addendum schema, the measurement
  contract and every addendum is an `ArtifactPin`: its original path, its
  receipt-local immutable snapshot path and the SHA-256 of the exact bytes.
  The runner publishes each snapshot atomically before opening the execution
  log. The acceptance tool reads only those portable receipt-local bytes and
  rejects a missing snapshot or digest mismatch (P-02, P-03). Git revisions,
  commit ancestry and whole-tree state are optional navigation metadata and do
  not decide acceptance or resume compatibility.
- An amendment creates version 2 with a new document; version 1 data,
  contradictions and failed trials stay committed. Confirmation under a new
  version starts from fresh samples. A version bump never reclassifies a failed
  confirmatory trial as a pilot (P-22, P-13).
- The addendum schema identity is `zen3-benchmark-addendum-v1`; receipts carry
  `zen3-benchmark-receipt-v1`; acceptance summaries carry
  `zen3-benchmark-acceptance-v1`; runner plans carry `zen3-benchmark-plan-v1`.

## Contract clause coverage

| Contract clause | Protocol section | Checks |
|---|---|---|
| Protocol identity and freezing: one versioned protocol, independently versioned addenda, receipts pin all identities | Identity and versioning; Addenda | P-01, P-02, P-03 |
| Freezing: exploratory pilots labelled and excluded; all numeric settings committed before the first confirmatory trial; amendments retain data | Roles and freezing; Frozen shared settings | P-03, P-04, P-13, P-15, P-22 |
| Holdout reserved for calibrated selectors and final integration | Roles and freezing | P-03 (addendum validation) |
| Statistics: paired/interleaved/randomized A/B, resampling unit, ratio of medians, bootstrap interval, paired handling | Sampling design; Estimator and interval | P-15, P-16, P-20 |
| Multiple comparisons within predeclared families; selection across candidate trials | Families and selection | P-20, family summary |
| Non-regression as an equivalence margin | Decision rule | P-20 |
| Worthwhile-effect thresholds and complexity budgets by family with rationale; no blanket 15% or 1.5x | Addenda | P-03 |
| Record negative, inconclusive, unavailable and not-material cells; retain the established implementation | Outcomes; Completeness | P-14, P-21 |
| Host and execution: release builds finished before timing, exclusive lock, runtime-observed topology, affinity, workers, SMT, governor, capabilities; 1/6/12/24 arms with runtime CPU IDs; inapplicable arms carry a reason | Host and execution | P-05 to P-09, P-14, P-17 |
| Bounded resumable runs, execution log announced before the first bounded run, checkpoints not repeated | Execution log and checkpoints | P-10, P-11, P-12 |
| Comparable operations and costs: setup, conversion, batch fill, dispatch in whole-consumer cells; isolated kernels separate | Cell schema | P-17 (conversion costs) |
| Decoder comparisons: matched-algorithm and fastest quality-compatible arms, quality tolerances, BER/FER intervals, stopping contracts | Decoder cells | P-18, P-19 |
| Receipts: raw samples, exact commands, seeds, identities, provenance, interpretation and acceptance summary under `dev/bench_results` | Receipt layout | P-01 to P-12, summary files |
| Production change needs a fresh pinned before/after measurement | Roles and freezing | P-05 (source identity) |

## Roles and freezing

Each cell in an addendum carries a `role`:

- `exploratory` cells are pilots. Their data informs workload selection,
  budgets and thresholds and never yields a pass; the tool reports them as
  `pilot` (P-13). Pilots use between `pilot_min_pairs` and `pilot_max_pairs`
  pairs.
- `confirmatory` cells decide adoption on exactly `confirmatory_pairs` fresh
  pairs.
- `holdout` cells confirm calibrated selectors and final integration on fresh
  samples that took no part in selection; addenda whose purpose is
  `selector-calibration` or `final-integration` must declare them.

A confirmatory or holdout cell is confirmatory only when every required setting
it depends on is resolved. A `null` in a nullable addendum field is an
unresolved setting; the tool lists the unresolved names and records the cell as
`not-confirmatory` instead of defaulting a value (P-13, P-20). The required
settings are: `effect.measurement_resolution`, its content-pinned
`effect.resolution_evidence`, the objective's margins (`worthwhile_speedup` and `equivalence_margin` for
improvement and non-regression cells, `material_gap_threshold` and
`equivalence_margin` for comparator-gap cells) and
`complexity_budget.max_added_source_lines` for adoptable cells.

The receipt's addendum pin and first `campaign-start` record are the freeze
proof. The pin names immutable receipt-local bytes, and the runner records the
complete pin before any `cell-start`. `frozen.frozen_utc` is descriptive
metadata. When `measurement_resolution` is set, `resolution_evidence` names a
repository-relative pilot `receipt.json` and its SHA-256. Before opening the
campaign log, the runner captures that receipt at
`inputs/resolution-evidence/receipt.json`; the acceptance tool verifies the
snapshot's digest and `pilot` label and rejects missing, self-referential or
non-pilot evidence (P-03). Thus the pilot evidence and all settings derived
from it are frozen before confirmation begins. A run with a timing override
records `settings_deviation: true`; the tool then reports every cell as
`not-confirmatory` (P-04).

Failed confirmations stay. The family ledger `family_wise.prior_trials` lists
every earlier confirmatory receipt with its digest and recorded outcome, and
`prior_confirmatory_trials` equals the ledger length; the tool verifies each
listed receipt exists with that digest (P-22). Re-freezing an addendum after a
failed confirmation therefore grows the family and widens every later interval.

## Sampling design

The resampling unit is the **paired execution**: one fresh baseline child and
one fresh candidate child, launched adjacently by the runner in a
seed-determined order. Each child performs `windows_per_execution` timing
windows of `window_target_ms` target length after its own calibration
(`timing::execution_windows_configured`); its per-execution value is the median
nanoseconds per call over its windows. Fresh processes remove carried-over
allocator, cache and frequency state between arms; adjacency and randomized
order cancel slow drift within pairs [HoeflerBelli2015] [KaliberaJones2013].

Pair orders are counterbalanced: every block of two consecutive pairs contains
one baseline-first and one candidate-first execution in an order drawn from the
cell seed (`abtest::pair_orders`). The cell seed is
`abtest::bootstrap_seed(campaign_seed, cell_key)`; it drives both the order and
the bootstrap so the acceptance tool reproduces both (P-16).

Cache-state policy is declared per cell and applied by the arm, which reports
`cache_state_applied`; a mismatch invalidates the cell (P-17):

- `cold`: no warm-up pass before the first window; fresh buffers per process.
- `warm`: one untimed pass over the working set before calibration.
- `streaming`: the working set rotates through the eight fixture banks of
  `timing::FIXTURE_BANKS`, so successive calls do not reuse cache-resident data.

Outlier policy: no sample is removed. A window slower than
`flagged_window_factor` times its cell's median is flagged and counted
(`abtest::flagged_windows`); a confirmatory cell whose flagged fraction exceeds
`max_flagged_fraction` is `unstable` and must be re-run. Medians and
whole-pair resampling make the estimator robust to the retained outliers.

## Estimator and interval

The estimator is the **speedup of medians**

$$
\hat{s} = \frac{\operatorname{median}_i\, b_i}{\operatorname{median}_i\, c_i},
$$

where $b_i$ and $c_i$ are the baseline and candidate per-execution values of
pair $i$. Values above 1 favour the candidate. The interval is the percentile
bootstrap [Efron1979] [EfronTibshirani1993]: draw $n$ pairs with replacement
$B$ times, recompute $\hat{s}$ on each draw, sort the replicates and take the
nearest-rank quantiles at $\alpha_c/2$ and $1 - \alpha_c/2$, where $\alpha_c$
is the per-comparison error rate below. Resampling whole pairs keeps the
pairing intact. The generator is xoshiro256** seeded through SplitMix64
[BlackmanVigna2021] [Steele2014], implemented in `abtest.rs` without an external
dependency, so the interval is bit-reproducible from the raw samples and the
seed (P-20).

## Families and selection

The family is the set of confirmatory and holdout cells in one addendum plus
its prior confirmatory trials; its size $m$ is `family_comparisons()`. The
family-wise level is `family_alpha`, controlled by Bonferroni [Dunn1961]: each
comparison uses $\alpha_c = \alpha / m$, so each interval is computed at
confidence $1 - \alpha / m$. Bonferroni is chosen because it is valid under
arbitrary dependence between cells and applies directly to confidence
intervals; its conservatism is acceptable at the family sizes this epic
declares. Selection across candidate trials is accounted for twice: pilots
never enter confirmation (fresh samples), and every confirmatory attempt ever
spent in the family counts in $m$ through the ledger (P-22). Each candidate
identity gets `max_confirmatory_attempts_per_candidate` confirmatory attempt
per protocol version; a further attempt needs a new candidate identity or a
new protocol version and still counts in the family.

## Decision rule

With margins $\theta_{\text{imp}} > 1$ (worthwhile speedup or material-gap
threshold) and $\theta_{\text{eq}} \ge 1$ (equivalence margin) and interval
$[\ell, u]$ (`abtest::decide`):

- `improved` when $\ell \ge \theta_{\text{imp}}$;
- `not-worse` when $\ell \ge 1/\theta_{\text{eq}}$ and not improved: the
  candidate is at most a factor $\theta_{\text{eq}}$ slower at the family
  confidence, an equivalence statement in the sense of two one-sided tests
  [Schuirmann1987], never the absence of a significant difference;
- `regressed` when $u < 1/\theta_{\text{eq}}$;
- `inconclusive` otherwise: the interval spans a margin and the cell needs more
  evidence under a new trial.

The cell outcome combines the decision with the cell objective: an improvement
cell passes on `improved` and records `not-material` on `not-worse`; a
non-regression cell passes on `improved` or `not-worse`; a comparator-gap cell
uses the material-gap threshold as $\theta_{\text{imp}}$ and its `improved`
means a material gap in the comparator's favour that needs attribution.
`regressed` is `fail`, `inconclusive` stays `inconclusive`. There is no blanket
15% or 1.5x threshold: every family declares its own margins with a rationale
tied to consumer benefit and to its pilot-measured resolution, and the schema
rejects a worthwhile speedup that lies inside that resolution (P-03).

## Frozen shared settings

These values are `SHARED_SETTINGS` in `src/protocol.rs`; the test
`protocol_document_pins_the_frozen_shared_settings` keeps this table equal to
the code.

| Setting | Value | Justification |
|---|---|---|
| `family_alpha` | `0.05` | Two-sided family-wise error rate; the conventional level, applied per family rather than per cell. |
| `bootstrap_resamples` | `10000` | Percentile intervals need at least about a thousand replicates [EfronTibshirani1993]; ten thousand keeps the Monte-Carlo error of the interval endpoints well below the declared resolutions at negligible cost. |
| `confirmatory_pairs` | `24` | Four counterbalanced blocks of six pairs; enough pairs for a non-degenerate percentile interval of a median-based statistic while bounding a confirmatory cell at 24 seconds of timed windows by construction (24 pairs, two arms, five windows of 100 ms) before calibration and process start-up. Fixed in advance so confirmation is never data-adaptive. |
| `pilot_min_pairs` | `6` | One counterbalanced block; the smallest pilot that still estimates a resolution. |
| `pilot_max_pairs` | `24` | Pilots never exceed a confirmatory sample so they cannot masquerade as confirmation. |
| `windows_per_execution` | `5` | The retained calibration protocol's window count (`timing::WINDOWS`); five windows give a per-execution median robust to one disturbed window. |
| `window_target_ms` | `100` | Long enough that timer and loop overhead are negligible for nanosecond kernels; short enough that whole-consumer executions stay bounded. |
| `flagged_window_factor` | `2` | A window at least twice the median is a disturbance, not variance of the workload. |
| `max_flagged_fraction` | `0.1` | More than a tenth of flagged windows means the host was not quiet; the cell is re-run rather than trimmed. |
| `max_pilot_trials_per_cell` | `8` | Bounds exploratory search per cell; a family declares a value up to this cap. |
| `max_confirmatory_attempts_per_candidate` | `1` | One confirmatory attempt per candidate identity and protocol version; the family ledger counts every attempt. |
| `child_timeout_seconds` | `120` | The retained campaign child timeout (`campaign::CHILD_TIMEOUT_SECONDS`). |
| `quality_confidence` | `0.95` | BER/FER intervals use the Wilson score interval at 95% [Wilson1927] [BrownCaiDasGupta2001]. |

## Addenda

An addendum freezes one family. Its schema is
[`addendum.schema.json`](addendum.schema.json); `protocol::FamilyAddendum`
decodes it with unknown fields rejected and validates the semantics the schema
cannot express (P-03). It declares:

- family identity, owning issue, purpose (`kernel-family`, `consumer-family`,
  `decoder-family`, `selector-calibration`, `final-integration`) and
  description;
- the effect rule: worthwhile speedup with rationale, pilot-measured
  measurement resolution with the content-pinned pilot receipt path and digest that
  observed it, equivalence margin with rationale, material-gap threshold with
  rationale;
- the complexity budget: new unsafe kernels allowed, added source lines
  allowed, maintenance rationale;
- the family-wise declaration: alpha (must equal the frozen value), prior
  confirmatory trials and their ledger;
- the search budget and the holdout declaration;
- the cells: identifier, objective (`improvement`, `non-regression`,
  `comparator-gap`), role, workload identity and sizes, metric kind
  (`kernel-isolated` or `whole-consumer`), scaling (`single-core-latency`,
  `sustained-throughput`, `multicore-throughput`), core arm, declared workers
  and nested-pool policy, cache state, arm build identities
  (`conservative-portable`, `tuned-portable`, `native`, `external`), whether
  conversion and setup costs are included (mandatory for whole-consumer cells),
  and the decoder fields below when applicable.

Addenda cannot override shared settings: the schema pins `alpha`,
`max_confirmatory_attempts_per_candidate` and the quality confidence to their
frozen values and caps the pilot budget.

### Decoder cells

A decoder cell declares its arm kind (`matched-algorithm` or
`fastest-quality-compatible`), the code identity with dimensions and
parity-check digest, the input identity (LLR source and digest, frame count,
seed, codeword source `all-zero`, `random` or `both`, SNR), precision,
schedule, normalization kind and factor, iteration cap, stopping contract
(`syndrome`, `crc` with polynomial, `fixed`, `none`), batching with batch-fill
accounting, the predeclared quality tolerance and any rate matching. Receipts
record for both arms the frames, frame and bit errors, FER and BER with Wilson
intervals, iteration distribution, memory and latency, and the settings the arm
actually used. Matched-algorithm arms must report settings identical to the
declaration (P-19); fastest-quality-compatible arms are `quality-incompatible`
when the candidate's FER upper bound exceeds `fer_ratio_max` times the
baseline's FER upper bound (P-19). Iteration counts under different stopping
contracts are reported, never compared as if equivalent.

## Host and execution

- Builds finish before timed work. Arms are release executables whose bytes
  are digested at run time; the receipt records each arm's build identity,
  executable digest, arguments, environment, RUSTFLAGS and tuning profile
  (P-09). Native, tuned-portable and conservative-portable builds are separate
  arms, never mixed in one arm.
- Timed work runs under `dev/scripts/ccx1-bench-flock.sh`; the runner refuses
  to measure unless it inherits the held lock descriptor and an independent
  `flock` attempt conflicts (`host::inherited_lock`), and journals the lock
  evidence (P-07). No Cargo command runs under the lock.
- The runner observes at run time the CPU model and flags, kernel, per-CPU
  governors, SMT state, its own affinity mask, the sysfs topology (core,
  package, thread siblings, last-level cache sharing), load average and
  available memory (`host::HostObservation`), and journals the observation
  (P-06). Worker counts are reported by the runner and by every arm
  (`workers_observed`) and must equal the declaration (P-08, P-17).
- Core arms: `single-core`, `physical-cores-6`, `physical-cores-12` and
  `logical-cpus-24`. `host::resolve_core_arm` resolves actual CPU IDs inside
  the observed mask and topology, one logical CPU per physical core for the
  physical arms, preferring one last-level-cache domain for the six-core arm.
  An arm the mask cannot satisfy is recorded `unavailable` with the resolver's
  reason and no samples (P-14). Every arm child reports the affinity it
  observed, which must equal the resolved set (P-17).
- Single-core latency, sustained throughput and multicore throughput are
  separate cells with separate metric kinds; kernel-isolated and
  whole-consumer measurements never share a cell.

## Execution log and checkpoints

The runner opens the canonical append-only execution log
(`journal::ExecutionLog`) and prints
`GF2_CAMPAIGN_EXECUTION_LOG=<path>` before its first bounded run, then journals
an `execution-log-announced` record; the tool requires that record before the
first `cell-start` (P-10). Each completed cell is accepted into the immutable
checkpoint store (`journal::CheckpointStore`) under the campaign's resume
identity; a resumed session journals an omission for every completed cell and
never starts it again, so the log holds exactly one `cell-start` and one
`cell-complete` per cell across all sessions (P-11). The receipt pins the log
and checkpoint manifest digests and every cell's checkpoint unit digest
(P-12). A campaign is finalized only from a `complete` terminal record;
`paused`, `budget-exhausted` and `failed` sessions resume under the same stage
and identity.

## Outcomes and completeness

Every declared cell appears in the receipt exactly once, as measured or as
unavailable with a reason (P-21). Outcomes are `pass`, `not-material`, `fail`,
`inconclusive`, `unstable`, `unavailable`, `pilot`, `not-confirmatory`,
`quality-incompatible` and `invalid`; all are retained in the summary and the
Markdown table. A receipt is `accepted` when no error-severity finding exists
and `rejected` otherwise; `qualifies` is true only when every non-exploratory
cell passed. A rejected or non-qualifying receipt is committed like any other.

## Receipt layout

Each bounded campaign writes one directory under the owning issue's
`dev/bench_results/<issue>/` area, for example
`dev/bench_results/f547c394/2026-09-07-f547c394-confirmation/`:

| File | Content |
|---|---|
| `plan.json` | The runner plan: arms, cells, opaque cases, seed, lock path, wrapper, budget. |
| `receipt.json` | `zen3-benchmark-receipt-v1`: identities, source, toolchain, host observation, lock evidence, worker report, log and checkpoint digests, arms, and every cell with its raw windows, per-execution values, observed CPUs and the runner's claim. |
| `execution.log` | The append-only journal of every session. |
| `checkpoints/` | The immutable manifest and accepted units. |
| `acceptance-summary.json` | `zen3-benchmark-acceptance-v1` from `benchmark-acceptance`. |
| `acceptance-summary.md` | Markdown rendered from the summary alone. |

The launcher script beside the directory records the exact commands. Receipt
labels are `smoke`, `pilot`, `confirmation` and `holdout`; the protocol smoke
uses a pilot receipt to set its resolution followed by a separate confirmatory
receipt. Neither is a performance result about gf2.

## Acceptance rules

| Rule | Check |
|---|---|
| P-01 | Receipt schema is `zen3-benchmark-receipt-v1`. |
| P-02 | Protocol, contract and addendum-schema pins name the canonical source paths, literal receipt-local snapshot paths and 64-hex digests; missing snapshots and digest mismatches reject. |
| P-03 | The addendum matches its receipt-local snapshot, its exact pin appears in campaign-start before any cell, it validates against the schema and semantic rules, and resolution evidence is a distinct digest-matched pilot receipt snapshot. |
| P-04 | Receipt settings equal the frozen shared settings unless a deviation is declared; a deviation makes every cell non-confirmatory. |
| P-05 | The manifest and every declared producing/build input match their receipt-local content snapshot, and the toolchain is recorded. Source-control metadata is informational. |
| P-06 | The host observation carries model, flags, kernel, governors, affinity and topology. |
| P-07 | Lock evidence carries an absolute lock path, the holder PID, the observation and the wrapper. |
| P-08 | The worker report carries the runner thread count. |
| P-09 | Every arm named by a cell is described with an executable digest. |
| P-10 | The execution log matches its digest, replays under the journal rules, opens with `campaign-start`, and was announced before the first cell. |
| P-11 | Session count and resumed flag match the log, the terminal state is `complete`, and no cell starts or completes more than once. |
| P-12 | The checkpoint manifest and every measured cell's unit match their digests and keys. |
| P-13 | Cells are unique, declared, and match the declared role and core arm. |
| P-14 | Measured cells resolved CPUs; unavailable cells carry a reason and no samples. |
| P-15 | Pair counts follow the role, executions carry the frozen window count with positive windows, and per-execution values are the medians of their windows. |
| P-16 | Pair orders equal the seed-determined counterbalanced sequence. |
| P-17 | Observed CPUs, cache state, worker counts and conversion-cost reporting match the declaration. |
| P-18 | Decoder cells carry BER/FER evidence whose Wilson intervals recompute. |
| P-19 | Matched-algorithm arms used the declared settings; fastest-quality-compatible arms stay within the predeclared FER tolerance. |
| P-20 | The bootstrap interval and decision recompute from the raw pairs at the family confidence; a contradicting runner claim rejects the cell. |
| P-21 | Every declared cell is present in the receipt. |
| P-22 | Every prior trial in the family ledger exists with its recorded digest. |
