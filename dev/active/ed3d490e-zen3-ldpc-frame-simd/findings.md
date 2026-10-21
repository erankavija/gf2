# Inter-frame LDPC decoder: outcomes and limits

> **Diátaxis Type:** Research

Outcome record of story `ed3d490e` (jit:6230e0b6). The
[measurement contract](../1a379447-zen3-cpu-performance/measurement-contract.md)
and the [frozen protocol](../1a379447-zen3-cpu-performance/f547c394/protocol.md)
with its [version-4 amendment](../1a379447-zen3-cpu-performance/f547c394/amendment-v4.md)
govern every timed cell; the [story plan](plan.md) is the design this record
is judged against.

This record states conclusions and points to the evidence that carries them;
it copies no measured or derived figure. The figures live in the generated
[outcome tables](../../bench_results/ed3d490e/tables.md) ("the tables"), written
by [`survey/make-outcome-tables.py`](survey/make-outcome-tables.py) from the
committed receipts; in the
[profile summary](../../bench_results/f434ddd3/profile/profile.md) of the
profile series; in the
[timing tables](../../bench_results/f63a2464/timing-tables.md) of the candidate
evaluation `f63a2464`; and in the receipts those files name. A pointer names a
table section as *part › section* and a row by its family and cell. Outcome
labels (`pass`, `fail`, `regressed`, `not-material`, `pilot`) are quoted as
the evaluator records them.

## What the story delivers

`LdpcBatchDecoder` of `gf2-coding` decodes min-sum-family frames with one frame
per lane of an eight-lane f32 register, over the portable and AVX2 lane kernels
of `gf2-kernels-simd`. It implements the batch soft-decoder trait of
`gf2-coding`, which `PooledBatchDecoder` and the `gf2-sim` decode stages
consume. The float flooding contract is unchanged: the
[numerical contract](../../../docs/reference/ldpc-decoder-numerical-contract.md),
section *Batch clauses*, states per-frame equality with the single-frame
`LdpcDecoder` in posterior bit patterns, hard decisions, iteration count and
flags. Each wave takes the lane route or the single-frame route by the decode
selectors of the coding tuning section
([`route.rs`](../../../crates/gf2-coding/src/ldpc/route.rs)).

## Committed decode selectors

The committed selectors are the decode family of the coding owner envelope
[`gf2-0a77ecfc-20261013t011147z-815998.json`](../../../crates/gf2-coding/data/tuning-profiles/gf2-0a77ecfc-20261013t011147z-815998.json):

- `decode.lane_min_frames` and `decode.lane_max_edges` are stated at the
  values the calibration campaign selects (tables, *Decode selector
  calibration*, rows of both fields; receipt
  [`gf2-0a77ecfc-20261013t011147z-815998.md`](../../benchmarks/tuning_profiles/gf2-0a77ecfc-20261013t011147z-815998.md),
  section *Argmin and threshold decisions*).
- `decode.granules_per_task` is omitted, so it resolves to its conservative
  value ([campaign findings](calibration/campaign-findings.md), F-04).
- The envelope states no encode selector.

The holdout confirmation retains these selectors under its frozen decision
rule ([holdout collection](../1a379447-zen3-cpu-performance/7695aaee/collection.md),
section *Decision*).

`gf2-coding` installs no envelope by itself. A process that installs none
resolves the conservative family of
[`tuning.rs`](../../../crates/gf2-coding/src/tuning.rs) (`LANE_MIN_FRAMES`,
`LANE_MAX_EDGES`, `GRANULES_PER_TASK`), which closes the lane route, so every
wave decodes frame by frame. A process that installs the committed envelope
opens the lane route under the two stated bounds.
[`tuning_profile_committed.rs`](../../../crates/gf2-coding/tests/tuning_profile_committed.rs)
installs the envelope in a fresh process and checks routes and results, and
[`ldpc_decode_dispatch.rs`](../../../crates/gf2-coding/tests/ldpc_decode_dispatch.rs)
checks the routes with no envelope installed.

The committed `lane_max_edges` equals the edge count of the DVB-T2 normal
rate-1/2 code, the largest code of the calibration grid
([premeasurement protocol](calibration/premeasurement-protocol.md), §2 codes
table), and its decision records `grid_boundary_limited` true.

## Outcomes by comparison

### Lane route against the single-frame route

Tables, *Before-after results of the lane route › Outcomes*. The baseline arm
of every cell is the single-frame route and the candidate the lane route, both
under selectors that open the lane route to every wave size and edge count.

- **One worker** (`ldpc-batch-single-worker-v1`, confirmation). The lane route
  meets the improvement margin at a full wave on both codes and passes
  non-regression with an incomplete final wave (cells `*-lane-w1-b11`). At a
  batch of one frame, DVB-T2 records `not-worse` and `pass` and NR records
  `regressed` and `fail`. The receipt does not qualify
  ([collection](../1a379447-zen3-cpu-performance/32bfe9f8/collection.md)).
- **Several workers** (`ldpc-batch-multicore-v1`, confirmation). The lane
  route records `improved` and `pass` on the NR code at six and twelve
  physical cores. It records `regressed` and `fail` on DVB-T2 at six and twelve
  physical cores and twenty-four logical CPUs, and on NR at twenty-four logical
  CPUs. The receipt does not qualify
  ([collection](../1a379447-zen3-cpu-performance/a03daf3f/collection.md)).
- **Batch sizes** (`ldpc-batch-size-sweep-v1`, pilot). Every cell is
  exploratory and decides nothing; the per-frame times by batch size are in
  *Before-after results of the lane route › Batch-size sweep*.

### Lane route against AFF3CT, matched algorithm

Tables, *AFF3CT matched-algorithm results › Outcomes*. The baseline arm is the
gf2 lane route and the candidate AFF3CT's flooding f32 normalized min-sum
decoder [Cassagne2019], so `improved` states a material gap in AFF3CT's favour
and `regressed` states AFF3CT slower than the lane route beyond the
equivalence margin ([collection](../1a379447-zen3-cpu-performance/7eddc5e2/collection.md)).

- At one worker and a full wave, both codes record `regressed` and `fail`:
  AFF3CT is the slower arm.
- At a batch of one frame, both codes record `improved` and `pass`: AFF3CT is
  the faster arm.
- At twelve physical cores, DVB-T2 records `improved` and `pass` (AFF3CT
  faster) and NR records `not-worse` and `not-material`.

The receipt is accepted, does not qualify and selects nothing. AFF3CT's
matched SIMD modes, flooding normalized min-sum INTER and normalized min-sum
INTRA, are unavailable in the pinned build and carry no cell (family
description of the [addendum](../1a379447-zen3-cpu-performance/87a6bb36/addendum.json)).

### Lane route against AFF3CT, fastest compatible

Tables, *AFF3CT fastest-compatible results › Outcomes*. The family is an
exploratory pilot on one worker; every cell is `pilot` and decides nothing.
The pilot's intervals lie above one in the layered f32 and i16 INTER cells,
favouring AFF3CT, and below one in the sum-product INTRA cells, favouring the
lane route. Every cell carries a P-19 note that the candidate FER upper bound
exceeds the predeclared tolerance (tables, *Campaigns › Findings*), so the
recorded corpus admits no AFF3CT mode as quality-compatible and the pilot
ranks no arm. The arms differ in schedule and stopping granularity, so their
iteration columns are not compared (*AFF3CT fastest-compatible results ›
Decoder quality*).

### Memory, batch fill and latency

- **Workspace.** The decoder's lane workspace per worker is a diagnostic of
  every gf2 lane arm (*Arm measurements*, column *Lane workspace*). The AFF3CT
  arms report none.
- **Peak resident set.** The [peak-memory record](../../bench_results/ed3d490e/preparation/peak-memory.json)
  measures the process memory of each arm at one configuration (*Peak memory*
  of each part). The lane and single-frame arms of one code have overlapping
  intervals, so the process peak does not resolve the lane workspace. The gf2
  lane arm's interval lies above the matched AFF3CT arm's on both codes.
- **Batch fill.** The gf2 arms report the stage probe of the decoder call at a
  zero iteration cap (*Arm measurements*, column *Batch fill*); the AFF3CT
  arms report no stage probe, so their batch fill is unmeasured.
- **Single-frame latency.** *Single-frame latency* of the matched and
  before-after parts lists the batch-of-one cells. No fastest-compatible cell
  decodes a batch of one frame.

### Decode selector calibration and holdout

The calibration campaign is measured and published
([campaign findings](calibration/campaign-findings.md)). The holdout
confirmation is accepted and qualifies: every confirmatory and holdout cell
records `pass`, the tuned-portable build against the conservative-portable
build and the native build against the tuned-portable build (tables, *Decode
selector holdout › Outcomes*;
[outcome table](../1a379447-zen3-cpu-performance/7695aaee/outcome.md), *Builds
and holdout*). Every holdout cell runs one worker.

### Profile series

The series of `f434ddd3` profiles both routes on both codes at one, six, twelve
and twenty-four workers ([record](../1a379447-zen3-cpu-performance/3bb47302/record.md);
tables, *Profile series*).

- In every lane case `lane-variable-step` is the largest category, ahead of
  `lane-check-step`; transposition in and out holds a small share (profile
  summary, *Sampled shares by category*). The one-worker lane cases carry the
  fewest samples, which their headings state.
- Per frame, the lane route executes fewer instructions than the single-frame
  route, and each case's instructions per frame are unchanged across worker
  counts (profile summary, *Counters and time per frame*).
- The lane route's per-worker slowdown against one worker exceeds the
  single-frame route's at every worker count, and as workers increase its IPC
  falls while its cache-misses over cache-references ratio rises (same table;
  *Ratios*, rows `slowdown:*`).
- The lane-over-single-frame ratios (*Ratios*, rows `gap:*`) agree in
  direction with the multicore confirmation: below one for NR at six and
  twelve workers, above one for DVB-T2 at six workers and more and for NR at
  twenty-four.

## Inter-frame and QC-aware intra-frame cells against the canonical baseline

No quantized, layered or QC-aware candidate enters the decoder. The
[decision record](../1a379447-zen3-cpu-performance/f63a2464/decision-record.md),
section *Decisions after the confirmations*, rules:

- family Q, quantized min-sum: no proceed, closed by stop rule S2;
- family L, layered min-sum: no proceed, closed by stop rule S2;
- family QC, QC-aware intra-frame: no proceed; its three confirmation
  campaigns exceed the frozen cap of one, so it has no valid confirmatory
  verdict ([corrections](../1a379447-zen3-cpu-performance/f63a2464/corrections.md),
  C-09; repository owner's ruling of 2026-10-05).

The canonical `LdpcDecoder` stays the production decoder, and the numerical
contract stays the float flooding one.

The comparison below is descriptive and decides nothing for either layout. The
common baseline is the canonical `LdpcDecoder` single-frame decode of f32
flooding normalized min-sum on the 5G NR BG1 lifting-384 code [ThreeGpp2017],
over the same recorded `c077a88b` corpus, on the same host and with the same
per-worker batch of eight frames.

| Configuration | Inter-frame lane route (tables) | QC-aware intra-frame prototype (timing tables) |
|---|---|---|
| One worker, full wave | *Before-after results of the lane route › Outcomes*, `nr-bg1-z384-lane-w1-b8` | `v4-r1-f63a2464-ldpc-qc-intra-frame-single-worker-confirmation`, `nr-bg1-z384-qc-w1` |
| One frame, one worker | same, `nr-bg1-z384-lane-w1-b1` | same, `nr-bg1-z384-qc-latency` |
| Six physical cores | same, `nr-bg1-z384-lane-p6-b8` | `v4-r1-f63a2464-ldpc-qc-intra-frame-multicore-confirmation`, `nr-bg1-z384-qc-w6` |
| Twelve physical cores | same, `nr-bg1-z384-lane-p12-b8` | same, `nr-bg1-z384-qc-w12` |
| Twenty-four logical CPUs | same, `nr-bg1-z384-lane-l24-b8` | same, `nr-bg1-z384-qc-w24` |
| AFF3CT flooding f32 on one worker | *AFF3CT matched-algorithm results › Outcomes*, `nr-bg1-z384-matched-w1-b8` and `nr-bg1-z384-matched-w1-b1` | `v4-r1-f63a2464-ldpc-qc-comparator-single-worker-confirmation`, both cells |

As the evaluators record them, the QC-aware cells against the canonical
baseline record `pass` in every configuration, the multicore cells included,
while the lane route records `fail` at one frame and at twenty-four logical
CPUs. Against the shared AFF3CT arm, the QC-aware prototype records the
external arm as slower at a full wave and at one frame; the lane route records
it as slower at a full wave and faster at one frame. The QC-aware cells'
recorded outcomes carry no deciding force (decision record, as above).

Limits of the comparison:

- The baselines are distinct executables: the inter-frame family's
  `gf2-batch-single-frame-nr-bg1-z384` is `LdpcBatchDecoder` on the
  single-frame route, and the QC family's `gf2-nms-f32-nr-bg1-z384` is an
  `LdpcDecoder` arm of the `f63a2464` harness. Their digests are in each
  receipt's `arms` entry. Each ratio is to its own family's baseline
  measurement, in a different campaign and session; no cell pairs the two
  layouts.
- The QC-aware family covers the NR code only; the DVB-T2 code has no QC arm
  (`f63a2464` [findings](../1a379447-zen3-cpu-performance/f63a2464/findings.md),
  *Limits and problems*). The inter-frame DVB-T2 cells have no QC counterpart.
- The QC-aware prototype is an experiment outside the production decoder path;
  the plan's DEC-10 rejects a paired cell with it because the frozen budget
  closes its family.

srsRAN [Srsran2026] and OpenAirInterface [OpenAirInterface2026] do not build on the host, and xdsopl
[Xdsopl2026] exposes no normalized min-sum rule, so none supplies a measurable
matched arm and AFF3CT is the only measured comparator
([comparator survey](../1a379447-zen3-cpu-performance/c077a88b/findings.md),
*Pinned arms and upstream status*).

## Criterion status

| Criterion | Status | Evidence |
|---|---|---|
| REQ-01 | MET for the evidence; adoption domain open | Every campaign is accepted and verified from its execution log, with the finding count of its row (tables, *Campaigns*); each receipt's `inputs/` pins the contract, protocol and family addendum. Before-after confirmations exist at one worker and at several. Negative outcomes stand as recorded: the `fail` cells of both before-after confirmations, the matched confirmation's `fail` and `not-material` cells, and the fastest-compatible P-19 notes. The committed envelope opens the lane route at every worker count, while only the one-worker evidence qualifies (residual gap R-01, tracked by `@/issue/d06a99ed`). |
| REQ-07 | MET | The [plan](plan.md) and the [terminal audit](terminal-audit.md) specify precision, lane layout, scalar references, workspace ownership, per-frame termination, partial batches and integration points before implementation; the [feasibility record](../../bench_results/ed3d490e/intrinsic-feasibility/feasibility.json) compiles the register forms at Rust 1.95. |
| REQ-08 | MET | Batch trait with default paths ([`batch_soft_decoder_contract.rs`](../../../crates/gf2-coding/tests/batch_soft_decoder_contract.rs)); lane kernels in `gf2-kernels-simd` with their [assembly artefact](../../../crates/gf2-kernels-simd/src/x86/asm/ldpc_lanes.asm.txt); `gf2-sim` stages over the trait; inherent batch functions removed (`@/issue/e4e2ba52`). Order and results for batches below, at and above eight frames and an incomplete final wave: [`ldpc_batch_contract.rs`](../../../crates/gf2-coding/tests/ldpc_batch_contract.rs). The default batch path of `LdpcDecoder` is a hard decision, a contract `@/issue/e29fd776` tracks. |
| REQ-09 | MET | Posterior-level equality on every route and backend, with mixed convergence and nonconvergence (`ldpc_batch_contract.rs`); worker counts and scheduling ([`batch_decode_worker_determinism.rs`](../../../crates/gf2-sim/tests/batch_decode_worker_determinism.rs), [`pooled_batch_decoder.rs`](../../../crates/gf2-coding/tests/pooled_batch_decoder.rs)); checkpoint resume (batch-decode resume tests of [`checkpoint_compat.rs`](../../../crates/gf2-sim/tests/checkpoint_compat.rs)); portable fallback (backend tests of `ldpc_batch_contract.rs`). |
| REQ-10 | PARTLY MET | Matched results are separate from fastest-compatible results (two parts of the tables). Single-frame latency, sustained throughput and a twelve-physical-core cell are measured against the matched AFF3CT arm. Unavailable as recorded: AFF3CT batch fill (no stage probe), AFF3CT workspace (not reported; process peak only), the matched INTER and INTRA SIMD modes (absent from the pinned build). The supported INTRA and INTER modes enter only the one-worker exploratory pilot at their native wave, with no single-frame latency, batch-size or multicore cell. No AFF3CT cell runs on logical CPUs. The batch-size sweep and the SMT cells compare the lane route with the single-frame route only. |
| REQ-11 | MET | No quantization or schedule change enters; the numerical contract places quantized alphabets and layered schedules outside the contract (*Scope*) and states no channel scaling and no saturation. |
| REQ-12 | MET | The decision record is committed and closed with the rulings above; the inter-frame and QC-aware cells stand beside each other in the preceding section, and neither is forced to win. |
| REQ-13 | PARTLY MET | Selectors live in the coding section with one route decision and no private threshold; the coding owner is calibrated through the offline campaign, which imports committed core and algebra owners ([premeasurement protocol](calibration/premeasurement-protocol.md), §6, with its [corrections](calibration/premeasurement-protocol-corrections.md)); the holdout confirmation qualifies with fill, transposition and workspace inside the call. Batch width has no selector: one wave is eight frames by the plan's DEC-03. Worker configuration enters only as `granules_per_task`, measured in a pool of four workers and omitted; the route selectors read no worker count, and the holdout reconfirms at one worker only (R-01, tracked by `@/issue/d06a99ed`). |
| REQ-14 | MET | The [numerical contract](../../../docs/reference/ldpc-decoder-numerical-contract.md) states the sign and tie rules, punctured and filler semantics and the accumulation order; `ldpc_batch_contract.rs` covers random and all-zero codewords, special values, mixed convergence and nonconvergence, and rate-matched NR frames with punctured and filler positions; the lane kernel suite of `gf2-kernels-simd` asserts negative zero. No quantized or changed-schedule candidate enters, so the clauses on scaling, saturation and quality bounds have no subject. |

## Residual gaps and their limits

No statement here claims an optimum; each gap's limit is what the evidence
shows within the story's search budget.

**R-01: the lane route loses to the single-frame route on several workers.**
The multicore confirmation records `fail` on DVB-T2 at six workers and more and
on NR at twenty-four logical CPUs, and the matched DVB-T2 twelve-core cell
records AFF3CT as faster. Measured limit: the profile series shows unchanged
instructions per frame with falling IPC and a rising cache-miss ratio as
workers increase (profile summary, *Counters and time per frame*), so the added
time is stall time, not added work. The lane workspace per worker (tables,
*Arm measurements*) is the state each worker streams; that shared cache or
memory bandwidth is the binding resource is an interpretation of the counters,
and no experiment varies the workspace at a fixed worker count. Under the
committed envelope these configurations take the lane route, because the route
decision reads no worker count. Tracked follow-up: `@/issue/d06a99ed`
calibrates the lane route against the worker configuration.

**R-02: the lane variable step dominates one-worker time.** Measured limit:
`lane-variable-step` is the largest category in every lane case (profile
summary). The [committed assembly](../../../crates/gf2-kernels-simd/src/x86/asm/ldpc_lanes.asm.txt)
of `lane_variable_step_avx2_unchecked` reaches each edge through a 32-bit
slot-to-edge index load, a bounds compare and a memory-operand `vaddps` into
one accumulator, so a variable's belief is one dependent add chain over
index-addressed 32-byte loads. That chain is the order the contract's
*Accumulation order* clause fixes. The AFF3CT modes the exploratory pilot
records ahead of the lane route, layered INTER in f32 and i16, run a schedule
or alphabet outside the contract. Preserved experiments: families L and Q of
`f63a2464`, closed by S2 on quality (decision record).

**R-03: single-frame latency.** A batch of one frame on the lane route records
`fail` against the single-frame route on NR, and AFF3CT is faster than the lane
route at one frame on both codes. The committed `lane_min_frames` sends a
one-frame wave to the single-frame route, and no cell measures the
single-frame route against AFF3CT at one frame. Measured limit: the
single-frame cases of the profile series place the time in `check-node-loop`,
`min-sum-reduction` and `variable-node-update` (profile summary, `sf-*`
cases). Preserved experiment: the QC-aware intra-frame prototype, whose
benefit axis is single-frame latency, measured without deciding force (timing
tables).

**R-04: the cost of fixed waves is not isolated.** A wave runs until its
slowest lane terminates (plan, DEC-08). The plan states that the batch-size
sweep and the profile series measure what the fixed policy costs; both measure
time with the policy in place and carry no arm without it, so the policy's own
cost is unmeasured. Tracked follow-up: `@/issue/f768c925` measures it.

**R-05: calibration grid ends.** `decode.lane_max_edges` and
`decode.granules_per_task` record `grid_boundary_limited` true (tables, *Decode
selector calibration*); the campaign measures no candidate beyond either end.
A code with more edges than the grid's largest takes the single-frame route.

## Limits of the evidence

- The timed evidence covers two codes, DVB-T2 normal rate 1/2 and NR BG1
  lifting 384, each at one recorded operating point of the `c077a88b` corpus,
  on one host.
- No arrival process is modelled: every timed call decodes a ready batch, so
  batch formation and queueing delay are unmeasured.
- The recorded corpus certifies no quality admission for a fastest-compatible
  arm; the P-19 notes leave every such arm unadmitted.
- The holdout confirmation and the calibration of the two route selectors run
  one worker; `granules_per_task` is measured in a pool of four.
- Peak resident set is process memory at one configuration per arm; it is no
  allocation census of the decoder state.
- The profile categories are sample shares with Wilson intervals; the
  one-worker lane cases hold the fewest samples.

## Recorded contradictions carried forward

- The [recategorization record](../1a379447-zen3-cpu-performance/92c53589/record.md)
  of `92c53589` records four statements that cite the `07ca8585` re-sampled
  profile; each keeps its text with a pointer to the record. Three are in the
  `07ca8585` findings, "The residual gap": the gf2 record cases carry no
  `edge-position-search`, `min-sum-input-vec`, `min-sum-dispatch` or
  `min-sum-reduction` category (contradicted for `min-sum-reduction`);
  `check-node-loop` is the dominant named decoder work that remains
  (contradicted for `gf2-dvb-l24`); the sampled categories of the levers the
  issue spends disappear (contradicted for `min-sum-reduction` and
  `syndrome-termination`). The fourth is in the story plan, *Outcome and
  criterion approach*: the re-sampled profile "leaves the shared check-node
  loop as the dominant named decoder work", contradicted as the record's
  second row states. The verdicts and their evidence are the record's table.
- The decision record's frozen confirmatory cap and family QC's three
  confirmation campaigns contradict each other
  ([corrections](../1a379447-zen3-cpu-performance/f63a2464/corrections.md),
  C-09).
- The holdout pilot's first window run is superseded, not voided, and stays
  committed ([attempt record](../1a379447-zen3-cpu-performance/67517a7b/superseded-pilot-attempt.md)).
- The calibration's premeasurement protocol keeps its pinned bytes; its
  [corrections](calibration/premeasurement-protocol-corrections.md) state where
  the current state differs.
- The plan's statement on what measures the fixed-wave policy (DEC-08) is not
  supported by the evidence, as R-04 states; the plan keeps its text.
