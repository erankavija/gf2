# Confirming the shipped GF(2^8) axpy lane

> **Diátaxis Type:** Explanation

Issue `@/issue/ad2a6a58` is the vector confirmation leaf of
[the cached-table design](../613574db/design.md). It measures the GF(2^8)
product-table lane `gf2-core` ships against the route `FieldVec::axpy` takes
without it, as a protocol-version-4 A/B family with a pilot and a
confirmation.

## What a pair compares

Both arms of every pair are one executable, `gf256-axpy-arm`, built from the
shipped crate with the `simd` and `test-support` features. `test-support` is
where `gf2m::byte_table` compiles the process-global lane switch and the lane
witness, so a release benchmark executable reaches both without any change to
production selection behaviour; the design fixes this
([design](../613574db/design.md) § Contracts this design preserves, scalar
fallback). The baseline arm calls `force_scalar_gf256_table(true)` before its
first dispatch, so every GF(2^8) call runs the consumer's scalar element loop;
the candidate arm leaves the switch clear. One executable, one build identity,
one set of operands, one entry point, and the lane is the only difference.

Enabling `test-support` costs the shipped dispatch one relaxed atomic load per
`axpy` call — per call, not per element — and it is the same cost on both arms
of a pair, so it cancels in the ratio the estimator forms. It is the price of
holding the baseline arm on the scalar lane at all.

Each execution reports the lane after its timed windows, because a `cold` cell
performs its first dispatch inside its first window and no earlier reading
would describe the measured calls. The reported path carries the witness's
lane name and the number of product tables the process built, so a first-touch
execution is distinguishable from one that found the table cached.

## Stages

| Stage | Artifact | State |
|---|---|---|
| Frozen pilot family | [`addendum-v4-axpy-pilot.json`](addendum-v4-axpy-pilot.json) | committed before launch |
| Genesis ledger | [`axpy-family-ledger.jsonl`](../../bench_results/ad2a6a58/axpy-family-ledger.jsonl) | open and empty |
| Runner-wire smoke | [`survey/runner-smoke.txt`](survey/runner-smoke.txt) | every arm, operation and cache state through the real runner |
| Pilot campaign | `dev/bench_results/ad2a6a58/r1-axpy-pilot` | queued for the benchmark window |
| Confirmation addendum | `addendum-v4-axpy-confirmation.json` | frozen from the committed pilot receipt |
| Confirmation campaign | `dev/bench_results/ad2a6a58/r1-axpy-confirmation` | queued after the freeze |
| Published outcome | [`tables.md`](../../bench_results/ad2a6a58/tables.md) | generated from committed receipts |

The freezer derives the measurement resolution from the whole pilot, dropped
cells included, so a cold cell's width sizes the confirmation's margins even
though no cold cell is retained. A resolution the frozen margins cannot clear
is not a reason to narrow the pilot after the fact: the freezer's margin
options replace a margin with a new rationale, and its record names the
replacement.

The pilot covers both element representations at five shapes: 1 KiB cold,
4 KiB warm, 128 KiB warm, 8 MiB warm and 2 MiB rotated through the eight
fixture banks. The frozen addendum states which six of those ten cells the
confirmation retains, why each dropped cell is dropped, and the rule that
decides retention or removal of the shipped lane; the freezer's derivation
record repeats the selection beside the confirmation addendum.

## After the window

A receipt directory carries the producing-input snapshot, whose nested
`Cargo.lock` files `.gitignore` excludes: committing a receipt means
`git add -f` on them followed by
`python3 dev/scripts/check-receipt-input-snapshots.py`, which reads the index
rather than the working tree and fails CI on an omission.

The post-window session commits the pilot receipt, runs
`dev/bench_results/ad2a6a58/run-axpy-confirmation.sh freeze`, commits the
confirmation addendum and its derivation record, queues the confirmation line,
and then publishes: `run-axpy-confirmation.sh tables` regenerates the result
tables from the committed receipts, the family ledger and the pinned
vector-family confirmation, and the findings state each cell's recorded
verdict, the retention decision the frozen rule yields and whether the
direction agrees with that pinned receipt. Every quantitative statement points
at a generated table row rather than repeating it.
