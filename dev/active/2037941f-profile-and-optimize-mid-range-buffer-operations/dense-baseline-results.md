# Dense-parity baseline results

> **Diátaxis Type:** Research
>
> **Owning issue:** `c73ffa25`

The [isolated fused-parity receipt](../../bench_results/2037941f/2037941f-dense-isolated-fused-parity/v4-r1-pilot/receipt.json)
and [allocated matvec receipt](../../bench_results/2037941f/2037941f-dense-allocated-matvec/v4-r1-pilot/receipt.json)
pin the frozen inputs, arm executables, runtime observations, raw paired
samples, checkpoints, commands, and append-only execution logs. Their
respective [isolated acceptance summary](../../bench_results/2037941f/2037941f-dense-isolated-fused-parity/v4-r1-pilot/acceptance-summary.md)
and [allocated acceptance summary](../../bench_results/2037941f/2037941f-dense-allocated-matvec/v4-r1-pilot/acceptance-summary.md)
record accepted exploratory pilots. The source of each cell's decision,
interval, sample count, and finding count is its acceptance summary.

Each receipt's `execution.log` contains one `cell-start` and one
`cell-complete` for every addendum cell, all with the declared pair count and
`measured` status, followed by `complete`. The receipt-local checkpoint
manifest and unit files preserve the bounded sessions. The
[isolated ledger](../../bench_results/2037941f/dense-isolated-fused-parity-ledger.jsonl)
and [allocated ledger](../../bench_results/2037941f/dense-allocated-matvec-ledger.jsonl)
retain their pilot reservations. These identity comparisons measure baseline
resolution; they select no production candidate. The allocated summary also
preserves the scalar-reference decisions for the declared warm cells.

The [release validation](survey/dense-baseline-validation.txt) and
[shared staged smoke](survey/dense-baseline-smoke.txt) establish output parity,
canonical bit indexing, zero tail padding, boundary behavior, and resumability.
The [assembly indexes](dense-baseline-preparation.md#release-assembly) identify
the current fused SIMD and scalar paths. Cost attribution requires the
scheduled profile.

## Profile and portfolio

The preserved [profile execution log](../../bench_results/2037941f/dense-baseline-profile/execution.log)
records a launch failure in its first cell before a completed profile pass.
The request encoder appends a newline to compact JSON, which the arm's
byte-exact fresh-child parser rejects. The
[wire regression](survey/test-dense-profile-request.py) checks the encoder's
actual output. The [profile launcher](survey/run-dense-profile.sh) uses a fresh
output directory for its next run, so the failed attempt remains intact.

The portfolio remains bounded by the [frozen addendum](dense-parity-addendum.md#search-and-stopping-rules).
The retained fused AND-popcount route and the rejected generic carry-save
route remain its prior evidence. No distinct fusion identity is selected while
the profile provides no cost signal. The profile's counter and annotation
evidence determines whether this issue freezes one supported candidate or a
no-candidate outcome before any candidate samples are collected.
