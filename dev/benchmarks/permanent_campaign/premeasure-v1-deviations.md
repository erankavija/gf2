# Premeasurement v1 deviation record

This file records deviations between the frozen `premeasure-v1` contract
(`premeasure-plan-v1.md`, `premeasure-plan-v1.csv`) and the evidence the frozen
tooling actually produces, together with their recorded resolutions. Per
`@/inv/falsification-preserved`, the contradiction and its evidence stay
recorded here; the plan and the receipts are not rewritten.

## D-01: gpu_hip processes measure their cell twice

**Observed.** Every completed `gpu_hip` process in the durable
`premeasure-v1` cohort (campaign root
`/data/gf2-campaigns/b8206228/premeasure-recovery-20260824`) writes a scratch
CSV with two data rows where the collection contract expected one. At the
2026-08-25 collection, 359 positions across both sessions carried the sole
validity reason `data_row_count_2`; no CPU-backend position did.

**Root cause** (verified in the frozen harness source at `1350d5b4`,
binary `1198cce4…`). `grid_specs` in
`dev/research/permanent-sampling-feas/src/main.rs` emits two GPU specs per
cell, one per entry of `GPU_BATCHES = [256, 1024]`. The runner invokes
`grid --only q=…,n=…,backend=gpu_hip --batch-size <M>`;
`configured_grid_specs` applies the `--batch-size` override to every spec
*before* the `--only` filter runs, so both GPU specs carry the plan's M when
the filter matches on `backend=gpu_hip` alone, and the harness measures the
cell once per spec slot. The two rows follow the identical fixed protocol and
draw from disjoint reserved seed sub-blocks (the first row from the coerced
M=256 grid slot, the second from the native M=1024 slot).

**Resolution** (owner decision, 2026-08-25, recorded against jit issue
`7a816262`; escalation log in the epic's `progress.json`). The measurement
behaviour of the frozen harness and runner is not changed mid-campaign. The
collector — a recovery projection over already-observed evidence, outside the
pinned measurement identity — accepts a two-row `gpu_hip` scratch as
structurally valid and defines the process measurement as the **native-M grid
slot, the second data row**. The first row remains in the scratch file as a
recorded, unused replicate; it enters no ledger, candidate, or pooled figure.
Any other row count, and a two-row scratch on a CPU backend, remains
structurally invalid. The rule is fixed by grid-slot identity before any
measured value is compared, so it is not a result-dependent selection.

The implementing change and its behavioural tests live in
`dev/scripts/permanent-campaign-runner.sh` (`parse_premeasure_scratch`) and
`dev/scripts/permanent-campaign-premeasure.test.sh` (t11). The per-process
receipts, scratch files, and exit statuses produced before the amendment are
byte-identical evidence; only their collection projection changed.
