# CPU LDPC throughput gap: steady-state profile

> **Diátaxis Type:** Explanation

Survey for `3be770d5`. It measures and ranks; it changes no production decoder.
The [measurement contract](../1a379447-zen3-cpu-performance/measurement-contract.md)
and the [shared protocol](../f547c394/protocol.md) govern every timed cell.
A receipt pins, snapshots and is evaluated under the version it names: the
three pilots name [version 3](../f547c394/amendment-v3.md) and the two matched
confirmations name [version 4](../f547c394/amendment-v4.md), which admits a
version-3 pilot as resolution evidence because both versions derive the pilot
resolution identically. The [plan](plan.md) maps each criterion to what
`c077a88b` already settles.
Five campaigns are measured and independently accepted, and one profile series
of nine sessions carries the sampled shares and counters.

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
[survey/harness](survey/harness/Cargo.toml). Each receipt's placement and
thread counts are projected in the
[tables](../../bench_results/3be770d5/tables.md) "Steady-state campaigns"
section, one row per cell and arm.

Before timing, an untimed replay of every recorded frame through the
reused-decoder, multi-worker paths reproduces the prepared per-frame error
vectors and the gf2 iteration distribution
([validation](../../bench_results/3be770d5/preparation/validation.jsonl)),
and every arm of every pilot cell passes a validation-role run with all
placement checks
([arm validation](../../bench_results/3be770d5/preparation/arm-validation.jsonl)).
Executables, toolchain, AFF3CT commit and static-library digest, shim flags
and input identities are in the
[build identity](../../bench_results/3be770d5/preparation/build-identity.json),
and every campaign refuses to start against an arm executable whose digest
differs from it.

The confirmations measure the arm executables the pilots measured, which the
[executable identity table](../../bench_results/3be770d5/preparation/executable-identity.md)
records. Two rules make that the required choice rather than a convenience.
A candidate identity hashes executable bytes, and the confirmatory attempt cap
is one attempt per candidate identity and protocol version, so rebuilt bytes
are a different candidate: a confirmation on them would not confirm what the
pilot resolved, and the pilot-to-confirmation chain the family ledger exists to
track would be broken. And the protocol treats git revisions, commit ancestry
and whole-tree state as navigation metadata that decide neither acceptance nor
resume compatibility, because a receipt pins the exact bytes it measured; the
standard a build has to meet is reproducible from the recorded identity, not
built from the tree at run time.

The arms therefore link a campaign-support library older than the runner's:
they were built before protocol v4 landed, and v4 changed that library's result
encoding, so rebuilding them from the merged tree yields different bytes while
the profiling executables reproduce theirs exactly. The mismatch is confined to
measurement plumbing. It changes no decoder, since the v4 merge touches no
crate under `crates/` and no arm source, and it is not assumed to be harmless:
the pinned arms and the v4 runner are run together end to end, through the real
runner to an accepted receipt, before either confirmation is queued.

## Comparison contract

Matched arms keep the numerical contract of the comparison harness: the same
digested AList, the same recorded f32 LLRs, normalized min-sum at factor 0.75,
flooding, iteration cap 50 and syndrome stopping, one frame per decoder
invocation. The fastest-compatible family pairs gf2 with AFF3CT horizontal
layered f32, layered f32 INTER and layered i16 INTER; precision, schedule and
native wave size differ and stay labelled in [arms.json](survey/arms.json).
All arms report the `c077a88b` prepared quality evidence, whose BER/FER
counts, intervals and iteration distributions each receipt carries and the
[tables](../../bench_results/3be770d5/tables.md) project per cell and arm.
This survey adds no BER/FER samples. That corpus cannot certify quality
admission for any fastest-compatible mode under P-19, so that family is
exploratory only and selects nothing. The fixed-point DVB excess frame
failures `c077a88b` records remain contradicting evidence against adopting
quantization without a new numerical contract.

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
accepted pilots and run in a later window. Both matched confirmations are
frozen ([single worker](addendum-ldpc-steady-matched-single-worker.json),
[multicore](addendum-ldpc-steady-matched-multicore.json), each with its
[derivation](addendum-ldpc-steady-matched-single-worker-derivation.json)
record) and each keeps every cell of its family: no cell is trimmed and no
question split to satisfy the multiple-comparison correction, because at a
first attempt's family alpha P-20's tail rule admits a family of this size.
Each confirmation's own acceptance summary reports the status its cells
actually reach. The runner observes topology, SMT,
governors, affinity and load, holds the exclusive full-host lock, and pins the
contract, protocol, addendum and producing closure in every receipt
([launcher](../../bench_results/3be770d5/run-campaign.sh)). No production code
changes, so no before/after pair is owed.

## Profile design

[run-profile.sh](survey/run-profile.sh) runs a declared number of sessions,
each under its own full-host acquisition. `perf` starts disabled and the
driver enables it only around the profiled dispatches; the driver tolerates
the trailing NUL of each acknowledgement on the control channel, and a session
that leaves no usable case stops the series inside the window. Record cases
sample user cycles at a fixed period for gf2 at one and 24 workers and AFF3CT
flooding at one worker; every sampled address is resolved to its inline chain
and assigned a category by source-span rules
([summarizer](survey/summarize-profile.py)); shares carry Wilson intervals
over pooled samples and per-session ranges. Stat cases record two
non-multiplexed counter groups for both arms at one, six, twelve and 24
workers; per-session figures carry order-statistic median intervals, and
per-worker slowdown and gf2-over-AFF3CT ratios carry bootstrap intervals.

Every sample a case records reaches a category. The pooled denominator of a
case is the SAMPLE count `perf report --stats` reports for it, and a case whose
address listing and recorded total disagree stops the summary rather than
reporting shares over an incomplete denominator. Samples whose instruction
pointer `perf` resolves to no object carry the `unmapped-ip` category: on this
host they are the kernel-space addresses its own report lists as `[k]`, which
`cycles:u` samples through interrupt skid and an unprivileged session cannot
map. The rule is equality, not a tolerance, and `summarize-profile.py
--self-test` checks the three address shapes, that the unmapped shape reaches
the pooled total, and that a listing short of its recorded total is rejected.

A bottleneck counts as single-core when its share at 24 workers stays within
its one-worker Wilson interval and gf2's per-worker slowdown interval overlaps
AFF3CT's; it counts as multicore saturation when gf2's slowdown interval lies
above AFF3CT's and the L1d or cache miss ratio rises with the worker count.
Both halves are evaluated mechanically in the tables' "Single-core work
against multicore saturation" section.

The first series
([v3-r1-steady-profile](../../bench_results/3be770d5/v3-r1-steady-profile/))
produces zero usable samples: every profiled case of all nine sessions exits
on the `perf` control channel, so no case writes its record and the summarizer
excludes all of them. `perf` answers a control command with its
acknowledgement tag and the trailing NUL of the C string literal, and the
driver that ran it read the NUL of one answer as the head of the next. **No
conclusion in this document rests on that series**, and its
[generated summary](../../bench_results/3be770d5/v3-r1-steady-profile/profile.md)
reports zero samples in every table. The series stays: its session
directories, the per-case statuses, that summary and the
[window job log](../../bench_results/3be770d5/v3-r1-steady-profile-window-job.log)
are the record of the outcome, and the second series neither replaces nor
amends them.

The two series are named `v3-r1-steady-profile` and `v4-r1-steady-profile`,
after the campaign generation of the window each runs in. A profile series
pins no protocol version and is evaluated under none, so the change of run id
marks no change of method: the cases, the sampling period, the counter groups,
the session count and the summarizer are the same in both, and what separates
them is the acknowledgement handling and the session guard. The fix moves one
executable and no other: the
[executable identity table](../../bench_results/3be770d5/preparation/executable-identity.md)
joins the digest every committed measurement records to the digest the build
identity records.

The conditions the first series ran under are projected beside it
([conditions](../../bench_results/3be770d5/v3-r1-steady-profile-conditions.md),
from its launcher log and host record), so the second series can be compared
against like conditions rather than against an assumption. They read as a
self-generated load: the first session begins on an idle host, load climbs
across the early sessions to a plateau and stays there, and session duration
stays flat across that whole range, the first session's small excess being
the census and call-graph work only it performs. A rising outside load would
have lengthened the later sessions. This is a bound, not an isolation: the
cases themselves occupy every logical CPU, so a load average taken at a
session boundary cannot separate the series' own work from anything else, and
what the full-host mutex each session holds excludes is other measurement.
The second series records the same quantities in its own
[repetitions log](../../bench_results/3be770d5/v4-r1-steady-profile/repetitions.log)
and [host record](../../bench_results/3be770d5/v4-r1-steady-profile/host.txt).

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
checks that relation and projects both files, and the
[profile summary](../../bench_results/3be770d5/v4-r1-steady-profile/profile.md)
repeats the check on the census the first profile session records.

## Lever ranking

Each lever removes a set of profile categories. Its predicted single-worker
speedup is at least $1/(1-s_{\text{lo}})$, where $s_{\text{lo}}$ is the lower
Wilson bound of the removed share; a later confirmed speedup whose upper
bound falls below that value refutes the attribution. Levers rank by
$s_{\text{lo}}$, taking the smaller of the two codes' bounds; a lever no
category isolates takes no rank and carries a labelled basis. The ranks, the
shares, the intervals and the implied speedups are in the tables' "Lever
ranking" section, derived from [levers.json](survey/levers.json) and the
profile summary. Downstream work: `07ca8585` (allocation and edge traversal),
`f63a2464` (quantized, layered, QC-aware) and `ed3d490e` (inter-frame SIMD).

| Lever | gf2 mechanism (source evidence) | Removed categories | Falsifiable workload experiment |
|---|---|---|---|
| Allocation removal | per-edge `Vec<f32>` in `boxplus_minsum_n`; per-iteration syndrome vectors | `allocator`, `min-sum-input-vec` | Census of the changed decoder shows zero steady-state allocations; single-worker steady cells meet the prediction. |
| Canonical edge indexing | linear `find_check_position` and `check_to_var_message` scans | `edge-position-search` | Precomputed edge index with unchanged arithmetic; NR gains more than DVB, as the structural counts predict, or the attribution is refuted. |
| Flat check/variable-major layout | `Vec<Vec<Llr>>` messages, `Vec<Vec<usize>>` neighbors | none isolated: labelled estimate from counters | Flat arrays with identical arithmetic; L1d misses per frame fall and the twelve-worker per-worker slowdown shrinks, else refuted. |
| Shared min/second-min/sign | one reduction over d_c - 1 gathered inputs per output edge | upper bound from `check-node-loop`, `min-sum-reduction`, `min-sum-dispatch` | Two-pass check update with the signed-zero rule settled; cycles per frame fall at least by the structural gather ratio times the measured share. |
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
`dev/bench_results/3be770d5/run-campaign.sh FAMILY MODE RUN_ID prepare`
followed by `window` for each family and mode, and
`dev/active/3be770d5/survey/run-profile.sh DIR` for the profile series. Each
command resumes under its own identity; the script headers state their
contracts. A confirmation refuses to run until its addendum is committed and
unmodified, so freezing it with
[freeze-addendum.py](survey/freeze-addendum.py) and committing it precede the
window. The summary step of a profile series reads files and times nothing,
so it runs after the mutex and can be repeated over a completed series.

## Results

[summarize.py](survey/summarize.py) regenerates the
[tables](../../bench_results/3be770d5/tables.md) from the committed receipts,
the structural counts and the committed profile summary;
[summarize-profile.py](survey/summarize-profile.py) regenerates the
[profile](../../bench_results/3be770d5/v4-r1-steady-profile/profile.md) from
the nine session directories. Those two documents are the only numeric
projections, and every statement below points at the section and cell that
carries its figures. [freeze-addendum.py](survey/freeze-addendum.py) derives
each matched confirmation addendum mechanically from its accepted pilot.

### The per-core gap

The matched single-worker confirmation puts AFF3CT ahead on both codes: each
cell's paired bootstrap interval of the time ratio lies wholly above one, far
from it, no window is flagged, and independent acceptance accepts the receipt
(tables "Steady-state campaigns",
`v4-r1-3be770d5-ldpc-steady-single-worker-confirmation`). Both arms decode the
same recorded frames of the same parity-check matrix under the same schedule,
precision, normalization, iteration cap and stopping rule, and each worker's
per-frame decisions are checked against the frozen `c077a88b` evidence, so the
gap is decode work rather than a difference in what is decoded. The pilot of
this family observes the same direction and decides nothing; the confirmation
is what fixes the margins.

The NR BG1 ratio exceeds the DVB-T2 ratio at one worker, the direction the
degree-structure lever predicts from the structural counts, whose per-edge
search and gather costs are higher for the NR graph (tables "Structural work
per flooding iteration" and the two single-worker cells). The comparison
between two cells is descriptive: the family declares no such comparison and
the correction covers none.

### Where the single-worker time goes

Three categories carry most of gf2's single-worker samples on both codes: the
linear edge-position searches, the check-node loop, and the allocator together
with the per-edge input vector the check update allocates (profile "Sampled
shares by category", `gf2-dvb-w1` and `gf2-nr-w1`). The same tables give AFF3CT's split, where the min-sum update
rule dominates and no allocator category appears at all, which the structural
counts predict: AFF3CT allocates nothing per iteration.

One libc region carries a further share of gf2's single-worker samples under
no symbol, so it stays in its own `other:libc.so.6` category rather than being
folded into a lever. The first session's call-graph record names its caller:
it is reached through `boxplus_minsum_n`, the `collect` into the per-edge
`Vec<f32>` and that vector's trusted extend, so it is the copy that fills the
vector the allocation lever removes
([callers](../../bench_results/3be770d5/v4-r1-steady-profile/rep-01/callgraph/gf2-dvb-w1.callers.txt)).
The ranked allocation share is therefore a lower bound on what per-edge vector
construction costs, and the lever's own experiment, a census showing zero
steady-state allocations, is what would settle the rest.

### Single-core work against multicore saturation

The predeclared test is evaluated in the tables' "Single-core work against
multicore saturation" section, both halves, from the committed summary. Its
share-stability half fires only for categories at or below a fraction of a
percent of the one-worker samples: on both codes every category with a
double-digit single-worker share moves outside its one-worker Wilson interval
by twenty-four workers, in one direction or the other. Its saturation half
fires at six and twelve physical cores on DVB-T2 and
at six on NR, where gf2's per-worker slowdown interval lies wholly above
AFF3CT's and a miss ratio rises; it does not fire at twenty-four logical CPUs,
where gf2's slowdown interval lies wholly below AFF3CT's on both codes.

The deficit this survey has to attribute is therefore not multicore
saturation. It is present at one worker, where no sharing exists, at the
margins the single-worker confirmation fixes, and it narrows rather than
widens as both decoders take the whole machine. What the shares do between one
and twenty-four workers is a separate recorded observation: they move, in both
directions by category, and the flat-layout lever's experiment is the one that
tests a layout explanation for that movement.

### Ranked levers

The two levers that remove whole categories at the top of the single-worker
profile are canonical edge indexing and allocation removal, in that order on
both codes, and the shares, intervals and implied lower-bound speedups are in
the tables' "Lever ranking" section. Dispatch, syndrome and termination follow
at single-digit shares, and batch conversion is negligible at one worker,
which is itself the answer to whether the matched gap is a conversion
artifact.

Three levers take no rank because no category isolates them, and each carries
its labelled basis in the same section: the shared min/second-min/sign
reduction, bounded above by its containing categories scaled by the structural
gathers per edge; the flat layout, carried by the L1d miss ratio at one and
twenty-four workers; and the degree-structure and QC-aware levers, carried by
the exact position-search comparisons per edge. Inter-frame SIMD and
quantized/layered decoding are comparator estimates from the fastest-compatible
cells and adopt nothing.

The upper bound on the shared-reduction lever exceeds the ranked share of
allocation removal on both codes. It takes no rank because its bound is an
upper bound and the rule ranks by a lower bound; the two-pass experiment in
the lever table is what would give it one.

### Construction and conversion

AFF3CT's decoder construction costs far more than gf2's in every cell of every
family; the steady-state operation puts it outside the timed window and
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

### Criteria

| Criterion | Status | Evidence |
|---|---|---|
| REQ-01 | MET | Five accepted receipts cover one worker, six and twelve physical cores and twenty-four logical CPUs on both codes, with journaled topology, affinity, observed worker counts, thread counts, SMT state and build identities; the two matched confirmations fix the decision margins. Tables "Steady-state campaigns"; placement per cell and arm in each campaign's second table. |
| REQ-02 | MET | Nine profile sessions give measured time shares with Wilson intervals and per-session ranges for both decoders at one and twenty-four workers, over categories that include allocation, edge traversal, reduction, dispatch, conversion and syndrome work; at least three categories carry double-digit single-worker shares on both codes. The single-core against saturation test is evaluated mechanically in its own tables section. Profile "Sampled shares by category" and "Counters and time per frame". |
| REQ-03 | MET | Every REQ-03 lever has a mechanism in the gf2 source, a removed-category set or a labelled basis, and a falsifiable workload experiment; the ranked levers carry measured lower bounds and the implied speedups. Tables "Lever ranking"; mechanisms and experiments in the lever table above. |
| REQ-04 | MET | Matched arms keep the numerical contract on identical recorded LLRs; the fastest-compatible modes are separated, labelled and reported with their P-19 notes; every cell carries iteration distributions and BER/FER counts with intervals in its receipt's quality table. Tables "Steady-state campaigns", quality table per campaign. |
| REQ-05 | MET | Each receipt pins the contract, protocol, addendum, ledger and producing closure, and independent acceptance accepted each; the void profile series is preserved as falsified evidence rather than discarded. No production change, so no before/after pair is owed. |
