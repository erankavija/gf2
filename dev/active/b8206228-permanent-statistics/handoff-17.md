# Handoff — Empirical permanent statistics of random matrices over small prime fields (b8206228) — session 21

**Date:** 2026-08-29
**Session number:** 21
**Prior handoffs:** `handoff.md` (session 3), `handoff-2.md` (4), `handoff-3.md` (5), `handoff-4.md` (6), `handoff-5.md` (7), `handoff-6.md` (8), `handoff-7.md` (10), `handoff-8.md` (11), `handoff-9.md` (12), `handoff-10.md` (14), `handoff-11.md` (15), `handoff-12.md` (16), `handoff-13.md` (17), `handoff-14.md` (18), `handoff-15.md` (19), `handoff-16.md` (20). Their unresolved traps remain in force.

## Current state

- Epic: `b8206228` — state: backlog; epic gates remain pending.
- Wave in progress: wave 8 of 13.
- Children summary: 15 done, 2 in_progress, 0 ready, 13 backlog, 0 rejected across the 30-item execution-wave plan.
- Active claims: `7a816262` — `agent:codex`, claimed 2026-08-17, rework count 2 = MAX; `ec22205e` — `agent:worker`, claimed 2026-08-29, rework count 2 = MAX, v5 evidence committed, all registered gates must be rerun before closure.
- Open escalations: None.
- Progress file: `progress.json` in `dev/active/b8206228-permanent-statistics`, the epic artifact directory returned by `jit doc dir b8206228 dev/active` (reflects the above).

## What just happened

- Owner authorized immediate execution of the independently reviewed determinant v5 cohort.
- Final preflight proved HEAD `3d1c536a`, a clean worktree, source-closure byte identity with reviewed source commit `015e2cda`, binary SHA-256 `3285beacce410ffaa7d0dbbf9e8500545b38324c800264d8c9a59ad8359788d0`, absent v5 output/scratch paths, and no Cargo/rustc/nextest process.
- Ran exactly one cohort under `dev/scripts/ccx1-bench-flock.sh`; no replacement or retry occurred. The wrapper reported that it could not raise niceness in the sandbox but continued with observed affinity `6-11`.
- The cohort wrote exactly 315 outcomes across 63 cells. All 315 are measured; there are no failed, censored, replaced, or extended outcomes. The recorded timing window spans 411.392731 seconds.
- Rendered and validated the canonical artifacts. Receipt SHA-256: `d3779aa3817bbefc995b858527bf418b4d3931a8f5406281eb4f0b71a7b45e77`; report SHA-256: `839ab3f1f22a995cc32bc6a7102f4c6aad721b6eff5713d3976df36838d2334a`.
- The largest projected upper 95% interval endpoint is 50.523272 seconds at the fixed campaign sample counts; every cell fits the 43,200-second operational ceiling conservatively.
- Committed immutable evidence at `0273d0e3` and linked the v5 CSV/report to `ec22205e`. The raw five-process scratch files remain under `/tmp/gf2-ec22205e-determinant-cost-v5` for diagnostics; they are not additional cohort outcomes.
- Per the owner's instruction, stopped this session after preserving the run and writing this handoff. Formal v5 gates have not yet been rerun; their recorded statuses still describe the superseded v4 review chain.

## What to do next

- [ ] Revalidate the committed v5 CSV/report byte-for-byte with `python3 dev/benchmarks/permanent_campaign/determinant_cost_v5.py validate --receipt dev/benchmarks/permanent_campaign/determinant-cost-all-cells-v5.csv --report dev/benchmarks/permanent_campaign/determinant-cost-all-cells-v5.md`.
- [ ] Run every registered `ec22205e` gate against the clean current tree: `cargo-ci`, then `code-review`, `doc-review`, and `research-review`. Do not trust the prior v4 pass/fail records.
- [ ] Read the full lead-review protocol and perform the cumulative six-tier review, including every historical code/research finding, stale-narrative sweep, deferred-item audit, and canonical-cutover check.
- [ ] If all gates and lead review pass, close `ec22205e` and record the closure in `progress.json`. Rework count is at MAX; any new failure requires escalation rather than source repair.
- [ ] Resume `7a816262` only after `ec22205e` closes. Finalize backend selection and the root campaign manifest under the recorded `cha_cha20` and 12/12-finite eligibility decisions.

## Traps — do not repeat these

- **Do NOT rerun, replace, extend, or repair the v5 cohort.** The preregistered one-shot execution is complete with 315 measured outcomes and committed at `0273d0e3`; a second run would violate the cohort boundary.
- **Do NOT edit the v5 CSV or rendered report.** Regeneration from modified rows or source would destroy immutability. Validate the committed bytes; if a formal gate finds a defect, escalate because rework is at MAX.
- **Do NOT treat evidence commit `0273d0e3` as the producing source revision.** Every receipt row correctly records runtime HEAD `3d1c536a`; the later commit only adds the immutable artifacts.
- **Do NOT infer elevated process priority.** The wrapper emitted `nice: cannot set niceness: Permission denied`; the receipt records the actual host, powersave governor, enabled boost, and observed affinity `6-11`. The evidence supports the large determinant budget margin, not close performance comparisons.
- **Do NOT treat the `/tmp` scratch files as replacement candidates or extra observations.** They are the five raw process outputs already merged into the committed 315-row receipt.
- **Do NOT reuse the prior formal gate statuses.** Cargo/code/doc passes and the research failure are from v4; evaluate all four gates again against v5.
- **Do NOT add v4/v5 dispatch, aliases, `--cohort`, or legacy schema reading.** Canonical-cutover remains binding; historical v3/v4 evidence stays immutable and unlinked from the active issue.
- **Do NOT run Cargo/nextest concurrently with another build or test process.** The repository shares one target directory.
- All unresolved traps in `handoff-16.md` and earlier handoffs remain in force.

## Open questions needing invoker input

None.

## Reference artefacts

- Epic: `jit issue show b8206228`
- Active freeze: `jit issue show 7a816262`
- Determinant prerequisite: `jit issue show ec22205e`
- Progress: `dev/active/b8206228-permanent-statistics/progress.json`
- Prior handoff: `dev/active/b8206228-permanent-statistics/handoff-16.md`
- V5 preregistration: `dev/benchmarks/permanent_campaign/determinant-cost-preregistration-v5.md`
- V5 machine receipt: `dev/benchmarks/permanent_campaign/determinant-cost-all-cells-v5.csv`
- V5 rendered report: `dev/benchmarks/permanent_campaign/determinant-cost-all-cells-v5.md`
- V5 runner and validator: `dev/benchmarks/permanent_campaign/determinant_cost_v5.py`
- V5 behavioral tests: `dev/benchmarks/permanent_campaign/test_determinant_cost_v5.py`
- Producing source revision: `3d1c536a110d05bcba6603f5a72c0ea185dc9bde`
- Evidence commit: `0273d0e3`
- Raw diagnostic scratch: `/tmp/gf2-ec22205e-determinant-cost-v5`
