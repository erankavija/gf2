# CPU LDPC throughput gap: steady-state profile

> **Diátaxis Type:** Explanation

Survey for `3be770d5`. It measures and ranks; it changes no production decoder.
The [measurement contract](../1a379447-zen3-cpu-performance/measurement-contract.md)
and [protocol v3](../f547c394/protocol.md) govern every timed cell; the
[plan](plan.md) maps each criterion to what `c077a88b` already settles.
Results sections are WAITING-ON-WINDOW: no timed run has happened yet.

## Question

`c077a88b` compares whole calls in which AList parsing and decoder
construction sit inside every timed call. This survey compares the decoders a
receiver keeps: each worker builds its decoder once, and a timed call makes
every worker decode the declared per-worker batch of recorded frames,
including LLR conversion, the dispatch to the workers and extraction of the
information-window decisions. Construction is reported as setup. The family
descriptions in the three pilot addenda are the authoritative operation
definitions:
[single worker](addendum-ldpc-steady-matched-single-worker-pilot.json),
[multicore](addendum-ldpc-steady-matched-multicore-pilot.json) and
[fastest compatible](addendum-ldpc-steady-fastest-compatible-pilot.json).

## Workers, placement and correctness

Each worker is a thread pinned to one CPU of the arm the protocol resolver
returns at run time; every worker decodes the identical batch, so a
multi-worker call measures saturation without a load-imbalance tail. gf2
workers each build a decoder over a clone of the recorded-AList code, the
per-worker model of `gf2-sim`'s `ldpc_bler_sweep`; AFF3CT workers own AFF3CT's
own `clone()` of one decoder built through the unchanged `c077a88b` shim.
Every arm writes a placement report (assigned CPU, affinity read back, CPUs
observed during calls, process threads) that the runner journals, and fails
when a worker leaves its CPU, when a thread beyond the workers exists (the
cells declare no nested pool), or when any worker's decisions differ from the
frozen per-frame error vector. The harness is
[survey/harness](survey/harness/Cargo.toml).

Before timing, an untimed replay of every recorded frame through the
reused-decoder, multi-worker paths reproduces the prepared per-frame error
vectors and the gf2 iteration distribution
([validation](../../bench_results/3be770d5/preparation/validation.jsonl)),
and every arm of every pilot cell passes a validation-role run with all
placement checks
([arm validation](../../bench_results/3be770d5/preparation/arm-validation.jsonl)).
Executables, toolchain, AFF3CT commit and static-library digest, shim flags
and input identities are in the
[build identity](../../bench_results/3be770d5/preparation/build-identity.json).

## Comparison contract

Matched arms keep the numerical contract of the comparison harness: the same
digested AList, the same recorded f32 LLRs, normalized min-sum at factor 0.75,
flooding, iteration cap 50 and syndrome stopping, one frame per decoder
invocation. The fastest-compatible family pairs gf2 with AFF3CT horizontal
layered f32, layered f32 INTER and layered i16 INTER; precision, schedule and
native wave size differ and stay labelled in [arms.json](survey/arms.json).
All arms report the `c077a88b` prepared quality evidence, whose BER/FER
counts, intervals and iteration distributions are projected in its
[tables](../../bench_results/c077a88b/tables.md) "Quality on identical
recorded inputs"; this survey adds no BER/FER samples. Under v3 that corpus
cannot certify quality admission for any fastest-compatible mode, so that
family is exploratory only and selects nothing. The fixed-point DVB excess
frame failures `c077a88b` records remain contradicting evidence against
adopting quantization without a new numerical contract.

The source evidence records a signed-zero difference a shared reduction must
settle: AFF3CT's sign treats negative zero as negative, gf2's scalar min-sum
as positive, and gf2's AVX2 min-sum kernel does both, by sign bit in its
vector lanes and by comparison in its scalar tail
([source evidence](survey/source-evidence.json), lever `numerical-contract`).

## Families, statistics and host

The two matched families are the two questions REQ-01 and REQ-02 separate:
the per-core gap and the gap under multicore saturation. One family holding
both would be `not-confirmatory` under P-20's tail rule; the split follows the
questions, and each family keeps its own append-only ledger from genesis.
Pilots run first; confirmation addenda freeze resolution and margins from the
accepted pilots and run in a later window. The runner observes topology, SMT,
governors, affinity and load, holds the exclusive full-host lock, and pins the
contract, protocol, addendum and producing closure in every receipt
([launcher](../../bench_results/3be770d5/run-campaign.sh)). No production code
changes, so no before/after pair is owed.

## Profile design

[run-profile.sh](survey/run-profile.sh) runs a declared number of sessions,
each under its own full-host acquisition. `perf` starts disabled and the
driver enables it only around the profiled dispatches. Record cases sample
user cycles at a fixed period for gf2 at one and 24 workers and AFF3CT
flooding at one worker; every sampled address is resolved to its inline chain
and assigned a category by source-span rules
([summarizer](survey/summarize-profile.py)); shares carry Wilson intervals
over pooled samples and per-session ranges. Stat cases record two
non-multiplexed counter groups for both arms at one, six, twelve and 24
workers; per-session figures carry order-statistic median intervals, and
per-worker slowdown and gf2-over-AFF3CT ratios carry bootstrap intervals.

A bottleneck counts as single-core when its share at 24 workers stays within
its one-worker Wilson interval and gf2's per-worker slowdown interval overlaps
AFF3CT's; it counts as multicore saturation when gf2's slowdown interval lies
above AFF3CT's and the L1d or cache miss ratio rises with the worker count.

## Structural and allocation evidence

[structural-costs.json](../../bench_results/3be770d5/preparation/structural-costs.json)
derives, from each recorded graph and the loop structure of both decoders,
the per-iteration gathers, position-search comparisons, reduction calls,
allocations and syndrome work. They are exact derived counts, not timings. Per
edge, the NR graph costs gf2's loops more search comparisons and gathers than
the DVB graph, while AFF3CT's indexed passes stay proportional to the edges.
The [allocation census](../../bench_results/3be770d5/preparation/alloc-census.jsonl)
records every heap request of each frame it decodes by size. Every census
frame allocates exactly one vector per edge per iteration plus two syndrome
vectors per syndrome check, and frees all of them; the
[tables](../../bench_results/3be770d5/tables.md) "Allocation census" section
checks that relation and projects both files, and the profile summary
repeats the check on the census the first profile session records.

## Lever ranking

Each lever removes a set of profile categories. Its predicted single-worker
speedup is at least $1/(1-s_{\text{lo}})$, where $s_{\text{lo}}$ is the lower
Wilson bound of the removed share; a later confirmed speedup whose upper
bound falls below that value refutes the attribution. Levers rank by
$s_{\text{lo}}$; a lever no category isolates is ranked by a labelled estimate
from the structural counts. Downstream work: `07ca8585` (allocation and edge
traversal), `f63a2464` (quantized, layered, QC-aware) and `ed3d490e`
(inter-frame SIMD).

| Lever | gf2 mechanism (source evidence) | Removed categories | Falsifiable workload experiment |
|---|---|---|---|
| Allocation removal | per-edge `Vec<f32>` in `boxplus_minsum_n`; per-iteration syndrome vectors | `allocator`, `min-sum-input-vec` | Census of the changed decoder shows zero steady-state allocations; single-worker steady cells meet the prediction. |
| Canonical edge indexing | linear `find_check_position` and `check_to_var_message` scans | `edge-position-search` | Precomputed edge index with unchanged arithmetic; NR gains more than DVB, as the structural counts predict, or the attribution is refuted. |
| Flat check/variable-major layout | `Vec<Vec<Llr>>` messages, `Vec<Vec<usize>>` neighbors | none isolated: labelled estimate from counters | Flat arrays with identical arithmetic; L1d misses per frame fall and the twelve-worker per-worker slowdown shrinks, else refuted. |
| Shared min/second-min/sign | one reduction over d_c - 1 gathered inputs per output edge | `check-node-loop`, `min-sum-dispatch`, `min-sum-reduction` beyond two passes per check | Two-pass check update with the signed-zero rule settled; cycles per frame fall at least by the structural gather ratio times the measured share. |
| Inter-frame SIMD | none today | none: comparator estimate | AFF3CT's layered f32 scalar-to-INTER ratio in the fastest-compatible cells bounds the gain; a gf2 lane-batched decoder is refuted if it falls below the matched-scalar gap closed by the scalar levers. |
| Quantized/layered decoding | none today | none: comparator estimate | Layered f32 and i16 INTER cells with iteration distributions; any adoption needs a new numerical contract and quality evidence the current corpus cannot give. |
| QC-aware intra-frame work | none today | none: labelled estimate | Lifted-block NR update against the flat scalar update on the NR cells; refuted if it does not beat the flat layout at one worker. |
| Batch conversion | f32-to-`Llr` copy and bit extraction per frame | `conversion-output` | Receipt pack and dispatch probes against the call time; a conversion-free path must gain at least its share. |
| Syndrome and termination | hard-decision vector plus full syndrome each iteration | `syndrome-termination` | Soft early-exit syndrome on posteriors; gain at least the share at unchanged iteration counts. |
| Degree structure | search and gather cost grow with check and variable degree | per-code shares | The NR gap exceeds the DVB gap in the single-worker cells; refuted otherwise. |
| Dispatch | lazy kernel table and indirect call per edge; pool barriers per call | `min-sum-dispatch`, `dispatch` | Direct inlined reduction; pool dispatch probe stays negligible against the call. |

## Results

WAITING-ON-WINDOW: the three pilot receipts, the profile summary
(`profile.md` in the profile directory) and, after the freeze, the two
matched confirmations. [summarize.py](survey/summarize.py) regenerates the
[tables](../../bench_results/3be770d5/tables.md) from the committed receipts;
they and `profile.md` are the only numeric projections.
[freeze-addendum.py](survey/freeze-addendum.py) derives each matched
confirmation addendum mechanically from its accepted pilot.

| Criterion | Status | Evidence or remaining work |
|---|---|---|
| REQ-01 | WAITING-ON-WINDOW | Pilot campaigns frozen; confirmations after the pilot freeze. |
| REQ-02 | WAITING-ON-WINDOW | Profile series frozen; structural counts and census recorded. |
| REQ-03 | WAITING-ON-WINDOW | Ranking rule and experiments above; ranks need the profile shares. |
| REQ-04 | MET for contract and inputs; results WAITING-ON-WINDOW | Validation evidence and addenda above; fastest-compatible pilot pending. |
| REQ-05 | WAITING-ON-WINDOW | Receipts and acceptance pending; no production change. |
