# Matvec ragged-tail measurement addendum, second family

> **Diátaxis Type:** Reference
>
> **Addendum identity:** `7dbe1393-matvec-ragged-tail-addendum-v2`
>
> **Owning issue:** `7dbe1393`
>
> **Frozen UTC:** `2026-10-11T09:26:12Z`

This addendum freezes a second family for the question of the
[ragged-tail addendum](matvec-ragged-tail-addendum.md), identity
`7dbe1393-matvec-ragged-tail-v1`. It repeats that family's pilot on a quiet
host and states only what differs; every section of that addendum it does not
restate holds here unchanged.

## Why the pilot is repeated

The pilot of family `7dbe1393-matvec-ragged-tail` ran on a host that carried
other work. Its receipt,
[`receipt.json`](../../../bench_results/7dbe1393/7dbe1393-matvec-ragged-tail/v4-r1-pilot/receipt.json)
of campaign `v4-r1-7dbe1393-matvec-ragged-tail`, records the host load at each
session start in `session_hosts[].load_average`, and
[`pilot-resolution.md`](pilot-resolution.md) tabulates those observations
beside the outcome `resolution-insufficient` they accompany. That family ends
with its pilot, and its receipts, ledger and outcome stay as recorded.

## Family

| Field | Value |
|---|---|
| Family identity | `7dbe1393-matvec-ragged-tail-v2` |
| Purpose | `consumer-family` |
| Ledger | `dev/bench_results/7dbe1393/matvec-ragged-tail-v2-ledger.jsonl`, an empty genesis file before the pilot |
| Holdout | none declared |

The family's ledger holds no reservation of the first family, whose ledger
holds no confirmatory reservation; this family's first confirmation is
attempt 1 and the *Sampling and comparisons* arithmetic of the first addendum
applies to it.

## Arms, cells and rules

The arms, the six cells with their workload seeds, the sampling, the margins,
the resolution ceiling, the complexity budget, the decision rule and the
search and stopping rule are those of the first addendum. The harness holds
the cell table once and both families transcribe it; its contract test
`the_second_ragged_tail_family_repeats_the_first_on_its_own_ledger` holds the
two transcriptions equal outside the family identity, description and ledger.

## Host condition

The pilot's queue line starts the launcher only below a one-minute load
average of 2 and otherwise leaves the job queued. The decision rule reads the
confirmation's acceptance summary, and this family's resolution and decision
read this family's receipts alone.
