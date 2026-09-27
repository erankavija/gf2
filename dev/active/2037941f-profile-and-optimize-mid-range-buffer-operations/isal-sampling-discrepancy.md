# ISA-L pilot sampling discrepancy

> **Diátaxis Type:** Reference

The [logical addendum](logical-buffer-addendum.md#cache-warmup-and-sampling)
requires 24 pairs for every exploratory cell. The ISA-L pilot executes six
pairs per cell. These are different sampling plans.

The committed [machine plan](../../bench_results/2037941f/2037941f-logical-isal-base-gap/v4-r1-pilot/plan.json)
sets `pilot_pairs` to `null` on each cell. The canonical runner therefore uses
`SHARED_SETTINGS.pilot_min_pairs`, which is six
(`dev/tools/tuning-campaign-support/src/protocol.rs:86`, `:1066`). The queued
ISA-L invocation does not supply `--pilot-pairs 24`. The
[receipt](../../bench_results/2037941f/2037941f-logical-isal-base-gap/v4-r1-pilot/receipt.json),
execution log and generated tables disclose the executed count correctly.
Runner acceptance establishes conformance to that machine plan; it does not
establish compliance with the addendum's 24-pair sentence.

The [generated stopping calculation](../../bench_results/2037941f/isal-base-gap-outcome.md)
uses the actual six-pair receipt and reports resolution-insufficient. The
receipt supplies no confirmatory or production-adoption evidence. Its bytes,
the original sampling requirement, and this discrepancy remain preserved.

## Approved exception: ISA-PILOT-6

The invoker approves `ISA-PILOT-6` on 2026-09-27: this explicitly nonconforming
six-pair exploratory pilot is the terminal ISA-L comparison, with no
confirmation and no production-adoption claim. The exception applies only to
receipt SHA-256
`b479513125bf558cfe4bb70d31697e089a60d86ba1f8b17dd5bcdfbbfc05d009`.
The 24-pair sampling requirement and the discrepancy remain visible. No other
family, margin, resolution ceiling, or sample changes. The exception terminates
with this comparison; it authorizes no future six-pair family run.

Research review `bd066df1` passes with the discrepancy as an advisory. Its
resolution is this approved exception, not a claim that six satisfies the
original requirement.
