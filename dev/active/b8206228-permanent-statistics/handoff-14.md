# Handoff — Empirical permanent statistics of random matrices over small prime fields (b8206228) — session 18

**Date:** 2026-08-29
**Session number:** 18
**Prior handoffs:** `handoff.md` (session 3), `handoff-2.md` (4), `handoff-3.md` (5), `handoff-4.md` (6), `handoff-5.md` (7), `handoff-6.md` (8), `handoff-7.md` (10), `handoff-8.md` (11), `handoff-9.md` (12), `handoff-10.md` (14), `handoff-11.md` (15), `handoff-12.md` (16), `handoff-13.md` (17). Their unresolved traps remain in force.

## Current state

- Epic: `b8206228` — state: backlog; epic gates remain pending.
- Wave in progress: wave 8 of 13.
- Children summary: 15 done, 2 in_progress, 0 ready, 13 backlog, 0 rejected across the 30-item execution-wave plan.
- Active claims: `7a816262` — `agent:codex`, claimed 2026-08-17, rework count 2 = MAX; `ec22205e` — `agent:worker`, claimed 2026-08-29, rework count 2 = MAX after the owner-approved reset.
- Open escalations: `ec22205e` needs owner authorization for one final bounded failure-retention repair or a decision to leave wave 8 blocked. No measurement schedule is requested at the current HEAD.
- Progress file: `progress.json` in `dev/active/b8206228-permanent-statistics`, the epic artifact directory returned by `jit doc dir b8206228 dev/active` (reflects the above).

## What just happened

- Recorded the owner's A/A freeze decisions: `cha_cha20` is canonical, and backend eligibility requires 12/12 finite measurements with incomplete/censored arms preserved but ineligible.
- Completed the v4 determinant cohort and all ordinary formal gates; research-review then failed on absent raw RNG identity, absent timing intervals, and an unpinned fixed-N/twelve-hour baseline.
- Escalated at the original rework MAX. The owner chose a canonical v5 rerun, required canonical-cutover, and directed the lead to return for scheduling only when preparation was ready.
- Committed v5 preregistration before producing source at `ac550959`; committed the v5-only runner/test/harness cutover at `6348b0c9`. Historical v3/v4 preregistrations, CSVs, and reports remain byte-identical evidence, not compatibility inputs.
- Prepared release binary `target/release/deps/determinant_companion-214102561bb5c0a5`, SHA-256 `3285beacce410ffaa7d0dbbf9e8500545b38324c800264d8c9a59ad8359788d0`; self-check, Rust 1.95 focused checks, and Python tests passed without timing.
- Independent readiness review found that an addressed but truncated scratch row could be mislabeled measured and prevent the complete censored receipt. Rework-2 committed `1ddcc266`, validating complete scratch rows before measurement classification and adding signal/exit-zero truncation regressions.
- Final independent review proved an earlier truncation point remains: if the interrupted row ends before `q`, `n`, or `process_index` is parseable, address indexing raises `int(None)` before the 315 planned outcomes can be written. Rework is at MAX; escalated instead of taking another source pass.
- Cut the issue's active document references from v4 to the v5 preregistration, runner, and tests. The v5 receipt/report do not exist and are not linked.
- No benchmark wrapper/run command was invoked. No v5 CSV, rendered report, or `/tmp/gf2-ec22205e-determinant-cost-v5` scratch directory exists.

## What to do next

- [ ] Check the owner response to the `ec22205e` MAX-rework escalation.
- [ ] If the owner authorizes the recommended repair, reset the counter explicitly and dispatch one bounded v5-only fix: accept only an incomplete unparseable final scratch record as interrupted output, preserve complete preceding rows, and synthesize censoring for every missing planned address. Keep duplicate complete addresses and non-tail malformed data fail-closed.
- [ ] Require tests for truncation before parseable address fields, valid-address truncation, exit-zero harness censoring/nonzero cohort result, complete 315-row persistence/reload, and preservation of all cumulative RNG/interval/baseline/canonical-cutover findings.
- [ ] Obtain a fresh independent readiness PASS at the repaired HEAD. Any further failure returns to the owner; do not schedule or time on a failed review.
- [ ] Once readiness passes, report the exact HEAD, binary path/hash, clean source closure, absent output paths, and expected 10–15 minute uninterrupted CPU window to the owner; wait for the owner's schedule.
- [ ] At the authorized time only, run the single locked v5 cohort, then render, validate, commit immutable evidence, link the v5 CSV/report, and run all registered formal gates before closing `ec22205e`.
- [ ] Resume `7a816262` only after `ec22205e` closes; then finalize backend selection and the root campaign manifest under the already recorded A/A decisions.

## Traps — do not repeat these

- **Do NOT start or schedule v5 timing at `1ddcc266`.** Final readiness review reproduces `int(None)` when a signal truncates the final scratch row before its address fields are parseable; the complete receipt is not written.
- **Do NOT consider valid-address truncation coverage exhaustive.** Rework-2 tests truncation after `sample_count`; the Rust harness uses sequential `writeln!`, so interruption can leave any prefix of the final CSV record. Handle the unparseable final tail explicitly.
- **Do NOT turn malformed scratch input into a permissive legacy reader.** Only the realistically interrupted final record may be discarded into planned censoring; duplicate complete addresses and non-tail corruption remain hard errors.
- **Do NOT add v4/v5 dispatch, aliases, `--cohort`, or a legacy schema reader.** The owner explicitly requires canonical-cutover. Active runner, tests, Rust harness, and schema remain v5-only; historical v3/v4 evidence stays immutable.
- **Do NOT rewrite or delete historical v3/v4 preregistrations, CSVs, or reports.** They are falsification evidence even though the issue's active links now point to v5.
- **Do NOT omit the benchmark wrapper from the dirty-source closure.** The original v3 code-review failure proved this makes provenance unreliable; v5 includes and mutation-tests `dev/scripts/ccx1-bench-flock.sh`.
- **Do NOT conflate the pooled raw estimator with the uncertainty interval.** V5 preregisters a five-process mean and two-sided 95% Student-t interval separately; the projected upper interval endpoint controls the 43,200-second verdict.
- **Do NOT reconstruct RNG or budget identity only from source history.** Every v5 row must carry the MMIX LCG algorithm/version/entry mapping and the baseline path, revision, SHA-256, ceiling, reserve, and productive-compute allowance.
- **Do NOT run concurrent Cargo/nextest suites or unapproved measurement.** The shared target is serialized. CPU determinant timing awaits the owner's exact schedule; GPU-heavy work retains the standing post-02:00 restriction.
- All unresolved traps in `handoff-13.md` and earlier handoffs remain in force.

## Open questions needing invoker input

- Question: May the lead take one final bounded source-only repair after the MAX-rework failure?
  - Context: V5 preparation is otherwise complete and independently audited, but early interruption of the final scratch row can still abort before the promised complete censored receipt is written.
  - Options: A) reset the counter for one tail-aware failure-retention repair plus fresh independent review; B) stop the v5 rerun and leave `ec22205e`, wave 8, and the epic blocked.
  - Recommendation: A. The remaining defect is local and precisely reproduced, and the repair does not change the preregistered statistics, schema, historical evidence, or canonical-cutover.

## Reference artefacts

- Epic: `jit issue show b8206228`
- Active freeze: `jit issue show 7a816262`
- Determinant prerequisite: `jit issue show ec22205e`
- Progress: `dev/active/b8206228-permanent-statistics/progress.json`
- Prior handoff: `dev/active/b8206228-permanent-statistics/handoff-13.md`
- V5 preregistration: `dev/benchmarks/permanent_campaign/determinant-cost-preregistration-v5.md`
- V5 runner: `dev/benchmarks/permanent_campaign/determinant_cost_v5.py`
- V5 tests: `dev/benchmarks/permanent_campaign/test_determinant_cost_v5.py`
- Historical v4 research evidence: `dev/benchmarks/permanent_campaign/determinant-cost-all-cells-v4.csv`, `determinant-cost-all-cells-v4.md`
- Protocol baseline: `dev/simulation_results/permanent-zero-fraction/protocol.md` at `7901430a324616b00b92c9fc23e3b3dbee66c291`
- Readiness commits: `ac550959` (preregistration), `6348b0c9` (canonical source cutover), `1ddcc266` (valid-address truncation repair)
