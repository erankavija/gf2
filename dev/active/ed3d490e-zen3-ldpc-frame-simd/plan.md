# Plan: Vectorize LDPC decoding across frames (ed3d490e)

> Planning node: 2133d15f. Authoritative graph:
> [breakdown.json](breakdown.json).

## Outcome and criterion approach

The story delivers one decoder: the float flooding min-sum decoder with
independent frames in the lanes of an AVX2 register. It reaches callers through
a general batch soft-decoder trait of `gf2-coding`, whose first specialised
implementer it is, over isolated `gf2-kernels-simd` kernels; `gf2-sim` stages
consume the trait. It is calibrated through the offline tuning system and measured under the
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
| REQ-01 | Every production change has a before/after family; each family holds at most the cell count its ledger's first attempt admits; preparation is untimed and each timed run is a queue line; negative outcomes are recorded as evaluated. | the arm entries (`gf2-benchmark-arms`, `aff3ct-benchmark-arms`, `intra-arm-quality-record`, the four `arms-*` entries, `tuned-profile-arm`), the tool entries (`addendum-generator`, `campaign-launcher`, `plan-smoke-runner`), the `*-preparation` and `*-collection` entries, the `lane-profile-*` entries, `profile-summarizer-lane-rules`, `outcome-tables`, `outcome-publication` | Contract `measurement-authority` |
| REQ-07 | This plan is the design; the contracts below fix LLR precision, lane layout, scalar reference, workspace ownership, termination, partial batches, integration points and the numerical contract. A permanent contract page precedes every implementation entry. | `decoder-numerical-contract-page` | Profile and feasibility evidence above |
| REQ-08 | `gf2-coding` owns a batch soft-decoder trait with default paths for every soft decoder and one shared contract function; the LDPC lane decoder, the rate-matched NR decoder and the DVB-T2 concatenated decoder are its specialised implementers; `gf2-kernels-simd` owns the lane kernels; one generic `gf2-sim` stage consumes the trait; the inherent LDPC batch functions are removed. | `batch-soft-decoder-trait`, `batch-decoder-contract-suite`, `lane-kernel-portable`, `ldpc-lane-batch-decoder`, `batch-conformance-suite`, `lane-kernel-avx2`, `batch-decoder-avx2-route`, `batch-worker-pool`, the four `decode-batch-*` entries, `nr-rate-matched-batch-decode`, `dvb-t2-concat-batch-decode`, the `sim-*` entries, `batch-decoding-reference-pages` | Contracts `batch-soft-decoder-trait`, `ldpc-lane-decoder`, `lane-kernel-interface`; ledger topics `decoder-traits`, `implementers`, `stage-boundary` |
| REQ-09 | A lane is a frame, so a frame's result is a function of its own LLRs; one shared suite holds every route to the single-frame decoder, and worker count, scheduling, resume and fallback are asserted. | `batch-decoder-contract-suite`, `batch-conformance-suite`, `lane-kernel-avx2`, `batch-decoder-avx2-route`, `batch-worker-pool`, `sim-batch-worker-determinism`, `sim-batch-resume-identity` | Contract `float-numerical-contract` |
| REQ-10 | Latency, batch fill, throughput and workspace memory are cells or recorded diagnostics of the before/after and AFF3CT families; batch sizes and core arms are swept; matched and fastest-compatible results stay in separate families and tables. | the arm entries, the five `*-family-preparation` entries, the per-family `*-pilot-collection` and `*-confirmation-collection` entries, `arms-peak-memory-record`, `outcome-tables`, `outcome-publication` | Contract `comparison-arms` |
| REQ-11 | No quantization and no schedule change is introduced; the contract page states their exclusion. | `decoder-numerical-contract-page`, `outcome-publication` | Decision record; DEC-02 |
| REQ-12 | The bounded record is committed and closed; the outcome record sets inter-frame and QC-aware intra-frame cells against their common baseline. | `outcome-publication` | Decision record; DEC-10 |
| REQ-13 | Decode selectors join the coding tuning section; a coding owner producer calibrates them through the offline tuning campaign; a `selector-calibration` family reconfirms the profile on holdout inputs with fill, transposition and workspace costs inside the call. | `decode-selector-family`, `decode-route-decision`, `coding-tuning-producer`, `profile-composer-coding-owner`, the two `campaign-*-coding-owner` entries, `tuned-profile-arm`, the `decoder-calibration-*` and `selector-holdout-*` entries, `decoder-dispatch-verification`, `batch-decoding-reference-pages` | Contract `tuning-owner-mechanism`; DEC-09 |
| REQ-14 | The canonical contract gains a permanent page with the batch clauses; the suite covers random and all-zero codewords, mixed convergence and nonconvergence, signed zero, ties and punctured and filler inputs. | `decoder-numerical-contract-page`, `lane-kernel-portable`, `batch-conformance-suite`, `nr-rate-matched-batch-decode` | [Numerical-contract review](../f63a2464/numerical-contract-review.md) |

## Shared architectural contracts

### `float-numerical-contract` [plan-fixed] — unchanged float contract, bit-exact per frame

The supported LLR precision is f32: `Llr` wraps `f32` (ledger topic
`precision`). The contract is the canonical one of
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

### `batch-soft-decoder-trait` [implementation-produced] — general batch soft-decoder trait of gf2-coding

A trait beside `SoftDecoder` and `IterativeSoftDecoder`, in the style of the
modem's `BatchMapper` and `BatchSoftDemapper` (ledger topic
`batch-trait-style`): LLR frames in, caller-provided decoded words and
per-frame outcomes out, input order preserved, by mutable reference because an
implementer may own a workspace, with the implementer's preferred batch
granularity and no steady-state allocation required of an implementer. The two
single-frame entry points differ (topic `decoder-traits`), so there are two
default paths: every `SoftDecoder` is a batch decoder through a per-frame loop
over `decode_soft_with_result`, and an adapter holding an iteration cap covers
every `IterativeSoftDecoder` through `decode_iterative`. For the LDPC and GLDPC
decoders the first path is their hard-decision `decode_soft` (topic
`implementers`), so their batch decoding goes through the adapter or a
specialised implementer. One outcome type serves the crate, and one contract
function, which every implementer runs, asserts order, per-frame equality with
the single-frame decode, batch sizes below, at and above the granularity, an
incomplete final group and mixed convergence. The trait covers soft-decision
decoding of whole block frames; hard-decision decoders, streaming decoders and
batch encoding have their own entry points (topics `decoder-traits`,
`batch-encoding`). Parallel decoding is expressed once over the trait: a pooled
batch decoder owns one inner decoder per worker and implements the trait. A
`gf2-sim` stage processes by shared reference with a default-constructed
scratch (topic `stage-boundary`), so one generic stage builds its decoder in
the scratch from a factory and names no decoder type.

### `ldpc-lane-decoder` [implementation-produced] — LDPC implementer of the batch trait

A single-worker value built from an `LdpcCode`, a `DecoderConfig`, an iteration
cap and the choice of codewords or message bits as decoded words. It owns
every lane array, sized at construction, decodes without steady-state
allocation, exposes each frame's posterior, and reports its route without
decoding and its workspace bytes. Its workspace and kernels are LDPC-specific.
The single-frame `LdpcDecoder` stays the canonical reference and the
single-frame route.

### `decode-selectors` [implementation-produced] — decode family of the coding tuning section

`lane_min_frames`, `lane_max_edges` and `granules_per_task` in the coding-owned
section. One typed route decision in the LDPC module reads them and nothing
else. The
conservative family selects the single-frame route, as the encode family's
conservative values select the scalar reference.

### `measured-arms` [implementation-produced] — benchmark arms with build identity

The gf2 single-frame and lane arms and the AFF3CT arms of `comparison-arms`,
built once in a workspace of their own, with a committed build identity
record, validation record and runner smoke record; the smoke record completes
the contract. Every family
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
| decoder-numerical-contract-page | State the LDPC decoder numerical contract with its batch clauses | task | The float decoder contract with its lane clauses has one permanent statement | float-numerical-contract, lane-termination | REQ-07, REQ-11, REQ-14, CONTRACT-REVIEW-F63A2464, DECISION-RECORD-F63A2464, SOURCE-EVIDENCE | creates 1, touches 1 | — | — |
| lane-kernel-portable | Provide the portable lane kernels of the min-sum decoder | task | Lane kernel bundle with a portable implementation that follows the float contract | float-numerical-contract, lane-layout, lane-termination | REQ-08, REQ-09, REQ-14, SOURCE-EVIDENCE, MSRV-FEASIBILITY, UPDATE-07CA8585 | creates 1, touches 1 | — | decoder-numerical-contract-page |
| decode-selector-family | Carry decoder selectors in the coding tuning section | task | Decode selector family lives in the coding tuning section with its codec | tuning-owner-mechanism | REQ-13, CALIBRATION-PLAN-63BAD95D, SOURCE-EVIDENCE | touches 3 | — | — |
| decode-route-decision | Decide the LDPC batch route from the decode selectors | task | One typed route decision turns decode selectors into the LDPC batch route | decode-selectors | REQ-13, SOURCE-EVIDENCE | creates 1, touches 1 | — | decode-selector-family |
| batch-soft-decoder-trait | Define the batch soft-decoder trait with its default paths | task | Batch soft-decoder trait with two default paths and one outcome type | — | REQ-08, SOURCE-EVIDENCE | touches 3 | — | — |
| batch-decoder-contract-suite | Assert the batch decoder contract over the existing soft decoders | task | Shared batch contract function passes over five soft decoders | batch-soft-decoder-trait | REQ-08, REQ-09, SOURCE-EVIDENCE | creates 1, touches 1 | — | batch-soft-decoder-trait |
| ldpc-lane-batch-decoder | Decode LDPC frame batches in lanes behind the batch decoder trait | task | LDPC lane decoder implements the batch trait with owned workspace and per-frame termination | float-numerical-contract, lane-layout, lane-termination, lane-kernel-interface, decode-selectors, batch-soft-decoder-trait | REQ-08, SOURCE-EVIDENCE, UPDATE-07CA8585 | creates 1, touches 3 | — | lane-kernel-portable, decode-route-decision, batch-decoder-contract-suite |
| batch-conformance-suite | Assert LDPC batch decoding at the posterior level across codes | task | Posterior-level suite holds each LDPC batch route to the single-frame decoder | float-numerical-contract, lane-termination, ldpc-lane-decoder | REQ-08, REQ-09, REQ-14, CONTRACT-REVIEW-F63A2464, MEASUREMENT-CONTRACT | creates 1 | — | ldpc-lane-batch-decoder |
| lane-kernel-avx2 | Vectorize the lane kernels with AVX2 | task | AVX2 lane backend bit-identical to the portable backend behind safety contracts | float-numerical-contract, lane-layout, lane-termination, lane-kernel-interface | REQ-08, REQ-09, MSRV-FEASIBILITY, SOURCE-EVIDENCE | creates 2, touches 2 | — | batch-conformance-suite |
| batch-decoder-avx2-route | Route LDPC batch decoding to the AVX2 lanes by capability | task | LDPC batch decoder selects the AVX2 lane backend by feature with tested fallbacks | ldpc-lane-decoder, lane-kernel-interface, decode-selectors, float-numerical-contract | REQ-08, REQ-09, SOURCE-EVIDENCE | creates 1, touches 4 | — | lane-kernel-avx2 |
| batch-worker-pool | Decode batches across workers through one pooled batch decoder | task | One pooled batch decoder over the trait with worker-count invariant results | batch-soft-decoder-trait, decode-selectors | REQ-08, REQ-09, SOURCE-EVIDENCE | creates 2, touches 1 | — | batch-decoder-contract-suite, decode-selector-family |
| decode-batch-test-callers | Move the tests of the inherent LDPC batch functions to the batch trait | task | Tests of LDPC batch decoding run through the batch trait | batch-soft-decoder-trait, ldpc-lane-decoder | REQ-08, SOURCE-EVIDENCE | touches 2 | — | batch-decoder-avx2-route, batch-worker-pool |
| decode-batch-bench-callers | Move the benchmarks of the inherent LDPC batch functions to the batch trait | task | Benchmarks of LDPC batch decoding run through the batch trait | batch-soft-decoder-trait, ldpc-lane-decoder | REQ-08, SOURCE-EVIDENCE | touches 3 | — | batch-decoder-avx2-route, batch-worker-pool |
| decode-batch-howto-page | Show parallel LDPC decoding through the batch trait in the how-to page | task | How-to page shows parallel LDPC decoding through the batch trait | batch-soft-decoder-trait, ldpc-lane-decoder | REQ-08, SOURCE-EVIDENCE | touches 1 | — | batch-decoder-avx2-route, batch-worker-pool |
| decode-batch-removal | Remove the inherent LDPC batch functions | task | Inherent LDPC batch functions are gone with one batch decoding path left | batch-soft-decoder-trait | REQ-08, SOURCE-EVIDENCE | touches 1 | — | decode-batch-test-callers, decode-batch-bench-callers, decode-batch-howto-page |
| nr-rate-matched-batch-decode | Decode rate-matched 5G NR frame batches in lanes | task | Rate-matched NR batch decoder implements the trait with single-frame-equal results | batch-soft-decoder-trait, ldpc-lane-decoder, float-numerical-contract | REQ-08, REQ-14, CONSUMERS-EDA07788, SOURCE-EVIDENCE | touches 2 | — | batch-conformance-suite |
| dvb-t2-concat-batch-decode | Decode DVB-T2 concatenated frame batches in lanes | task | DVB-T2 concatenated batch decoder implements the trait with single-frame-equal results | batch-soft-decoder-trait, ldpc-lane-decoder | REQ-08, SOURCE-EVIDENCE | touches 2 | — | batch-conformance-suite |
| sim-batch-decode-stage | Provide a simulation decode stage over the batch decoder trait | task | Generic gf2-sim decode stage consumes the batch soft-decoder trait | batch-soft-decoder-trait | REQ-08, SOURCE-EVIDENCE | creates 1, touches 1 | — | batch-soft-decoder-trait |
| sim-cpu-ldpc-stage-batch | Decode CPU LDPC stage batches through the batch decode stage | task | CPU LDPC stage is a thin consumer of the batch decode stage | batch-soft-decoder-trait, ldpc-lane-decoder | REQ-08, SOURCE-EVIDENCE | touches 1 | — | sim-batch-decode-stage, batch-decoder-avx2-route |
| sim-nr-decode-stage-batch | Decode NR stage batches through the batch decode stage | task | NR decode stage is a thin consumer of the batch decode stage | batch-soft-decoder-trait | REQ-08, SOURCE-EVIDENCE, CONSUMERS-12FDEB5B | touches 1 | — | sim-batch-decode-stage, nr-rate-matched-batch-decode, batch-decoder-avx2-route |
| sim-dvb-t2-stage-batch | Decode DVB-T2 stage batches through the batch decode stage | task | DVB-T2 decode stage is a thin consumer of the batch decode stage | batch-soft-decoder-trait | REQ-08, SOURCE-EVIDENCE | touches 1 | — | sim-batch-decode-stage, dvb-t2-concat-batch-decode, batch-decoder-avx2-route |
| sim-bler-sweep-batch | Decode BLER sweep slices through the batch decoder trait | task | BLER sweep is a thin consumer of the batch decoder trait | batch-soft-decoder-trait, ldpc-lane-decoder | REQ-08, SOURCE-EVIDENCE, PROFILE-3BE770D5 | touches 1 | — | batch-decoder-avx2-route |
| sim-batch-worker-determinism | Show worker-count determinism of batch-decoding pipelines | task | Batch-decoding pipelines give identical results across workers and fallbacks | batch-soft-decoder-trait, float-numerical-contract | REQ-09, MEASUREMENT-CONTRACT | touches 1 | — | sim-cpu-ldpc-stage-batch, sim-nr-decode-stage-batch, sim-dvb-t2-stage-batch |
| sim-batch-resume-identity | Show checkpoint resume identity of batch-decoding campaigns | task | Checkpointed batch-decoding campaigns resume to the uninterrupted results | batch-soft-decoder-trait, float-numerical-contract | REQ-09, MEASUREMENT-CONTRACT | touches 1 | — | sim-cpu-ldpc-stage-batch, sim-nr-decode-stage-batch, sim-dvb-t2-stage-batch |
| gf2-benchmark-arms | Build the gf2 batch decoder benchmark arms | simulation | Two gf2 benchmark arms of the LDPC batch decoder with per-frame quality checks | measurement-authority, ldpc-lane-decoder | REQ-01, REQ-10, MEASUREMENT-CONTRACT, PROTOCOL-V4, WORKER-BRIEF, PROFILE-3BE770D5 | creates 1 | — | batch-decoder-avx2-route |
| aff3ct-benchmark-arms | Provide the AFF3CT benchmark arms of the batch decoder comparison | simulation | AFF3CT arms catalogued by digest with one INTRA arm executable | measurement-authority, comparison-arms | REQ-01, REQ-10, MEASUREMENT-CONTRACT, PROTOCOL-V4, WORKER-BRIEF, COMPARATOR-C077A88B | creates 1, touches 1 | — | gf2-benchmark-arms |
| intra-arm-quality-record | Prepare the quality record of the sum-product INTRA arm | simulation | Prepared per-frame quality record of the INTRA arm | measurement-authority, comparison-arms | REQ-01, REQ-10, MEASUREMENT-CONTRACT, PROTOCOL-V4, COMPARATOR-C077A88B | creates 1 | — | aff3ct-benchmark-arms |
| arms-build-identity | Record the build identity of the benchmark arms | simulation | Build identity record of the benchmark arms | measurement-authority | REQ-01, MEASUREMENT-CONTRACT, PROTOCOL-V4, WORKER-BRIEF | creates 2 | — | aff3ct-benchmark-arms |
| arms-quality-validation | Validate the benchmark arms against their quality records | simulation | Validation record of the benchmark arms against prepared quality | measurement-authority, comparison-arms | REQ-01, REQ-10, MEASUREMENT-CONTRACT, PROTOCOL-V4, WORKER-BRIEF | creates 1 | — | arms-build-identity, intra-arm-quality-record |
| arms-peak-memory-record | Record the peak memory of the benchmark arms | simulation | Peak resident memory record of the benchmark arms | measurement-authority | REQ-10, MEASUREMENT-CONTRACT, DECISION-RECORD-F63A2464 | creates 2 | — | arms-build-identity |
| arms-runner-smoke | Smoke the benchmark arms through the shared runner | simulation | Runner smoke record of the benchmark arms | measurement-authority | REQ-01, MEASUREMENT-CONTRACT, PROTOCOL-V4, WORKER-BRIEF | creates 1 | — | arms-quality-validation |
| addendum-generator | Generate family addenda with their plans from family declarations | simulation | One generator writes each family's addendum with its plan from a declaration | measurement-authority, measured-arms | REQ-01, MEASUREMENT-CONTRACT, PROTOCOL-V4, WORKER-BRIEF, UPDATE-07CA8585 | creates 1 | — | arms-runner-smoke |
| campaign-launcher | Launch the batch decoder campaigns resumably | simulation | One resumable launcher runs each protocol family of the story | measurement-authority, measured-arms | REQ-01, MEASUREMENT-CONTRACT, PROTOCOL-V4, WORKER-BRIEF, UPDATE-07CA8585 | creates 1 | — | arms-runner-smoke |
| plan-smoke-runner | Smoke a family plan through the shared runner | simulation | One smoke runner proves each family plan's wire contract | measurement-authority, measured-arms | REQ-01, MEASUREMENT-CONTRACT, PROTOCOL-V4, WORKER-BRIEF | creates 1 | — | addendum-generator |
| single-worker-family-preparation | Freeze the single-worker before-after family | simulation | Single-worker before-after pilot family frozen with its queue line | measurement-authority, measured-arms | REQ-01, REQ-10, MEASUREMENT-CONTRACT, PROTOCOL-V4, WORKER-BRIEF | creates 2, touches 1 | — | campaign-launcher, plan-smoke-runner |
| multicore-family-preparation | Freeze the multicore before-after family | simulation | Multicore before-after pilot family frozen with its queue line | measurement-authority, measured-arms | REQ-01, REQ-10, MEASUREMENT-CONTRACT, PROTOCOL-V4, WORKER-BRIEF | creates 2, touches 1 | — | campaign-launcher, plan-smoke-runner |
| batch-sweep-family-preparation | Freeze the batch-size sweep family | simulation | Exploratory batch-size sweep family frozen with its queue line | measurement-authority, measured-arms | REQ-01, REQ-10, MEASUREMENT-CONTRACT, PROTOCOL-V4, WORKER-BRIEF | creates 2, touches 1 | — | campaign-launcher, plan-smoke-runner |
| matched-family-preparation | Freeze the matched AFF3CT family | simulation | Matched AFF3CT pilot family frozen with its queue line | measurement-authority, measured-arms, comparison-arms | REQ-01, REQ-10, MEASUREMENT-CONTRACT, PROTOCOL-V4, WORKER-BRIEF | creates 2, touches 1 | — | campaign-launcher, plan-smoke-runner |
| fastest-compatible-family-preparation | Freeze the fastest-compatible AFF3CT family | simulation | Exploratory fastest-compatible AFF3CT family frozen with its queue line | measurement-authority, measured-arms, comparison-arms | REQ-01, REQ-10, MEASUREMENT-CONTRACT, PROTOCOL-V4, WORKER-BRIEF | creates 2, touches 1 | — | campaign-launcher, plan-smoke-runner |
| single-worker-pilot-collection | Collect the single-worker before-after pilot | simulation | Accepted pilot receipt with the confirmation addendum frozen for single-worker before-after protocol family | measurement-authority | REQ-01, REQ-10, MEASUREMENT-CONTRACT, PROTOCOL-V4, WORKER-BRIEF | creates 2, touches 1 | — | single-worker-family-preparation |
| multicore-pilot-collection | Collect the multicore before-after pilot | simulation | Accepted pilot receipt with the confirmation addendum frozen for multicore before-after protocol family | measurement-authority | REQ-01, REQ-10, MEASUREMENT-CONTRACT, PROTOCOL-V4, WORKER-BRIEF | creates 2, touches 1 | — | multicore-family-preparation |
| batch-sweep-pilot-collection | Collect the batch-size sweep pilot | simulation | Accepted exploratory pilot receipt with its outcome recorded for batch-size sweep protocol family | measurement-authority | REQ-01, REQ-10, MEASUREMENT-CONTRACT, PROTOCOL-V4, WORKER-BRIEF | creates 1 | — | batch-sweep-family-preparation |
| matched-pilot-collection | Collect the matched AFF3CT pilot | simulation | Accepted pilot receipt with the confirmation addendum frozen for matched AFF3CT protocol family | measurement-authority, comparison-arms | REQ-01, REQ-10, MEASUREMENT-CONTRACT, PROTOCOL-V4, WORKER-BRIEF | creates 2, touches 1 | — | matched-family-preparation |
| fastest-compatible-pilot-collection | Collect the fastest-compatible AFF3CT pilot | simulation | Accepted exploratory pilot receipt with its outcome recorded for fastest-compatible AFF3CT protocol family | measurement-authority, comparison-arms | REQ-01, REQ-10, MEASUREMENT-CONTRACT, PROTOCOL-V4, WORKER-BRIEF | creates 1 | — | fastest-compatible-family-preparation |
| single-worker-confirmation-collection | Collect the single-worker before-after confirmation | simulation | Confirmation receipt accepted with outcomes recorded as evaluated for single-worker before-after protocol family | measurement-authority | REQ-01, REQ-10, MEASUREMENT-CONTRACT, PROTOCOL-V4, WORKER-BRIEF | creates 1 | — | single-worker-pilot-collection |
| multicore-confirmation-collection | Collect the multicore before-after confirmation | simulation | Confirmation receipt accepted with outcomes recorded as evaluated for multicore before-after protocol family | measurement-authority | REQ-01, REQ-10, MEASUREMENT-CONTRACT, PROTOCOL-V4, WORKER-BRIEF | creates 1 | — | multicore-pilot-collection |
| matched-confirmation-collection | Collect the matched AFF3CT confirmation | simulation | Confirmation receipt accepted with outcomes recorded as evaluated for matched AFF3CT protocol family | measurement-authority, comparison-arms | REQ-01, REQ-10, MEASUREMENT-CONTRACT, PROTOCOL-V4, WORKER-BRIEF | creates 1 | — | matched-pilot-collection |
| coding-tuning-producer | Produce decode selector measurements from a coding owner harness | task | Coding owner producer measures the decode selector fields under the offline tuning protocol | tuning-owner-mechanism, decode-selectors, ldpc-lane-decoder, batch-soft-decoder-trait | REQ-13, CALIBRATION-PLAN-63BAD95D, SOURCE-EVIDENCE | creates 2, touches 1 | — | batch-decoder-avx2-route, batch-worker-pool |
| profile-composer-coding-owner | Emit a coding owner envelope from the profile composer | task | Profile composer writes a coding owner envelope with a three-section complete envelope | tuning-owner-mechanism | REQ-13, CALIBRATION-PLAN-63BAD95D, SOURCE-EVIDENCE | touches 1 | — | coding-tuning-producer |
| campaign-driver-coding-owner | Admit gf2-coding as an owner in the tuning campaign driver | task | Campaign driver handles a measured coding owner beside imported owners | tuning-owner-mechanism | REQ-13, CALIBRATION-PLAN-63BAD95D, SOURCE-EVIDENCE | touches 2 | — | profile-composer-coding-owner |
| campaign-validator-coding-owner | Accept a coding owner campaign in the tuning campaign validator | task | Independent validator accepts a coding owner campaign stage | tuning-owner-mechanism | REQ-13, CALIBRATION-PLAN-63BAD95D, SOURCE-EVIDENCE | touches 1 | — | campaign-driver-coding-owner |
| decoder-calibration-preparation | Declare the decoder calibration campaign | simulation | Decoder calibration campaign declared with protocol amendment, manifest and queue line | tuning-owner-mechanism, measurement-authority | REQ-01, REQ-13, CALIBRATION-PLAN-63BAD95D, MEASUREMENT-CONTRACT, WORKER-BRIEF | creates 3, touches 1 | — | campaign-validator-coding-owner |
| decoder-calibration-collection | Commit the calibrated decode profile | simulation | Coding owner envelope with calibrated decode selectors committed with its receipt | tuning-owner-mechanism, measurement-authority, decode-selectors | REQ-01, REQ-13, CALIBRATION-PLAN-63BAD95D, MEASUREMENT-CONTRACT, WORKER-BRIEF | creates 1, touches 1 | — | decoder-calibration-preparation |
| tuned-profile-arm | Build the tuned-profile benchmark arm of the batch decoder | simulation | Tuned-profile gf2 arm that takes its selectors from the installed envelope | measurement-authority, calibrated-decode-profile, measured-arms | REQ-13, CALIBRATION-PLAN-63BAD95D, MEASUREMENT-CONTRACT, PROTOCOL-V4 | touches 3 | — | decoder-calibration-collection, lane-profile-executable |
| selector-holdout-preparation | Freeze the decode selector holdout family | simulation | Selector holdout family frozen with holdout declaration, smoke record and queue line | measurement-authority, calibrated-decode-profile, measured-arms, tuning-owner-mechanism | REQ-01, REQ-13, CALIBRATION-PLAN-63BAD95D, MEASUREMENT-CONTRACT, PROTOCOL-V4, WORKER-BRIEF | creates 3, touches 1 | — | tuned-profile-arm, campaign-launcher, plan-smoke-runner |
| selector-holdout-pilot-collection | Collect the decode selector holdout pilot | simulation | Accepted holdout pilot receipt with the confirmation addendum frozen | measurement-authority, calibrated-decode-profile | REQ-01, REQ-13, MEASUREMENT-CONTRACT, PROTOCOL-V4, WORKER-BRIEF | creates 1, touches 2 | — | selector-holdout-preparation |
| selector-holdout-confirmation-collection | Collect the decode selector holdout confirmation | simulation | Holdout confirmation receipt accepted with the profile's outcome recorded as evaluated | measurement-authority, calibrated-decode-profile | REQ-01, REQ-13, MEASUREMENT-CONTRACT, PROTOCOL-V4, WORKER-BRIEF | touches 1 | — | selector-holdout-pilot-collection |
| decoder-dispatch-verification | Verify decoder dispatch under the committed profile | task | Production decode routes observed under the committed, conservative and fallback configurations | calibrated-decode-profile, decode-selectors, ldpc-lane-decoder | REQ-13, CALIBRATION-PLAN-63BAD95D | creates 2, touches 1 | — | decoder-calibration-collection |
| batch-decoding-reference-pages | Describe batch LDPC decoding in the permanent pages | task | Permanent pages state batch decoding routes, selectors and supported configurations | batch-soft-decoder-trait, ldpc-lane-decoder, decode-selectors | REQ-08, REQ-13, SOURCE-EVIDENCE | touches 3 | — | decoder-dispatch-verification, decode-batch-howto-page |
| lane-profile-executable | Build the profiling executable of the batch decoder | simulation | Profiling executable of the LDPC batch decoder with a recorded digest | measurement-authority, measured-arms | REQ-01, PROFILE-3BE770D5, MEASUREMENT-CONTRACT | touches 2 | — | arms-runner-smoke |
| profile-summarizer-lane-rules | Categorize lane kernel samples in the profile summarizer | simulation | Profile summarizer categorizes lane kernel and transposition samples | measurement-authority | REQ-01, PROFILE-3BE770D5, UPDATE-07CA8585 | touches 1 | — | lane-profile-executable |
| lane-profile-launcher | Launch the profile series of the batch decoder | simulation | Resumable launcher of the batch decoder profile series | measurement-authority | REQ-01, PROFILE-3BE770D5, UPDATE-07CA8585, WORKER-BRIEF | creates 1 | — | lane-profile-executable |
| lane-profile-preparation | Prepare the profile series of the batch decoder | simulation | Profile series of the batch decoder prepared with its queue line | measurement-authority | REQ-01, PROFILE-3BE770D5, MEASUREMENT-CONTRACT, WORKER-BRIEF | creates 1, touches 1 | — | lane-profile-launcher |
| lane-profile-collection | Summarize the profile series of the batch decoder | simulation | Generated profile summary of the batch decoder from the measured series | measurement-authority | REQ-01, PROFILE-3BE770D5, MEASUREMENT-CONTRACT, WORKER-BRIEF | creates 1 | — | lane-profile-preparation, profile-summarizer-lane-rules |
| outcome-tables | Generate the outcome tables of the inter-frame decoder | simulation | Generated outcome tables from the committed evidence | measurement-authority, comparison-arms | REQ-01, REQ-10, MEASUREMENT-CONTRACT, WORKER-BRIEF, COMPARATOR-C077A88B | creates 2 | — | single-worker-confirmation-collection, multicore-confirmation-collection, batch-sweep-pilot-collection, matched-confirmation-collection, fastest-compatible-pilot-collection, selector-holdout-confirmation-collection, lane-profile-collection, arms-peak-memory-record |
| outcome-publication | Report the inter-frame decoder outcomes with their limits | task | Outcome record with criterion status and residual limits | measurement-authority, comparison-arms, float-numerical-contract | REQ-01, REQ-10, REQ-11, REQ-12, MEASUREMENT-CONTRACT, WORKER-BRIEF, DECISION-RECORD-F63A2464, COMPARATOR-C077A88B, PROFILE-3BE770D5 | creates 1 | — | outcome-tables, batch-decoding-reference-pages, sim-batch-worker-determinism, sim-batch-resume-identity, sim-bler-sweep-batch, decode-batch-removal |

```mermaid
flowchart LR
    N0["decoder-numerical-contract-page: State the LDPC decoder numerical contract with its batch clauses"]
    N1["lane-kernel-portable: Provide the portable lane kernels of the min-sum decoder"]
    N2["decode-selector-family: Carry decoder selectors in the coding tuning section"]
    N3["decode-route-decision: Decide the LDPC batch route from the decode selectors"]
    N4["batch-soft-decoder-trait: Define the batch soft-decoder trait with its default paths"]
    N5["batch-decoder-contract-suite: Assert the batch decoder contract over the existing soft decoders"]
    N6["ldpc-lane-batch-decoder: Decode LDPC frame batches in lanes behind the batch decoder trait"]
    N7["batch-conformance-suite: Assert LDPC batch decoding at the posterior level across codes"]
    N8["lane-kernel-avx2: Vectorize the lane kernels with AVX2"]
    N9["batch-decoder-avx2-route: Route LDPC batch decoding to the AVX2 lanes by capability"]
    N10["batch-worker-pool: Decode batches across workers through one pooled batch decoder"]
    N11["decode-batch-test-callers: Move the tests of the inherent LDPC batch functions to the batch trait"]
    N12["decode-batch-bench-callers: Move the benchmarks of the inherent LDPC batch functions to the batch trait"]
    N13["decode-batch-howto-page: Show parallel LDPC decoding through the batch trait in the how-to page"]
    N14["decode-batch-removal: Remove the inherent LDPC batch functions"]
    N15["nr-rate-matched-batch-decode: Decode rate-matched 5G NR frame batches in lanes"]
    N16["dvb-t2-concat-batch-decode: Decode DVB-T2 concatenated frame batches in lanes"]
    N17["sim-batch-decode-stage: Provide a simulation decode stage over the batch decoder trait"]
    N18["sim-cpu-ldpc-stage-batch: Decode CPU LDPC stage batches through the batch decode stage"]
    N19["sim-nr-decode-stage-batch: Decode NR stage batches through the batch decode stage"]
    N20["sim-dvb-t2-stage-batch: Decode DVB-T2 stage batches through the batch decode stage"]
    N21["sim-bler-sweep-batch: Decode BLER sweep slices through the batch decoder trait"]
    N22["sim-batch-worker-determinism: Show worker-count determinism of batch-decoding pipelines"]
    N23["sim-batch-resume-identity: Show checkpoint resume identity of batch-decoding campaigns"]
    N24["gf2-benchmark-arms: Build the gf2 batch decoder benchmark arms"]
    N25["aff3ct-benchmark-arms: Provide the AFF3CT benchmark arms of the batch decoder comparison"]
    N26["intra-arm-quality-record: Prepare the quality record of the sum-product INTRA arm"]
    N27["arms-build-identity: Record the build identity of the benchmark arms"]
    N28["arms-quality-validation: Validate the benchmark arms against their quality records"]
    N29["arms-peak-memory-record: Record the peak memory of the benchmark arms"]
    N30["arms-runner-smoke: Smoke the benchmark arms through the shared runner"]
    N31["addendum-generator: Generate family addenda with their plans from family declarations"]
    N32["campaign-launcher: Launch the batch decoder campaigns resumably"]
    N33["plan-smoke-runner: Smoke a family plan through the shared runner"]
    N34["single-worker-family-preparation: Freeze the single-worker before-after family"]
    N35["multicore-family-preparation: Freeze the multicore before-after family"]
    N36["batch-sweep-family-preparation: Freeze the batch-size sweep family"]
    N37["matched-family-preparation: Freeze the matched AFF3CT family"]
    N38["fastest-compatible-family-preparation: Freeze the fastest-compatible AFF3CT family"]
    N39["single-worker-pilot-collection: Collect the single-worker before-after pilot"]
    N40["multicore-pilot-collection: Collect the multicore before-after pilot"]
    N41["batch-sweep-pilot-collection: Collect the batch-size sweep pilot"]
    N42["matched-pilot-collection: Collect the matched AFF3CT pilot"]
    N43["fastest-compatible-pilot-collection: Collect the fastest-compatible AFF3CT pilot"]
    N44["single-worker-confirmation-collection: Collect the single-worker before-after confirmation"]
    N45["multicore-confirmation-collection: Collect the multicore before-after confirmation"]
    N46["matched-confirmation-collection: Collect the matched AFF3CT confirmation"]
    N47["coding-tuning-producer: Produce decode selector measurements from a coding owner harness"]
    N48["profile-composer-coding-owner: Emit a coding owner envelope from the profile composer"]
    N49["campaign-driver-coding-owner: Admit gf2-coding as an owner in the tuning campaign driver"]
    N50["campaign-validator-coding-owner: Accept a coding owner campaign in the tuning campaign validator"]
    N51["decoder-calibration-preparation: Declare the decoder calibration campaign"]
    N52["decoder-calibration-collection: Commit the calibrated decode profile"]
    N53["tuned-profile-arm: Build the tuned-profile benchmark arm of the batch decoder"]
    N54["selector-holdout-preparation: Freeze the decode selector holdout family"]
    N55["selector-holdout-pilot-collection: Collect the decode selector holdout pilot"]
    N56["selector-holdout-confirmation-collection: Collect the decode selector holdout confirmation"]
    N57["decoder-dispatch-verification: Verify decoder dispatch under the committed profile"]
    N58["batch-decoding-reference-pages: Describe batch LDPC decoding in the permanent pages"]
    N59["lane-profile-executable: Build the profiling executable of the batch decoder"]
    N60["profile-summarizer-lane-rules: Categorize lane kernel samples in the profile summarizer"]
    N61["lane-profile-launcher: Launch the profile series of the batch decoder"]
    N62["lane-profile-preparation: Prepare the profile series of the batch decoder"]
    N63["lane-profile-collection: Summarize the profile series of the batch decoder"]
    N64["outcome-tables: Generate the outcome tables of the inter-frame decoder"]
    N65["outcome-publication: Report the inter-frame decoder outcomes with their limits"]
    N0 --> N1
    N2 --> N3
    N4 --> N5
    N1 --> N6
    N3 --> N6
    N5 --> N6
    N6 --> N7
    N7 --> N8
    N8 --> N9
    N5 --> N10
    N2 --> N10
    N9 --> N11
    N10 --> N11
    N9 --> N12
    N10 --> N12
    N9 --> N13
    N10 --> N13
    N11 --> N14
    N12 --> N14
    N13 --> N14
    N7 --> N15
    N7 --> N16
    N4 --> N17
    N17 --> N18
    N9 --> N18
    N17 --> N19
    N15 --> N19
    N9 --> N19
    N17 --> N20
    N16 --> N20
    N9 --> N20
    N9 --> N21
    N18 --> N22
    N19 --> N22
    N20 --> N22
    N18 --> N23
    N19 --> N23
    N20 --> N23
    N9 --> N24
    N24 --> N25
    N25 --> N26
    N25 --> N27
    N27 --> N28
    N26 --> N28
    N27 --> N29
    N28 --> N30
    N30 --> N31
    N30 --> N32
    N31 --> N33
    N32 --> N34
    N33 --> N34
    N32 --> N35
    N33 --> N35
    N32 --> N36
    N33 --> N36
    N32 --> N37
    N33 --> N37
    N32 --> N38
    N33 --> N38
    N34 --> N39
    N35 --> N40
    N36 --> N41
    N37 --> N42
    N38 --> N43
    N39 --> N44
    N40 --> N45
    N42 --> N46
    N9 --> N47
    N10 --> N47
    N47 --> N48
    N48 --> N49
    N49 --> N50
    N50 --> N51
    N51 --> N52
    N52 --> N53
    N59 --> N53
    N53 --> N54
    N32 --> N54
    N33 --> N54
    N54 --> N55
    N55 --> N56
    N52 --> N57
    N57 --> N58
    N13 --> N58
    N30 --> N59
    N59 --> N60
    N59 --> N61
    N61 --> N62
    N62 --> N63
    N60 --> N63
    N44 --> N64
    N45 --> N64
    N41 --> N64
    N46 --> N64
    N43 --> N64
    N56 --> N64
    N63 --> N64
    N29 --> N64
    N64 --> N65
    N58 --> N65
    N22 --> N65
    N23 --> N65
    N21 --> N65
    N14 --> N65
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
| DEC-07 batch API | The batch decode API is a general trait of `gf2-coding`, not an LDPC type: contract `batch-soft-decoder-trait`. The LDPC lane decoder is its first specialised implementer, `gf2-sim` stages consume the trait, and `LdpcDecoder::decode_batch` and `decode_batch_with_config` are removed with their callers moved (ledger topic `decode-batch-callers`). Rejected an LDPC-only batch type, which leaves every other soft decoder and every stage with a private loop. |
| DEC-11 default batch paths | Two paths: every `SoftDecoder` decodes batches through a per-frame loop over `decode_soft_with_result`, and iterative batch decoding goes through the adapter with an iteration cap or a specialised implementer. For the LDPC and GLDPC decoders the first path is a hard decision; the rustdoc and the reference page state it and the contract tests assert it. Rejected a trait tied to `IterativeSoftDecoder`, which excludes OSD and ORBGRAND. |
| DEC-12 decoded words | The trait writes decoded words of an implementer-reported length; the LDPC implementer chooses codewords or message bits at construction. Rejected a codeword-only trait, which changes what OSD, ORBGRAND and GLDPC return. |
| DEC-08 wave policy | Fixed waves: a wave lasts until its slowest lane terminates, the wave semantics of AFF3CT's INTER modes [Cassagne2019]. Rejected refilling a terminated lane, which adds a per-lane restart path; the batch-size sweep and the profile series measure what the fixed policy costs. |
| DEC-09 calibration mechanism | `gf2-coding` is the third owner of the offline tuning campaign: its producer, the driver, the composer and the validator. Rejected calibration under the Zen 3 protocol alone, which leaves the coding envelope without the campaign's validator. |
| DEC-10 QC comparison | The outcome record sets inter-frame and QC-aware intra-frame cells against their common canonical baseline from committed receipts, as a descriptive comparison. Rejected a paired cell with the QC prototype arm, which measures again a family the frozen budget closed. |
| Risk: workspace size | A wave holds eight frames of messages per worker, so the lane route can leave the cache where the single-frame route does not. `lane_max_edges` closes the lane route by code size, the multicore family and the profile counters measure it, and workspace bytes are reported. |
| Risk: slowest lane | Under fixed waves a nonconverging frame holds its wave to the iteration cap. The effect is measured, not predicted; DEC-08 names the alternative. |
| Risk: default path semantics | The default batch path of an iterative decoder is its non-iterative `decode_soft`. The trait's rustdoc and the reference pages state it, and the adapter with an iteration cap is the iterative path. |
| Risk: campaign tooling | The third owner changes a driver, a composer and a validator that pin committed campaigns. Each entry requires the committed two-owner campaigns to validate unchanged. |
| Risk: window capacity | The story queues pilots and confirmations of four confirmable families, two exploratory families, one calibration campaign and one profile series; collection entries report which runs are measured and which wait. |
| Re-homed dependencies | None. The planning node's one dependency, `f63a2464`, is complete. |

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
