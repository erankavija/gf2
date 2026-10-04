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
the current fused SIMD and scalar paths. The completed
[profile summary](../../bench_results/2037941f/dense-baseline-profile-r2/profile-summary.md)
attributes sampled costs and counter intervals.

## Profile and portfolio

The preserved [failed launch](../../bench_results/2037941f/dense-baseline-profile/execution.log)
records the rejected newline request. The
[wire regression](survey/test-dense-profile-request.py) protects the byte-exact
fresh-child parser. The completed
[profile log](../../bench_results/2037941f/dense-baseline-profile-r2/execution.log)
and [profile summary](../../bench_results/2037941f/dense-baseline-profile-r2/profile-summary.md)
retain the current cost evidence. The [portfolio outcome](../1a379447-zen3-cpu-performance/c73ffa25/outcome.md)
records the frozen-rule no-candidate decision, preserves the established fused
AND-popcount path and carry-save no-win, and authorizes no production change.
