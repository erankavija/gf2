# Execution batching guidance for epic b8206228

> **Diátaxis Type:** How-to — dispatch guidance for `jit-execution-lead` when
> running this epic's breakdown. Owner-approved 2026-08-09.

The breakdown's plan-review rounds split several thin leaves onto the critical
path (20 issues deep). The graph is approved and stays as-is; recover the
dispatch overhead by assigning each serial run below to **one worker**, who
implements and closes each issue in sequence. Every issue still closes
individually through its own gates, and the lead runs all gate transitions —
workers never invoke `jit gate` or state updates.

| Batch | Serial run (dependency order) | Rationale |
|---|---|---|
| A | `rng-dependency-migration` → `rng-exception-record` → `stats-crate-foundation` → `stats-sampler` | One `gf2-stats` bring-up chain; the middle two leaves are minutes of work each (two Cargo.toml comments; an empty crate skeleton). |
| B | `stats-wilson-interval` → `stats-clopper-pearson` → `stats-exact-tests` | Three estimator leaves in one module (`crates/gf2-stats/src/intervals.rs` + exact tests); shared context dominates. |
| C | `checkpoint-generalization` → `checkpoint-caller-migration` | Genericise, then move the one existing caller; the migration leaf is two criteria over pre-existing tests. |
| D | `driver-batch-parallel` → `driver-cpu-backends` | New path plus the selection layer over it; both edit the campaign backend module. |

Leaves not listed dispatch normally. Batching changes worker assignment only:
no issues are merged, no criteria change, and the per-issue gate record stays
intact.
