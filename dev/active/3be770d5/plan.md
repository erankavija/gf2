# Plan: CPU LDPC throughput gap profile

> **Diátaxis Type:** Explanation

Plan for `3be770d5`. The issue measures and ranks; it changes no production
decoder. The [measurement contract](../1a379447-zen3-cpu-performance/measurement-contract.md)
and the [shared protocol](../f547c394/protocol.md) govern every timed cell,
each receipt under the version it names: the pilots
[version 3](../f547c394/amendment-v3.md) and the matched confirmations
[version 4](../f547c394/amendment-v4.md).

## What `c077a88b` already establishes

| Criterion | Established by `c077a88b` | Pointer |
|---|---|---|
| REQ-01 | Topology, SMT and affinity observation for one pinned worker per arm; source/build identities of both arms. No worker-count normalization: every cell is a single-instance, single-worker call. | [findings](../c077a88b/findings.md) "Limits and follow-up"; [build identity](../../bench_results/c077a88b/v3-preparation/build-identity.json) |
| REQ-02 | No profile. Setup/conversion diagnostics only. | [tables](../../bench_results/c077a88b/tables.md) "Matched-algorithm timing" |
| REQ-03 | Names the follow-up issues; ranks nothing. | findings "Limits and follow-up" |
| REQ-04 | Identical digested AList and recorded LLR bundles, NMS 0.75, flooding, cap 50, syndrome stopping; fastest-compatible AFF3CT modes with precision, schedule and wave size; recorded-corpus BER/FER counts, intervals and iteration distributions. | [tables](../../bench_results/c077a88b/tables.md) "Quality on identical recorded inputs"; [validation](../../bench_results/c077a88b/v3-preparation/validation.json) |
| REQ-05 | Receipts naming protocol version 3, with pinned contract, protocol and addenda for **whole-call** cells, where AList parsing and decoder construction are inside every timed call. | [matched confirmation](../../bench_results/c077a88b/v3-r1-c077a88b-ldpc-matched-algorithm-confirmation/receipt.json) |

The whole-call cells answer a consumer-latency question in which AFF3CT's
decoder construction dominates the DVB call. A reused-decoder comparison is
unmeasured there; this issue measures it.

## What this issue adds

1. **Steady-state throughput cells.** Each worker constructs its decoder
   once, before timing; construction is reported as setup. A timed call makes
   every worker decode the declared per-worker batch of recorded frames,
   including LLR conversion, dispatch to the workers and extraction of the
   information-window decisions. Workers are threads pinned one per resolved
   CPU. AFF3CT workers use AFF3CT's own module `clone()`, the route its
   multi-threaded simulator replicates modules with; gf2 workers each build a
   decoder from a cloned code, the per-worker model of `ldpc_bler_sweep`.
   Every worker decodes the identical batch, so per-worker work is equal and
   the multi-worker cells measure saturation without load-imbalance tails.
2. **Families.** Two questions with matched arms, plus one exploratory family:
   - `ldpc-steady-matched-single-worker-v1`: the per-core gap (1 worker).
   - `ldpc-steady-matched-multicore-v1`: the gap under multicore saturation
     (6 and 12 physical cores, 24 logical CPUs).
   - `ldpc-steady-fastest-compatible-v1`: gf2 against AFF3CT layered f32,
     layered f32 INTER and layered i16 INTER at one worker. Exploratory only:
     under v3 the recorded corpus cannot certify quality admission (P-19),
     so no confirmation is planned; its cells characterize the layered,
     quantized and inter-frame levers.

   REQ-01 names the one-worker comparison and the multicore arms separately,
   and REQ-02 asks to distinguish single-core bottlenecks from multicore
   saturation; the two matched families are those two questions. One
   eight-cell family would be `not-confirmatory` under P-20's tail rule;
   that consequence is recorded, not the reason for the split. Each family
   keeps its own append-only ledger from genesis.
3. **Repeated profile sessions.** A declared number of sessions, each under
   its own `--full-host` wrapper invocation: fixed-period user-cycle samples of
   the gf2 decode region by symbol and by inlined source line, `perf stat`
   counter groups at 1, 6, 12 and 24 workers for both arms, the AFF3CT
   flooding phase split, and one allocation census. Shares carry Wilson
   intervals over pooled samples; per-session figures carry order-statistic
   median intervals.
4. **Structural costs.** A generator reads each recorded AList and derives
   the per-iteration edge visits, position-search comparisons, reduction
   inputs and allocations that each implementation's loop structure implies.
   These are exact counts, labeled as derived, not timings.
5. **Correctness before timing.** Every timed child checks each worker's
   decisions for the batch against the prepared per-frame error vector and
   fails on any difference. An untimed validation replays the full
   recorded bundles through the reused-decoder and multi-worker paths.

## Lever ranking method

Each lever in REQ-03 gets a measured share (or labeled estimate), a
mechanism in the gf2 source, the comparator's corresponding structure from
AFF3CT source evidence, and a falsifiable workload experiment with a
predeclared outcome that would refute it. Ranking uses the lower Wilson bound
of the attributed share at one worker; multicore changes are reported beside
it rather than folded in.

## Window sequence

1. Window 1: the three pilots and the profile series.
2. Next session: accept the pilots, freeze the two matched confirmation
   addenda from their resolution evidence, publish them.
3. Window 2: the two matched confirmations.
4. Following session: generated tables, `profile.md` and the findings results.
