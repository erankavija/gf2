# Confirming the shipped GF(2^8) dense product

> **Diátaxis Type:** Explanation

Issue `@/issue/4c1e441f` is the dense-product confirmation leaf of
[the cached-table design](../613574db/design.md). It measures the GF(2^8)
product-table route `gf2-core` ships against the route `field::matrix::gemm`
takes without it, as a protocol-version-4 A/B family with a pilot and a
confirmation.

## What a pair compares

Both arms of every pair are one executable, `gf256-gemm-arm`, built from the
shipped crate with the `simd` and `test-support` features. `test-support` is
where `gf2m::byte_table` compiles the process-global lane switch and the lane
witness, so a release benchmark executable reaches both without any change to
production selection behaviour; the design fixes this
([design](../613574db/design.md) § Contracts this design preserves, scalar
fallback). The baseline arm calls `force_scalar_gf256_table(true)` before its
first dispatch, so every GF(2^8) product runs the route the library takes
without the table; the candidate arm leaves the switch clear. One executable,
one build identity, one set of operands, one entry point, and the lane is the
only difference.

Each representation is its own cell because the two reach different routes when
the dispatch declines: the runtime-context element runs a batched dot product
per output cell and the compile-time-configured value runs the whole-product
carry-less-multiply kernel. One cell cannot speak for both, and the arm reports
which route the declining lane names for the representation it holds.

Each execution reports the lane after its timed windows, because the witness
describes the calls that ran. The reported path carries the witness's lane name,
the number of product tables the process built, the declining route's name, and
the allocating calls and bytes one call of the cell's own body makes. That last
pair is the design's RISK-03, the scratch the accepted path allocates per call,
observed per cell rather than asserted. The executable links a counting global
allocator for it; the counters
are armed for one untimed probe call and disarmed for every timed window, where
they cost one relaxed load per allocation on both arms of a pair and cancel in
the ratio the estimator forms.

## Stages

| Stage | Artifact | State |
|---|---|---|
| Frozen pilot family | [`addendum-v4-dense-product-pilot.json`](addendum-v4-dense-product-pilot.json) | committed before launch |
| Genesis ledger | [`dense-product-family-ledger.jsonl`](../../bench_results/4c1e441f/dense-product-family-ledger.jsonl) | open and empty |
| Lane equivalence | [`conformance/lane-equivalence.txt`](conformance/lane-equivalence.txt) | both lanes on every declared shape |
| Arm smoke | [`survey/runner-smoke.txt`](survey/runner-smoke.txt) | every arm, cell and operation, untimed |
| Pilot campaign | `dev/bench_results/4c1e441f/r1-dense-product-pilot` | queued for the benchmark window |
| Confirmation addendum | `addendum-v4-dense-product-confirmation.json` | frozen from the committed pilot receipt |
| Confirmation campaign | `dev/bench_results/4c1e441f/r1-dense-product-confirmation` | queued after the freeze |
| Published outcome | [`tables.md`](../../bench_results/4c1e441f/tables.md) | generated from committed receipts |

Every stage that measures is preceded by an untimed one.
`survey/smoke-arms.sh` projects a throwaway plan from the frozen addendum,
validates it with the runner's own `check`, and drives every arm of every
declared cell through `gf256-gemm-smoke`, which speaks the runner's wire — its
case encoder, fresh-child sentinel, child environment and result parser — in the
`validation` position. Each arm performs one untimed dispatch and reports no
timing window; the smoke refuses one that does, and the record is a build input
of both campaigns, so a timed run refuses to launch until the smoke is
committed. Timing belongs to the queued window alone.

The freezer derives the measurement resolution from the whole pilot, dropped
cells included, so a dropped cell's width sizes the confirmation's margins. A
resolution the frozen margins cannot clear is not a reason to narrow the pilot
after the fact: the freezer's margin options replace a margin with a new
rationale, and its record names the replacement.

The pilot covers both element representations at three square dimensions and, at
the middle dimension, at the whole-matrix consumer boundary, where both operand
conversions and the output conversion join the product inside the window. The
frozen addendum states which six of those eight cells the confirmation retains,
why each dropped cell is dropped, and the rule that decides retention or removal
of the accelerated path; the freezer's derivation record repeats the selection
beside the confirmation addendum.

## Shared machinery

The family owns its declarations and nothing else. The plan projection, the
producing-input manifest, the result tables, the prior-receipt pin, the arm
smoke script and the execution-log verification are the shared forms under
`dev/scripts`, parameterized by the family that calls them; the lane switch, the
element representations and the lane witness are the vector family's arm library
`dev/active/ad2a6a58/survey/axpy-arm`; the byte-region conversions, the consumer
entry points, the runner wire and the smoke driver are the byte-field survey's
`dev/active/6c6b09b1/survey` crates. The vector family calls the same shared
forms, and each of its generated artifacts is byte-identical across the change.

## After the window

A receipt directory carries the producing-input snapshot, whose nested
`Cargo.lock` files `.gitignore` excludes: committing a receipt means
`git add -f` on them followed by
`python3 dev/scripts/check-receipt-input-snapshots.py`, which reads the index
rather than the working tree and fails CI on an omission.

The post-window session commits the pilot receipt, runs
`dev/bench_results/4c1e441f/run-dense-product-confirmation.sh freeze`, commits
the confirmation addendum and its derivation record, queues the confirmation
line, and then publishes: `run-dense-product-confirmation.sh tables` regenerates
the result tables from the committed receipts, the family ledger and the pinned
matrix-family confirmation, and the findings state each cell's recorded verdict,
the retention decision the frozen rule yields and whether the direction agrees
with that pinned receipt. Every quantitative statement points at a generated
table row rather than repeating it.
