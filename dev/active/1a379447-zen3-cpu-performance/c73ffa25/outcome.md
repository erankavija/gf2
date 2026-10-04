# Dense-parity baseline and candidate outcome (jit:c73ffa25)

> **Diátaxis Type:** Research

## Baseline and profile evidence

The [isolated fused-parity receipt](../../../bench_results/2037941f/2037941f-dense-isolated-fused-parity/v4-r1-pilot/receipt.json)
and [allocated matvec receipt](../../../bench_results/2037941f/2037941f-dense-allocated-matvec/v4-r1-pilot/receipt.json)
contain frozen inputs, source and executable identities, raw paired samples,
commands, runtime provenance, and append-only logs. Their
[isolated](../../../bench_results/2037941f/2037941f-dense-isolated-fused-parity/v4-r1-pilot/acceptance-summary.md)
and [allocated](../../../bench_results/2037941f/2037941f-dense-allocated-matvec/v4-r1-pilot/acceptance-summary.md)
acceptance summaries record accepted exploratory identity comparisons, with
each cell's interval and decision at its declared sample count. The allocated
summary also preserves the scalar-reference outcomes. The
[release validation](../../2037941f-profile-and-optimize-mid-range-buffer-operations/survey/dense-baseline-validation.txt)
and [shared smoke](../../2037941f-profile-and-optimize-mid-range-buffer-operations/survey/dense-baseline-smoke.txt)
record exact parity, bit indexing, zero tails, boundary cases, and resumability.

The [completed profile log](../../../bench_results/2037941f/dense-baseline-profile-r2/execution.log)
pins arm, tool, launcher, renderer, source-closure, and addendum digests and
records a completed attempt for every declared cell. The
[host record](../../../bench_results/2037941f/dense-baseline-profile-r2/host.txt)
contains runtime topology and toolchain observations. Each cell directory
contains the canonical `request.json` with its frozen seed and exact
`commands.txt`; for example, the [allocated short-stride request](../../../bench_results/2037941f/dense-baseline-profile-r2/matvec-r1024-8w-warm/attempt-1/request.json)
and [commands](../../../bench_results/2037941f/dense-baseline-profile-r2/matvec-r1024-8w-warm/attempt-1/commands.txt).
The [producing-input closure](../../2037941f-profile-and-optimize-mid-range-buffer-operations/survey/dense-producing-inputs.json)
and [frozen addendum](../../2037941f-profile-and-optimize-mid-range-buffer-operations/dense-parity-addendum.md)
define source and cell identities. The original
[failed launch](../../../bench_results/2037941f/dense-baseline-profile/execution.log)
remains preserved separately.

The [profile-request source](../../2037941f-profile-and-optimize-mid-range-buffer-operations/survey/dense-harness/src/bin/dense-campaign.rs)
emits the request for a frozen cell. The [cell table](../../2037941f-profile-and-optimize-mid-range-buffer-operations/survey/dense-harness/src/cells.rs)
derives its workload seed from the addendum's campaign seed through the
canonical [SplitMix64](../../../tools/tuning-campaign-support/src/abtest.rs);
the [fixture source](../../2037941f-profile-and-optimize-mid-range-buffer-operations/survey/dense-harness/src/fixture.rs)
defines matrix/vector fill order, cache banks, and tail masking. These source
paths occur in the producing-input closure pinned by the profile log.

The [generated profile summary](../../../bench_results/2037941f/dense-baseline-profile-r2/profile-summary.md)
reports per-call counters and sampled-symbol shares with order-statistic
intervals over the recorded passes. Every published symbol appears in every
raw cycle report. Each retained `perf report` command uses `--percent-limit 0`,
and the renderer consumes every reported sample row. An absent symbol has zero
observed sampled hits in that full report. The
[canonical renderer](../../2037941f-profile-and-optimize-mid-range-buffer-operations/survey/summarize-dense-profile.py)
reproduces the summary byte for byte from the committed profile directory.
Counter rates include profile-process setup divided by observed calls; the
paired campaign receipts remain the operation-latency evidence.

## Cost attribution

The profile's stable sampled symbols put the existing AVX2 fused
`and_popcnt` body in the principal measured path of the isolated and allocated
SIMD cells. The allocated short-stride row also attributes work to
`BitMatrix::matvec_simd` and the bundle entry. Its
[release assembly](../../2037941f-profile-and-optimize-mid-range-buffer-operations/survey/asm/dense-baseline/simd/simd-matvec.asm.txt)
shows output allocation, repeated bundle calls, parity folds, and output-bit
appends. The [kernel assembly](../../2037941f-profile-and-optimize-mid-range-buffer-operations/survey/asm/dense-baseline/simd/avx2-fused.asm.txt)
and [sampled annotation](../../../bench_results/2037941f/dense-baseline-profile-r2/matvec-r1024-64w-warm/attempt-1/cycles-annotate-1.txt)
show vector AND, nibble lookup, accumulation, and horizontal reduction. The
summary's instruction and cache-counter intervals distinguish short, wide,
warm, and streaming work. The raw reports do not isolate an allocator, spill,
or bandwidth gain assignable to a new full-count fusion. The
[pinned source ledger](../../2037941f-profile-and-optimize-mid-range-buffer-operations/survey/dense-parity-source-evidence.json)
identifies the public matvec route, its per-row bundle call, and the existing
kernel body.

## Portfolio decision

**No candidate is selected under REQ-05.** The
[frozen cost boundary](../../2037941f-profile-and-optimize-mid-range-buffer-operations/dense-parity-addendum.md#operations-and-cost-boundaries)
permits replacing the same one-row `and_popcnt_fn` body, returning the same
full count, and measures its effect through allocated public `matvec`. The
profile locates work in that already fused body but identifies no concrete
distinct full-count fusion with a profile-backed mechanism for the frozen
whole-consumer anchor rule.
Batching rows or packing output bits targets the observed row-loop share but
changes the declared one-row comparator. The generic carry-save comparator
remains the preserved [non-qualifying result](../../5cbb6545/findings.md); the
scalar four-accumulator route remains a reference arm. Neither is relabelled
a new candidate.

The search stops before a candidate pilot. There is no candidate identity,
target-cell subset, implementation budget allocation, confirmation, or
production change to freeze. The retained fused AND-popcount route remains
selected. This conclusion is limited to the frozen portfolio. The external
M4RI comparison has its own qualified contract and selection issue.

## Criterion disposition

- **REQ-01:** Both accepted exploratory receipts
  contain raw samples, snapshots, commands, logs, provenance, and acceptance
  rows.
- **REQ-02:** The release validation and shared smoke cover the declared
  scalar/SIMD semantics and boundary behavior.
- **REQ-03:** The profile, raw annotations, counters,
  and release assembly attribute the observed kernel, row-loop, output, and
  memory work while stating unresolved allocator, spill, and bandwidth limits.
- **REQ-04–05:** The profile supports no distinct
  in-bound fusion. The empty portfolio and preserved carry-save no-win exhaust
  this issue's search stop.
- **REQ-06:** The canonical matvec and kernel paths remain unchanged; this
  outcome authorizes no production route.
