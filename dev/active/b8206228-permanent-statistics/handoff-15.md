# Handoff — Empirical permanent statistics of random matrices over small prime fields (b8206228) — session 19

**Date:** 2026-08-29
**Session number:** 19
**Prior handoffs:** `handoff.md` (session 3), `handoff-2.md` (4), `handoff-3.md` (5), `handoff-4.md` (6), `handoff-5.md` (7), `handoff-6.md` (8), `handoff-7.md` (10), `handoff-8.md` (11), `handoff-9.md` (12), `handoff-10.md` (14), `handoff-11.md` (15), `handoff-12.md` (16), `handoff-13.md` (17), `handoff-14.md` (18). Their unresolved traps remain in force.

## Current state

- Epic: `b8206228` — state: backlog; epic gates remain pending.
- Wave in progress: wave 8 of 13.
- Children summary: 15 done, 2 in_progress, 0 ready, 13 backlog, 0 rejected across the 30-item execution-wave plan.
- Active claims: `7a816262` — `agent:codex`, claimed 2026-08-17, rework count 2 = MAX; `ec22205e` — `agent:worker`, claimed 2026-08-29, rework count 1 after the owner's second reset, but MAX_SAME_FINDING_REPEATS is reached.
- Open escalations: `ec22205e` needs owner authorization for the exact missing-column-`None` versus explicit-empty-string correction, or a decision to leave wave 8 blocked. No measurement schedule is requested at the current HEAD.
- Progress file: `progress.json` in `dev/active/b8206228-permanent-statistics`, the epic artifact directory returned by `jit doc dir b8206228 dev/active` (reflects the above).

## What just happened

- Owner selected option A on the session-18 MAX-rework escalation, authorizing one final bounded tail-aware source repair and a fresh independent review before scheduling.
- Recorded the reset at `5951c438`; dispatched the frontier preparation worker with no timing, wrapper, formal-gate, or JIT authority.
- Worker committed `8af81b55`. Exact-header scratch reading now skips a physically short final record before address parsing, retains complete preceding rows, fills missing planned addresses with status-specific censoring, and keeps non-tail truncation, duplicate/extra/wrong-process, identity/arithmetic, and header failures hard.
- Python tests reached 23/23; binary self-check passed; the release binary remains `target/release/deps/determinant_companion-214102561bb5c0a5` with SHA-256 `3285beacce410ffaa7d0dbbf9e8500545b38324c800264d8c9a59ad8359788d0`.
- Fresh independent review reproduced every prior tail case as closed and the aggregate 315-row/63-summary censored receipt as valid.
- The review found one HIGH boundary defect: `scratch_row_is_incomplete` treats both missing-column `None` and explicit `""` as interruption. A full-width final row with an empty required field is censored instead of rejected. The test suite lacks this exact distinction.
- This is the third review of the same failure-retention root cause, reaching MAX_SAME_FINDING_REPEATS. Escalated rather than applying the one-condition correction without owner approval.
- No benchmark wrapper/run command was invoked. No v5 CSV, rendered report, or `/tmp/gf2-ec22205e-determinant-cost-v5` scratch directory exists.

## What to do next

- [ ] Check the owner response to the `ec22205e` same-finding escalation.
- [ ] If option A is authorized, reset the same-finding audit explicitly and dispatch one exact v5-only correction: only `None` means a structurally short record; a full-width row containing `""` must flow into strict identity/arithmetic validation and fail.
- [ ] Add a regression for a full-width final row with an empty required field, while retaining all 23 tail, corruption, persistence, provenance, statistics, and canonical-cutover tests.
- [ ] Obtain a fresh independent readiness PASS. Any further finding returns to the owner; do not schedule or time on a failed review.
- [ ] Once readiness passes, report exact HEAD, binary path/hash, clean source closure, absent artifact paths, and expected 10–15 minute uninterrupted CPU window to the owner; wait for the owner's schedule.
- [ ] At the authorized time only, run the single locked v5 cohort, then render, validate, commit immutable evidence, link the v5 CSV/report, and run all registered formal gates before closing `ec22205e`.
- [ ] Resume `7a816262` only after `ec22205e` closes; finalize backend selection and the root campaign manifest under the recorded A/A decisions.

## Traps — do not repeat these

- **Do NOT conflate CSV missing columns with explicit empty values.** `csv.DictReader` uses `None` for a physically short record and `""` for a present empty field. Only the former is authorized interrupted-tail evidence; the latter is a complete malformed row and must fail hard.
- **Do NOT start or schedule timing at `8af81b55`.** The final readiness review is FAIL even though all real truncation cases now retain a complete receipt.
- **Do NOT broaden the next repair.** The exact remaining correction is the `None`-versus-empty predicate plus its regression; preserve strict handling of non-tail, duplicate, extra, wrong-process, identity/arithmetic, over-wide, quoted/multiline, and header-drift cases.
- **Do NOT add v4/v5 dispatch, aliases, `--cohort`, or a legacy schema reader.** Canonical-cutover remains binding; historical v3/v4 evidence stays immutable.
- **Do NOT run concurrent Cargo/nextest suites or unapproved measurement.** The shared target is serialized and the CPU determinant cohort awaits the owner's schedule after readiness PASS.
- All unresolved traps in `handoff-14.md` and earlier handoffs remain in force.

## Open questions needing invoker input

- Question: May the lead apply the exact missing-column-`None` versus explicit-empty-string correction after the same-finding threshold?
  - Context: Physical scratch truncation is now retained correctly, but the classifier also forgives a full-width final row with an empty required field; this is the third review on the same root cause.
  - Options: A) authorize the exact predicate correction, regression, and fresh independent review; B) stop v5 and leave `ec22205e`, wave 8, and the epic blocked.
  - Recommendation: A. The remaining defect is one precisely reproduced condition, while every other cumulative readiness surface passes.

## Reference artefacts

- Epic: `jit issue show b8206228`
- Active freeze: `jit issue show 7a816262`
- Determinant prerequisite: `jit issue show ec22205e`
- Progress: `dev/active/b8206228-permanent-statistics/progress.json`
- Prior handoff: `dev/active/b8206228-permanent-statistics/handoff-14.md`
- V5 preregistration: `dev/benchmarks/permanent_campaign/determinant-cost-preregistration-v5.md`
- V5 runner: `dev/benchmarks/permanent_campaign/determinant_cost_v5.py`
- V5 tests: `dev/benchmarks/permanent_campaign/test_determinant_cost_v5.py`
- Historical v4 evidence: `dev/benchmarks/permanent_campaign/determinant-cost-all-cells-v4.csv`, `determinant-cost-all-cells-v4.md`
- Readiness commits: `ac550959` (preregistration), `6348b0c9` (canonical cutover), `1ddcc266` (valid-address truncation), `8af81b55` (structurally short tail retention)
