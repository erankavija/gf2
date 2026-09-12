# CPU LDPC throughput gap: steady-state profile

> **Diátaxis Type:** Explanation

Survey for `3be770d5`. It measures and ranks; it changes no production decoder.
The [measurement contract](../1a379447-zen3-cpu-performance/measurement-contract.md)
and [protocol v3](../f547c394/protocol.md) govern every timed cell; the
[plan](plan.md) maps each criterion to what `c077a88b` already settles.
The three pilot campaigns are measured and accepted; the profile shares and
the two matched confirmations are not yet measured, so the sections that rest
on them stay WAITING-ON-WINDOW.

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
The gf2 arms reach that kernel through the runtime kernel table the same
source evidence records, and its internal disagreement is filed as
`39cbde20`. Each arm's decisions are checked frame by frame against the
frozen `c077a88b` evidence and match it, so the measured gap does not rest on
the discrepancy; the shared-reduction lever does, because a two-pass check
update has to fix one rule for both paths.

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

The first series
([v3-r1-steady-profile](../../bench_results/3be770d5/v3-r1-steady-profile/))
carries no figure: every profiled case of all nine sessions exited on the
`perf` control channel, so no case wrote its record and the summarizer
excluded all of them. `perf` answers a control command with its
acknowledgement tag and the trailing NUL of the C string literal, and the
driver read the NUL of one answer as the head of the next. Its session
directories, the statuses, the summary they produce and the
[window job log](../../bench_results/3be770d5/v3-r1-steady-profile-window-job.log)
stay as the record of that outcome. The driver now tolerates the padding and
the series stops at a session that leaves no usable case, so the failure
surfaces in the window instead of at the summary.

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

## Reproduction

From the worktree root: build the harness with the command the build identity
records, extract the `c077a88b` recorded-input archive into
`target/ldpc-inputs`, record the preparation with
[record-preparation.py](survey/record-preparation.py), then run
`dev/bench_results/3be770d5/run-campaign.sh FAMILY pilot RUN_ID prepare`
followed by `window` for each family, and
`dev/active/3be770d5/survey/run-profile.sh DIR` for the profile series. Each
command resumes under its own identity; the script headers state their
contracts.

## Results

[summarize.py](survey/summarize.py) regenerates the
[tables](../../bench_results/3be770d5/tables.md) from the committed receipts;
they and the profile series' own `profile.md` are the only numeric
projections, and every statement below points at the section and cell that
carries its figures.
[freeze-addendum.py](survey/freeze-addendum.py) derives each matched
confirmation addendum mechanically from its accepted pilot.

### The per-core gap

Every matched single-worker cell of the accepted pilot puts AFF3CT ahead:
the paired bootstrap interval of the time ratio lies wholly above one on both
codes, far from it, and no window is flagged (tables "Steady-state campaigns",
`v3-r1-3be770d5-ldpc-steady-single-worker-pilot`). Both arms decode the same
recorded frames of the same parity-check matrix under the same schedule,
precision, normalization, iteration cap and stopping rule, and each worker's
per-frame decisions are checked against the frozen `c077a88b` evidence, so the
gap is decode work rather than a difference in what is decoded. The cells are
exploratory: the pilot observes the resolution and decides nothing, and the
confirmation of this family is what fixes its margins.

The NR BG1 ratio exceeds the DVB-T2 ratio at one worker, the direction the
degree-structure lever predicts from the structural counts, whose per-edge
search and gather costs are higher for the NR graph (tables "Structural work
per flooding iteration" and the two single-worker cells). The comparison
between two cells is descriptive: the family declares no such comparison and
the correction covers none.

### Saturation is not where the gap comes from

The matched multicore pilot measures the same operation at six physical
cores, twelve physical cores and twenty-four logical CPUs, every worker
pinned, every worker's placement journaled and checked
(`v3-r1-3be770d5-ldpc-steady-multicore-pilot`). On both codes the ratio at
twenty-four logical CPUs is smaller than at six physical cores, with intervals
that do not overlap, so gf2 loses relatively less ground as both decoders
saturate the machine. The deficit this survey has to attribute is therefore
single-core work; the profile shares are what will attribute it, and they are
the measurement still owed. This ordering between cells is descriptive for the
same reason as above.

### Construction and conversion

AFF3CT's decoder construction costs far more than gf2's, and it grows with the
worker count; the steady-state operation puts it outside the timed window and
reports it as setup, beside the per-call conversion and dispatch costs, in
each receipt's untimed diagnostics table. Conversion and dispatch stay small
against the call in every cell, so the matched gap is not a conversion
artifact.

### Fastest-compatible modes

The exploratory family widens the gap further with layered and inter-frame
modes, the fixed-point inter-frame mode most
(`v3-r1-3be770d5-ldpc-steady-fastest-compatible-pilot`). Every cell of it
carries a P-19 note, so this corpus establishes no quality admission for any
of those modes and the family selects nothing. The receipt's own quality
table carries the contradicting evidence directly: on DVB-T2 the fixed-point
inter-frame candidate fails more frames than the baseline it is compared with,
and its iterations run to the cap.

### What the window still owes

The profile shares (REQ-02) and the share-ranked levers (REQ-03) need the
re-run series, and the two matched confirmations need their own window job.
The lever table's mechanisms, removed categories and falsifiable experiments
are committed; what no evidence supports yet is the rank order, because the
rule ranks by a measured lower bound.

| Criterion | Status | Evidence or remaining work |
|---|---|---|
| REQ-01 | MET | Three accepted pilot receipts cover one worker, six and twelve physical cores and twenty-four logical CPUs on both codes, with journaled topology, affinity, observed worker counts, thread counts, SMT state and build identities; the matched confirmations add frozen decision margins. |
| REQ-02 | WAITING-ON-WINDOW | Structural counts and the allocation census are recorded; no measured time share exists until the re-run profile series replaces the void first one. |
| REQ-03 | WAITING-ON-WINDOW | Ranking rule, mechanisms and experiments above; the ranks need the profile shares. The degree-structure prediction already holds in the single-worker cells. |
| REQ-04 | MET | Matched arms keep the numerical contract on identical recorded LLRs; the fastest-compatible modes are separated, labelled and reported with their P-19 notes, iteration distributions and the prepared BER/FER counts and intervals. |
| REQ-05 | MET for the published campaigns | Each receipt pins the contract, protocol, addendum, ledger and producing closure, and independent acceptance accepted each; the void profile series is preserved rather than discarded. No production change, so no before/after pair is owed. |
