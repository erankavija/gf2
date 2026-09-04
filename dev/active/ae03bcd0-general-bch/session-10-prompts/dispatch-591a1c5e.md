You are implementing issue 591a1c5e in the current JIT-managed repository.

**Title:** Migrate gf2-coding benchmarks — `jit issue show 591a1c5e` for the full record.
Migrate the gf2-coding Criterion benchmarks consuming legacy BCH constructors to the canonical model, preserving benchmark IDs where the measured behavior is unchanged.

- [hard] REQ-01: Each bench in this task's inventory compiles and runs on the canonical model, keeping comparable benchmark IDs for the non-regression comparison.

Gates: `cargo-ci`, `code-review` (the lead runs them on the merged tree).

Inventory (design file group `cutover-benches`): `crates/gf2-coding/benches/bch_parallel.rs`, `crates/gf2-coding/benches/batch_operations.rs`. Nothing else.

## What matters here

- Benchmark IDs are consumed by the non-regression comparison: `dev/bench_results/88ca7d2f/` (the pre-cutover baseline receipt, 88ca7d2f) records twelve `bch_*` Criterion IDs and its `run.sh` collects them by name; `dev/active/4e732b56/workload-selection.md` §§ 4–7 fix the workloads. Keep every ID whose measured behavior is unchanged (same code parameters, same batch sizes, same cache state) byte-identical; where the canonical model changes what is measured, say so in the bench's rustdoc and report it — do not silently rename.
- The DVB-T2 rows use the canonical DVB-T2 code (97410c80) at the shortened lengths; the mother-length cells stay only where a dated amendment in the survey names them.
- Run each bench once in `--release` with a short measurement (`--warm-up-time 1 --measurement-time 1` or Criterion's `--quick`/`--test` mode) to prove it runs; paste the ID list Criterion prints, and diff it against the baseline receipt's ID list.
- Do not take or commit any measurement receipt; d1b4f85e and fd9d5416 own measurements.

## Verification before you report (paste outputs)
- `./scripts/cargo-budget.sh cargo bench -p gf2-coding --bench bch_parallel --no-run` and the same for `batch_operations`; then the short runs above.
- `./scripts/cargo-ci.sh` from the worktree root; paste the per-step table.
- The REQ grep at HEAD (empty over the two files); `git diff --stat main...HEAD`; the leak-check output.

## Report
Under 2500 characters first: branch tip, per-bench summary, the ID diff against the baseline receipt, the cargo-ci table, footprint findings, anything unverified.
