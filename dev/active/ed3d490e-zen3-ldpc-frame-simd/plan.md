# Plan: Vectorize LDPC decoding across frames (ed3d490e)

> Planning node: 2133d15f. Authoritative graph:
> [breakdown.json](breakdown.json).

## Outcome and criterion approach

The story delivers one decoder: the float flooding min-sum decoder with
independent frames in the lanes of an AVX2 register, as a reusable `gf2-coding`
batch API over isolated `gf2-kernels-simd` kernels, consumed thinly by
`gf2-sim`, calibrated through the offline tuning system and measured under the
[measurement contract](../1a379447-zen3-cpu-performance/measurement-contract.md).
The [candidate decision record](../f63a2464/decision-record.md) closes the
quantized, layered and QC-aware families with no candidate proceeding, so no
such candidate enters the scope and the numerical contract does not change.

The profile this design follows is committed: the
[lever ranking](../../bench_results/3be770d5/tables.md) carries inter-frame
SIMD as a comparator estimate with no gf2 mechanism, and the
[re-sampled profile](../../bench_results/07ca8585/v4-r1-resampled-profile/profile.md)
leaves the shared check-node loop as the dominant named decoder work after the
canonical layout and shared reduction of
[`07ca8585`](../07ca8585/findings.md). Code claims are in
[survey/source-evidence.json](survey/source-evidence.json), cited below by
topic; the register-width forms the kernel needs compile at Rust 1.95 and agree
with the canonical scalar statements in the
[feasibility record](../../bench_results/ed3d490e/intrinsic-feasibility/feasibility.json),
written by [survey/record-intrinsic-feasibility.py](survey/record-intrinsic-feasibility.py)
from the probe crate [survey/intrinsics-probe](survey/intrinsics-probe/src/lib.rs).

| Criterion | Approach | Covered by | Evidence / open gap |
|---|---|---|---|
| REQ-01 | Every production change has a before/after family; each family holds at most the cell count its ledger's first attempt admits; preparation is untimed and each timed run is a queue line; negative outcomes are recorded as evaluated. | `measurement-arms`, the `*-preparation` and `*-collection` entries, `outcome-publication` | Contract `measurement-authority` |
| REQ-07 | This plan is the design; the contracts below fix LLR precision, lane layout, scalar reference, workspace ownership, termination, partial batches, integration points and the numerical contract. A permanent contract page precedes every implementation entry. | `decoder-numerical-contract-page` | Profile and feasibility evidence above |
| REQ-08 | `gf2-coding` owns a batch decoder type; `gf2-kernels-simd` owns the lane kernels; `gf2-sim` stages hand batches over. Order and results hold for batch sizes below, at and above the wave width. | `lane-kernel-portable`, `batch-decoder-api`, `batch-conformance-suite`, `lane-kernel-avx2`, `batch-decoder-avx2-route`, `nr-rate-matched-batch-decode`, the `sim-*-batch` entries, `batch-decoding-reference-pages` | Contracts `lane-kernel-interface`, `batch-decode-api`; OPEN-1 |
| REQ-09 | A lane is a frame, so a frame's result is a function of its own LLRs; one shared suite holds every route to the single-frame decoder, and worker count, scheduling, resume and fallback are asserted. | `batch-conformance-suite`, `lane-kernel-avx2`, `batch-decoder-avx2-route`, `batch-worker-partition`, `sim-batch-determinism` | Contract `float-numerical-contract` |
| REQ-10 | Latency, batch fill, throughput and workspace memory are cells or recorded diagnostics of the before/after and AFF3CT families; batch sizes and core arms are swept; matched and fastest-compatible results stay in separate families and tables. | `measurement-arms`, the before-after and comparator entries, `outcome-publication` | Contract `comparison-arms` |
| REQ-11 | No quantization and no schedule change is introduced; the contract page states their exclusion. | `decoder-numerical-contract-page`, `outcome-publication` | Decision record; DEC-02 |
| REQ-12 | The bounded record is committed and closed; the outcome record sets inter-frame and QC-aware intra-frame cells against their common baseline. | `outcome-publication` | Decision record; OPEN-4 |
| REQ-13 | Decode selectors join the coding tuning section; a coding owner producer calibrates them through the offline tuning campaign; a `selector-calibration` family reconfirms the profile on holdout inputs with fill, transposition and workspace costs inside the call. | `decode-selector-family`, `coding-tuning-producer`, the two `campaign-*-coding-owner` entries, the `decoder-calibration-*` and `selector-holdout-*` entries, `decoder-dispatch-verification` | Contract `tuning-owner-mechanism`; OPEN-3 |
| REQ-14 | The canonical contract gains a permanent page with the batch clauses; the suite covers random and all-zero codewords, mixed convergence and nonconvergence, signed zero, ties and punctured and filler inputs. | `decoder-numerical-contract-page`, `lane-kernel-portable`, `batch-conformance-suite`, `nr-rate-matched-batch-decode` | [Numerical-contract review](../f63a2464/numerical-contract-review.md) |

## Shared architectural contracts

### `float-numerical-contract` [plan-fixed] — unchanged float contract, bit-exact per frame

The supported LLR precision is f32: `Llr` wraps `f32` and the `llr-f64` feature
enables nothing (ledger topic `precision`). The contract is the canonical one of
`LdpcDecoder`: sign by comparison against zero, strict-comparison minima with
the first position kept on a tie, the three min-sum scalings, no channel
scaling and no saturation, strict-comparison hard decision, sequential belief
accumulation over a variable's canonical slots, and syndrome termination after
the variable update (topics `sign-rule`, `minimum-rule`, `scaling-rule`,
`hard-decision`, `accumulation-order`, `termination`). Punctured, truncated and
filler positions are prepared by the rate-matching layer into ordinary
mother-code LLRs (topic `rate-matching`). A lane carries one frame, so nothing
is reassociated, and a frame's posterior bit patterns, hard decisions, iteration
count and flags equal the single-frame decoder's for every batch size and lane
position. Sum-product frames take the single-frame decoder (topic
`sum-product`).

### `lane-layout` [plan-fixed] — eight frames per AVX2 register over canonical edges

One wave is eight frames. Both message directions are arrays indexed by
canonical edge id and then by lane, so a check's edges are one contiguous run
of 32-byte-aligned lane vectors and the variable step reaches an edge through
one slot-to-edge load (topic `layout-canonical`). Channel LLRs and beliefs are
indexed by variable and then by lane. Hard decisions are one byte per variable
whose bit is the lane, so a check's syndrome over eight frames is an exclusive
or of bytes. The lane form mirrors the two-array structure of the single-frame
workspace (topic `workspace`).

### `lane-termination` [plan-fixed] — per-frame termination, idle lanes and partial batches

A frame terminates as the single-frame decoder does. The variable step takes an
active-lane mask and does not write the belief, the outgoing messages or the
hard-decision bit of an inactive lane, so a terminated lane holds its terminal
state while the wave continues and changes no other lane. A wave ends when no
lane is active or the iteration cap is reached. A final wave with fewer than
eight frames leaves its remaining lanes idle from the start at zero LLRs. A
wave whose occupied lanes fall below the `lane_min_frames` selector decodes its
frames on the single-frame route.

### `lane-kernel-interface` [implementation-produced] — lane types and kernel bundle in gf2-kernels-simd

The eight-lane f32 type and a function bundle of the check step and the masked
variable step over lane arrays and canonical layout slices, with a safe
portable implementation that is the kernel-level scalar reference and one
behavioural suite every backend runs. The AVX2 backend is published through
runtime detection and is the only `unsafe` in the story.

### `batch-decode-api` [implementation-produced] — batch decoder type of gf2-coding

A single-worker value built from an `LdpcCode` and a `DecoderConfig` that owns
every lane array, sized at construction, decodes a slice of frames into
caller-provided codewords and per-frame outcomes in input order without
steady-state allocation, exposes each frame's posterior, reports its route
without decoding and its workspace bytes, and has a parallel entry point with
one workspace per worker. The single-frame `LdpcDecoder` stays the canonical
reference and the single-frame route.

### `decode-selectors` [implementation-produced] — decode family of the coding tuning section

`lane_min_frames`, `lane_max_edges` and `waves_per_task` in the coding-owned
section, with one typed route decision that reads nothing else. The
conservative family selects the single-frame route, as the encode family's
conservative values select the scalar reference.

### `measured-arms` [implementation-produced] — benchmark arms with build identity

The gf2 single-frame and lane arms and the AFF3CT arms of `comparison-arms`,
built once in a workspace of their own, validated against the prepared
per-frame quality record and smoked through the shared runner. Every family
measures these executables by digest.

### `calibrated-decode-profile` [implementation-produced] — committed coding owner envelope

The coding owner envelope the decoder calibration campaign publishes, with its
measurement provenance. A selector the campaign omits keeps its conservative
value.

### `measurement-authority` [plan-fixed] — contract, protocol version 4 and worker brief

The [measurement contract](../1a379447-zen3-cpu-performance/measurement-contract.md),
the [protocol](../f547c394/protocol.md) with its
[version-4 amendment](../f547c394/amendment-v4.md) and the
[worker brief](../1a379447-zen3-cpu-performance/worker-brief.md) govern every
timed cell. Each family has its own append-only ledger and at most the cell
count its first confirmatory attempt admits, recomputed from the tooling. A
timed run is a queue line for the benchmark window; preparation and collection
are separate entries. The steady-state operation and the recorded corpus are
those of [`3be770d5`](../3be770d5/findings.md) and
[`c077a88b`](../c077a88b/findings.md), and each timed execution checks its
per-frame errors against the prepared quality record.

### `comparison-arms` [plan-fixed] — AFF3CT arms the pinned build admits

The comparator is AFF3CT [Cassagne2019] at the pin of
[`c077a88b`](../c077a88b/findings.md). The matched arm is its flooding f32
normalized min-sum decoder. Its horizontal-layered normalized min-sum INTER
modes in f32 and i16 and its float sum-product INTRA mode are fastest-compatible
arms: exploratory, labelled by precision, schedule and update rule, and
selecting nothing, because that survey admits no arm as quality-compatible.
Flooding INTER and normalized min-sum INTRA are unavailable in the pinned build
and carry no samples. srsRAN [Srsran2026] and OpenAirInterface
[OpenAirInterface2026] do not build on the host and xdsopl [Xdsopl2026] exposes
no normalized rule.

### `tuning-owner-mechanism` [plan-fixed] — coding owner of the offline tuning system

Decoder selectors are calibrated as section 2 of the
[core calibration plan](../1a379447-zen3-cpu-performance/63bad95d/calibration-plan.md)
describes the mechanism: an owner producer run by the campaign launcher
publishes a versioned owner envelope, and a protocol family of purpose
`selector-calibration` reconfirms the profile on holdout cells. The coding
section has one wire identity and an owner codec and no producer, and the
campaign driver, composer and validator admit two owners (topics
`tuning-section`, `tuning-owners`). The story adds the coding producer and the
third owner; its campaign measures the coding owner and imports the committed
core and algebra envelopes, so it depends on no core calibration.

## Generated decomposition overview

<!-- jit:breakdown-overview:begin -->
| Key | Title | Type | Outcome | Contracts | Sources | Footprint | Landing | Depends on |
|---|---|---|---|---|---|---|---|---|
| decoder-numerical-contract-page | State the LDPC decoder numerical contract with its batch clauses | task | The float decoder contract with its lane clauses has one permanent statement | float-numerical-contract, lane-termination | REQ-07, REQ-11, REQ-14, CONTRACT-REVIEW-F63A2464, DECISION-RECORD-F63A2464, SOURCE-EVIDENCE | creates 1, touches 2 | — | — |
| lane-kernel-portable | Provide the portable lane kernels of the min-sum decoder | task | Lane kernel bundle with a portable implementation that follows the float contract | float-numerical-contract, lane-layout, lane-termination | REQ-08, REQ-09, REQ-14, SOURCE-EVIDENCE, MSRV-FEASIBILITY, UPDATE-07CA8585 | creates 1, touches 1 | — | decoder-numerical-contract-page |
| decode-selector-family | Carry decoder selectors in the coding tuning section | task | Decode selectors with a typed route decision live in the coding tuning section | tuning-owner-mechanism | REQ-13, CALIBRATION-PLAN-63BAD95D, SOURCE-EVIDENCE | creates 1, touches 6 | — | — |
| batch-decoder-api | Decode LDPC frame batches through a reusable batch decoder | task | Batch decoder type with owned lane workspace and per-frame termination | float-numerical-contract, lane-layout, lane-termination, lane-kernel-interface, decode-selectors | REQ-08, SOURCE-EVIDENCE, UPDATE-07CA8585 | creates 1, touches 3 | — | lane-kernel-portable, decode-selector-family |
| batch-conformance-suite | Assert batch decoding against the single-frame decoder across codes | task | Shared posterior-level suite holds each batch route to the single-frame decoder | float-numerical-contract, lane-termination, batch-decode-api | REQ-08, REQ-09, REQ-14, CONTRACT-REVIEW-F63A2464, MEASUREMENT-CONTRACT | creates 1 | — | batch-decoder-api |
| lane-kernel-avx2 | Vectorize the lane kernels with AVX2 | task | AVX2 lane backend bit-identical to the portable backend behind safety contracts | float-numerical-contract, lane-layout, lane-termination, lane-kernel-interface | REQ-08, REQ-09, MSRV-FEASIBILITY, SOURCE-EVIDENCE | creates 2, touches 2 | — | batch-conformance-suite |
| batch-decoder-avx2-route | Route batch decoding to the AVX2 lanes by capability | task | Batch decoder selects the AVX2 lane backend by feature with tested fallbacks | batch-decode-api, lane-kernel-interface, decode-selectors, float-numerical-contract | REQ-08, REQ-09, SOURCE-EVIDENCE | creates 1, touches 4 | — | lane-kernel-avx2 |
| batch-worker-partition | Partition batch decoding across workers deterministically | task | Parallel batch entry point with per-worker workspaces and worker-count invariant results | batch-decode-api, decode-selectors, float-numerical-contract | REQ-09, SOURCE-EVIDENCE | touches 3 | — | batch-decoder-avx2-route |
| nr-rate-matched-batch-decode | Decode rate-matched 5G NR frame batches | task | Rate-matched NR decoder decodes frame batches with single-frame-equal results | batch-decode-api, float-numerical-contract | REQ-08, REQ-14, CONSUMERS-EDA07788, SOURCE-EVIDENCE | touches 3 | — | batch-conformance-suite |
| sim-cpu-ldpc-stage-batch | Decode CPU LDPC stage batches through the batch decoder | task | CPU LDPC stage is a thin consumer of the batch decoder | batch-decode-api | REQ-08, SOURCE-EVIDENCE | touches 1 | — | batch-decoder-avx2-route |
| sim-nr-decode-stage-batch | Decode NR stage batches through the batch decoder | task | NR decode stage is a thin consumer of the rate-matched batch entry point | batch-decode-api | REQ-08, SOURCE-EVIDENCE, CONSUMERS-12FDEB5B | touches 1 | — | nr-rate-matched-batch-decode, batch-decoder-avx2-route |
| sim-bler-sweep-batch | Decode BLER sweep slices through the batch decoder | task | BLER sweep is a thin consumer of the batch decoder | batch-decode-api | REQ-08, SOURCE-EVIDENCE, PROFILE-3BE770D5 | touches 1 | — | batch-decoder-avx2-route |
| sim-batch-determinism | Show seeded simulation determinism with batch decoding | task | Batch-decoding pipelines are deterministic across workers, resume and fallbacks | batch-decode-api, float-numerical-contract | REQ-09, MEASUREMENT-CONTRACT | touches 3, uncertain | — | sim-cpu-ldpc-stage-batch, sim-nr-decode-stage-batch |
| measurement-arms | Build the batch decoder benchmark arms | simulation | Validated benchmark arms for the batch decoder with pinned AFF3CT comparators | measurement-authority, comparison-arms, batch-decode-api | REQ-01, REQ-10, MEASUREMENT-CONTRACT, PROTOCOL-V4, WORKER-BRIEF, COMPARATOR-C077A88B, PROFILE-3BE770D5 | creates 3 | — | batch-decoder-avx2-route |
| before-after-families-preparation | Freeze the before and after decoder families | simulation | Before and after pilot families frozen with ledgers, launcher, smoke record and queue lines | measurement-authority, measured-arms | REQ-01, REQ-10, MEASUREMENT-CONTRACT, PROTOCOL-V4, WORKER-BRIEF, UPDATE-07CA8585 | creates 6, touches 1 | — | measurement-arms |
| comparator-families-preparation | Freeze the AFF3CT comparison families | simulation | AFF3CT matched and fastest-compatible pilot families frozen with queue lines | measurement-authority, comparison-arms, measured-arms | REQ-01, REQ-10, MEASUREMENT-CONTRACT, PROTOCOL-V4, WORKER-BRIEF, COMPARATOR-C077A88B | creates 2, touches 4 | — | before-after-families-preparation |
| before-after-pilot-collection | Collect the before and after pilots | simulation | Accepted before and after pilot receipts with frozen confirmation addenda | measurement-authority | REQ-01, REQ-10, MEASUREMENT-CONTRACT, PROTOCOL-V4, WORKER-BRIEF | creates 3, touches 1 | — | before-after-families-preparation |
| comparator-pilot-collection | Collect the AFF3CT comparison pilots | simulation | Accepted comparison pilot receipts with the matched confirmation addendum frozen | measurement-authority, comparison-arms | REQ-01, REQ-10, MEASUREMENT-CONTRACT, PROTOCOL-V4, WORKER-BRIEF, COMPARATOR-C077A88B | creates 1, touches 2 | — | comparator-families-preparation |
| before-after-confirmation-collection | Collect the before and after confirmations | simulation | Before and after confirmation receipts accepted with outcomes recorded as evaluated | measurement-authority | REQ-01, REQ-10, MEASUREMENT-CONTRACT, PROTOCOL-V4, WORKER-BRIEF | touches 1 | — | before-after-pilot-collection |
| comparator-confirmation-collection | Collect the matched AFF3CT confirmation | simulation | Matched AFF3CT confirmation receipt accepted with its gap recorded as evaluated | measurement-authority, comparison-arms | REQ-01, REQ-10, MEASUREMENT-CONTRACT, PROTOCOL-V4, WORKER-BRIEF | touches 1 | — | comparator-pilot-collection |
| coding-tuning-producer | Produce decode selector measurements from a coding owner harness | task | Coding owner producer measures the decode selector fields under the offline tuning protocol | tuning-owner-mechanism, decode-selectors, batch-decode-api | REQ-13, CALIBRATION-PLAN-63BAD95D, SOURCE-EVIDENCE | creates 2, touches 1 | — | batch-worker-partition |
| campaign-driver-coding-owner | Admit gf2-coding as an owner in the tuning campaign driver | task | Campaign driver with composer handles a measured coding owner beside imported owners | tuning-owner-mechanism | REQ-13, CALIBRATION-PLAN-63BAD95D, SOURCE-EVIDENCE | touches 3 | — | coding-tuning-producer |
| campaign-validator-coding-owner | Accept a coding owner campaign in the tuning campaign validator | task | Independent validator accepts a coding owner campaign stage | tuning-owner-mechanism | REQ-13, CALIBRATION-PLAN-63BAD95D, SOURCE-EVIDENCE | touches 1 | — | campaign-driver-coding-owner |
| decoder-calibration-preparation | Declare the decoder calibration campaign | simulation | Decoder calibration campaign declared with protocol amendment, manifest and queue line | tuning-owner-mechanism, measurement-authority | REQ-01, REQ-13, CALIBRATION-PLAN-63BAD95D, MEASUREMENT-CONTRACT, WORKER-BRIEF | creates 3, touches 1 | — | campaign-validator-coding-owner |
| decoder-calibration-collection | Commit the calibrated decode profile | simulation | Coding owner envelope with calibrated decode selectors committed with its receipt | tuning-owner-mechanism, measurement-authority, decode-selectors | REQ-01, REQ-13, CALIBRATION-PLAN-63BAD95D, MEASUREMENT-CONTRACT, WORKER-BRIEF | creates 2, touches 1 | — | decoder-calibration-preparation |
| selector-holdout-preparation | Freeze the decode selector holdout family | simulation | Selector holdout family frozen with holdout declaration, smoke record and queue line | measurement-authority, calibrated-decode-profile, measured-arms, tuning-owner-mechanism | REQ-01, REQ-13, CALIBRATION-PLAN-63BAD95D, MEASUREMENT-CONTRACT, PROTOCOL-V4, WORKER-BRIEF | creates 2, touches 4 | — | decoder-calibration-collection, comparator-families-preparation |
| selector-holdout-pilot-collection | Collect the decode selector holdout pilot | simulation | Accepted holdout pilot receipt with the confirmation addendum frozen | measurement-authority, calibrated-decode-profile | REQ-01, REQ-13, MEASUREMENT-CONTRACT, PROTOCOL-V4, WORKER-BRIEF | creates 1, touches 2 | — | selector-holdout-preparation |
| selector-holdout-confirmation-collection | Collect the decode selector holdout confirmation | simulation | Holdout confirmation receipt accepted with the profile's outcome recorded as evaluated | measurement-authority, calibrated-decode-profile | REQ-01, REQ-13, MEASUREMENT-CONTRACT, PROTOCOL-V4, WORKER-BRIEF | touches 1 | — | selector-holdout-pilot-collection |
| decoder-dispatch-verification | Verify decoder dispatch under the committed profile | task | Production decode routes observed under the committed, conservative and fallback configurations | calibrated-decode-profile, decode-selectors, batch-decode-api | REQ-13, CALIBRATION-PLAN-63BAD95D | creates 1, touches 1 | — | decoder-calibration-collection |
| batch-decoding-reference-pages | Describe batch LDPC decoding in the permanent pages | task | Permanent pages state batch decoding routes, selectors and supported configurations | batch-decode-api, decode-selectors | REQ-08, REQ-13, SOURCE-EVIDENCE | touches 3 | — | decoder-dispatch-verification |
| lane-profile-preparation | Prepare the profile series of the batch decoder | simulation | Profile series of the batch decoder arms prepared with its queue line | measurement-authority, measured-arms | REQ-01, PROFILE-3BE770D5, MEASUREMENT-CONTRACT, WORKER-BRIEF | creates 1, touches 2, uncertain | — | measurement-arms |
| lane-profile-collection | Summarize the profile series of the batch decoder | simulation | Generated profile summary of the batch decoder arms from the measured series | measurement-authority | REQ-01, PROFILE-3BE770D5, MEASUREMENT-CONTRACT, WORKER-BRIEF | creates 1 | — | lane-profile-preparation |
| outcome-publication | Report the inter-frame decoder outcomes with their limits | task | Outcome record with generated tables, criterion status and residual limits | measurement-authority, comparison-arms, float-numerical-contract | REQ-01, REQ-10, REQ-11, REQ-12, MEASUREMENT-CONTRACT, WORKER-BRIEF, DECISION-RECORD-F63A2464, COMPARATOR-C077A88B, PROFILE-3BE770D5 | creates 3 | — | before-after-confirmation-collection, comparator-confirmation-collection, selector-holdout-confirmation-collection, lane-profile-collection, batch-decoding-reference-pages, sim-batch-determinism, sim-bler-sweep-batch |

```mermaid
flowchart LR
    N0["decoder-numerical-contract-page: State the LDPC decoder numerical contract with its batch clauses"]
    N1["lane-kernel-portable: Provide the portable lane kernels of the min-sum decoder"]
    N2["decode-selector-family: Carry decoder selectors in the coding tuning section"]
    N3["batch-decoder-api: Decode LDPC frame batches through a reusable batch decoder"]
    N4["batch-conformance-suite: Assert batch decoding against the single-frame decoder across codes"]
    N5["lane-kernel-avx2: Vectorize the lane kernels with AVX2"]
    N6["batch-decoder-avx2-route: Route batch decoding to the AVX2 lanes by capability"]
    N7["batch-worker-partition: Partition batch decoding across workers deterministically"]
    N8["nr-rate-matched-batch-decode: Decode rate-matched 5G NR frame batches"]
    N9["sim-cpu-ldpc-stage-batch: Decode CPU LDPC stage batches through the batch decoder"]
    N10["sim-nr-decode-stage-batch: Decode NR stage batches through the batch decoder"]
    N11["sim-bler-sweep-batch: Decode BLER sweep slices through the batch decoder"]
    N12["sim-batch-determinism: Show seeded simulation determinism with batch decoding"]
    N13["measurement-arms: Build the batch decoder benchmark arms"]
    N14["before-after-families-preparation: Freeze the before and after decoder families"]
    N15["comparator-families-preparation: Freeze the AFF3CT comparison families"]
    N16["before-after-pilot-collection: Collect the before and after pilots"]
    N17["comparator-pilot-collection: Collect the AFF3CT comparison pilots"]
    N18["before-after-confirmation-collection: Collect the before and after confirmations"]
    N19["comparator-confirmation-collection: Collect the matched AFF3CT confirmation"]
    N20["coding-tuning-producer: Produce decode selector measurements from a coding owner harness"]
    N21["campaign-driver-coding-owner: Admit gf2-coding as an owner in the tuning campaign driver"]
    N22["campaign-validator-coding-owner: Accept a coding owner campaign in the tuning campaign validator"]
    N23["decoder-calibration-preparation: Declare the decoder calibration campaign"]
    N24["decoder-calibration-collection: Commit the calibrated decode profile"]
    N25["selector-holdout-preparation: Freeze the decode selector holdout family"]
    N26["selector-holdout-pilot-collection: Collect the decode selector holdout pilot"]
    N27["selector-holdout-confirmation-collection: Collect the decode selector holdout confirmation"]
    N28["decoder-dispatch-verification: Verify decoder dispatch under the committed profile"]
    N29["batch-decoding-reference-pages: Describe batch LDPC decoding in the permanent pages"]
    N30["lane-profile-preparation: Prepare the profile series of the batch decoder"]
    N31["lane-profile-collection: Summarize the profile series of the batch decoder"]
    N32["outcome-publication: Report the inter-frame decoder outcomes with their limits"]
    N0 --> N1
    N1 --> N3
    N2 --> N3
    N3 --> N4
    N4 --> N5
    N5 --> N6
    N6 --> N7
    N4 --> N8
    N6 --> N9
    N8 --> N10
    N6 --> N10
    N6 --> N11
    N9 --> N12
    N10 --> N12
    N6 --> N13
    N13 --> N14
    N14 --> N15
    N14 --> N16
    N15 --> N17
    N16 --> N18
    N17 --> N19
    N7 --> N20
    N20 --> N21
    N21 --> N22
    N22 --> N23
    N23 --> N24
    N24 --> N25
    N15 --> N25
    N25 --> N26
    N26 --> N27
    N24 --> N28
    N28 --> N29
    N13 --> N30
    N30 --> N31
    N18 --> N32
    N19 --> N32
    N27 --> N32
    N31 --> N32
    N29 --> N32
    N12 --> N32
    N11 --> N32
```
<!-- jit:breakdown-overview:end -->

## Material risks and owner decisions

| Risk / decision | Resolution and rationale |
|---|---|
| DEC-01 precision and rules | Chosen f32 and the min-sum family on the lane route; sum-product stays on the single-frame route. Rejected an f64 lane form, because no f64 decoder exists, and a lane sum-product, because `tanh` and `atanh` have no bit-exact vector form. |
| DEC-02 numerical contract | Chosen the canonical contract unchanged with bit-exact per-frame results, tested at the posterior level. Rejected any tolerance-based equivalence: the decision record admits no changed contract. |
| DEC-03 lane layout | Chosen lane = frame over canonical check-major edges, one register of eight frames per wave. Rejected lanes over a node's own edges, which reassociate the belief sum, and multi-register waves, for which no committed evidence exists. |
| DEC-04 scalar references | Chosen two: `LdpcDecoder` is the canonical reference of results, and a safe portable lane backend is the kernel-level reference and the fallback without AVX2. Rejected a per-frame fallback alone, which leaves lane masking untested on a host without AVX2. |
| DEC-05 selector home and default | Chosen a decode family inside the one coding-owned section, conservative values selecting the single-frame route. Rejected a second coding section, which breaks the one-section-per-owner shape of the campaign tooling, and a private threshold. |
| DEC-06 existing kernel | The horizontal `minsum_fn` kernel is not used: its lanes take the IEEE sign bit, the divergence `39cbde20` owns. |
| OPEN-1 public API shape (blocks `batch-decoder-api` and its dependents) | Recommended and written into the manifest: a separate batch decoder type, frames as a slice of `[Llr]` views, results into caller buffers, and `LdpcDecoder::decode_batch` routed through it. Alternatives: batch methods on `LdpcDecoder`, which put a second workspace inside the reference decoder; a flat frame-major LLR slice, which makes `gf2-sim` flatten its frame vectors. |
| OPEN-2 wave policy (blocks `batch-decoder-api`) | Recommended and written into the manifest: fixed waves, a wave lasting until its slowest lane terminates, which is the wave semantics of AFF3CT's INTER modes [Cassagne2019]. Alternative: refilling a terminated lane with the next frame, which removes the slowest-lane cost and adds a per-lane restart path; the batch-size sweep and the profile series measure what the fixed policy costs. |
| OPEN-3 REQ-13 mechanism scope (blocks `coding-tuning-producer` and the `campaign-*`, `decoder-calibration-*`, `selector-holdout-*` and `decoder-dispatch-verification` entries) | Recommended and written into the manifest: `gf2-coding` becomes the third owner of the offline tuning campaign. Alternative: calibrate under the Zen 3 protocol alone and compose the coding envelope outside the campaign, which removes the two `campaign-*` entries and leaves the envelope without the campaign's validator. |
| OPEN-4 REQ-12 comparison form (blocks `outcome-publication`) | Recommended and written into the manifest: a descriptive comparison of inter-frame and QC-aware intra-frame cells against their common canonical baseline from committed receipts. Alternative: an exploratory paired cell with the QC prototype arm, which measures again a family the frozen budget closed. |
| Risk: workspace size | A wave holds eight frames of messages per worker, so the lane route can leave the cache where the single-frame route does not. `lane_max_edges` closes the lane route by code size, the multicore family and the profile counters measure it, and workspace bytes are reported. |
| Risk: slowest lane | Under fixed waves a nonconverging frame holds its wave to the iteration cap. The effect is measured, not predicted; OPEN-2 names the alternative. |
| Risk: campaign tooling | The third owner changes a driver, a composer and a validator that pin committed campaigns. Each entry requires the committed two-owner campaigns to validate unchanged. |
| Risk: window capacity | The story queues pilots and confirmations of four confirmable families, two exploratory families, one calibration campaign and one profile series; collection entries report which runs are measured and which wait. |
| Re-homed dependencies | None. The planning node's one dependency, `f63a2464`, is complete. |
| Reported outside scope | `llr-f64` is a feature of `gf2-coding` and `gf2-sim` that enables nothing. |

## Investigation sources

Manifest source identifiers resolve as follows; the criteria `REQ-01` and
`REQ-07` to `REQ-14` are the story's own.

| Source | Location |
|---|---|
| `MEASUREMENT-CONTRACT` | [measurement-contract.md](../1a379447-zen3-cpu-performance/measurement-contract.md) |
| `PROTOCOL-V4` | [protocol.md](../f547c394/protocol.md), [amendment-v4.md](../f547c394/amendment-v4.md) |
| `WORKER-BRIEF` | [worker-brief.md](../1a379447-zen3-cpu-performance/worker-brief.md) |
| `DECISION-RECORD-F63A2464` | [decision-record.md](../f63a2464/decision-record.md), [findings.md](../f63a2464/findings.md), [corrections.md](../f63a2464/corrections.md) |
| `CONTRACT-REVIEW-F63A2464` | [numerical-contract-review.md](../f63a2464/numerical-contract-review.md) |
| `PROFILE-3BE770D5` | [findings.md](../3be770d5/findings.md) |
| `UPDATE-07CA8585` | [findings.md](../07ca8585/findings.md) |
| `COMPARATOR-C077A88B` | [findings.md](../c077a88b/findings.md) |
| `CONSUMERS-EDA07788` | [findings.md](../eda07788/findings.md) |
| `CONSUMERS-12FDEB5B` | [findings.md](../12fdeb5b/findings.md) |
| `CALIBRATION-PLAN-63BAD95D` | [calibration-plan.md](../1a379447-zen3-cpu-performance/63bad95d/calibration-plan.md) |
| `SOURCE-EVIDENCE` | [survey/source-evidence.json](survey/source-evidence.json) |
| `MSRV-FEASIBILITY` | [feasibility.json](../../bench_results/ed3d490e/intrinsic-feasibility/feasibility.json) |
