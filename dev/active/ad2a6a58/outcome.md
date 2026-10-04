# Shipped GF(2^8) axpy lane: confirmation outcome (jit:ad2a6a58)

> **Diátaxis Type:** Research

The shipped GF(2^8) product-table lane of `FieldVec::axpy` is retained. Every
confirmatory cell of the frozen family records `pass`, and the direction
agrees with the pinned vector-family confirmation in every cell that receipt
shares with this family. The [stage plan](plan.md) states what a pair
compares; the generated [tables](../../bench_results/ad2a6a58/tables.md) hold
every figure this record points at.

## Evidence

| Stage | Receipt | Acceptance summary | Execution log |
|---|---|---|---|
| Pilot | [`receipt.json`](../../bench_results/ad2a6a58/r1-axpy-pilot/receipt.json) | [`acceptance-summary.md`](../../bench_results/ad2a6a58/r1-axpy-pilot/acceptance-summary.md) | [`execution.log`](../../bench_results/ad2a6a58/r1-axpy-pilot/execution.log) |
| Confirmation | [`receipt.json`](../../bench_results/ad2a6a58/r1-axpy-confirmation/receipt.json) | [`acceptance-summary.md`](../../bench_results/ad2a6a58/r1-axpy-confirmation/acceptance-summary.md) | [`execution.log`](../../bench_results/ad2a6a58/r1-axpy-confirmation/execution.log) |

The [confirmation addendum](addendum-v4-axpy-confirmation.json) is derived
from the committed pilot receipt by the canonical freezer; its
[derivation record](confirmation-derivation-axpy.txt) names the resolution, the
replaced equivalence margin, and the retained and dropped cells with the reason
for each drop. The [family ledger](../../bench_results/ad2a6a58/axpy-family-ledger.jsonl)
holds one reservation per campaign, the pilot's at the genesis predecessor and
the confirmation's chained to it; the tables' *Family ledger* section lists
both, and the family has no voided attempt.

## Campaign verification

Each campaign is verified from its own execution log with the shared
[log verifier](../../scripts/verify-campaign-log.py) against its receipt and
the plan committed beside it: every declared cell starts, checkpoints and
completes once with status `measured` at its declared pair count, the session
that reached its cell budget closes with `paused`, and the resumed session
closes with the single terminal `complete` record. Each
[launcher log](../../bench_results/ad2a6a58/r1-axpy-confirmation/launcher.log)
keeps the invocation, the digests of the addendum, plan and executables, each
session's exit status and the host load average at start and finish.

The canonical evaluator, `benchmark-acceptance` over each receipt directory,
reproduces both committed summary files byte for byte. Each summary's verdict
and qualification are on its stage's first line in the tables, and its finding
count is on its row of the tables' *Source* section.

## Verdict per cell

The confirmation is the family's first attempt and all of its cells are
confirmatory. The tables' *Confirmation* section holds one row per cell with
its outcome, pair count, bootstrap interval and flagged-window count.

| Cell | Role | Recorded outcome | Table row |
|---|---|---|---|
| `axpy-4k-element` | confirmatory | `pass` | *Confirmation*, `axpy-4k-element` |
| `axpy-128k-element` | confirmatory | `pass` | *Confirmation*, `axpy-128k-element` |
| `axpy-2m-stream-element` | confirmatory | `pass` | *Confirmation*, `axpy-2m-stream-element` |
| `axpy-4k-wide` | confirmatory | `pass` | *Confirmation*, `axpy-4k-wide` |
| `axpy-128k-wide` | confirmatory | `pass` | *Confirmation*, `axpy-128k-wide` |
| `axpy-2m-stream-wide` | confirmatory | `pass` | *Confirmation*, `axpy-2m-stream-wide` |
| `axpy-1k-cold-element` | exploratory | `pilot` | *Pilot*, `axpy-1k-cold-element` |
| `axpy-1k-cold-wide` | exploratory | `pilot` | *Pilot*, `axpy-1k-cold-wide` |
| `axpy-8m-element` | exploratory | `pilot` | *Pilot*, `axpy-8m-element` |
| `axpy-8m-wide` | exploratory | `pilot` | *Pilot*, `axpy-8m-wide` |

No cell records `fail`, `not-material`, `regressed`, `inconclusive` or
`not-confirmatory`. Each confirmatory interval lies wholly above the frozen
worthwhile-speedup threshold in the addendum's `effect` block, which is what the
evaluator's `improved` decision in the acceptance summary states.

The four exploratory cells carry pilot evidence only and no confirmatory
verdict. The derivation record gives the reason each was dropped from the
confirmatory selection. The two 1 KiB cold cells are the family's only cells
below the smallest confirmatory size and the only ones whose candidate
executions build the product table inside a timed window: their *Pilot* rows
show the flagged windows that build produces and the widest intervals of the
family, and their pilot estimates lie on the same side of one as every other
row. The pilot's lane witness shows the table build in the candidate arm of
every cell, cold or warm. The first-touch regime therefore has an exploratory
direction and no confirmed size.

## Lane witness

The tables' *Confirmation lane witness* section records the path each arm
reported after its timed windows: every baseline execution of every cell
reports the declined lane and every candidate execution the product-table
lane, at the executions count the cell's pair count implies. A pair therefore
differs in the lane the frozen question names.

## Agreement with the pinned vector-family confirmation

The [pin](pinned-vector-confirmation.json) names the accepted confirmation
receipt of the earlier vector family under
`dev/bench_results/19513245/r1-vector-confirmation/` by path and SHA-256; the
tables' *Source* section repeats both digests from the files themselves. That
receipt is cited for agreement and contributes no sample to this family.

The tables' *Direction agreement* section compares cell by cell. The three
confirmatory cells the pinned receipt also measures — `axpy-4k-element`,
`axpy-128k-element` and `axpy-128k-wide` — record `agrees`. The other three
confirmatory cells have no pinned counterpart and their rows say so; the
pinned receipt's `axpy-8m-element` cell has a counterpart only in this
family's exploratory pilot.

The sizes do not match. In each of the three shared rows this family's
estimate is the larger of the two, and the pinned interval of the same cell in
the pinned acceptance summary lies below this family's interval. The gap is
widest in the two element-representation rows and narrowest in the wide row.
The two campaigns measure different paths, as the note above that table
states, so the pinned sizes are not a prediction this result confirms or
refutes.

## Decision

The frozen rule in the family description of both addenda retains the
accelerated lane when no confirmatory cell records `fail` and at least one
records `pass`, and removes it otherwise. The confirmation's acceptance
summary records `pass` in all of its cells with an accepted, qualifying
verdict, so the rule yields retention: `gf2-core` keeps the cached
product-table lane as the route `FieldVec::axpy` takes for GF(2^8).

## Untimed smoke

The committed smoke records, [pilot](survey/pilot-smoke.json) and
[confirmation](survey/confirmation-smoke.json), are written by the shared
`benchmark-ab-runner smoke` over a throwaway plan of each stage: both arms of
every declared cell, one dispatch each in the validation position, no timing
window. They describe the arm built from the current tree. The record each
timed campaign pins as a build input is `survey/runner-smoke.txt` in that
receipt's producing-input snapshot; it comes from a family-local driver over
the runner's request encoder and result parser, and covers the pilot plan's
arms and cells, of which the confirmation's are a subset.
