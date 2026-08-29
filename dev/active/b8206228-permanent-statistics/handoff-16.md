# Handoff — Empirical permanent statistics of random matrices over small prime fields (b8206228) — session 20

**Date:** 2026-08-29
**Session number:** 20
**Prior handoffs:** `handoff.md` (session 3), `handoff-2.md` (4), `handoff-3.md` (5), `handoff-4.md` (6), `handoff-5.md` (7), `handoff-6.md` (8), `handoff-7.md` (10), `handoff-8.md` (11), `handoff-9.md` (12), `handoff-10.md` (14), `handoff-11.md` (15), `handoff-12.md` (16), `handoff-13.md` (17), `handoff-14.md` (18), `handoff-15.md` (19). Their unresolved traps remain in force.

## Current state

- Epic: `b8206228` — state: backlog; epic gates remain pending.
- Wave in progress: wave 8 of 13.
- Children summary: 15 done, 2 in_progress, 0 ready, 13 backlog, 0 rejected across the 30-item execution-wave plan.
- Active claims: `7a816262` — `agent:codex`, claimed 2026-08-17, rework count 2 = MAX; `ec22205e` — `agent:worker`, claimed 2026-08-29, rework count 2 = MAX after the owner's second reset, but independent premeasurement readiness review is PASS.
- Open escalations: None. `ec22205e` is waiting only for the owner's measurement schedule.
- Progress file: `progress.json` in `dev/active/b8206228-permanent-statistics`, the epic artifact directory returned by `jit doc dir b8206228 dev/active` (reflects the above).

## What just happened

- Owner selected option A on the same-finding escalation, authorizing exactly the missing-column-`None` versus explicit-empty-string correction and its regression.
- Recorded the authorization at `ac11a5e2`; dispatched the frontier worker without timing, wrapper, Cargo, formal-gate, or JIT authority.
- Worker committed `015e2cda`: only `None` is interrupted-tail eligible; full-width empty identity and numeric fields now fail strict validation. Python tests reached 24/24 and binary self-check passed.
- Fresh independent readiness review returned PASS with zero findings. It rechecked physical truncation before process/q/n, valid-address truncation, exit-zero censoring, empty/header-only input, all malformed/hard-failure cases, 315-row persistence, canonical-cutover, historical evidence, RNG identity, uncertainty arithmetic, pinned baseline, stale narrative, and deferred items.
- Reviewed binary: `target/release/deps/determinant_companion-214102561bb5c0a5`; SHA-256 `3285beacce410ffaa7d0dbbf9e8500545b38324c800264d8c9a59ad8359788d0`.
- Source closure and worktree are clean. No v5 CSV, rendered report, or `/tmp/gf2-ec22205e-determinant-cost-v5` scratch directory exists. No benchmark timing or wrapper invocation has occurred.

## What to do next

- [ ] Obtain the owner's exact uncontended measurement window; reserve 10–15 uninterrupted minutes. Do not start early.
- [ ] Immediately before launch, verify the current clean HEAD, confirm every path in `RELEVANT_SOURCE_PATHS` is byte-identical to reviewed source commit `015e2cda`, verify the exact binary SHA-256, CPU affinity admission, and absence of v5 receipt/report/scratch paths. Later JIT readiness/handoff commits are outside the source closure; abort if any producing path differs.
- [ ] Run exactly one locked cohort with `./dev/scripts/ccx1-bench-flock.sh python3 dev/benchmarks/permanent_campaign/determinant_cost_v5.py run --binary target/release/deps/determinant_companion-214102561bb5c0a5 --output dev/benchmarks/permanent_campaign/determinant-cost-all-cells-v5.csv --scratch-dir /tmp/gf2-ec22205e-determinant-cost-v5`.
- [ ] Preserve every process outcome. Do not repair, replace, extend, or rerun based on measurements or censoring.
- [ ] Render and validate the v5 report, commit immutable CSV/report evidence, update issue links, and run all registered formal gates before closing `ec22205e`.
- [ ] Resume `7a816262` only after `ec22205e` closes; finalize backend selection and the root campaign manifest under the recorded A/A decisions.

## Traps — do not repeat these

- **Do NOT start timing before the owner's scheduled window.** Readiness PASS authorizes asking for the schedule, not autonomous execution.
- **Do NOT rebuild, edit, commit, or otherwise change the source closure during the scheduled run.** The reviewed HEAD and binary digest are part of the producing identity.
- **Do NOT retry or replace any failed/censored outcome.** V5 preregisters one five-process cohort; its complete receipt preserves contradictions and failures.
- **Do NOT conflate missing columns with explicit empty values.** Only `DictReader` `None` values identify a structurally short interrupted tail; a present empty field is malformed and fails hard.
- **Do NOT add v4/v5 dispatch, aliases, `--cohort`, or legacy schema reading.** Canonical-cutover remains binding; historical v3/v4 evidence stays immutable.
- **Do NOT run concurrent Cargo/nextest or other substantial CPU work during measurement.** The determinant cohort needs an uncontended host and exclusive wrapper affinity `6-11`.
- All unresolved traps in `handoff-15.md` and earlier handoffs remain in force.

## Open questions needing invoker input

- Question: What exact local date and start time should the lead use for the one-shot determinant v5 cohort?
  - Context: Independent readiness review passed for the producing source closure at commit `015e2cda`; later commits record readiness metadata only. The CPU cohort needs 10–15 uninterrupted minutes and must not begin before the owner's chosen window.
  - Options: Any owner-selected uncontended window with at least 15 minutes reserved.
  - Recommendation: Choose a window when no other repository build, test, benchmark, simulation, or external CPU-heavy load will run.

## Reference artefacts

- Epic: `jit issue show b8206228`
- Active freeze: `jit issue show 7a816262`
- Determinant prerequisite: `jit issue show ec22205e`
- Progress: `dev/active/b8206228-permanent-statistics/progress.json`
- Prior handoff: `dev/active/b8206228-permanent-statistics/handoff-15.md`
- V5 preregistration: `dev/benchmarks/permanent_campaign/determinant-cost-preregistration-v5.md`
- V5 runner: `dev/benchmarks/permanent_campaign/determinant_cost_v5.py`
- V5 tests: `dev/benchmarks/permanent_campaign/test_determinant_cost_v5.py`
- Historical v4 evidence: `dev/benchmarks/permanent_campaign/determinant-cost-all-cells-v4.csv`, `determinant-cost-all-cells-v4.md`
- Readiness commits: `ac550959` (preregistration), `6348b0c9` (canonical cutover), `1ddcc266` (valid-address truncation), `8af81b55` (short-tail retention), `015e2cda` (explicit-empty rejection)
