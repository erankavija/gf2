# Compatible LDPC decoder benchmark arms on Zen 3

> **Diátaxis Type:** Explanation

Survey for `c077a88b`. No production kernel changes are included. The
[generated tables](../../bench_results/c077a88b/tables.md) are the numerical
projection of the quality reports and finalized campaign receipts. The
[measurement contract](../1a379447-zen3-cpu-performance/measurement-contract.md)
and [frozen protocol](../f547c394/protocol.md) govern both families.

## Question and method

The survey compares the gf2 f32 flooding normalized-min-sum decoder with
openly accessible implementations on the DVB-T2 normal rate-1/2 code and
NR BG1 lifting-384 **mother code**. Matched-algorithm and quality-compatible
comparisons have separate addenda, pilots, confirmations and tables.

The input manifests pin the parity-check matrices, recorded little-endian
f32 LLRs and transmitted codewords by SHA-256. The input generator is
`survey/harness/src/bin/ldpc-make-inputs.rs`; the source and exact recorded
bytes are in the preparation evidence's `source-inputs/` archives. Each
bundle contains all-zero and encoded random messages, and the generator
checks every codeword against the parity-check matrix. Input noise settings
are experimental choices recorded in the manifests, not inferred performance
or claims about a universal waterfall threshold.

Both arms score codeword positions $0 \ldots K-1$. AFF3CT's codec uses this
information window when no encoder supplies positions. The C ABI adapter
uses that same mapping explicitly. The validator compares the actual decoded
bytes across arms and reports agreement with Wilson intervals. Nonconverged
frames and fixed-point disagreements remain in the evidence.

## Pinned arms and upstream status

Exact revisions, recursive submodule pins, compiler flags, executable hashes,
and runtime AFF3CT version output are in
[build-identity.json](../../bench_results/c077a88b/v3-preparation/build-identity.json).
[Upstream observations](../../bench_results/c077a88b/2026-09-08-c077a88b-preparation/upstream-status.json)
distinguish a pinned historical release from a maintained upstream. External
project keys are [Cassagne2019], [Srsran2026], [Xdsopl2026] and
[OpenAirInterface2026]; these resolve in the repository citation registry.

| Project | Pin and license | Survey disposition |
|---|---|---|
| AFF3CT | v4.7.0; MIT | Measured shortlist: flooding NMS f32; horizontal-layered NMS f32 scalar, f32 INTER and i16 INTER. |
| srsRAN Project | release_25_10; AGPL-3.0 | Archived historical source. Upstream names OCUDU as its maintained successor. Mother-code input and syndrome stopping are not expressible through the pinned API. The preserved configuration attempt fails on missing MbedTLS; no successful library build is claimed. |
| xdsopl/LDPC | Pinned master revision in build identity; 0BSD | DVB parity-check identity is validated. Source is available; no successor notice appears on the inspected upstream page. Matched NMS is not expressible; MS/OMS/corrected-MS remain unmeasured quality-compatible possibilities. |
| OpenAirInterface | 2026.w36; Collaborative Standards Software License v1.0 | Source available. System CMake configuration detects AVX2 and disables AVX512, then fails on missing SCTP development dependencies. No decoder binary or timing result is claimed. |

OpenAirInterface's license file is preserved with its source. It contains a
research-limited patent grant and a separate FRAND provision; this survey
records the source terms rather than classifying it as an unrestricted
open-source license. OAI remains a source-available, unavailable-to-measure
entry. Closed-source decoders are excluded. The local network cannot resolve
GitHub's API hostname; the web connector supplies the maintained-upstream
check. Dynamic OAI release history is not independently verified as latest.

## What AFF3CT actually compiled and selected

The observed AFF3CT executable reports **AVX2**, multiple precisions and
version v4.7.0. The actual compile record contains `-std=gnu++11`, `-O3`,
`-march=native` and `-funroll-loops`, together with its ABI definitions.
The shim compiles against these definitions and obtains its backend name
from `mipp::InstructionFullType`. Every validation row records the decoder
class, observed backend and native wave size; these are not inferred from
CMake's requested flags. The Rust release arms use the toolchain and native
flags in the build record; a separate Rust 1.95 release check passes.

[The capability screen](../../bench_results/c077a88b/2026-09-08-c077a88b-preparation/capabilities-nr.json)
retains the command, output and outcome for each schedule, update rule,
precision and SIMD strategy it tries. It is bounded by frame count and wall
time and never holds the timing mutex. Its own simulator throughput is
exploratory screening only, not a paired gf2 comparison.

The pinned factory guards flooding and vertical-layered INTER with
`__cpp_aligned_new`, which this C++11 build does not provide. **NMS flooding
INTER and NMS INTRA are unavailable in this build.** Horizontal-layered
INTER supports f32 and i16 and rejects i8. Float SPA INTRA **is supported**;
it has a different update rule and is not a matched NMS arm. The screen
also preserves supported scalar vertical-layered and fixed-point modes.

The timed shortlist follows the capability screen: horizontal-layered NMS
INTER is the leading supported screen configuration in each admitted precision;
the screen retains its raw throughput and quality counts. Scalar horizontal
NMS is the within-schedule reference. These screen measurements select
candidates only; their simulator inputs and timing are not the paired
recorded-input campaign. The measured shortlist tests horizontal-layered f32
scalar against both supported INTER precisions. This compares actual implementation candidates
without assuming the widest batch wins. SPA INTRA, vertical-layered and
other update rules are screened, not asserted to be globally slower across
all workloads or parameter choices. No absolute optimum is claimed.

## Mathematical and adapter contracts

| Concern | AFF3CT and gf2 cells | Other surveyed interfaces |
|---|---|---|
| Matrix and input | Identical digested AList and LLR bundles; full mother-code LLRs for NR. | xdsopl DVB accumulator table reconstructs the identical matrix. srsRAN accepts the shortened NR block and reconstructs the punctured prefix. OAI prefix input mapping is unvalidated; the copy loop does accept caller prefix values. |
| Precision and normalization | Matched: f32 NMS factor 0.75. Quality arms label f32/i16 and horizontal layering. i16 adapter scales LLRs by 16, rounds with `floor(x + 0.5)` and clips to its signed limit. | srsRAN hard-codes NMS factor 0.8 and int8 LLRs. xdsopl exposes MS, OMS and corrected MS rather than NMS. OAI uses clipped int8 two-pass min-sum check-node processing. |
| Iterations and stopping | Cap 50, syndrome depth 1; the first passing iteration terminates. INTER stops a native wave according to AFF3CT's packed syndrome decision. | srsRAN exposes CRC termination or no early stop, not syndrome termination. OAI selects parity-check termination or its CRC callback; its counter follows an initial uncounted iteration. These contracts cannot enter the matched table unchanged. |
| Puncturing and rate matching | Both frozen cells have zero punctured prefix. No rate-matched transmission is represented. | A punctured NR cell would be a different recorded-input experiment. Discarding nonzero mother-code LLRs would change this operation and is not a validated adapter for it. |
| Fillers | Neither recorded cell contains fillers. Filler semantics are untested and not expressible as a result of these cells. | srsRAN's filler count and OAI's codeblock dimensions need a separately recorded shortened/punctured configuration. |
| Output | Exactly the information window, one 0/1 byte per bit. | xdsopl separates data and parity arrays. srsRAN packs information bits; OAI output ordering is not runtime-validated because its arm is unavailable. |

The [source corrections](../../bench_results/c077a88b/2026-09-08-c077a88b-preparation/source-corrections.json) preserve counterevidence to the earlier OAI puncturing inference. Its copy loop accepts caller-supplied prefix LLRs: mother-code input is unvalidated, not proven impossible. OAI is unavailable here because its build fails, and no adapter correctness is claimed.

Source locations for these claims are recorded in the committed
[source-evidence report](../../bench_results/c077a88b/2026-09-07-c077a88b-adapter-validation/source-evidence.json)
and can be read in the pinned source archives. The
[xdsopl code identity evidence](../../bench_results/c077a88b/2026-09-07-c077a88b-adapter-validation/xdsopl-code-identity.json)
compares adjacency sets, including the parity staircase orientation.

The fixed-point DVB arm's additional frame failures contradict the premise
that the nominal widest supported mode is automatically an admissible fastest
arm. The failed quality comparison is preserved, not removed by changing
normalization or the tolerance after measurement. The NR result is assessed
separately. Quality tolerance is frozen in each family addendum.

## Timing, batching and quality accounting

Matched calls decode one frame. Quality-compatible calls process the same
sixteen-frame batch in both arms: gf2 serially, AFF3CT through native scalar,
eight-frame or sixteen-frame waves. The declared warm cache policy fixes fixture bank zero: the first recorded
frame for matched latency, and the first sixteen recorded frames (both
codeword classes) for batch throughput. The full bundles supply quality
validation. A warm pass traverses the same timed working set, and the
canonical timing library calibrates and records the protocol's five windows.
No timing window equates a multi-frame wave with one gf2 frame.

Each call includes decoder construction and destruction, input conversion,
dispatch and output extraction. Both arms read the identical recorded AList and construct their native sparse
matrix and decoder within every call. Neither initializes an encoder.
This is a whole-call adapter boundary. It
does not establish steady-state decoder throughput or a kernel speedup; a
reused-decoder comparison is unmeasured.
LLR bundle loading, digest verification and quality simulation are outside timing;
AList loading is inside timing.
AFF3CT conversion and wave reordering occur inside decode calls; separately
reported zero pack/unpack costs mean no *additional* charge outside those
windows, not that conversion takes zero time. All timed INTER batches are
fully filled from already recorded frames. No arrival process is modelled:
batch queueing delay is therefore unmeasured, not declared zero for a live link.

Quality is a single deterministic replay of each full recorded bundle,
prepared under the CPU budget on one CPU without either side of the timing
mutex. Timed children read those exact quality objects. Fresh timing processes
do not constitute new BER/FER samples. Counts, iteration distributions and process RSS appear in the generated quality
table. FER uses Wilson intervals. BER uses the protocol's Hoeffding interval on
independent frame error fractions, allowing arbitrary bit dependence within a
frame. Quality admission uses the protocol's paired same-frame FER bound with
its ledger-derived comparison confidence. Each prepared report retains the exact
ordered per-frame error vector; repeated timing executions do not add samples.

Flooding iteration counts come from a virtual hook delegating to AFF3CT's
own iteration implementation. Layered counts are **upper bounds on a ladder**
of iteration caps. The adapter checks that ladder in increasing order without
assuming a monotone convergence predicate. INTER counts apply to a native
wave; they are not compared directly with scalar per-frame counts. Process
RSS includes runtime and observation allocations; it is not an isolated
measurement of decoder-owned memory. Untimed validation latency is diagnostic
only. The performance tables use exclusive-lock timing windows.

## Evidence lifecycle and reproducibility

`survey/snapshot-inputs.py` constructs the family producing manifest over source,
build inputs, data archives and prepared quality. The typed `producing_manifest`
field in the saved runner plan selects that manifest. The shared runner captures
its exact content through `ProducingInputs`; `CampaignFacts` checks the plan
selection against the opening snapshot. Omission selects the historical shared
manifest. No environment override selects producing provenance.

Protocol-v3 fixtures preserve exact decoder frame vectors, settings, aggregate
counts, intervals and iteration evidence across child executions. Process RSS
and untimed latency may vary; each child's diagnostics remain in its checkpoint.
The [red/green evidence](../../bench_results/c077a88b/v3-preparation/shared-contract-red.log)
and full shared suite exercise both contracts. These changes affect measurement
tooling and survey adapters; they change no production decoder behavior.

The [immutable v1 index](../../bench_results/c077a88b/superseded-v1-evidence.json)
pins every imported historical evidence file by SHA-256. V1 acceptance summaries
remain historical evidence, not v3 claims. Earlier missing-quality, asymmetric
setup and encoder-construction failures remain visible, together with the r4
recorded-AList pilots. None supplies v3 resolution evidence.

The canonical matched and quality-family ledgers retain retrospective entries
for the v1 exploratory campaigns, including the interrupted attempt. Their
origin records cite exact plans, addenda and available receipts. These entries
claim no retrospective premeasurement lock acquisition and spend zero
confirmatory comparisons. V3 campaigns reserve their own append-only entries
before measurement and freeze receipt-local ledger prefixes.

[Reproduction and resume commands](resume-confirmation.md) separate preparation,
serialized full-host sessions, finalization and independent acceptance. The
freeze script derives the conservative widest pilot relative interval half-width
from the independently accepted v3 pilot and pins its exact receipt, addendum and
ledger snapshots. Confirmations retain the frozen corpus, tolerance and all six
quality candidates. A P-19 note does not structurally prohibit measuring them;
it prevents a quality-admissible performance conclusion.

## Criterion status

The fresh v3 preparation replays the recorded corpus with exact per-frame vectors.
The corpus cannot establish the frozen quality non-inferiority claim under v3:
the paired confidence bound remains too wide even for candidates with identical
observed frame failures. This is insufficient quality evidence, not proof that
all candidates have worse FER. The DVB fixed-point excess failures remain a
separate observed negative result. No tolerance or corpus change masks either
finding.

V3 pilot and confirmation publication is in progress. The generated tables
project finalized receipts only; no unmeasured confirmation is claimed.

| Criterion | Status | Evidence or remaining work |
|---|---|---|
| REQ-01 | PARTIAL | Rust 1.95 release arms, immutable source/input pins and tested v3 contracts; v3 timing and publication pending. No production kernel change requires before/after measurement. |
| REQ-02 | PARTIAL | Matched f32 NMS and supported layered f32 SIMD arms validated; v3 confirmation pending. Matched NMS SIMD remains unavailable in the pinned C++11 build. |
| REQ-03 | PARTIAL | Capability screen and full-corpus per-frame quality evidence retained. V3 cannot certify quality admission on this corpus; predeclared candidate timing remains pending. |
| REQ-04 | MET | Pinned sources, maintained-upstream observations, xdsopl matrix identity and preserved OAI/srsRAN build failures. |
| REQ-05 | MET | Validated mother-code and information-window adapters; explicit unexpressible or unverified puncturing, filler, rate-matching and stopping contracts. |
| REQ-06 | PARTIAL | Adapter evidence and immutable v1 history published in the worker changes; fresh v3 pilots and confirmation remain pending. |

## Limits and follow-up

The library decoder is a single-worker consumer in this survey. Six/twelve
physical-core and 24-logical-CPU orchestration are outside these single-instance
cells; no multi-worker scaling is claimed. Timing over the full quality corpus, partial INTER batches, live batch
arrival delay, punctured/filler NR inputs, OCUDU, and xdsopl decoding throughput
are unmeasured. Screened configurations do not establish an exhaustive optimum.
The falsifiable allocation/edge-traversal explanation is tracked by `07ca8585`;
quantized/layered and QC-aware alternatives by `f63a2464`; inter-frame
vectorization by `ed3d490e`. This survey supplies comparator evidence for
those investigations and does not treat an unprofiled hypothesis as attribution.
Production kernel changes and adoption decisions belong to that implementation work.
