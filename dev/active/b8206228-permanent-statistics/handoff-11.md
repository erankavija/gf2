# Handoff — Empirical permanent statistics of random matrices over small prime fields (b8206228) — session 15

**Date:** 2026-08-24
**Session number:** 15
**Prior handoffs:** `handoff.md` (3), `handoff-2.md` (4), `handoff-3.md` (5), `handoff-4.md` (6), `handoff-5.md` (7), `handoff-6.md` (8), `handoff-7.md` (10), `handoff-8.md` (11), `handoff-9.md` (12), `handoff-10.md` (14). Their unresolved traps remain in force.

## Current state

- Epic: `b8206228` — backlog; epic gates remain pending until all work items close.
- Wave in progress: wave 8 of 13. The reconciled topological frontier is `7a816262` plus `1947125e`.
- Label-matching work items excluding the epic: 77 total — 62 done, 2 in progress, 13 backlog, 0 ready/rejected.
- Active claims:
  - `7a816262` — `agent:codex`, claimed `2026-08-17T19:18:31Z`; doc-review and research-review pending; rework count 2 is MAX.
  - `1947125e` — `agent:codex-terra`, claimed `2026-08-24T08:22:19Z`; code-review passed at `d8f6cb01`, cargo-ci pending.
- Open escalations: none. The `1947125e` field-scope decision and the rare-event graph amendment are owner-approved and recorded in `progress.json`.
- Premeasurement: no simulation process is running at this snapshot. Two transient user timers are active and waiting: resume at 02:00 EEST and collect at 07:40 EEST on 2026-08-25.
- Durable premeasurement state: 540 of 1,440 positions are terminal — 539 completed, one signal-censored at position 539, and 900 unstarted.
- Progress file: `dev/active/b8206228-permanent-statistics/progress.json`, reconciled from the live graph and host state on 2026-08-24.

## What just happened

- Ran `jit recover`; it removed one stale claims lock. The JIT store and repository validation are healthy.
- Reconciled the live dependency graph. Historical unfinished entries `7a816262` and `3f664839` moved forward in the progress view, and owner-created `8ec10dde` was added. No dependency edge changed during reconciliation.
- Closed `a39bb161` after cargo-ci, code-review, and lead review; closure commit is `fbe7bef1`.
- Resolved `1947125e`'s impossible all-five-backends wording through the owner-approved field-scoped amendment at `b9c50d97`. Its shared conformance implementation is present through `d8f6cb01`; code-review passed with zero findings, while cargo-ci intentionally waits for the measurement host to become quiescent.
- Replaced the estimator-only rare-event line with the owner-approved exact compressed-state path and independent importance-sampling cross-check at `b9c50d97`; hard criteria were strengthened at `0bf6e7a1`.
- Prepared a fresh frozen cohort at source revision `1350d5b4`, harness binary hash prefix `1198cce4`, under `/data/gf2-campaigns/b8206228/premeasure-recovery-20260824`.
- The first durable session wrote 539 completed receipts and one terminal signal-censored receipt. Position 539 is `q=3,n=26,A`, `accelerator/gpu_hip`, with `exit.status` 130; 900 positions remain.
- Hardened collection at main commit `1cb1bc47` so bounded signal interruption remains censored evidence instead of being retried or discarded.
- Verified `gf2-b8206228-premeasure-frozen.timer` and `gf2-b8206228-premeasure-collect.timer` through the explicit user bus. Their services are inactive until their timers fire, and both referenced worktrees are clean.

## What to do next

- [ ] Preserve `.agents/worktrees/agent-a39bb161` and `.agents/worktrees/agent-389aa4de`; the scheduled units execute from them. Do not prune, reset, or repurpose either worktree.
- [ ] At 02:00 EEST, let `gf2-b8206228-premeasure-frozen.timer` resume the same durable `premeasure-v1` cohort with its 19,800-second cap. While its service is active, do not run Cargo, nextest, benchmarks, simulations, or other device work.
- [ ] After 07:40 EEST, inspect both service statuses plus `premeasure-resume-20260825.log`, `premeasure-collect-20260825.log`, and `premeasure-collect-20260825.exit.status`. The collector exits 75 if premeasurement is still active; otherwise raw return 0 means complete and 7 means valid but incomplete.
- [ ] If collection reports incomplete, re-arm the same stable root and run ID for another bounded session. Resume missing positions only; retain position 539 as terminal censored evidence.
- [ ] Once the measurement host is quiescent, run and persist `1947125e`'s cargo-ci gate on the merged tree, confirm all gate statuses, perform the lead review, close the issue, and commit JIT state separately.
- [ ] Continue `7a816262` until every schedule position has terminal evidence, then generate and verify the selection ledger and frozen manifest before its doc-review, research-review, and lead review. Any new review failure at rework count MAX requires escalation rather than an unapproved repair.
- [ ] After both wave-8 issues close, dispatch wave 9: `3f664839` and `73317b2e`. Re-read their amended live descriptions before planning.
- [ ] Run `jit validate` after every JIT state batch and update `progress.json` whenever the frontier, timer state, or receipt totals change.

## Traps

- Do not trust `handoff-10.md` or `progress.json` at commit `2adc8bcd` as current state. Both predate the owner graph amendment, `1947125e` implementation, durable receipts, and timer installation; follow this handoff and the live JIT store.
- Do not conclude that no timers exist when sandboxed `systemctl --user` cannot connect. This host needs `XDG_RUNTIME_DIR=/run/user/1000` and `DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/1000/bus` in that context; the explicit bus revealed two waiting timers.
- Do not remove or modify either timer worktree. The frozen runner worktree is deliberately pinned at `1350d5b4`; the collector worktree is pinned at `3014ad87`, whose collector change is tree-equivalent to main `1cb1bc47`.
- Do not run the collector from the frozen runner worktree. Only the collector worktree and main carry the signal-preserving hardening.
- Do not repair, delete, supersede, or rerun schedule position 539. It is valid terminal censored evidence with exit status 130, and the hardened collector is designed to preserve it.
- Do not run Cargo, nextest, benchmarks, simulations, or competing GPU work while the timed premeasurement service is active. Preserve measurement isolation and the shared Cargo target/lock discipline.
- `jit recover` may require host context when sandbox permissions make the shared `.git/jit` store appear unwritable. This session removed one stale claims lock; recover before lifecycle mutation after another interruption.
- All unresolved traps from prior sessions remain in force. Follow `handoff-10.md` and its prior-handoff chain rather than copying selected warnings forward.

## Open questions

- None. Prior owner decisions for `a39bb161`, `53d8e438`, `1947125e`, and the rare-event graph are resolved and recorded.

## Reference artefacts

- Epic: `jit issue show b8206228`
- Active manifest freeze: `jit issue show 7a816262`
- Active conformance suite: `jit issue show 1947125e`
- Next-wave design: `jit issue show 3f664839`
- New exact path: `jit issue show 8ec10dde`
- Reconciled progress: `dev/active/b8206228-permanent-statistics/progress.json`
- Prior handoff: `dev/active/b8206228-permanent-statistics/handoff-10.md`
- Frozen-manifest plan: `dev/active/b8206228-permanent-statistics/premeasure-plan-v1.md`
- Durable campaign root: `/data/gf2-campaigns/b8206228/premeasure-recovery-20260824`
- Resume log: `/data/gf2-campaigns/b8206228/premeasure-recovery-20260824/premeasure-resume-20260825.log`
- Collector log and status: `/data/gf2-campaigns/b8206228/premeasure-recovery-20260824/premeasure-collect-20260825.log`, `/data/gf2-campaigns/b8206228/premeasure-recovery-20260824/premeasure-collect-20260825.exit.status`
- Relevant commits: `b9c50d97`, `0bf6e7a1`, `d8f6cb01`, `fbe7bef1`, `1cb1bc47`
